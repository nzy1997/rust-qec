from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
import time
from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path
from typing import Any

from benchmarks.rstim_vs_stim_simulator import run_speed_case

PACKAGE_DIR = Path(__file__).resolve().parent
FIXTURES_DIR = PACKAGE_DIR / "fixtures"
REPO_ROOT = PACKAGE_DIR.parents[1]
FULL_CASE_LABEL = "stim-style-surface-dem-sample-d11-r100-b1024"
EXPECTED_VARIANTS = ["stim-sample-dem", "rstim-sample-dem"]
EXPECTED_GENERATION_COMMAND = (
    "stim analyze_errors --decompose_errors < "
    "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim "
    "> benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem"
)
build_rstim = run_speed_case.build_rstim


@dataclass(frozen=True, slots=True)
class DemCase:
    label: str
    dem_path: Path
    metadata_path: Path
    shots: int
    expected_detectors: int
    expected_observables: int


def sha256_file(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def repo_relative_path(path: Path, *, repo_root: Path = REPO_ROOT) -> str:
    resolved_path = path.resolve()
    resolved_root = repo_root.resolve()
    try:
        return resolved_path.relative_to(resolved_root).as_posix()
    except ValueError:
        return str(resolved_path)


def repo_relative_command(command: list[str], *, repo_root: Path = REPO_ROOT) -> list[str]:
    normalized: list[str] = []
    resolved_root = repo_root.resolve()
    for arg in command:
        path = Path(arg)
        if path.is_absolute():
            try:
                normalized.append(path.resolve().relative_to(resolved_root).as_posix())
                continue
            except ValueError:
                pass
        normalized.append(arg)
    return normalized


FULL_CASE = DemCase(
    label=FULL_CASE_LABEL,
    dem_path=FIXTURES_DIR / "stim_surface_code_rotated_memory_z_d11_r100.dem",
    metadata_path=FIXTURES_DIR / "stim_surface_code_rotated_memory_z_d11_r100.dem.metadata.json",
    shots=1024,
    expected_detectors=12000,
    expected_observables=1,
)

DEM_CASES = {FULL_CASE.label: FULL_CASE}


def case_by_label(label: str) -> DemCase:
    try:
        return DEM_CASES[label]
    except KeyError as error:
        raise ValueError(f"unknown benchmark case: {label}") from error


def _mismatch(detail: str) -> ValueError:
    return ValueError(f"DEM metadata mismatch: {detail}")


def _load_metadata(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise _mismatch(f"missing metadata file: {path}") from error
    except json.JSONDecodeError as error:
        raise _mismatch(f"invalid metadata JSON in {path}: {error}") from error
    if not isinstance(payload, dict):
        raise _mismatch(f"metadata file must contain a JSON object: {path}")
    return payload


def _require_value(metadata: dict[str, Any], key: str) -> Any:
    if key not in metadata:
        raise _mismatch(f'metadata missing required field "{key}"')
    return metadata[key]


def _require_equal(actual: Any, expected: Any, detail: str) -> None:
    if actual != expected:
        raise _mismatch(detail)


def _resolve_metadata_path(raw_path: object, *, metadata_path: Path) -> Path:
    path = Path(str(raw_path))
    if path.is_absolute():
        return path.resolve()
    return (metadata_path.parent / path).resolve()


def load_and_validate_dem_case(case: DemCase) -> tuple[str, dict[str, object]]:
    dem_text = case.dem_path.read_text()
    metadata = _load_metadata(case.metadata_path)

    _require_equal(_require_value(metadata, "case_label"), case.label, "case label does not match")
    _require_equal(
        _resolve_metadata_path(_require_value(metadata, "dem_path"), metadata_path=case.metadata_path),
        case.dem_path.resolve(),
        "dem path does not match",
    )
    _require_equal(
        _require_value(metadata, "dem_sha256"),
        sha256_file(case.dem_path),
        "dem hash does not match",
    )
    _require_equal(_require_value(metadata, "shots"), case.shots, "shot count does not match")
    _require_equal(
        _require_value(metadata, "expected_detectors"),
        case.expected_detectors,
        "detector count does not match",
    )
    _require_equal(
        _require_value(metadata, "expected_observables"),
        case.expected_observables,
        "observable count does not match",
    )
    _require_equal(
        _require_value(metadata, "generation_command"),
        EXPECTED_GENERATION_COMMAND,
        "generation command does not match",
    )

    source_path_value = _require_value(metadata, "source_circuit_path")
    if not isinstance(source_path_value, str) or not source_path_value.strip():
        raise _mismatch("source circuit path is empty")
    source_path = _resolve_metadata_path(source_path_value, metadata_path=case.metadata_path)
    if not source_path.is_file():
        raise _mismatch("source circuit path does not exist")
    _require_equal(
        _require_value(metadata, "source_circuit_sha256"),
        sha256_file(source_path),
        "source circuit hash does not match",
    )

    return dem_text, metadata


def run_timed_command(command: list[str], dem_text: str, *, cwd: Path | None = None) -> tuple[int, str, int]:
    started_ns = time.perf_counter_ns()
    completed = subprocess.run(
        command,
        cwd=cwd,
        input=dem_text,
        text=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        check=False,
    )
    finished_ns = time.perf_counter_ns()
    return completed.returncode, completed.stderr.strip(), finished_ns - started_ns


def summarize_records(records: list[dict[str, object]], case: DemCase) -> dict[str, object]:
    variants: list[dict[str, object]] = []
    issues: list[dict[str, object]] = []
    present_variants = sorted({str(record["tool_variant"]) for record in records})
    for tool_variant in EXPECTED_VARIANTS:
        variant_records = [record for record in records if record["tool_variant"] == tool_variant]
        measured_completed = [
            int(record["elapsed_ns"])
            for record in variant_records
            if record["status"] == "completed" and record.get("phase") == "measure"
        ]
        failed_records = [record for record in variant_records if record["status"] != "completed"]
        latest_record = variant_records[-1] if variant_records else None
        failed_record = failed_records[0] if failed_records else None
        status = str(failed_record["status"]) if failed_record is not None else (
            str(latest_record["status"]) if latest_record is not None else "missing"
        )
        median_wall_time_ns = statistics.median(measured_completed) if measured_completed else None
        median_shots_per_second = (
            (case.shots * 1_000_000_000) / float(median_wall_time_ns)
            if median_wall_time_ns not in (None, 0)
            else None
        )
        failure_reason = None
        stderr = None
        if failed_record is not None:
            exit_code = failed_record.get("exit_code")
            failure_reason = (
                f"command exited with code {exit_code}" if exit_code is not None else "command failed"
            )
            stderr = failed_record.get("stderr")
            issues.append(
                {
                    "case_label": case.label,
                    "workload": "sample_dem",
                    "tool_variant": tool_variant,
                    "status": status,
                    "failure_reason": failure_reason,
                    "stderr": stderr,
                }
            )
        variants.append(
            {
                "tool_variant": tool_variant,
                "sample_count": len(measured_completed),
                "median_wall_time_ns": median_wall_time_ns,
                "median_shots_per_second": median_shots_per_second,
                "status": status,
                "failure_reason": failure_reason,
                "stderr": stderr,
            }
        )

    return {
        "cases": [
            {
                "case_label": case.label,
                "workload": "sample_dem",
                "tier": "report_only",
                "expected_variants": list(EXPECTED_VARIANTS),
                "present_variants": present_variants,
                "variants": variants,
            }
        ],
        "issues": issues,
    }


def render_report(summary: dict[str, object]) -> str:
    lines = ["# DEM Sampling Report", ""]
    for case in summary.get("cases", []):
        if not isinstance(case, dict):
            continue
        lines.append(f"## {case['case_label']}")
        lines.append("")
        lines.append(f"- workload: {case['workload']}")
        lines.append(f"- expected_variants: {', '.join(case['expected_variants'])}")
        lines.append(f"- present_variants: {', '.join(case['present_variants'])}")
        lines.append("")
        lines.append("| variant | status | samples | median wall time (ns) |")
        lines.append("| --- | --- | ---: | ---: |")
        for variant in case.get("variants", []):
            if not isinstance(variant, dict):
                continue
            lines.append(
                f"| {variant['tool_variant']} | {variant['status']} | {variant['sample_count']} | "
                f"{variant['median_wall_time_ns'] if variant['median_wall_time_ns'] is not None else 'n/a'} |"
            )
        lines.append("")
    return "\n".join(lines)


def _iter_rounds(warmup_rounds: int, measure_rounds: int) -> list[tuple[str, int]]:
    rounds: list[tuple[str, int]] = []
    rounds.extend(("warmup", index) for index in range(warmup_rounds))
    rounds.extend(("measure", index) for index in range(measure_rounds))
    return rounds


def _record_for_variant(
    *,
    case: DemCase,
    tool_variant: str,
    phase: str,
    round_index: int,
    command: list[str],
    returncode: int,
    stderr: str,
    elapsed_ns: int,
) -> dict[str, object]:
    status = "completed" if returncode == 0 else "tool_failed"
    return {
        "case_label": case.label,
        "workload": "sample_dem",
        "tool_variant": tool_variant,
        "phase": phase,
        "round_index": round_index,
        "shots": case.shots,
        "status": status,
        "elapsed_ns": elapsed_ns,
        "exit_code": returncode,
        "stderr": stderr or None,
        "command": list(command),
    }


def run_dem_speed_case(
    args: argparse.Namespace,
    repo_root: Path = REPO_ROOT,
    command_line: list[str] | None = None,
) -> None:
    case = case_by_label(args.case)
    dem_text, metadata = load_and_validate_dem_case(case)
    rstim_binary = build_rstim(args.profile, repo_root=repo_root)

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    raw_path = out_dir / "raw.jsonl"
    summary_path = out_dir / "summary.json"
    report_path = out_dir / "report.md"
    environment_path = out_dir / "environment.json"

    rstim_command_path = repo_relative_path(rstim_binary, repo_root=repo_root)
    commands = [
        ("stim-sample-dem", ["stim", "sample_dem", "--shots", str(case.shots)]),
        ("rstim-sample-dem", [rstim_command_path, "sample_dem", "--shots", str(case.shots)]),
    ]
    records: list[dict[str, object]] = []
    for phase, round_index in _iter_rounds(args.warmup_rounds, args.measure_rounds):
        for tool_variant, command in commands:
            returncode, stderr, elapsed_ns = run_timed_command(command, dem_text, cwd=repo_root)
            records.append(
                _record_for_variant(
                    case=case,
                    tool_variant=tool_variant,
                    phase=phase,
                    round_index=round_index,
                    command=command,
                    returncode=returncode,
                    stderr=stderr,
                    elapsed_ns=elapsed_ns,
                )
            )

    raw_path.write_text("".join(json.dumps(record, sort_keys=True) + "\n" for record in records))
    summary = summarize_records(records, case)
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    report_path.write_text(render_report(summary))

    environment = run_speed_case.collect_suite_environment(
        profile=args.profile,
        case_labels=[case.label],
        warmup_rounds=args.warmup_rounds,
        measure_rounds=args.measure_rounds,
        rstim_binary_path=rstim_binary,
        command_line=repo_relative_command(
            list(sys.argv if command_line is None else command_line),
            repo_root=repo_root,
        ),
    )
    source_circuit_path = _resolve_metadata_path(metadata["source_circuit_path"], metadata_path=case.metadata_path)
    environment.update(
        {
            "case_label": case.label,
            "rstim_binary_path": repo_relative_path(rstim_binary, repo_root=repo_root),
            "dem_path": repo_relative_path(case.dem_path, repo_root=REPO_ROOT),
            "dem_sha256": str(metadata["dem_sha256"]),
            "source_circuit_path": repo_relative_path(source_circuit_path, repo_root=REPO_ROOT),
            "source_circuit_sha256": metadata.get("source_circuit_sha256"),
            "generation_command": metadata.get("generation_command"),
            "expected_detectors": case.expected_detectors,
            "expected_observables": case.expected_observables,
        }
    )
    run_speed_case.write_environment(environment_path, environment)

    failed_records = [record for record in records if record["status"] != "completed"]
    if failed_records:
        failed_record = failed_records[0]
        tool_variant = str(failed_record["tool_variant"])
        exit_code = failed_record.get("exit_code")
        failure_reason = (
            f"command exited with code {exit_code}" if exit_code is not None else "command failed"
        )
        stderr = failed_record.get("stderr")
        detail = f"{tool_variant} failed: {failure_reason}"
        if isinstance(stderr, str) and stderr.strip():
            detail = f"{detail}: {stderr.strip()}"
        raise RuntimeError(detail)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run one DEM sampling benchmark case.")
    parser.add_argument("--profile", choices=["debug", "release"], required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--warmup-rounds", type=int, default=1)
    parser.add_argument("--measure-rounds", type=int, default=5)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        run_dem_speed_case(args, command_line=sys.argv if argv is None else [sys.argv[0], *argv])
    except (OSError, RuntimeError, subprocess.CalledProcessError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
