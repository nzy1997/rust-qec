from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_rsmp_v1_compression_evidence as checker


WORK_ROOT = Path("/tmp/rstim-rsmp-v1-evidence-work")
GLOBAL_HEADER_LEN = 152
BLOCK_HEADER_LEN = 108
TRAILER_MAGIC = b"RSMPEND\0"
GLOBAL_MAGIC = b"RSTMSMP\0"
BLOCK_MAGIC = b"RSMPBLK\0"
CODEC_BY_ID = {
    0: "empty",
    1: "syndrome_dense_v1",
    2: "syndrome_sparse_leb128_v1",
    3: "free_dense_v1",
}


def sha256_bytes(data: bytes) -> str:
    return checker.sha256_bytes(data)


def sha256_file(path: Path) -> str:
    return checker.sha256_file(path)


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(checker.canonical_json_text(payload), encoding="utf-8")


def write_raw(path: Path, records: list[dict[str, Any]]) -> None:
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in records), encoding="utf-8")


def run_command(argv: list[str], *, stdout: int | None = subprocess.PIPE) -> subprocess.CompletedProcess[bytes]:
    result = subprocess.run(argv, cwd=REPO_ROOT, stdout=stdout, stderr=subprocess.PIPE)
    if result.returncode != 0:
        stderr = result.stderr.decode("utf-8", errors="replace")
        raise RuntimeError(f"command failed ({result.returncode}): {' '.join(argv)}\n{stderr}")
    return result


def command_text(argv: list[str]) -> str:
    return " ".join(argv)


def u16(data: bytes, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 2], "little")


def u64(data: bytes, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 8], "little")


def parse_archive(path: Path) -> dict[str, Any]:
    data = path.read_bytes()
    if len(data) < GLOBAL_HEADER_LEN or data[:8] != GLOBAL_MAGIC:
        raise RuntimeError(f"{path} is not an rsmp archive")
    info: dict[str, Any] = {
        "M": u64(data, 48),
        "D": u64(data, 56),
        "L": u64(data, 64),
        "rank": u64(data, 72),
        "shots": u64(data, 80),
        "circuit_sha256": data[88:120].hex(),
        "blocks": [],
    }
    offset = GLOBAL_HEADER_LEN
    while True:
        magic = data[offset : offset + 8]
        if magic == TRAILER_MAGIC:
            break
        if magic != BLOCK_MAGIC:
            raise RuntimeError(f"{path} has unexpected archive magic at byte {offset}")
        header = data[offset : offset + BLOCK_HEADER_LEN]
        syndrome_codec = u16(header, 36)
        free_codec = u16(header, 38)
        syndrome_compressed = u64(header, 52)
        free_compressed = u64(header, 68)
        block = {
            "block_index": u64(header, 12),
            "first_shot": u64(header, 20),
            "shot_count": u64(header, 28),
            "syndrome_codec": CODEC_BY_ID[syndrome_codec],
            "free_codec": CODEC_BY_ID[free_codec],
            "syndrome_uncompressed_bytes": u64(header, 44),
            "syndrome_compressed_bytes": syndrome_compressed,
            "free_uncompressed_bytes": u64(header, 60),
            "free_compressed_bytes": free_compressed,
            "logical_payload_sha256": header[76:108].hex(),
        }
        info["blocks"].append(block)
        offset += BLOCK_HEADER_LEN + syndrome_compressed + free_compressed
    info["blocks_count"] = len(info["blocks"])
    info["archive_bytes"] = len(data)
    info["archive_sha256"] = sha256_bytes(data)
    return info


def high_entropy_bytes() -> bytes:
    payload = bytearray()
    counter = 0
    prefix = b"rstim-rsmp-v1-high-entropy-v1:"
    while len(payload) < checker.HIGH_ENTROPY_RAW_BYTES:
        payload.extend(sha256_bytes_raw(prefix + counter.to_bytes(8, "little")))
        counter += 1
    return bytes(payload[: checker.HIGH_ENTROPY_RAW_BYTES])


def high_entropy_argv() -> list[str]:
    return [
        "python3",
        "-m",
        "benchmarks.rstim_vs_stim_simulator.run_rsmp_compression",
        "generate-high-entropy",
        checker.HIGH_ENTROPY_GENERATOR_ID,
    ]


def sha256_bytes_raw(data: bytes) -> bytes:
    import hashlib

    return hashlib.sha256(data).digest()


