from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from benchmarks.rstim_vs_stim_simulator import inspect_fixture_load
from benchmarks.rstim_vs_stim_simulator.validate_cases import load_manifest, validate_manifest
from benchmarks.rstim_vs_stim_simulator.verify_correctness import (
    compare_sample_sets,
    parse_01_samples,
    resolve_case_input_path,
    select_columns,
    select_pairs,
)


PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]
MODULE_NAME = "benchmarks.rstim_vs_stim_simulator.run_frame_instruction_wide_benchmark"
EXPECTED_CASE_ID = "stim_surface_d11_r100"
EXPECTED_STIM_VERSION = "1.15.0"
EXPECTED_FIXTURE_SHA256 = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
EXPECTED_MANIFEST_SHA256 = "9fc35393f362f709e90bfd64ab08eda5140844974a7e685fd1e5614f67e0c921"
TIMER_SCOPE = "process_spawn_stdout_stderr_drain_exit"
OUTPUT_FORMAT = "b8"
CORRECTNESS_MODE = "detect"
CORRECTNESS_OUTPUT_FORMAT = "01"
EXPECTED_OUTPUT_BITS = 12_121
EXPECTED_BYTES_PER_SHOT = 1_516
EXPECTED_OUTPUT_BYTES = 1_552_384
EXPECTED_DETECTORS = 12_000
EXPECTED_OBSERVABLES = 1
EXPECTED_OPERATION_TOTALS = {
    "X_ERROR": {"instructions": 203, "targets": 24_362, "iterator_builds": 203, "attempt_count": 24_946_688},
    "DEPOLARIZE1": {"instructions": 200, "targets": 12_000, "iterator_builds": 200, "attempt_count": 12_288_000},
    "DEPOLARIZE2": {"instructions": 400, "pairs": 44_000, "iterator_builds": 400, "attempt_count": 45_056_000},
}
OPERATION_ORDER = tuple(EXPECTED_OPERATION_TOTALS)
ARTIFACT_FILES = (
    "raw.jsonl",
    "summary.json",
    "report.md",
    "environment.json",
    "fixture-load.json",
    "correctness-summary.json",
)


class RunnerError(RuntimeError):
    pass


@dataclass(frozen=True)
class CliResult:
    exit_code: int
    stdout: bytes
    stderr: bytes
    elapsed_ns: int


@dataclass(frozen=True)
class MeasurementSummary:
    elapsed_ns: int
    stdout_sha256: str
    actual_output_bytes: int
    expected_output_bytes: int
    output_bits: int
    bytes_per_shot: int


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


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


