#!/usr/bin/env python3
from __future__ import annotations

import copy
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Callable


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_rsmp_v1_compression_evidence as checker


CHECKER = REPO_ROOT / "tools" / "check_rsmp_v1_compression_evidence.py"
ARTIFACT_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_raw(path: Path, records: list[dict[str, Any]]) -> None:
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in records), encoding="utf-8")


def load_raw(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def measurement_logical_digest(row: dict[str, Any]) -> str:
    return checker.measurement_logical_digest(
        row["case_id"],
        row["circuit"]["canonical_text_sha256"],
        row["dimensions"],
        row["measurement_input"]["raw_b8_bytes"],
        row["measurement_input"]["sha256"],
    )


def synthetic_stim_baselines(measurement_sha: str, raw_bytes: int, direct_bytes: int) -> dict[str, Any]:
    baselines: dict[str, Any] = {}
    for index, fmt in enumerate(checker.REQUIRED_STIM_BASELINE_FORMATS):
        artifact_bytes = raw_bytes if fmt == "b8" else raw_bytes + 1000 * (index + 1)
        artifact_sha = measurement_sha if fmt == "b8" else f"{index + 7:064x}"[-64:]
        baselines[fmt] = {
            "artifact": {
                "argv": ["tool://stim", "convert", "--out_format", fmt],
                "bytes": artifact_bytes,
                "sha256": artifact_sha,
            },
            "direct_zstd": {
                "argv": ["tool://rstim", "rsmp_zstd_frame", "--level", "3"],
                "input_sha256": artifact_sha,
                "bytes": direct_bytes,
                "sha256": f"{direct_bytes + index:064x}"[-64:],
            },
            "roundtrip_b8": {
                "argv": ["tool://stim", "convert", "--out_format", "b8"],
                "sha256": measurement_sha,
            },
        }
    return baselines


def base_record(
    case_id: str,
    semantic_role: str,
    *,
    catalog_case_id: str | None,
    m: int,
    d: int,
    l: int,
    rank: int,
    shots: int,
    circuit_path: str,
    circuit_sha256: str,
    source_sha256: str,
) -> dict[str, Any]:
    free_width = m - rank
    raw_bytes = shots * ((m + 7) // 8)
    blocks = (shots + 4095) // 4096
    measurement_sha = checker.PINNED_BENCHMARK_SHA256 if case_id == checker.BENCHMARK_CASE_ID else f"{len(case_id):064x}"[-64:]
    if case_id == checker.HIGH_ENTROPY_CASE_ID:
        raw_bytes = checker.HIGH_ENTROPY_RAW_BYTES
        measurement_sha = checker.HIGH_ENTROPY_RAW_SHA256
    archive_bytes = 100_000 if case_id == checker.BENCHMARK_CASE_ID else raw_bytes + 100
    direct_bytes = 200_000 if case_id == checker.BENCHMARK_CASE_ID else raw_bytes + 200
    if case_id == checker.HIGH_ENTROPY_CASE_ID:
        archive_bytes = checker.HIGH_ENTROPY_RAW_BYTES + 100
        direct_bytes = checker.HIGH_ENTROPY_RAW_BYTES + 200
    block_rows = []
    remaining = shots
    first_shot = 0
    for block_index in range(blocks):
        shot_count = min(4096, remaining)
        syndrome_uncompressed = (rank * shot_count + 7) // 8 if rank else 0
        free_uncompressed = (free_width * shot_count + 7) // 8 if free_width else 0
        block_rows.append(
            {
                "block_index": block_index,
                "first_shot": first_shot,
                "shot_count": shot_count,
                "syndrome_codec": "empty" if rank == 0 else "syndrome_sparse_leb128_v1",
                "free_codec": "empty" if free_width == 0 else "free_dense_v1",
                "syndrome_uncompressed_bytes": syndrome_uncompressed,
                "syndrome_compressed_bytes": 0 if rank == 0 else max(9, syndrome_uncompressed // 10),
                "free_uncompressed_bytes": free_uncompressed,
                "free_compressed_bytes": 0 if free_width == 0 else free_uncompressed + 9,
                "logical_payload_sha256": f"{block_index + 1:064x}"[-64:],
            }
        )
        remaining -= shot_count
        first_shot += shot_count
    row = {
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
            "canonical_text_sha256": circuit_sha256,
            "source_sha256": source_sha256,
        },
        "dimensions": {
            "M": m,
            "D": d,
            "L": l,
            "rank": rank,
            "free_width": free_width,
            "shots": shots,
            "blocks": blocks,
        },
        "measurement_input": {
            "format": "b8",
            "generator": "rstim_sample" if case_id != checker.HIGH_ENTROPY_CASE_ID else checker.HIGH_ENTROPY_GENERATOR_ID,
            "generator_sha256": checker.HIGH_ENTROPY_GENERATOR_SHA256 if case_id == checker.HIGH_ENTROPY_CASE_ID else None,
            "argv": checker.PINNED_BENCHMARK_SAMPLE_ARGV if case_id == checker.BENCHMARK_CASE_ID else ["tool://rstim", "sample", "--shots", str(shots)],
            "raw_b8_bytes": raw_bytes,
            "sha256": measurement_sha,
            "logical_digest": "",
        },
        "direct_zstd": {
            "argv": ["tool://rstim", "rsmp_zstd_frame", "--level", "3"],
            "input_sha256": measurement_sha,
            "bytes": direct_bytes,
            "sha256": f"{direct_bytes:064x}"[-64:],
        },
        "rsmp_archive": {
            "argv": ["tool://rstim", "pack_samples", "--in_format", "b8"],
            "unpack_argv": ["tool://rstim", "unpack_samples", "--measurements_out_format", "b8"],
            "input_sha256": measurement_sha,
            "bytes": archive_bytes,
            "sha256": f"{archive_bytes:064x}"[-64:],
            "roundtrip_measurements_sha256": measurement_sha,
            "blocks": block_rows,
            "peak_logical_block_working_set_bytes": 0,
            "encode_elapsed_ns": 10_000,
            "decode_elapsed_ns": 20_000,
            "encode_throughput_bytes_per_second": raw_bytes * 100_000,
            "decode_throughput_bytes_per_second": raw_bytes * 50_000,
        },
        "detector_density": {
            "one_count": 0,
            "total_bits": d * shots,
            "ppm": 0,
        },
        "zstd_contract": copy.deepcopy(checker.ZSTD_CONTRACT),
    }
    row["measurement_input"]["logical_digest"] = measurement_logical_digest(row)
    row["rsmp_archive"]["peak_logical_block_working_set_bytes"] = checker.peak_working_set(row)
    if case_id == checker.BENCHMARK_CASE_ID:
        row["stim_baselines"] = synthetic_stim_baselines(measurement_sha, raw_bytes, direct_bytes)
    return row


def valid_records() -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    catalog = checker.load_catalog_cases()
    for index, requirement in enumerate(checker.REQUIRED_ROWS):
        if requirement.case_id == checker.HIGH_ENTROPY_CASE_ID:
            records.append(
                base_record(
                    checker.HIGH_ENTROPY_CASE_ID,
                    "high_entropy_control",
                    catalog_case_id=None,
                    m=1024,
                    d=0,
                    l=0,
                    rank=0,
                    shots=8192,
                    circuit_path="generated://high_entropy_no_detector",
                    circuit_sha256=checker.HIGH_ENTROPY_CIRCUIT_SHA256,
                    source_sha256=checker.HIGH_ENTROPY_CIRCUIT_SHA256,
                )
            )
        else:
            case = catalog[requirement.catalog_case_id]
            circuit_path = case["circuit_path"]
            shots = 1024 if requirement.case_id == checker.BENCHMARK_CASE_ID else case["shots"]
            records.append(
                base_record(
                    requirement.case_id,
                    requirement.semantic_role,
                    catalog_case_id=requirement.catalog_case_id,
                    m=case["measurement_count"],
                    d=case["detector_count"],
                    l=case["observable_count"],
                    rank=case["rank_H"],
                    shots=shots,
                    circuit_path=circuit_path,
                    circuit_sha256=case["circuit_sha256"],
                    source_sha256=sha256_file(REPO_ROOT / circuit_path),
                )
            )
    return records


def refresh_derived(bundle: Path) -> None:
    records = load_raw(bundle / "raw.jsonl")
    summary = checker.derive_summary(records)
    write_json(bundle / "summary.json", summary)
    (bundle / "report.md").write_text(checker.render_report(summary), encoding="utf-8")
    write_json(bundle / "artifact-sha256.json", {name: sha256_file(bundle / name) for name in ARTIFACT_FILES})


def write_valid_bundle(bundle: Path) -> None:
    bundle.mkdir(parents=True, exist_ok=True)
    write_raw(bundle / "raw.jsonl", valid_records())
    write_json(
        bundle / "environment.json",
        {
            "schema_version": checker.ENVIRONMENT_SCHEMA_VERSION,
            "evidence_format": checker.EVIDENCE_FORMAT,
            "producer": {"name": "rstim", "version": "rstim test", "rstim_binary": {"path": "target/release/nonexistent-rsmp-checker-test-rstim", "sha256": "0" * 64}},
            "generator": {"module": "benchmarks.rstim_vs_stim_simulator.run_rsmp_compression", "argv": ["python3", "-m", "benchmarks.rstim_vs_stim_simulator.run_rsmp_compression"]},
            "git": {"commit": "test", "dirty": True},
            "platform": {"os": "test-os", "target": "test-target", "rustc": "rustc test"},
            "cargo": {"lock_sha256": sha256_file(REPO_ROOT / "Cargo.lock"), "zstd": "0.13.3", "zstd-safe": "7.2.4", "zstd-sys": "2.0.16+zstd.1.5.7"},
            "zstd_info": {
                "crate_version": "0.13.3",
                "zstd_safe_crate_version": "7.2.4",
                "zstd_sys_crate_version": "2.0.16+zstd.1.5.7",
                "native_zstd_version": "1.5.7",
                "native_zstd_version_number": 10507,
                "level": 3,
                "single_threaded": True,
                "dictionary": "none",
                "long_distance_matching": False,
                "pledged_source_size": True,
                "frame_content_size": True,
                "frame_checksum": True,
            },
            "zstd_contract": copy.deepcopy(checker.ZSTD_CONTRACT),
            "stim": {
                "binary": {"path": "target/release/nonexistent-rsmp-checker-test-stim", "sha256": "0" * 64},
                "version": "1.16.0",
                "version_source": "stim-python-module",
            },
            "commands": {
                "benchmark_sample": checker.PINNED_BENCHMARK_SAMPLE_ARGV,
                "stim_baselines": {
                    fmt: {
                        "serialize": ["tool://stim", "convert", "--out_format", fmt],
                        "direct_zstd": ["tool://rstim", "rsmp_zstd_frame", "--level", "3"],
                        "roundtrip": ["tool://stim", "convert", "--out_format", "b8"],
                    }
                    for fmt in checker.REQUIRED_STIM_BASELINE_FORMATS
                },
            },
        },
    )
    refresh_derived(bundle)


def run_checker(bundle: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--results-dir", str(bundle)],
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )


def expect_failure(source: Path, name: str, mutate: Callable[[Path], None], expected: str) -> None:
    target = source.parent / name
    shutil.copytree(source, target)
    mutate(target)
    result = run_checker(target)
    if result.returncode == 0:
        raise AssertionError(f"{name} unexpectedly passed")
    if expected not in result.stdout:
        raise AssertionError(f"{name} did not mention {expected!r}; output was:\n{result.stdout}")


def mutate_raw(bundle: Path, mutate: Callable[[list[dict[str, Any]]], None], *, refresh: bool = True) -> None:
    records = load_raw(bundle / "raw.jsonl")
    mutate(records)
    write_raw(bundle / "raw.jsonl", records)
    if refresh:
        refresh_derived(bundle)


def mutate_json(bundle: Path, filename: str, mutate: Callable[[dict[str, Any]], None]) -> None:
    payload = load_json(bundle / filename)
    mutate(payload)
    write_json(bundle / filename, payload)
    write_json(bundle / "artifact-sha256.json", {name: sha256_file(bundle / name) for name in ARTIFACT_FILES})


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="rstim-rsmp-checker-test-") as raw_tmp:
        root = Path(raw_tmp)
        valid = root / "valid"
        write_valid_bundle(valid)
        result = run_checker(valid)
        if result.returncode != 0:
            raise AssertionError(f"valid bundle failed:\n{result.stdout}")

        historical_lock = root / "historical-lock"
        shutil.copytree(valid, historical_lock)
        mutate_json(
            historical_lock,
            "environment.json",
            lambda environment: environment["cargo"].update({"lock_sha256": "1" * 64}),
        )
        result = run_checker(historical_lock)
        if result.returncode != 0:
            raise AssertionError(f"historical lock provenance bundle failed:\n{result.stdout}")

        expect_failure(
            valid,
            "bad-benchmark-archive-bytes",
            lambda bundle: mutate_raw(
                bundle,
                lambda records: (
                    records[checker.BENCHMARK_ROW_INDEX]["rsmp_archive"].update({"bytes": checker.PINNED_BENCHMARK_RAW_BYTES // 5 + 1}),
                    records[checker.BENCHMARK_ROW_INDEX]["direct_zstd"].update({"bytes": checker.PINNED_BENCHMARK_RAW_BYTES}),
                ),
            ),
            "benchmark_raw_lt_20pct",
        )
        expect_failure(
            valid,
            "bad-benchmark-sha",
            lambda bundle: mutate_raw(
                bundle,
                lambda records: records[checker.BENCHMARK_ROW_INDEX]["measurement_input"].update({"sha256": "f" * 64}),
            ),
            "measurement_sha256",
        )
        expect_failure(
            valid,
            "missing-logical-digest",
            lambda bundle: mutate_raw(
                bundle,
                lambda records: records[0]["measurement_input"].pop("logical_digest"),
                refresh=False,
            ),
            "logical_digest",
        )
        expect_failure(
            valid,
            "stale-summary",
            lambda bundle: mutate_json(bundle, "summary.json", lambda payload: payload.update({"schema_version": 99})),
            "summary.json",
        )
        expect_failure(
            valid,
            "stale-report",
            lambda bundle: ((bundle / "report.md").write_text("stale\n", encoding="utf-8"), mutate_json(bundle, "artifact-sha256.json", lambda payload: payload.update({"report.md": sha256_file(bundle / "report.md")}))),
            "report.md",
        )
        expect_failure(
            valid,
            "artifact-mismatch",
            lambda bundle: write_json(
                bundle / "artifact-sha256.json",
                {**load_json(bundle / "artifact-sha256.json"), "raw.jsonl": "0" * 64},
            ),
            "artifact-sha256",
        )
        expect_failure(
            valid,
            "high-entropy-generator",
            lambda bundle: mutate_raw(
                bundle,
                lambda records: records[checker.HIGH_ENTROPY_ROW_INDEX]["measurement_input"].update({"generator_sha256": "1" * 64}),
            ),
            "high_entropy_generator_sha256",
        )
        expect_failure(
            valid,
            "non-level-3",
            lambda bundle: mutate_raw(
                bundle,
                lambda records: records[0]["zstd_contract"].update({"level": 4}),
            ),
            "zstd level",
        )
        expect_failure(
            valid,
            "direct-zstd-argv-level-4",
            lambda bundle: mutate_raw(
                bundle,
                lambda records: records[0]["direct_zstd"].update(
                    {"argv": ["tool://rstim", "rsmp_zstd_frame", "--level", "4", "3"]}
                ),
            ),
            "direct_zstd argv --level must be 3",
        )
        expect_failure(
            valid,
            "floating-threshold",
            lambda bundle: mutate_raw(
                bundle,
                lambda records: records[0]["zstd_contract"].update({"threshold_arithmetic": "floating_point"}),
            ),
            "threshold_arithmetic",
        )
        expect_failure(
            valid,
            "high-entropy-direct-denominator",
            lambda bundle: mutate_json(
                bundle,
                "summary.json",
                lambda payload: payload["gates"]["high_entropy_raw_le_102pct"].update(
                    {
                        "denominator_kind": "direct_zstd_bytes",
                        "rhs": 51 * payload["high_entropy_control"]["direct_zstd_bytes"],
                    }
                ),
            ),
            "high_entropy_raw_le_102pct",
        )
        expect_failure(
            valid,
            "missing-ptb64-baseline",
            lambda bundle: mutate_raw(
                bundle,
                lambda records: records[checker.BENCHMARK_ROW_INDEX]["stim_baselines"].pop("ptb64"),
                refresh=False,
            ),
            "missing required Stim baseline format: ptb64",
        )
        expect_failure(
            valid,
            "bad-r8-roundtrip-sha",
            lambda bundle: mutate_raw(
                bundle,
                lambda records: records[checker.BENCHMARK_ROW_INDEX]["stim_baselines"]["r8"]["roundtrip_b8"].update({"sha256": "e" * 64}),
            ),
            "r8 round-trip measurement SHA-256 mismatch",
        )

    print("PASS rsmp compression checker negative_controls=13")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
