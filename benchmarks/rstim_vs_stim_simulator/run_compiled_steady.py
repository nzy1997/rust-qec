from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import statistics
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, BinaryIO

from benchmarks.rstim_vs_stim_simulator import fair_cli_contract


READY = b"R"
SAMPLE = b"S"
RESULT = b"T"
STOP = b"P"
FINAL = b"F"
ERROR = b"E"
PROTOCOL_VERSION = 4
TIMER_SCOPE = "worker_sample_path_and_b8_output_excluding_precompile_and_transport"
VARIANTS = (
    "rstim-precompiled",
    "stim-precompiled",
    "rstim-interpreted",
    "stim-direct",
    "rstim-precompiled-atom-loss",
)
STIM_VARIANTS = {"stim-precompiled", "stim-direct"}
PRECOMPILED_VARIANTS = {
    "rstim-precompiled",
    "stim-precompiled",
    "rstim-precompiled-atom-loss",
}
ATOM_LOSS_VARIANT = "rstim-precompiled-atom-loss"
ATOM_LOSS_FIXTURE_PATH = Path(
    "benchmarks/rstim_vs_stim_simulator/fixtures/"
    "stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim"
)

PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]


class RunnerError(RuntimeError):
    pass


def write_frame(stream: BinaryIO, frame_type: bytes, payload: bytes) -> None:
    if len(frame_type) != 1:
        raise ValueError("frame type must be exactly one byte")
    stream.write(frame_type + struct.pack("<Q", len(payload)) + payload)
    stream.flush()


def _read_exact(stream: BinaryIO, size: int) -> bytes:
    chunks: list[bytes] = []
    remaining = size
    while remaining:
        chunk = stream.read(remaining)
        if not chunk:
            raise EOFError(f"unexpected EOF while reading {size} protocol bytes")
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def read_frame(stream: BinaryIO) -> tuple[bytes, bytes]:
    header = _read_exact(stream, 9)
    frame_type = header[:1]
    payload_size = struct.unpack("<Q", header[1:])[0]
    return frame_type, _read_exact(stream, payload_size)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _sha256_if_file(path: Path | None) -> str | None:
    if path is None or not path.is_file():
        return None
    return _sha256(path)


def _sha256_required(path: Path | None, label: str) -> str:
    digest = _sha256_if_file(path)
    if digest is None:
        raise RunnerError(f"could not identify {label} for portable provenance")
    return digest


def _repo_relative(path: Path) -> str:
    return path.resolve().relative_to(REPO_ROOT.resolve()).as_posix()


def _portable_worker_argv(variant: str, input_path: str, *, seed: int) -> list[str]:
    if variant in STIM_VARIANTS:
        return [
            "tool://python",
            "-m",
            "benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady",
            "--variant",
            variant,
            "--input",
            input_path,
            "--seed",
            str(seed),
        ]
    return [
        "tool://rstim-worker",
        "--variant",
        variant,
        "--input",
        input_path,
        "--seed",
        str(seed),
    ]


def _expected_lifecycle_counts(variant: str, sample_call_count: int) -> tuple[int, int]:
    if variant in PRECOMPILED_VARIANTS:
        return 1, 1
    if variant == "stim-direct":
        return sample_call_count, sample_call_count
    return 0, sample_call_count


def _decode_json(payload: bytes, *, context: str) -> dict[str, Any]:
    try:
        decoded = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RunnerError(f"invalid {context} JSON: {error}") from error
    if not isinstance(decoded, dict):
        raise RunnerError(f"invalid {context} JSON: expected object")
    return decoded


def _parse_command(value: str) -> list[str]:
    try:
        command = json.loads(value)
    except json.JSONDecodeError as error:
        raise argparse.ArgumentTypeError(f"worker command must be JSON: {error}") from error
    if not isinstance(command, list) or not command or not all(isinstance(item, str) and item for item in command):
        raise argparse.ArgumentTypeError("worker command must be a nonempty JSON string array")
    return command


def default_stim_worker_command() -> list[str]:
    return [
        "python3",
        "-m",
        "benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady",
    ]


def default_rstim_worker_command(profile: str) -> list[str]:
    return [f"target/{profile}/rstim_compiled_steady_worker"]