def count_b8_ones(data: bytes, bits_per_shot: int, shots: int) -> int:
    bytes_per_shot = (bits_per_shot + 7) // 8
    expected = bytes_per_shot * shots
    if len(data) != expected:
        raise RuntimeError(f"b8 detector output has {len(data)} bytes; expected {expected}")
    if bits_per_shot == 0:
        return 0
    full_bytes, tail_bits = divmod(bits_per_shot, 8)
    total = 0
    for shot in range(shots):
        start = shot * bytes_per_shot
        segment = data[start : start + bytes_per_shot]
        total += sum(byte.bit_count() for byte in segment[:full_bytes])
        if tail_bits:
            total += (segment[full_bytes] & ((1 << tail_bits) - 1)).bit_count()
    return total


def throughput(raw_bytes: int, elapsed_ns: int) -> int:
    if elapsed_ns <= 0:
        return 0
    return raw_bytes * 1_000_000_000 // elapsed_ns


def sample_measurements(rstim: str, circuit_path: str, shots: int, seed: int, case_id: str) -> tuple[bytes, list[str]]:
    argv = [
        rstim,
        "sample",
        "--shots",
        str(shots),
        "--seed",
        str(seed),
        "--out_format",
        "b8",
        "--in",
        circuit_path,
    ]
    if case_id == checker.BENCHMARK_CASE_ID and argv != checker.PINNED_BENCHMARK_SAMPLE_ARGV:
        raise RuntimeError("benchmark sample argv does not match the pinned issue command")
    result = run_command(argv)
    return result.stdout, argv


def stim_convert_cli_argv(stim: str, in_path: Path, in_format: str, out_path: Path, out_format: str, bits_per_shot: int) -> list[str]:
    return [
        stim,
        "convert",
        "--in",
        str(in_path),
        "--in_format",
        in_format,
        "--out",
        str(out_path),
        "--out_format",
        out_format,
        "--bits_per_shot",
        str(bits_per_shot),
    ]


def stim_convert_api_argv(in_path: Path, in_format: str, out_path: Path, out_format: str, bits_per_shot: int) -> list[str]:
    return [
        sys.executable,
        "-m",
        "benchmarks.rstim_vs_stim_simulator.stim_convert",
        "--in",
        str(in_path),
        "--in_format",
        in_format,
        "--out",
        str(out_path),
        "--out_format",
        out_format,
        "--bits_per_shot",
        str(bits_per_shot),
    ]


def build_stim_baselines(
    *,
    rstim: str,
    stim: str,
    case_work: Path,
    case_id: str,
    measurement_path: Path,
    measurement_sha256: str,
    measurement_argv: list[str],
    bits_per_shot: int,
) -> dict[str, Any]:
    baselines: dict[str, Any] = {}
    for fmt in checker.REQUIRED_STIM_BASELINE_FORMATS:
        artifact_path = case_work / f"stim_baseline.{fmt}"
        if fmt == "b8":
            # The canonical b8 measurement bytes are the source of truth.
            artifact_path.write_bytes(measurement_path.read_bytes())
            serialize_argv = list(measurement_argv)
        elif fmt == "ptb64":
            # The pinned Stim CLI streams single records and cannot write
            # ptb64, so use the pinned stim Python API helper instead.
            serialize_argv = stim_convert_api_argv(measurement_path, "b8", artifact_path, "ptb64", bits_per_shot)
            run_command(serialize_argv, stdout=None)
        else:
            serialize_argv = stim_convert_cli_argv(stim, measurement_path, "b8", artifact_path, fmt, bits_per_shot)
            run_command(serialize_argv, stdout=None)
        artifact = artifact_path.read_bytes()
        artifact_sha256 = sha256_bytes(artifact)

        direct_path = case_work / f"stim_baseline.{fmt}.zst"
        direct_argv = [rstim, "rsmp_zstd_frame", "--level", "3", "--in", str(artifact_path), "--out", str(direct_path)]
        run_command(direct_argv, stdout=None)
        direct_bytes = direct_path.read_bytes()

        roundtrip_path = case_work / f"stim_baseline.{fmt}.roundtrip.b8"
        roundtrip_argv = stim_convert_cli_argv(stim, artifact_path, fmt, roundtrip_path, "b8", bits_per_shot)
        run_command(roundtrip_argv, stdout=None)
        roundtrip_sha256 = sha256_file(roundtrip_path)
        if roundtrip_sha256 != measurement_sha256:
            raise RuntimeError(f"{case_id} {fmt} round-trip measurement SHA-256 mismatch")

        baselines[fmt] = {
            "artifact": {
                "argv": serialize_argv,
                "bytes": len(artifact),
                "sha256": artifact_sha256,
            },
            "direct_zstd": {
                "argv": direct_argv,
                "input_sha256": artifact_sha256,
                "bytes": len(direct_bytes),
                "sha256": sha256_bytes(direct_bytes),
            },
            "roundtrip_b8": {
                "argv": roundtrip_argv,
                "sha256": roundtrip_sha256,
            },
        }
    return baselines