def _probe_stdout(argv: list[str]) -> str:
    completed = subprocess.run(
        argv,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RunnerError(f"{argv[0]} exited with code {completed.returncode}: {detail}")
    return completed.stdout.strip()


def _probe_stdout_or_failed(argv: list[str]) -> str:
    try:
        return _probe_stdout(argv)
    except (OSError, RunnerError) as error:
        return f"failed: {error}"


def _extract_semver(text: str) -> str | None:
    match = re.search(r"\b(\d+\.\d+\.\d+)\b", text)
    return match.group(1) if match is not None else None


def _stim_version(stim_binary: Path) -> str:
    completed = subprocess.run(
        [str(stim_binary), "--version"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RunnerError(f"Stim version probe exited with code {completed.returncode}: {detail}")
    version = _extract_semver("\n".join([completed.stdout, completed.stderr]))
    if version is None:
        version = _extract_semver(_probe_stdout(["python3", "-c", "import stim; print(stim.__version__)"]))
    if version != EXPECTED_STIM_VERSION:
        raise RunnerError(f"Stim CLI must be version {EXPECTED_STIM_VERSION}; got {version or 'unknown'}")
    return version


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


def _git_commit() -> str:
    return _probe_stdout(["git", "rev-parse", "HEAD"])


def _git_dirty() -> bool:
    return bool(_probe_stdout(["git", "status", "--porcelain"]))


def time_cli(argv: list[str]) -> CliResult:
    started_ns = time.perf_counter_ns()
    process = subprocess.Popen(
        argv,
        cwd=REPO_ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    stdout, stderr = process.communicate()
    return CliResult(
        exit_code=process.returncode,
        stdout=stdout,
        stderr=stderr,
        elapsed_ns=time.perf_counter_ns() - started_ns,
    )


def _find_case(manifest: dict[str, Any], case_id: str) -> dict[str, Any]:
    cases = manifest.get("cases")
    if not isinstance(cases, list):
        raise RunnerError("manifest cases must be an array")
    for case in cases:
        if isinstance(case, dict) and case.get("case_id") == case_id:
            return case
    raise RunnerError(f'case "{case_id}" not found in manifest')


def load_case(manifest_path: Path, case_id: str) -> tuple[dict[str, Any], Path]:
    manifest = load_manifest(manifest_path)
    errors = validate_manifest(manifest, manifest_path.parent)
    if errors:
        raise RunnerError("; ".join(errors))
    case = _find_case(manifest, case_id)
    fixture = resolve_case_input_path(str(case["canonical_input_path"]), manifest_path.parent)
    return case, fixture


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, records: list[dict[str, Any]]) -> None:
    path.write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
        encoding="utf-8",
    )


def _load_telemetry(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        raise RunnerError(f"missing benchmark telemetry: {path}")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise RunnerError(f"benchmark telemetry is not valid JSON: {error}") from error
    if not isinstance(payload, dict) or not isinstance(payload.get("operations"), list):
        raise RunnerError("benchmark telemetry must contain an operations array")
    operations = payload["operations"]
    if not all(isinstance(item, dict) for item in operations):
        raise RunnerError("benchmark telemetry operations must be JSON objects")
    return operations


def aggregate_telemetry(
    operations: list[dict[str, Any]],
    *,
    case_id: str,
    seed: int,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for operation in OPERATION_ORDER:
        selected = [record for record in operations if record.get("operation") == operation]
        if not selected:
            raise RunnerError(f"benchmark telemetry missing operation {operation}")
        row: dict[str, Any] = {
            "case_id": case_id,
            "phase": "measured",
            "round_index": 0,
            "seed": seed,
            "operation": operation,
            "sampling_path": "sparse",
            "instructions": len(selected),
            "iterator_builds": sum(_require_int(record.get("iterator_builds"), f"{operation} iterator_builds") for record in selected),
            "attempt_count": sum(_require_int(record.get("attempt_count"), f"{operation} attempt_count") for record in selected),
        }
        paths = {record.get("sampling_path") for record in selected}
        if paths != {"sparse"}:
            raise RunnerError(f"{operation} telemetry must all use sparse path")
        if operation == "DEPOLARIZE2":
            row["pairs"] = sum(_require_int(record.get("pairs"), "DEPOLARIZE2 pairs") for record in selected)
        else:
            row["targets"] = sum(_require_int(record.get("targets"), f"{operation} targets") for record in selected)
        rows.append(row)
    _validate_operation_totals(rows)
    return rows


def _require_int(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise RunnerError(f"{label} must be an integer")
    return value


def _validate_operation_totals(rows: list[dict[str, Any]]) -> None:
    by_operation = {row["operation"]: row for row in rows}
    for operation, expected in EXPECTED_OPERATION_TOTALS.items():
        row = by_operation.get(operation)
        if row is None:
            raise RunnerError(f"missing raw operation row: {operation}")
        for field, expected_value in expected.items():
            if row.get(field) != expected_value:
                raise RunnerError(
                    f"{operation} {field} must be {expected_value}, got {row.get(field)!r}"
                )


def _measurement_summary(result: CliResult, *, expected_bytes: int) -> MeasurementSummary:
    if result.exit_code != 0:
        raise RunnerError(
            "measurement rstim sample failed: "
            + (result.stderr.decode(errors="replace").strip() or f"exit {result.exit_code}")
        )
    actual_bytes = len(result.stdout)
    if actual_bytes != expected_bytes:
        raise RunnerError(f"measurement output bytes must be {expected_bytes}, got {actual_bytes}")
    return MeasurementSummary(
        elapsed_ns=result.elapsed_ns,
        stdout_sha256=hashlib.sha256(result.stdout).hexdigest(),
        actual_output_bytes=actual_bytes,
        expected_output_bytes=expected_bytes,
        output_bits=EXPECTED_OUTPUT_BITS,
        bytes_per_shot=EXPECTED_BYTES_PER_SHOT,
    )


def _measurement_from_record(record: dict[str, Any]) -> MeasurementSummary:
    return MeasurementSummary(
        elapsed_ns=_require_int(record.get("elapsed_ns"), "elapsed_ns"),
        stdout_sha256=str(record["stdout_sha256"]),
        actual_output_bytes=_require_int(record.get("actual_output_bytes"), "actual_output_bytes"),
        expected_output_bytes=_require_int(record.get("expected_output_bytes"), "expected_output_bytes"),
        output_bits=_require_int(record.get("output_bits"), "output_bits"),
        bytes_per_shot=_require_int(record.get("bytes_per_shot"), "bytes_per_shot"),
    )


def attach_measurement(rows: list[dict[str, Any]], measurement: MeasurementSummary) -> list[dict[str, Any]]:
    enriched = []
    for row in rows:
        record = dict(row)
        record.update(
            {
                "elapsed_ns": measurement.elapsed_ns,
                "stdout_sha256": measurement.stdout_sha256,
                "actual_output_bytes": measurement.actual_output_bytes,
                "expected_output_bytes": measurement.expected_output_bytes,
                "output_bits": measurement.output_bits,
                "bytes_per_shot": measurement.bytes_per_shot,
                "output_format": OUTPUT_FORMAT,
                "timer_scope": TIMER_SCOPE,
            }
        )
        enriched.append(record)
    return enriched


def derive_summary(
    records: list[dict[str, Any]],
    *,
    measurement: MeasurementSummary | None = None,
) -> dict[str, Any]:
    if measurement is None:
        if not records:
            raise RunnerError("cannot derive summary from empty raw records")
        measurement = _measurement_from_record(records[0])
    operations = []
    for operation in OPERATION_ORDER:
        matches = [record for record in records if record.get("operation") == operation]
        if len(matches) != 1:
            raise RunnerError(f"raw records must contain exactly one {operation} row")
        row = dict(matches[0])
        operation_summary = {
            "operation": operation,
            "sampling_path": row["sampling_path"],
            "instructions": row["instructions"],
            "iterator_builds": row["iterator_builds"],
            "attempt_count": row["attempt_count"],
        }
        if operation == "DEPOLARIZE2":
            operation_summary["pairs"] = row["pairs"]
        else:
            operation_summary["targets"] = row["targets"]
        operations.append(operation_summary)
    return {
        "case_id": records[0]["case_id"],
        "seed": records[0]["seed"],
        "shots": 1024,
        "phase": "measured",
        "round_index": 0,
        "operations": operations,
        "totals": {
            "instructions": sum(row["instructions"] for row in operations),
            "iterator_builds": sum(row["iterator_builds"] for row in operations),
            "attempt_count": sum(row["attempt_count"] for row in operations),
        },
        "measurement": {
            "timer_scope": TIMER_SCOPE,
            "output_format": OUTPUT_FORMAT,
            "output_bits": measurement.output_bits,
            "bytes_per_shot": measurement.bytes_per_shot,
            "expected_output_bytes": measurement.expected_output_bytes,
            "actual_output_bytes": measurement.actual_output_bytes,
            "stdout_sha256": measurement.stdout_sha256,
            "elapsed_ns": measurement.elapsed_ns,
        },
    }


def render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# Instruction-Wide Frame-Noise Evidence",
        "",
        f"Case: `{summary['case_id']}`",
        f"Seed: `{summary['seed']}`",
        f"Timer scope: `{summary['measurement']['timer_scope']}`",
        "",
        "| Operation | Instructions/builds | Targets/pairs | Attempts |",
        "|---|---:|---:|---:|",
    ]
    for row in summary["operations"]:
        target_value = row.get("pairs", row.get("targets"))
        lines.append(
            f"| `{row['operation']}` | {row['iterator_builds']} | {target_value} | {row['attempt_count']} |"
        )
    totals = summary["totals"]
    target_total = sum(row.get("pairs", row.get("targets", 0)) for row in summary["operations"])
    lines.extend(
        [
            f"| **Total** | **{totals['iterator_builds']}** | **{target_total}** | **{totals['attempt_count']}** |",
            "",
            "Measurement output:",
            f"- bits per shot: {summary['measurement']['output_bits']}",
            f"- bytes per shot: {summary['measurement']['bytes_per_shot']}",
            f"- bytes for run: {summary['measurement']['actual_output_bytes']}",
            "",
        ]
    )
    return "\n".join(lines)


def _run_measurement(
    *,
    rstim_binary: Path,
    fixture: Path,
    shots: int,
    seed: int,
    telemetry_path: Path,
) -> tuple[CliResult, list[str]]:
    argv = [
        str(rstim_binary),
        "--benchmark-telemetry-json",
        str(telemetry_path),
        "sample",
        "--shots",
        str(shots),
        "--seed",
        str(seed),
        "--out_format",
        OUTPUT_FORMAT,
        "--in",
        str(fixture),
    ]
    return time_cli(argv), argv


def _run_detect(
    binary: Path,
    *,
    fixture: Path,
    shots: int,
    seed: int,
) -> tuple[CliResult, list[str]]:
    argv = [
        str(binary),
        CORRECTNESS_MODE,
        "--shots",
        str(shots),
        "--seed",
        str(seed),
        "--out_format",
        CORRECTNESS_OUTPUT_FORMAT,
        "--append_observables",
        "--in",
        str(fixture),
    ]
    return time_cli(argv), argv


def run_correctness(
    *,
    stim_binary: Path,
    rstim_binary: Path,
    fixture: Path,
    shots: int,
    seed: int,
) -> tuple[dict[str, Any], dict[str, list[str]]]:
    stim_result, stim_argv = _run_detect(stim_binary, fixture=fixture, shots=shots, seed=seed)
    rstim_result, rstim_argv = _run_detect(rstim_binary, fixture=fixture, shots=shots, seed=seed)
    if stim_result.exit_code != 0:
        raise RunnerError(f"stim detect failed: {stim_result.stderr.decode(errors='replace').strip()}")
    if rstim_result.exit_code != 0:
        raise RunnerError(f"rstim detect failed: {rstim_result.stderr.decode(errors='replace').strip()}")
    expected_bits = EXPECTED_DETECTORS + EXPECTED_OBSERVABLES
    expected_bytes = (expected_bits + 1) * shots
    stim_samples = parse_01_samples(
        stim_result.stdout.decode("ascii"),
        expected_bits=expected_bits,
        expected_shots=shots,
    )
    rstim_samples = parse_01_samples(
        rstim_result.stdout.decode("ascii"),
        expected_bits=expected_bits,
        expected_shots=shots,
    )
    selected_columns = select_columns(expected_bits, observable_count=EXPECTED_OBSERVABLES)
    selected_pairs = select_pairs(
        selected_columns,
        bit_count=expected_bits,
        observable_count=EXPECTED_OBSERVABLES,
    )
    comparison = compare_sample_sets(
        stim_samples,
        rstim_samples,
        columns=selected_columns,
        pairs=selected_pairs,
    )
    if comparison["status"] != "pass":
        raise RunnerError("detect comparison failed: " + "; ".join(comparison["failure_reasons"]))
    stim_hash = hashlib.sha256(stim_result.stdout).hexdigest()
    rstim_hash = hashlib.sha256(rstim_result.stdout).hexdigest()
    return (
        {
            "status": comparison["status"],
            "mode": CORRECTNESS_MODE,
            "output_format": CORRECTNESS_OUTPUT_FORMAT,
            "shots": shots,
            "seed": seed,
            "detectors": EXPECTED_DETECTORS,
            "observables": EXPECTED_OBSERVABLES,
            "expected_output_bytes": expected_bytes,
            "stim_output_bytes": len(stim_result.stdout),
            "rstim_output_bytes": len(rstim_result.stdout),
            "stim_stdout_sha256": stim_hash,
            "rstim_stdout_sha256": rstim_hash,
            "sample_count": comparison["sample_count"],
            "selected_columns": selected_columns,
            "selected_pairs": [list(pair) for pair in selected_pairs],
            "max_delta": comparison["max_delta"],
            "max_tolerance": comparison["max_tolerance"],
            "failure_reasons": comparison["failure_reasons"],
            "stim_elapsed_ns": stim_result.elapsed_ns,
            "rstim_elapsed_ns": rstim_result.elapsed_ns,
        },
        {"correctness_stim": stim_argv, "correctness_rstim": rstim_argv},
    )


def fixture_load_report(case: dict[str, Any], *, manifest_path: Path) -> dict[str, Any]:
    return inspect_fixture_load.build_report(case, manifest_path=manifest_path, base_dir=manifest_path.parent)


def write_artifact_hashes(out_dir: Path) -> dict[str, str]:
    payload = {filename: sha256_file(out_dir / filename) for filename in ARTIFACT_FILES}
    write_json(out_dir / "artifact-sha256.json", payload)
    return payload


def run_benchmark(args: argparse.Namespace) -> None:
    if args.case != EXPECTED_CASE_ID:
        raise RunnerError(f"frame instruction-wide evidence requires --case {EXPECTED_CASE_ID}")
    if args.profile != "release":
        raise RunnerError("frame instruction-wide evidence requires --profile release")
    if args.shots != 1024 or args.seed != 7 or args.warmup_rounds != 0 or args.measure_rounds != 1:
        raise RunnerError("frame instruction-wide evidence requires shots=1024 seed=7 warmup=0 measure=1")

    manifest_path = args.manifest.resolve()
    rstim_binary = _resolve_executable(args.rstim)
    stim_binary = _resolve_executable(args.stim)
    case, fixture = load_case(manifest_path, args.case)
    if sha256_file(fixture) != EXPECTED_FIXTURE_SHA256:
        raise RunnerError("fixture SHA-256 does not match canonical d11/r100 fixture")
    if sha256_file(manifest_path) != EXPECTED_MANIFEST_SHA256:
        raise RunnerError("manifest SHA-256 does not match canonical full manifest")
    stim_version = _stim_version(stim_binary)

    out_dir = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    fixture_load = fixture_load_report(case, manifest_path=manifest_path)
    write_json(out_dir / "fixture-load.json", fixture_load)

    with tempfile.TemporaryDirectory(dir=out_dir) as temp_dir:
        telemetry_path = Path(temp_dir) / "rstim-telemetry.json"
        measurement_result, measurement_argv = _run_measurement(
            rstim_binary=rstim_binary,
            fixture=fixture,
            shots=args.shots,
            seed=args.seed,
            telemetry_path=telemetry_path,
        )
        measurement = _measurement_summary(measurement_result, expected_bytes=EXPECTED_OUTPUT_BYTES)
        telemetry = _load_telemetry(telemetry_path)

    raw_rows = attach_measurement(
        aggregate_telemetry(telemetry, case_id=args.case, seed=args.seed),
        measurement,
    )
    write_jsonl(out_dir / "raw.jsonl", raw_rows)
    summary = derive_summary(raw_rows, measurement=measurement)
    write_json(out_dir / "summary.json", summary)
    (out_dir / "report.md").write_text(render_report(summary), encoding="utf-8")

    correctness, correctness_argv = run_correctness(
        stim_binary=stim_binary,
        rstim_binary=rstim_binary,
        fixture=fixture,
        shots=args.shots,
        seed=args.seed,
    )
    write_json(out_dir / "correctness-summary.json", correctness)

    environment = {
        "git_commit": _git_commit(),
        "git_dirty": _git_dirty(),
        "profile": args.profile,
        "case_id": args.case,
        "shots": args.shots,
        "seed": args.seed,
        "warmup_rounds": args.warmup_rounds,
        "measure_rounds": args.measure_rounds,
        "timer_scope": TIMER_SCOPE,
        "stim_version": stim_version,
        "rstim_version": _probe_stdout_or_failed([str(rstim_binary)]),
        "rustc_version": _probe_stdout_or_failed(["rustc", "--version"]),
        "os": platform.platform(),
        "cpu_model": _cpu_model(),
        "fixture": str(fixture),
        "fixture_sha256": sha256_file(fixture),
        "manifest": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "rstim_binary": str(rstim_binary),
        "rstim_binary_sha256": sha256_file(rstim_binary),
        "runner_argv": [sys.executable, "-m", MODULE_NAME, *sys.argv[1:]],
        "child_argv": {"measurement": measurement_argv, **correctness_argv},
        "artifact_sha256": {},
    }
    write_json(out_dir / "environment.json", environment)
    artifact_hashes = {filename: sha256_file(out_dir / filename) for filename in ARTIFACT_FILES}
    environment["artifact_sha256"] = {
        filename: digest for filename, digest in artifact_hashes.items() if filename != "environment.json"
    }
    write_json(out_dir / "environment.json", environment)
    write_artifact_hashes(out_dir)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run instruction-wide frame-noise evidence benchmark.")
    parser.add_argument("--case", required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--rstim", required=True)
    parser.add_argument("--stim", required=True)
    parser.add_argument("--profile", choices=["release"], required=True)
    parser.add_argument("--shots", type=int, required=True)
    parser.add_argument("--seed", type=int, required=True)
    parser.add_argument("--warmup-rounds", type=int, required=True)
    parser.add_argument("--measure-rounds", type=int, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        run_benchmark(args)
    except (OSError, RunnerError, ValueError, subprocess.SubprocessError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