def build_rstim_worker(profile: str) -> list[str]:
    command = ["cargo", "build", "--locked"]
    if profile == "release":
        command.append("--release")
    command.extend(["-p", "rstim", "--bin", "rstim_compiled_steady_worker"])
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RunnerError(f"failed to build rstim compiled steady worker: {detail}")
    return default_rstim_worker_command(profile)


def _probe(command: list[str]) -> dict[str, Any]:
    try:
        completed = subprocess.run(
            command,
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return {
            "command": command,
            "status": "failed",
            "exit_code": None,
            "stdout": "",
            "stderr": str(error),
        }
    return {
        "command": command,
        "status": "ok" if completed.returncode == 0 else "failed",
        "exit_code": completed.returncode,
        "stdout": completed.stdout.strip(),
        "stderr": completed.stderr.strip(),
    }


def _version_string(command: list[str]) -> str | None:
    result = _probe(command)
    if result["status"] == "ok" and result["stdout"]:
        return str(result["stdout"])
    return None


def _resolve_executable(raw: str) -> Path | None:
    resolved = shutil.which(raw)
    if resolved:
        return Path(resolved).resolve()
    candidate = Path(raw)
    if candidate.is_file():
        return candidate.resolve()
    repo_candidate = REPO_ROOT / raw
    if repo_candidate.is_file():
        return repo_candidate.resolve()
    return None


def _probe_stim_python() -> dict[str, Any]:
    result = _probe(
        [
            "python3",
            "-c",
            (
                "import importlib.machinery, json, stim, sys; "
                "suffixes = tuple(importlib.machinery.EXTENSION_SUFFIXES); "
                "extensions = [(name, getattr(module, '__file__', None)) "
                "for name, module in sys.modules.items() "
                "if name.startswith('stim.') "
                "and getattr(module, '__file__', None) "
                "and getattr(module, '__file__', '').endswith(suffixes)]; "
                "extensions.sort(key=lambda item: (not item[0].startswith('stim._stim'), item[0])); "
                "print(json.dumps({'version': stim.__version__, "
                "'package_path': getattr(stim, '__file__', None), "
                "'path': extensions[0][1] if extensions else None, "
                "'extension_module': extensions[0][0] if extensions else None}))"
            ),
        ]
    )
    if result["status"] != "ok":
        return {
            "status": "failed",
            "version": None,
            "path": None,
            "sha256": None,
            "stderr": result["stderr"],
        }
    try:
        payload = json.loads(str(result["stdout"]))
    except json.JSONDecodeError as error:
        return {
            "status": "failed",
            "version": None,
            "path": None,
            "sha256": None,
            "stderr": f"invalid stim probe JSON: {error}",
        }
    path = Path(payload["path"]).resolve() if payload.get("path") else None
    return {
        "status": "ok",
        "version": payload.get("version"),
        "package_path": payload.get("package_path"),
        "extension_module": payload.get("extension_module"),
        "path": str(path) if path is not None else None,
        "sha256": _sha256_if_file(path),
        "stderr": "",
    }


def _cpu_model() -> str:
    sysctl = _probe(["sysctl", "-n", "machdep.cpu.brand_string"])
    if sysctl["status"] == "ok" and sysctl["stdout"]:
        return str(sysctl["stdout"])
    return platform.processor() or platform.machine() or "unknown"


class WorkerSession:
    def __init__(self, command: list[str], *, input_path: Path, seed: int) -> None:
        self.command = [*command, "--input", str(input_path), "--seed", str(seed)]
        python_path = os.environ.get("PYTHONPATH")
        environment = dict(os.environ)
        environment["PYTHONPATH"] = str(REPO_ROOT) if not python_path else f"{REPO_ROOT}{os.pathsep}{python_path}"
        self.process = subprocess.Popen(
            self.command,
            cwd=REPO_ROOT,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        )
        assert self.process.stdin is not None
        assert self.process.stdout is not None
        assert self.process.stderr is not None
        self.stdin = self.process.stdin
        self.stdout = self.process.stdout
        self.stderr = self.process.stderr

    def read_ready(self) -> dict[str, Any]:
        frame_type, payload = read_frame(self.stdout)
        if frame_type == ERROR:
            raise RunnerError(f"worker error before ready: {payload.decode(errors='replace')}")
        if frame_type != READY:
            raise RunnerError(f"expected READY frame, got {frame_type!r}")
        return _decode_json(payload, context="READY")

    def sample(self, request_id: int, shots: int) -> tuple[int, bytes, int, int]:
        payload = json.dumps({"request_id": request_id, "shots": shots}, sort_keys=True).encode()
        write_frame(self.stdin, SAMPLE, payload)
        self.stdin.flush()
        frame_type, result_payload = read_frame(self.stdout)
        if frame_type == ERROR:
            raise RunnerError(f"worker error during sample: {result_payload.decode(errors='replace')}")
        if frame_type != RESULT:
            raise RunnerError(f"expected RESULT frame, got {frame_type!r}")
        if len(result_payload) < 32:
            raise RunnerError("RESULT payload is shorter than request, call, sample, and b8 timing fields")
        returned_request_id, sample_call_count, sample_elapsed_ns, b8_elapsed_ns = struct.unpack(
            "<QQQQ", result_payload[:32]
        )
        if returned_request_id != request_id:
            raise RunnerError(f"RESULT request id {returned_request_id} does not match {request_id}")
        return sample_call_count, result_payload[32:], sample_elapsed_ns, b8_elapsed_ns

    def stop(self) -> dict[str, Any]:
        write_frame(self.stdin, STOP, b"")
        self.stdin.flush()
        frame_type, payload = read_frame(self.stdout)
        if frame_type == ERROR:
            raise RunnerError(f"worker error during stop: {payload.decode(errors='replace')}")
        if frame_type != FINAL:
            raise RunnerError(f"expected FINAL frame, got {frame_type!r}")
        final = _decode_json(payload, context="FINAL")
        self.stdin.close()
        exit_code = self.process.wait()
        stderr = self.stderr.read().decode(errors="replace").strip()
        if exit_code != 0:
            detail = f": {stderr}" if stderr else ""
            raise RunnerError(f"worker exited with status {exit_code}{detail}")
        return final

    def abort(self) -> None:
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait()


def _validate_telemetry(
    telemetry: dict[str, Any],
    *,
    variant: str,
    fixture_sha256: str,
    measurement_count: int,
    bytes_per_shot: int,
    sample_call_count: int,
) -> None:
    compile_count, reference_build_count = _expected_lifecycle_counts(variant, sample_call_count)
    expected = {
        "variant": variant,
        "precompile_elapsed_ns": telemetry.get("precompile_elapsed_ns"),
        "compile_count": compile_count,
        "reference_build_count": reference_build_count,
        "sample_call_count": sample_call_count,
        "fixture_sha256": fixture_sha256,
        "measurement_count": measurement_count,
        "bytes_per_shot": bytes_per_shot,
    }
    precompile_elapsed_ns = telemetry.get("precompile_elapsed_ns")
    if not isinstance(precompile_elapsed_ns, int) or isinstance(precompile_elapsed_ns, bool):
        raise RunnerError("worker telemetry precompile_elapsed_ns must be an integer")
    if variant in PRECOMPILED_VARIANTS:
        if precompile_elapsed_ns <= 0:
            raise RunnerError("precompiled worker telemetry precompile_elapsed_ns must be positive")
    elif precompile_elapsed_ns != 0:
        raise RunnerError("direct worker telemetry precompile_elapsed_ns must be zero")
    for key, value in expected.items():
        if telemetry.get(key) != value:
            raise RunnerError(f"worker telemetry {key}: expected {value!r}, got {telemetry.get(key)!r}")


def _run_preflight(command: list[str], *, variant: str, seed: int) -> dict[str, Any]:
    with tempfile.TemporaryDirectory() as temp_dir:
        fixture = Path(temp_dir) / "known_answer.stim"
        fixture.write_text(
            "X 0\nLOSS(0) 0\nM 0\n" if variant == ATOM_LOSS_VARIANT else "X 0\nM 0\n",
            encoding="utf-8",
        )
        session = WorkerSession(command, input_path=fixture, seed=seed)
        try:
            ready = session.read_ready()
            _validate_telemetry(
                ready,
                variant=variant,
                fixture_sha256=_sha256(fixture),
                measurement_count=1,
                bytes_per_shot=1,
                sample_call_count=0,
            )
            call_count, data, _, _ = session.sample(0, 1)
            if call_count != 1 or data != b"\x01":
                raise RunnerError("known-answer preflight expected one result byte 0x01")
            final = session.stop()
            _validate_telemetry(
                final,
                variant=variant,
                fixture_sha256=_sha256(fixture),
                measurement_count=1,
                bytes_per_shot=1,
                sample_call_count=1,
            )
            return {
                "variant": variant,
                "argv": session.command,
                "result_hex": data.hex(),
                "ready": ready,
                "final": final,
            }
        except BaseException:
            session.abort()
            raise


def _run_variant(
    *,
    variant: str,
    command: list[str],
    input_path: Path,
    fixture_sha256: str,
    seed: int,
    shots: int,
    output_format: str,
    measurement_count: int,
    bytes_per_shot: int,
    expected_output_bytes: int,
    warmup_rounds: int,
    measure_rounds: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    session = WorkerSession(command, input_path=input_path, seed=seed)
    records: list[dict[str, Any]] = []
    total_rounds = warmup_rounds + measure_rounds
    try:
        ready = session.read_ready()
        _validate_telemetry(
            ready,
            variant=variant,
            fixture_sha256=fixture_sha256,
            measurement_count=measurement_count,
            bytes_per_shot=bytes_per_shot,
            sample_call_count=0,
        )
        records.append({"record_type": "ready", "variant": variant, "telemetry": ready})
        for request_id in range(total_rounds):
            call_count, data, sample_elapsed_ns, b8_elapsed_ns = session.sample(request_id, shots)
            if call_count != request_id + 1:
                raise RunnerError(f"{variant} RESULT sample count {call_count} does not match {request_id + 1}")
            if len(data) != expected_output_bytes:
                raise RunnerError(f"{variant} RESULT length {len(data)} does not match {expected_output_bytes}")
            records.append(
                {
                    "record_type": "sample",
                    "variant": variant,
                    "request_id": request_id,
                    "sample_call_count": call_count,
                    "shots": shots,
                    "output_format": output_format,
                    "warmup": request_id < warmup_rounds,
                    "sample_elapsed_ns": sample_elapsed_ns,
                    "b8_elapsed_ns": b8_elapsed_ns,
                    "worker_total_elapsed_ns": sample_elapsed_ns + b8_elapsed_ns,
                    "output_bytes": len(data),
                }
            )
        final = session.stop()
        _validate_telemetry(
            final,
            variant=variant,
            fixture_sha256=fixture_sha256,
            measurement_count=measurement_count,
            bytes_per_shot=bytes_per_shot,
            sample_call_count=total_rounds,
        )
        records.append({"record_type": "final", "variant": variant, "telemetry": final})
        return records, {"variant": variant, "command": session.command}
    except BaseException:
        session.abort()
        raise


def _summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    variants = []
    for variant in VARIANTS:
        measured = [
            record
            for record in records
            if record["record_type"] == "sample" and record["variant"] == variant and not record["warmup"]
        ]
        median_call = sorted(measured, key=lambda record: record["worker_total_elapsed_ns"])[
            len(measured) // 2
        ]
        variants.append(
            {
                "variant": variant,
                "sample_count": len(measured),
                "precompile_elapsed_ns": next(
                    record["telemetry"]["precompile_elapsed_ns"]
                    for record in records
                    if record["record_type"] == "ready" and record["variant"] == variant
                ),
                "median_call_sample_elapsed_ns": median_call["sample_elapsed_ns"],
                "median_call_b8_elapsed_ns": median_call["b8_elapsed_ns"],
                "median_worker_total_elapsed_ns": statistics.median(
                    record["worker_total_elapsed_ns"] for record in measured
                ),
            }
        )
    return {"measured_records": sum(item["sample_count"] for item in variants), "variants": variants}


def _render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# Split precompile, sample, and b8 benchmark",
        "",
        "| variant | sample_count | precompile_elapsed_ns | median_call_sample_elapsed_ns | median_call_b8_elapsed_ns | median_worker_total_elapsed_ns |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for variant in summary["variants"]:
        lines.append(
            f"| {variant['variant']} | {variant['sample_count']} | "
            f"{variant['precompile_elapsed_ns']} | "
            f"{variant['median_call_sample_elapsed_ns']} | "
            f"{variant['median_call_b8_elapsed_ns']} | "
            f"{variant['median_worker_total_elapsed_ns']} |"
        )
    lines.extend(["", f"Measured records: {summary['measured_records']}", ""])
    return "\n".join(lines)


def _collect_environment(
    *,
    args: argparse.Namespace,
    case: dict[str, Any],
    input_path: Path,
    atom_loss_input_path: Path,
    rstim_command: list[str],
    worker_details: list[dict[str, Any]],
    preflight_results: list[dict[str, Any]],
    stim_probe: dict[str, Any],
) -> dict[str, Any]:
    fair_manifest_path = args.manifest.resolve()
    source_manifest_path = (REPO_ROOT / case["source_manifest_path"]).resolve()
    stim_worker_module_path = (PACKAGE_DIR / "workers/stim_compiled_steady.py").resolve()
    python_executable = _resolve_executable("python3") or Path(sys.executable).resolve()
    rstim_worker_path = _resolve_executable(rstim_command[0])
    stim_extension_path = Path(str(stim_probe.get("path"))).resolve() if stim_probe.get("path") else None
    git_commit = _version_string(["git", "rev-parse", "HEAD"])
    rstim_version = _version_string([*rstim_command, "--version"])
    input_path_relative = _repo_relative(input_path)
    atom_loss_input_path_relative = _repo_relative(atom_loss_input_path)
    worker_argv = {
        variant: _portable_worker_argv(
            variant,
            atom_loss_input_path_relative if variant == ATOM_LOSS_VARIANT else input_path_relative,
            seed=args.seed,
        )
        for variant in VARIANTS
    }
    portable_preflight_results = []
    for item in preflight_results:
        portable = {**item}
        portable["argv"] = _portable_worker_argv(
            str(item["variant"]),
            "fixture://sample-b8-known-answer",
            seed=args.seed,
        )
        portable_preflight_results.append(portable)
    runtime_identities = [
        {
            "role": "tool://python",
            "version": _version_string([str(python_executable), "--version"]) or f"Python {platform.python_version()}",
            "basename": python_executable.name,
            "sha256": _sha256_required(python_executable, "Python executable"),
        },
        {
            "role": "tool://stim-extension",
            "version": str(stim_probe.get("version") or case["stim_version"]),
            "basename": stim_extension_path.name if stim_extension_path is not None else "",
            "sha256": _sha256_required(stim_extension_path, "Stim extension"),
        },
        {
            "role": "tool://stim-worker",
            "version": case["stim_version"],
            "basename": stim_worker_module_path.name,
            "sha256": _sha256(stim_worker_module_path),
        },
        {
            "role": "tool://rstim-worker",
            "version": str(rstim_version or ""),
            "basename": rstim_worker_path.name if rstim_worker_path is not None else "",
            "sha256": _sha256_required(rstim_worker_path, "rstim worker"),
        },
    ]
    return {
        "git_commit": git_commit,
        "os": platform.platform(),
        "cpu_model": _cpu_model(),
        "profile": args.profile,
        "timer_scope": TIMER_SCOPE,
        "seed_policy": "precompiled_and_rstim_interpreted_seed_once;stim_direct_seed_per_call",
        "stim_version": case["stim_version"],
        "stim_python_probe": {
            "status": stim_probe.get("status"),
            "version": stim_probe.get("version"),
            "extension_module": stim_probe.get("extension_module"),
        },
        "rstim_version": rstim_version,
        "rustc_version": _version_string(["rustc", "--version"]),
        "fair_manifest_path": _repo_relative(fair_manifest_path),
        "fair_manifest_sha256": _sha256(fair_manifest_path),
        "source_manifest_path": _repo_relative(source_manifest_path),
        "source_manifest_sha256": _sha256(source_manifest_path),
        "fixture_path": input_path_relative,
        "fixture_sha256": case["canonical_input_sha256"],
        "atom_loss_fixture_path": atom_loss_input_path_relative,
        "atom_loss_fixture_sha256": _sha256(atom_loss_input_path),
        "worker_argv": worker_argv,
        "canonical_worker_argv": worker_argv,
        "stim_worker_module_path": _repo_relative(stim_worker_module_path),
        "stim_worker_module_sha256": _sha256(stim_worker_module_path),
        "runtime_identities": runtime_identities,
        "protocol_version": PROTOCOL_VERSION,
        "seed": args.seed,
        "warmup_rounds": args.warmup_rounds,
        "measure_rounds": args.measure_rounds,
        "known_answer_preflight": portable_preflight_results,
        "workers": [{"variant": variant, "command": worker_argv[variant]} for variant in VARIANTS],
    }


def run_compiled_steady(args: argparse.Namespace) -> None:
    manifest = fair_cli_contract.load_manifest(args.manifest)
    case = fair_cli_contract.find_case(manifest, args.case)
    errors = fair_cli_contract.validate_case(case, manifest_path=args.manifest, repo_root=REPO_ROOT)
    if errors:
        raise RunnerError("fair CLI contract rejected case: " + "; ".join(errors))
    if case.get("stim_version") != "1.15.0":
        raise RunnerError("compiled steady-state runner requires stim==1.15.0")

    input_path = (REPO_ROOT / case["canonical_input_path"]).resolve()
    atom_loss_input_path = (REPO_ROOT / ATOM_LOSS_FIXTURE_PATH).resolve()
    if not atom_loss_input_path.is_file():
        raise RunnerError(f"atom-loss fixture does not exist: {atom_loss_input_path}")
    stim_command = args.stim_worker_command or default_stim_worker_command()
    rstim_command = args.rstim_worker_command or build_rstim_worker(args.profile)
    stim_probe = _probe_stim_python()
    if stim_probe.get("version") != "1.15.0":
        raise RunnerError(
            "compiled steady-state runner requires stim==1.15.0, "
            f"got {stim_probe.get('version')!r}"
        )

    preflight_results = []
    for variant in VARIANTS:
        base_command = stim_command if variant in STIM_VARIANTS else rstim_command
        preflight_results.append(
            _run_preflight([*base_command, "--variant", variant], variant=variant, seed=args.seed)
        )

    all_records: list[dict[str, Any]] = []
    worker_details: list[dict[str, Any]] = []
    for variant in VARIANTS:
        base_command = stim_command if variant in STIM_VARIANTS else rstim_command
        variant_input_path = atom_loss_input_path if variant == ATOM_LOSS_VARIANT else input_path
        records, details = _run_variant(
            variant=variant,
            command=[*base_command, "--variant", variant],
            input_path=variant_input_path,
            fixture_sha256=_sha256(variant_input_path),
            seed=args.seed,
            shots=case["shots"],
            output_format=case["output_format"],
            measurement_count=case["measurement_count"],
            bytes_per_shot=case["bytes_per_shot"],
            expected_output_bytes=case["expected_output_bytes"],
            warmup_rounds=args.warmup_rounds,
            measure_rounds=args.measure_rounds,
        )
        all_records.extend(records)
        worker_details.append(details)

    out_dir = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "raw.jsonl").write_text("".join(json.dumps(record, sort_keys=True) + "\n" for record in all_records))
    summary = _summary(all_records)
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    environment = _collect_environment(
        args=args,
        case=case,
        input_path=input_path,
        atom_loss_input_path=atom_loss_input_path,
        rstim_command=rstim_command,
        worker_details=worker_details,
        preflight_results=preflight_results,
        stim_probe=stim_probe,
    )
    (out_dir / "environment.json").write_text(json.dumps(environment, indent=2, sort_keys=True) + "\n")
    (out_dir / "report.md").write_text(_render_report(summary))
    artifact_hashes = {
        filename: _sha256(out_dir / filename)
        for filename in ("raw.jsonl", "summary.json", "report.md", "environment.json")
    }
    (out_dir / "artifact-sha256.json").write_text(
        json.dumps(artifact_hashes, indent=2, sort_keys=True) + "\n"
    )
    print(
        "PASS split precompile/sample/b8 lifecycle variants=5 "
        f"calls={args.warmup_rounds + args.measure_rounds} measured={summary['measured_records']}"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the compiled steady-state benchmark.")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--profile", choices=["release", "debug"], required=True)
    parser.add_argument("--warmup-rounds", type=int, default=2)
    parser.add_argument("--measure-rounds", type=int, default=7)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--stim-worker-command", type=_parse_command, help=argparse.SUPPRESS)
    parser.add_argument("--rstim-worker-command", type=_parse_command, help=argparse.SUPPRESS)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.warmup_rounds < 0 or args.measure_rounds < 1:
        print("warmup rounds must be nonnegative and measure rounds must be positive", file=sys.stderr)
        return 1
    try:
        run_compiled_steady(args)
    except (EOFError, OSError, RunnerError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
