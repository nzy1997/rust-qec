from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
from pathlib import Path
from typing import Any


PROTOCOL = "reference-build-v1"
TIMER_SCOPE = "reference_build_only"
EXPECTED_MANIFEST_SHA256 = "9fc35393f362f709e90bfd64ab08eda5140844974a7e685fd1e5614f67e0c921"
EXPECTED_REFERENCE_SHA256 = "d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d"
EXPECTED_MEASUREMENT_BITS = 12121
EXPECTED_PACKED_BYTES = 1516
EXPECTED_STIM_VERSION = "1.15.0"
CANONICAL_WARMUP_ROUNDS = 2
CANONICAL_MEASURE_ROUNDS = 7
STIM_VARIANT = "stim-reference-b8"
RSTIM_CANONICAL_VARIANT = "rstim-canonical-reference-b8"
RSTIM_DIRECT_VARIANT = "rstim-direct-repeat-reference-b8"
STIM_BACKEND = "stim_reference"
RSTIM_CANONICAL_BACKEND = "canonical_roundtrip"
RSTIM_DIRECT_BACKEND = "direct_inverse_repeat_folded"
BASELINE_SUMMARY_SHA256 = "614658cf8213b486752f1fe53b7d864561abbe41c2eefd799fc8fa34883270a5"
SEED_POLICY = "deterministic_no_seed_reference_builds"

PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]
MODULE_NAME = "benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark"
STIM_WORKER_MODULE = "benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build"
PYTHON_ROLE = "tool://python"
STIM_PYTHON_ROLE = "tool://stim-python"
RSTIM_WORKER_ROLE = "tool://rstim-reference-worker"
RSTIM_WORKER_VERSION = "rstim 0.1.1"


class RunnerError(RuntimeError):
    pass


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def default_stim_worker_argv(stim_python: str) -> list[str]:
    return [stim_python, "-m", STIM_WORKER_MODULE, "--protocol", PROTOCOL]


def default_rstim_worker_argv(rstim_worker: str, *, strategy: str = "direct") -> list[str]:
    argv = [rstim_worker, "--protocol", PROTOCOL]
    if strategy != "direct":
        argv.extend(["--strategy", strategy])
    return argv


def logical_stim_worker_argv() -> list[str]:
    return default_stim_worker_argv(STIM_PYTHON_ROLE)


def logical_rstim_worker_argv(*, strategy: str = "direct") -> list[str]:
    return default_rstim_worker_argv(RSTIM_WORKER_ROLE, strategy=strategy)


def _rstim_worker_has_phase_counters(variant: str) -> bool:
    return variant in {RSTIM_CANONICAL_VARIANT, RSTIM_DIRECT_VARIANT}