def build_row(
    *,
    rstim: str,
    stim: str,
    work_dir: Path,
    case_id: str,
    semantic_role: str,
    catalog_case_id: str | None,
    circuit_path: str,
    circuit_bytes: bytes,
    canonical_circuit_sha256: str,
    shots: int,
    measurements_b8: bytes,
    measurement_argv: list[str],
    generator: str,
    generator_sha256: str | None,
) -> dict[str, Any]:
    case_work = work_dir / case_id
    case_work.mkdir(parents=True, exist_ok=True)
    measurement_path = case_work / "measurements.b8"
    direct_path = case_work / "measurements.b8.zst"
    archive_path = case_work / "archive.rsmp"
    roundtrip_path = case_work / "roundtrip.measurements.b8"
    detections_path = case_work / "roundtrip.detectors.b8"
    observables_path = case_work / "roundtrip.observables.b8"
    if circuit_path.startswith("generated://"):
        circuit_file = case_work / "circuit.stim"
        circuit_file.write_bytes(circuit_bytes)
        circuit_arg = str(circuit_file)
    else:
        circuit_arg = circuit_path
    measurement_path.write_bytes(measurements_b8)
    measurement_sha256 = sha256_bytes(measurements_b8)

    direct_argv = [rstim, "rsmp_zstd_frame", "--level", "3", "--in", str(measurement_path), "--out", str(direct_path)]
    run_command(direct_argv, stdout=None)
    direct_bytes = direct_path.read_bytes()

    pack_argv = [
        rstim,
        "pack_samples",
        "--circuit",
        circuit_arg,
        "--shots",
        str(shots),
        "--in",
        str(measurement_path),
        "--in_format",
        "b8",
        "--out",
        str(archive_path),
    ]
    encode_start = time.perf_counter_ns()
    run_command(pack_argv, stdout=None)
    encode_elapsed_ns = time.perf_counter_ns() - encode_start

    unpack_argv = [
        rstim,
        "unpack_samples",
        "--circuit",
        circuit_arg,
        "--in",
        str(archive_path),
        "--measurements_out",
        str(roundtrip_path),
        "--measurements_out_format",
        "b8",
        "--detectors_out",
        str(detections_path),
        "--detectors_out_format",
        "b8",
        "--obs_out",
        str(observables_path),
        "--obs_out_format",
        "b8",
    ]
    decode_start = time.perf_counter_ns()
    run_command(unpack_argv, stdout=None)
    decode_elapsed_ns = time.perf_counter_ns() - decode_start
    roundtrip_sha256 = sha256_file(roundtrip_path)
    if roundtrip_sha256 != measurement_sha256:
        raise RuntimeError(f"{case_id} rsmp roundtrip measurement SHA-256 mismatch")

    archive_info = parse_archive(archive_path)
    m = int(archive_info["M"])
    d = int(archive_info["D"])
    l = int(archive_info["L"])
    rank = int(archive_info["rank"])
    raw_b8_bytes = len(measurements_b8)
    if raw_b8_bytes != checker.expected_b8_bytes(m, shots):
        raise RuntimeError(f"{case_id} raw b8 byte count does not match dimensions")
    stim_baselines: dict[str, Any] | None = None
    if case_id == checker.BENCHMARK_CASE_ID:
        stim_baselines = build_stim_baselines(
            rstim=rstim,
            stim=stim,
            case_work=case_work,
            case_id=case_id,
            measurement_path=measurement_path,
            measurement_sha256=measurement_sha256,
            measurement_argv=measurement_argv,
            bits_per_shot=m,
        )
    detectors = detections_path.read_bytes()
    detector_one_count = count_b8_ones(detectors, d, shots)
    detector_total_bits = d * shots
    density_ppm = 0 if detector_total_bits == 0 else detector_one_count * 1_000_000 // detector_total_bits

    dimensions = {
        "M": m,
        "D": d,
        "L": l,
        "rank": rank,
        "free_width": m - rank,
        "shots": shots,
        "blocks": int(archive_info["blocks_count"]),
    }
    row: dict[str, Any] = {
        "schema_version": checker.RAW_SCHEMA_VERSION,
        "evidence_format": checker.EVIDENCE_FORMAT,
        "case_id": case_id,
        "catalog_case_id": catalog_case_id,
        "semantic_role": semantic_role,
        "row_role": "high_entropy_control" if case_id == checker.HIGH_ENTROPY_CASE_ID else "semantic_fixture",
        "is_benchmark": case_id == checker.BENCHMARK_CASE_ID,
        "is_high_entropy_control": case_id == checker.HIGH_ENTROPY_CASE_ID,
        "circuit": {
            "path": circuit_path,
            "canonical_text_sha256": canonical_circuit_sha256,
            "source_sha256": sha256_bytes(circuit_bytes),
        },
        "dimensions": dimensions,
        "measurement_input": {
            "format": "b8",
            "generator": generator,
            "generator_sha256": generator_sha256,
            "argv": measurement_argv,
            "raw_b8_bytes": raw_b8_bytes,
            "sha256": measurement_sha256,
            "logical_digest": checker.measurement_logical_digest(
                case_id,
                canonical_circuit_sha256,
                dimensions,
                raw_b8_bytes,
                measurement_sha256,
            ),
        },
        "direct_zstd": {
            "argv": direct_argv,
            "input_sha256": measurement_sha256,
            "bytes": len(direct_bytes),
            "sha256": sha256_bytes(direct_bytes),
        },
        "rsmp_archive": {
            "argv": pack_argv,
            "unpack_argv": unpack_argv,
            "input_sha256": measurement_sha256,
            "bytes": int(archive_info["archive_bytes"]),
            "sha256": str(archive_info["archive_sha256"]),
            "roundtrip_measurements_sha256": roundtrip_sha256,
            "blocks": archive_info["blocks"],
            "peak_logical_block_working_set_bytes": 0,
            "encode_elapsed_ns": encode_elapsed_ns,
            "decode_elapsed_ns": decode_elapsed_ns,
            "encode_throughput_bytes_per_second": throughput(raw_b8_bytes, encode_elapsed_ns),
            "decode_throughput_bytes_per_second": throughput(raw_b8_bytes, decode_elapsed_ns),
        },
        "detector_density": {
            "one_count": detector_one_count,
            "total_bits": detector_total_bits,
            "ppm": density_ppm,
        },
        "zstd_contract": dict(checker.ZSTD_CONTRACT),
    }
    if stim_baselines is not None:
        row["stim_baselines"] = stim_baselines
    row["rsmp_archive"]["peak_logical_block_working_set_bytes"] = checker.peak_working_set(row)
    return row


def rustc_host() -> str:
    result = subprocess.run(["rustc", "-vV"], cwd=REPO_ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    if result.returncode != 0:
        return "unknown"
    for line in result.stdout.splitlines():
        if line.startswith("host: "):
            return line.split(": ", 1)[1]
    return "unknown"


def command_stdout_text(argv: list[str]) -> str:
    result = subprocess.run(argv, cwd=REPO_ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        return "unknown"
    return result.stdout.strip()


def git_commit() -> str:
    return command_stdout_text(["git", "rev-parse", "HEAD"])


def git_dirty() -> bool:
    result = subprocess.run(["git", "status", "--porcelain"], cwd=REPO_ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    return bool(result.stdout.strip()) if result.returncode == 0 else True


def prepare_temp_result(out_dir: Path, records: list[dict[str, Any]], environment: dict[str, Any]) -> Path:
    temp_dir = out_dir.parent / f".{out_dir.name}.tmp-{os.getpid()}"
    if temp_dir.exists():
        shutil.rmtree(temp_dir)
    temp_dir.mkdir(parents=True)
    write_raw(temp_dir / "raw.jsonl", records)
    summary = checker.derive_summary(records)
    write_json(temp_dir / "summary.json", summary)
    (temp_dir / "report.md").write_text(checker.render_report(summary), encoding="utf-8")
    write_json(temp_dir / "environment.json", environment)
    write_json(temp_dir / "artifact-sha256.json", {name: sha256_file(temp_dir / name) for name in checker.ARTIFACT_FILES})
    checker.check_bundle(temp_dir)
    return temp_dir


def publish_temp_result(temp_dir: Path, out_dir: Path) -> None:
    if out_dir.exists():
        if not out_dir.is_dir():
            raise RuntimeError(f"{out_dir} exists and is not a directory")
        entries = {path.name for path in out_dir.iterdir()}
        if entries - set(checker.REQUIRED_FILES):
            raise RuntimeError(f"refusing to replace {out_dir}: unexpected existing files")
        shutil.rmtree(out_dir)
    temp_dir.replace(out_dir)


def generate(args: argparse.Namespace) -> Path:
    rstim = args.rstim
    stim = args.stim
    if WORK_ROOT.exists():
        shutil.rmtree(WORK_ROOT)
    WORK_ROOT.mkdir(parents=True)
    catalog_path = args.catalog if args.catalog.is_absolute() else REPO_ROOT / args.catalog
    catalog = checker.load_catalog_cases_from_path(catalog_path)
    records: list[dict[str, Any]] = []
    for requirement in checker.REQUIRED_ROWS:
        if requirement.case_id == checker.HIGH_ENTROPY_CASE_ID:
            measurement_argv = high_entropy_argv()
            measurements = run_command(measurement_argv).stdout
            if sha256_bytes(measurements) != checker.HIGH_ENTROPY_RAW_SHA256:
                raise RuntimeError("high-entropy generator produced the wrong SHA-256")
            records.append(
                build_row(
                    rstim=rstim,
                    stim=stim,
                    work_dir=WORK_ROOT,
                    case_id=checker.HIGH_ENTROPY_CASE_ID,
                    semantic_role="high_entropy_control",
                    catalog_case_id=None,
                    circuit_path="generated://high_entropy_no_detector",
                    circuit_bytes=checker.HIGH_ENTROPY_CIRCUIT_TEXT.encode("utf-8"),
                    canonical_circuit_sha256=checker.HIGH_ENTROPY_CIRCUIT_SHA256,
                    shots=8192,
                    measurements_b8=measurements,
                    measurement_argv=measurement_argv,
                    generator=checker.HIGH_ENTROPY_GENERATOR_ID,
                    generator_sha256=checker.HIGH_ENTROPY_GENERATOR_SHA256,
                )
            )
            continue

        catalog_case = catalog[requirement.catalog_case_id]
        circuit_path = str(catalog_case["circuit_path"])
        shots = int(args.shots if requirement.case_id == checker.BENCHMARK_CASE_ID else catalog_case["shots"])
        seed = int(args.seed if requirement.case_id == checker.BENCHMARK_CASE_ID else 2)
        measurements, sample_argv = sample_measurements(rstim, circuit_path, shots, seed, requirement.case_id)
        records.append(
            build_row(
                rstim=rstim,
                stim=stim,
                work_dir=WORK_ROOT,
                case_id=requirement.case_id,
                semantic_role=requirement.semantic_role,
                catalog_case_id=requirement.catalog_case_id,
                circuit_path=circuit_path,
                circuit_bytes=(REPO_ROOT / circuit_path).read_bytes(),
                canonical_circuit_sha256=str(catalog_case["circuit_sha256"]),
                shots=shots,
                measurements_b8=measurements,
                measurement_argv=sample_argv,
                generator="rstim_sample",
                generator_sha256=None,
            )
        )

    benchmark = records[checker.BENCHMARK_ROW_INDEX]
    if benchmark["measurement_input"]["raw_b8_bytes"] != checker.PINNED_BENCHMARK_RAW_BYTES:
        raise RuntimeError("benchmark measurement byte count does not match pinned issue value")
    if benchmark["measurement_input"]["sha256"] != checker.PINNED_BENCHMARK_SHA256:
        raise RuntimeError("benchmark measurement SHA-256 does not match pinned issue value")

    zstd_info_path = WORK_ROOT / "zstd-info.json"
    run_command([rstim, "rsmp_zstd_info", "--out", str(zstd_info_path)], stdout=None)
    zstd_info = json.loads(zstd_info_path.read_text(encoding="utf-8"))
    locked_versions = checker.load_locked_package_versions()
    rstim_binary = Path(rstim)
    rstim_binary_for_hash = rstim_binary if rstim_binary.is_absolute() else REPO_ROOT / rstim_binary
    stim_resolved = shutil.which(stim)
    if stim_resolved is None:
        raise RuntimeError(f"could not resolve pinned stim executable: {stim}")
    stim_binary_path = Path(stim_resolved).resolve()
    stim_version = command_stdout_text([sys.executable, "-c", "import stim; print(stim.__version__)"])
    environment = {
        "schema_version": checker.ENVIRONMENT_SCHEMA_VERSION,
        "evidence_format": checker.EVIDENCE_FORMAT,
        "producer": {
            "name": "rstim",
            "version": command_stdout_text([rstim]),
            "rstim_binary": {
                "path": rstim,
                "sha256": sha256_file(rstim_binary_for_hash),
            },
        },
        "generator": {
            "module": "benchmarks.rstim_vs_stim_simulator.run_rsmp_compression",
            "module_sha256": sha256_file(Path(__file__).resolve()),
            "argv": sys.argv,
        },
        "git": {
            "commit": git_commit(),
            "dirty": git_dirty(),
        },
        "platform": {
            "os": platform.platform(),
            "machine": platform.machine(),
            "target": rustc_host(),
            "rustc": command_stdout_text(["rustc", "--version"]),
        },
        "cargo": {
            "lock_sha256": sha256_file(REPO_ROOT / "Cargo.lock"),
            "zstd": locked_versions.get("zstd"),
            "zstd-safe": locked_versions.get("zstd-safe"),
            "zstd-sys": locked_versions.get("zstd-sys"),
        },
        "zstd_info": zstd_info,
        "zstd_contract": dict(checker.ZSTD_CONTRACT),
        "stim": {
            "binary": {
                "path": str(stim_binary_path),
                "sha256": sha256_file(stim_binary_path),
            },
            "version": stim_version,
            "version_source": "stim-python-module",
        },
        "commands": {
            "benchmark_sample": checker.PINNED_BENCHMARK_SAMPLE_ARGV,
            "stim_baselines": {
                fmt: {
                    "serialize": baseline["artifact"]["argv"],
                    "direct_zstd": baseline["direct_zstd"]["argv"],
                    "roundtrip": baseline["roundtrip_b8"]["argv"],
                }
                for fmt, baseline in benchmark["stim_baselines"].items()
            },
            "rows": {row["case_id"]: {
                "measurement": row["measurement_input"]["argv"],
                "direct_zstd": row["direct_zstd"]["argv"],
                "pack": row["rsmp_archive"]["argv"],
                "unpack": row["rsmp_archive"]["unpack_argv"],
            } for row in records},
        },
        "inputs": {
            "catalog_path": str(args.catalog),
            "catalog_sha256": sha256_file(catalog_path),
            "benchmark_case": args.case,
            "high_entropy_generator": {
                "id": checker.HIGH_ENTROPY_GENERATOR_ID,
                "spec_sha256": checker.HIGH_ENTROPY_GENERATOR_SHA256,
                "raw_sha256": checker.HIGH_ENTROPY_RAW_SHA256,
            },
        },
    }
    temp_dir = prepare_temp_result(args.out_dir, records, environment)
    publish_temp_result(temp_dir, args.out_dir)
    return args.out_dir


def parse_high_entropy_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Emit the pinned rsmp v1 high-entropy control stream")
    parser.add_argument("generator_id")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args(argv)
    if args.generator_id != checker.HIGH_ENTROPY_GENERATOR_ID:
        raise SystemExit(f"generator_id must be {checker.HIGH_ENTROPY_GENERATOR_ID}")
    return args


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate rsmp v1 compression evidence")
    parser.add_argument("--rstim", required=True)
    parser.add_argument("--stim", default="stim")
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--case", required=True)
    parser.add_argument("--shots", required=True, type=int)
    parser.add_argument("--seed", required=True, type=int)
    parser.add_argument("--zstd-level", required=True, type=int)
    parser.add_argument("--out-dir", required=True, type=Path)
    args = parser.parse_args(argv)
    if args.case != checker.BENCHMARK_CASE_ID:
        raise SystemExit(f"--case must be {checker.BENCHMARK_CASE_ID}")
    if args.shots != 1024:
        raise SystemExit("--shots must be 1024 for the pinned benchmark")
    if args.seed != 7:
        raise SystemExit("--seed must be 7 for the pinned benchmark")
    if args.zstd_level != 3:
        raise SystemExit("--zstd-level must be 3")
    return args


def emit_high_entropy(args: argparse.Namespace) -> int:
    payload = high_entropy_bytes()
    if sha256_bytes(payload) != checker.HIGH_ENTROPY_RAW_SHA256:
        raise RuntimeError("high-entropy generator produced the wrong SHA-256")
    if args.out is None:
        sys.stdout.buffer.write(payload)
    else:
        args.out.write_bytes(payload)
    return 0


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["generate-high-entropy"]:
        return emit_high_entropy(parse_high_entropy_args(argv[1:]))
    args = parse_args(argv)
    out_dir = generate(args)
    print(f"Wrote checked rsmp v1 compression evidence to {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