def write_artifact_hashes(out_dir: Path) -> None:
    filenames = ("raw.jsonl", "summary.json", "baseline-summary.json", "report.md", "environment.json")
    payload = {filename: sha256_file(out_dir / filename) for filename in filenames}
    (out_dir / "artifact-sha256.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _resolve_executable(raw: str) -> Path:
    path = Path(raw)
    if path.is_absolute() or len(path.parts) > 1:
        candidate = path if path.is_absolute() else REPO_ROOT / path
        if candidate.is_file():
            return candidate.resolve()
        raise RunnerError(f"executable does not exist: {raw}")
    resolved = shutil.which(raw)
    if resolved is not None:
        return Path(resolved).resolve()
    repo_candidate = REPO_ROOT / raw
    if repo_candidate.is_file():
        return repo_candidate.resolve()
    raise RunnerError(f"executable not found: {raw}")


def _probe_stdout(command: list[str]) -> str:
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RunnerError(f"{command[0]} exited with code {completed.returncode}: {detail}")
    return completed.stdout.strip()


def _probe_stdout_or_failed(command: list[str]) -> str:
    try:
        return _probe_stdout(command)
    except (OSError, RunnerError) as error:
        return f"failed: {error}"


def _probe_stim_version(stim_python: str) -> str:
    return _probe_stdout([stim_python, "-c", "import stim; print(stim.__version__)"])


def _git_commit() -> str:
    return _probe_stdout(["git", "rev-parse", "HEAD"])


def _git_dirty() -> bool:
    return bool(_probe_stdout(["git", "status", "--porcelain"]))


def _cpu_model() -> str:
    try:
        completed = subprocess.run(
            ["sysctl", "-n", "machdep.cpu.brand_string"],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        completed = None
    if completed is not None and completed.returncode == 0 and completed.stdout.strip():
        return completed.stdout.strip()
    return platform.processor() or platform.machine() or "unknown"


class WorkerSession:
    def __init__(self, command: list[str]) -> None:
        python_path = os.environ.get("PYTHONPATH")
        environment = dict(os.environ)
        environment["PYTHONPATH"] = str(REPO_ROOT) if not python_path else f"{REPO_ROOT}{os.pathsep}{python_path}"
        self.command = command
        self.process = subprocess.Popen(
            command,
            cwd=REPO_ROOT,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            env=environment,
        )
        assert self.process.stdin is not None
        assert self.process.stdout is not None
        assert self.process.stderr is not None
        self.stdin = self.process.stdin
        self.stdout = self.process.stdout
        self.stderr = self.process.stderr

    def request(self, payload: dict[str, Any]) -> dict[str, Any]:
        self.stdin.write(json.dumps(payload, sort_keys=True) + "\n")
        self.stdin.flush()
        line = self.stdout.readline()
        if not line:
            stderr = self.stderr.read().strip()
            detail = f": {stderr}" if stderr else ""
            raise RunnerError(f"worker exited before response{detail}")
        try:
            response = json.loads(line)
        except json.JSONDecodeError as error:
            raise RunnerError(f"worker response is not valid JSON: {line!r}") from error
        if not isinstance(response, dict):
            raise RunnerError("worker response must be a JSON object")
        if response.get("type") == "error":
            raise RunnerError(f"worker error: {response.get('message')}")
        return response

    def close(self) -> None:
        if not self.stdin.closed:
            self.stdin.close()
        exit_code = self.process.wait(timeout=10)
        stderr = self.stderr.read().strip()
        if exit_code != 0:
            detail = f": {stderr}" if stderr else ""
            raise RunnerError(f"worker exited with status {exit_code}{detail}")

    def abort(self) -> None:
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait()


def _require_equal(actual: Any, expected: Any, message: str) -> None:
    if actual != expected:
        raise RunnerError(f"{message}: expected {expected!r}, got {actual!r}")


def _require_positive_int(value: Any, message: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise RunnerError(f"{message} must be a positive integer, got {value!r}")
    return value


def _require_canonical_round_counts(args: argparse.Namespace) -> None:
    if (
        args.warmup_rounds != CANONICAL_WARMUP_ROUNDS
        or args.measure_rounds != CANONICAL_MEASURE_ROUNDS
    ):
        raise RunnerError(
            "reference-build evidence requires "
            f"--warmup-rounds {CANONICAL_WARMUP_ROUNDS} "
            f"--measure-rounds {CANONICAL_MEASURE_ROUNDS}"
        )


def _validate_loaded(response: dict[str, Any], *, variant: str) -> None:
    _require_equal(response.get("protocol"), PROTOCOL, f"{variant} loaded protocol")
    _require_equal(response.get("type"), "loaded", f"{variant} loaded type")
    _require_equal(response.get("parse_count"), 1, f"{variant} parse_count")
    _require_equal(response.get("measurement_bits"), EXPECTED_MEASUREMENT_BITS, f"{variant} measurement_bits")


def _validate_packed_payload(response: dict[str, Any], *, variant: str) -> None:
    packed_base64 = response.get("packed_base64")
    if not isinstance(packed_base64, str) or not packed_base64:
        raise RunnerError(f"{variant} packed_base64 must be a nonempty string")
    try:
        decoded = base64.b64decode(packed_base64, validate=True)
    except (binascii.Error, ValueError) as error:
        raise RunnerError(f"{variant} packed_base64 must be strict base64") from error

    _require_equal(
        len(decoded),
        EXPECTED_PACKED_BYTES,
        f"{variant} decoded packed byte length",
    )
    decoded_sha256 = hashlib.sha256(decoded).hexdigest()
    _require_equal(
        decoded_sha256,
        EXPECTED_REFERENCE_SHA256,
        f"{variant} decoded packed bytes SHA-256",
    )
    _require_equal(response.get("byte_sha256"), decoded_sha256, f"{variant} byte_sha256")


def _validate_build_response(
    response: dict[str, Any],
    *,
    variant: str,
    backend: str,
    request_id: int,
) -> None:
    _require_equal(response.get("protocol"), PROTOCOL, f"{variant} response protocol")
    _require_equal(response.get("type"), "reference_built", f"{variant} response type")
    _require_equal(response.get("request_id"), request_id, f"{variant} request_id")
    _require_equal(response.get("backend"), backend, f"{variant} backend")
    _require_equal(response.get("parse_count"), 1, f"{variant} parse_count")
    _require_equal(
        response.get("reference_build_count"),
        request_id + 1,
        f"{variant} reference_build_count",
    )
    _require_equal(response.get("measurement_bits"), EXPECTED_MEASUREMENT_BITS, f"{variant} measurement_bits")
    _require_equal(response.get("packed_bytes"), EXPECTED_PACKED_BYTES, f"{variant} packed_bytes")
    _require_equal(response.get("timer_scope"), TIMER_SCOPE, f"{variant} timer_scope")
    _require_positive_int(response.get("elapsed_ns"), f"{variant} elapsed_ns")
    _validate_packed_payload(response, variant=variant)


def _run_variant(
    *,
    variant: str,
    backend: str,
    command: list[str],
    fixture: Path,
    warmup_rounds: int,
    measure_rounds: int,
) -> list[dict[str, Any]]:
    session = WorkerSession(command)
    records: list[dict[str, Any]] = []
    total_rounds = warmup_rounds + measure_rounds
    try:
        loaded = session.request(
            {"protocol": PROTOCOL, "type": "load", "fixture_path": str(fixture)}
        )
        _validate_loaded(loaded, variant=variant)
        for round_index in range(total_rounds):
            response = session.request(
                {
                    "protocol": PROTOCOL,
                    "type": "build_reference",
                    "request_id": round_index,
                    "include_phase_counters": _rstim_worker_has_phase_counters(variant),
                }
            )
            _validate_build_response(
                response,
                variant=variant,
                backend=backend,
                request_id=round_index,
            )
            record = {
                    "protocol": PROTOCOL,
                    "variant": variant,
                    "phase": "warmup" if round_index < warmup_rounds else "measured",
                    "round": round_index,
                    "elapsed_ns": response["elapsed_ns"],
                    "packed_base64": response["packed_base64"],
                    "packed_bytes": response["packed_bytes"],
                    "measurement_bits": response["measurement_bits"],
                    "byte_sha256": response["byte_sha256"],
                    "backend": response["backend"],
                    "timer_scope": response["timer_scope"],
                    "parse_count": response["parse_count"],
                    "reference_build_count": response["reference_build_count"],
            }
            if "phase_counters" in response:
                record["phase_counters"] = response["phase_counters"]
            records.append(record)
        session.close()
        return records
    except BaseException:
        session.abort()
        raise


def derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    variants: list[dict[str, Any]] = []
    for variant, backend in (
        (STIM_VARIANT, STIM_BACKEND),
        (RSTIM_CANONICAL_VARIANT, RSTIM_CANONICAL_BACKEND),
        (RSTIM_DIRECT_VARIANT, RSTIM_DIRECT_BACKEND),
    ):
        variant_records = [record for record in records if record["variant"] == variant]
        measured = [record for record in variant_records if record["phase"] == "measured"]
        elapsed = [record["elapsed_ns"] for record in measured]
        variants.append(
            {
                "variant": variant,
                "count": len(measured),
                "min_elapsed_ns": min(elapsed),
                "median_elapsed_ns": int(statistics.median(elapsed)),
                "max_elapsed_ns": max(elapsed),
                "measurement_bits": EXPECTED_MEASUREMENT_BITS,
                "packed_bytes": EXPECTED_PACKED_BYTES,
                "byte_sha256": EXPECTED_REFERENCE_SHA256,
                "backend": backend,
                "parse_count": 1,
                "final_reference_build_count": variant_records[-1]["reference_build_count"],
            }
        )
    canonical_median = next(
        item["median_elapsed_ns"]
        for item in variants
        if item["variant"] == RSTIM_CANONICAL_VARIANT
    )
    direct_median = next(
        item["median_elapsed_ns"]
        for item in variants
        if item["variant"] == RSTIM_DIRECT_VARIANT
    )
    direct_speedup = canonical_median / direct_median
    return {
        "protocol": PROTOCOL,
        "timer_scope": TIMER_SCOPE,
        "measured_records": 21,
        "direct_speedup": round(direct_speedup, 6),
        "variants": variants,
    }


def render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# Packed Reference-Build Evidence",
        "",
        "| variant | count | min_elapsed_ns | median_elapsed_ns | max_elapsed_ns | backend | parse_count | final_reference_build_count | byte_sha256 |",
        "| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- |",
    ]
    for variant in summary["variants"]:
        lines.append(
            f"| {variant['variant']} | {variant['count']} | {variant['min_elapsed_ns']} | "
            f"{variant['median_elapsed_ns']} | {variant['max_elapsed_ns']} | {variant['backend']} | "
            f"{variant['parse_count']} | {variant['final_reference_build_count']} | {variant['byte_sha256']} |"
        )
    lines.extend(["", f"direct_speedup={summary['direct_speedup']:.6f}"])
    return "\n".join(lines) + "\n"


def preserve_baseline_summary(out_dir: Path) -> None:
    source = out_dir / "summary.json"
    target = out_dir / "baseline-summary.json"
    if target.exists():
        if sha256_file(target) != BASELINE_SUMMARY_SHA256:
            raise RunnerError("baseline-summary.json SHA-256 mismatch")
        return
    if not source.is_file():
        raise RunnerError("cannot preserve baseline summary before summary.json exists")
    if sha256_file(source) != BASELINE_SUMMARY_SHA256:
        raise RunnerError("existing summary.json SHA-256 does not match required baseline")
    target.write_bytes(source.read_bytes())


def _runner_argv(args: argparse.Namespace) -> list[str]:
    return [
        PYTHON_ROLE,
        "-m",
        MODULE_NAME,
        "--fixture",
        _repo_relative_posix(args.fixture, "runner_argv fixture"),
        "--manifest",
        _repo_relative_posix(args.manifest, "runner_argv manifest"),
        "--stim-python",
        STIM_PYTHON_ROLE,
        "--rstim-worker",
        RSTIM_WORKER_ROLE,
        "--warmup-rounds",
        str(args.warmup_rounds),
        "--measure-rounds",
        str(args.measure_rounds),
        "--out-dir",
        _repo_relative_or_abs(args.out_dir),
    ]


def _repo_relative_or_abs(path: Path) -> str:
    resolved = path.resolve()
    try:
        return str(resolved.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def _repo_relative_posix(path: Path, label: str) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(REPO_ROOT).as_posix()
    except ValueError as error:
        raise RunnerError(f"{label} must be under repository root: {resolved}") from error


def collect_environment(
    *,
    args: argparse.Namespace,
    fixture: Path,
    manifest: Path,
    fixture_sha256: str,
    git_commit: str,
    git_dirty: bool,
    stim_version: str,
) -> dict[str, Any]:
    runner_python_path = Path(sys.executable).resolve()
    stim_python_path = _resolve_executable(str(args.stim_python))
    rstim_worker_path = _resolve_executable(str(args.rstim_worker))
    return {
        "profile": "release",
        "protocol": PROTOCOL,
        "timer_scope": TIMER_SCOPE,
        "seed_policy": SEED_POLICY,
        "fixture_path": _repo_relative_posix(fixture, "fixture"),
        "fixture_sha256": fixture_sha256,
        "manifest_path": _repo_relative_posix(manifest, "manifest"),
        "manifest_sha256": EXPECTED_MANIFEST_SHA256,
        "stim_version": stim_version,
        "worker_argv": {
            STIM_VARIANT: logical_stim_worker_argv(),
            RSTIM_CANONICAL_VARIANT: logical_rstim_worker_argv(strategy="canonical"),
            RSTIM_DIRECT_VARIANT: logical_rstim_worker_argv(),
        },
        "canonical_worker_argv": {
            STIM_VARIANT: logical_stim_worker_argv(),
            RSTIM_CANONICAL_VARIANT: logical_rstim_worker_argv(strategy="canonical"),
            RSTIM_DIRECT_VARIANT: logical_rstim_worker_argv(),
        },
        "runner_argv": _runner_argv(args),
        "runtime_identities": [
            {
                "role": PYTHON_ROLE,
                "version": platform.python_version(),
                "basename": runner_python_path.name,
                "sha256": sha256_file(runner_python_path),
            },
            {
                "role": STIM_PYTHON_ROLE,
                "version": stim_version,
                "basename": stim_python_path.name,
                "sha256": sha256_file(stim_python_path),
            },
            {
                "role": RSTIM_WORKER_ROLE,
                "version": RSTIM_WORKER_VERSION,
                "basename": "rstim_reference_build_worker",
                "sha256": sha256_file(rstim_worker_path),
            },
        ],
        "warmup_rounds": args.warmup_rounds,
        "measure_rounds": args.measure_rounds,
        "git_commit": git_commit,
        "git_dirty": git_dirty,
        "os": platform.platform(),
        "cpu_model": _cpu_model(),
        "rustc_version": _probe_stdout_or_failed(["rustc", "--version"]),
        "cargo_version": _probe_stdout_or_failed(["cargo", "--version"]),
        "python_version": platform.python_version(),
    }


def run_reference_build_benchmark(args: argparse.Namespace) -> None:
    _require_canonical_round_counts(args)
    fixture = args.fixture.resolve()
    manifest = args.manifest.resolve()
    if not fixture.is_file():
        raise RunnerError(f"fixture does not exist: {fixture}")
    if not manifest.is_file():
        raise RunnerError(f"manifest does not exist: {manifest}")
    _repo_relative_posix(fixture, "fixture")
    _repo_relative_posix(manifest, "manifest")
    manifest_sha256 = sha256_file(manifest)
    if manifest_sha256 != EXPECTED_MANIFEST_SHA256:
        raise RunnerError(
            "manifest SHA-256 must be "
            f"{EXPECTED_MANIFEST_SHA256}, got {manifest_sha256}"
        )
    fixture_sha256 = sha256_file(fixture)

    git_commit = _git_commit()
    git_dirty = _git_dirty()
    stim_version = _probe_stim_version(str(args.stim_python))
    if stim_version != EXPECTED_STIM_VERSION:
        raise RunnerError(f"requires stim=={EXPECTED_STIM_VERSION}, got {stim_version}")
    stim_command = default_stim_worker_argv(str(args.stim_python))
    rstim_canonical_command = default_rstim_worker_argv(
        str(args.rstim_worker), strategy="canonical"
    )
    rstim_direct_command = default_rstim_worker_argv(str(args.rstim_worker))

    records: list[dict[str, Any]] = []
    records.extend(
        _run_variant(
            variant=STIM_VARIANT,
            backend=STIM_BACKEND,
            command=stim_command,
            fixture=fixture,
            warmup_rounds=args.warmup_rounds,
            measure_rounds=args.measure_rounds,
        )
    )
    records.extend(
        _run_variant(
            variant=RSTIM_CANONICAL_VARIANT,
            backend=RSTIM_CANONICAL_BACKEND,
            command=rstim_canonical_command,
            fixture=fixture,
            warmup_rounds=args.warmup_rounds,
            measure_rounds=args.measure_rounds,
        )
    )
    records.extend(
        _run_variant(
            variant=RSTIM_DIRECT_VARIANT,
            backend=RSTIM_DIRECT_BACKEND,
            command=rstim_direct_command,
            fixture=fixture,
            warmup_rounds=args.warmup_rounds,
            measure_rounds=args.measure_rounds,
        )
    )

    out_dir = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "raw.jsonl").write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
        encoding="utf-8",
    )
    summary = derive_summary(records)
    preserve_baseline_summary(out_dir)
    (out_dir / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (out_dir / "report.md").write_text(render_report(summary), encoding="utf-8")
    environment = collect_environment(
        args=args,
        fixture=fixture,
        manifest=manifest,
        fixture_sha256=fixture_sha256,
        git_commit=git_commit,
        git_dirty=git_dirty,
        stim_version=stim_version,
    )
    (out_dir / "environment.json").write_text(
        json.dumps(environment, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    write_artifact_hashes(out_dir)
    print(
        "PASS packed reference-build evidence "
        f"variants=3 direct_speedup={summary['direct_speedup']:.6f}"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the packed reference-build benchmark.")
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--stim-python", required=True)
    parser.add_argument("--rstim-worker", required=True)
    parser.add_argument("--warmup-rounds", type=int, default=2)
    parser.add_argument("--measure-rounds", type=int, default=7)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        run_reference_build_benchmark(args)
    except (OSError, RunnerError, ValueError, subprocess.TimeoutExpired) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
