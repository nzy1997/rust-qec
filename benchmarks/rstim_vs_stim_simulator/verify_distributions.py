from __future__ import annotations

import argparse
import hashlib
import json
import math
import shlex
import subprocess
import sys
import shutil
import tomllib
from collections import Counter
from collections.abc import Sequence
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator.validate_distribution_cases import (
    load_manifest,
    validate_manifest,
)
from benchmarks.rstim_vs_stim_simulator.verify_correctness import (
    default_rstim_command,
    inject_bitflip,
)


STATUS_PASS = "pass"
STATUS_MISMATCH = "statistical_mismatch"
STATUS_STIM_FAILED = "stim_failed"
STATUS_RSTIM_FAILED = "rstim_failed"
TOLERANCE_FLOOR = 1e-12


def build_sample_command(tool_command: list[str], *, shots: int, seed: int) -> list[str]:
    return [
        *tool_command,
        "sample",
        "--shots",
        str(shots),
        "--seed",
        str(seed),
        "--out_format",
        "01",
    ]


def run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
    try:
        completed = subprocess.run(
            command,
            input=circuit,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        return {
            "command": command,
            "exit_code": None,
            "stderr": str(error),
            "stdout": "",
            "success": False,
            "stdin_source": "catalog:circuit",
        }

    return {
        "command": command,
        "exit_code": completed.returncode,
        "stderr": completed.stderr,
        "stdout": completed.stdout,
        "success": completed.returncode == 0,
        "stdin_source": "catalog:circuit",
    }


def parse_01_samples(stdout: str, *, expected_bits: int, expected_shots: int) -> list[str]:
    lines = [line.strip() for line in stdout.splitlines() if line.strip()]
    if len(lines) != expected_shots:
        raise ValueError(f"expected {expected_shots} shots, got {len(lines)}")

    for shot_index, line in enumerate(lines):
        if len(line) != expected_bits:
            raise ValueError(f"shot {shot_index}: expected {expected_bits} bits, got {len(line)}")
        if any(ch not in "01" for ch in line):
            raise ValueError(f"shot {shot_index}: output contains non-01 data")
    return lines


def _inject_bitflip(samples: list[str], *, rate: float) -> list[str]:
    if not 0.0 <= rate <= 1.0:
        raise ValueError("inject_rstim_bitflip_rate must be between 0 and 1")
    bit_samples = [[1 if ch == "1" else 0 for ch in sample] for sample in samples]
    mutated = inject_bitflip(bit_samples, rate=rate, seed=0)
    return ["".join(str(bit) for bit in row) for row in mutated]


def _outcome_tolerance(*, sample_count: int, expected_probability: float, z_score: float) -> float:
    if sample_count <= 0:
        return float("inf")
    variance_term = expected_probability * (1.0 - expected_probability)
    if expected_probability in (0.0, 1.0):
        variance_term = max(variance_term, TOLERANCE_FLOOR)
    variance = variance_term / sample_count
    return z_score * math.sqrt(max(0.0, variance))


def compare_distribution(
    samples: list[str],
    expected_distribution: dict[str, float],
    *,
    z_score: float = 5.0,
) -> dict[str, object]:
    sample_count = len(samples)
    observed_counts = Counter(samples)
    observed_frequencies = {
        outcome: count / sample_count for outcome, count in sorted(observed_counts.items())
    } if sample_count else {}

    failure_reasons: list[str] = []
    max_delta = 0.0
    max_tolerance = 0.0
    outcomes: list[dict[str, object]] = []

    for outcome in sorted(set(expected_distribution) | set(observed_counts)):
        expected_probability = float(expected_distribution.get(outcome, 0.0))
        observed_count = observed_counts.get(outcome, 0)
        observed_frequency = observed_count / sample_count if sample_count else 0.0
        delta = abs(observed_frequency - expected_probability)
        tolerance = _outcome_tolerance(
            sample_count=sample_count,
            expected_probability=expected_probability,
            z_score=z_score,
        )
        max_delta = max(max_delta, delta)
        max_tolerance = max(max_tolerance, tolerance)
        if delta > tolerance:
            failure_reasons.append(
                f"outcome {outcome} exceeds tolerance: observed={observed_frequency:.6f} "
                f"expected={expected_probability:.6f} delta={delta:.6f} tolerance={tolerance:.6f}"
            )
        outcomes.append(
            {
                "outcome": outcome,
                "expected_probability": expected_probability,
                "observed_count": observed_count,
                "observed_frequency": observed_frequency,
                "delta": delta,
                "tolerance": tolerance,
            }
        )

    return {
        "status": STATUS_PASS if not failure_reasons else STATUS_MISMATCH,
        "sample_count": sample_count,
        "observed_counts": dict(sorted(observed_counts.items())),
        "observed_frequencies": observed_frequencies,
        "outcomes": outcomes,
        "max_delta": max_delta,
        "max_tolerance": max_tolerance,
        "failure_reasons": failure_reasons,
    }


def _stable_run_record(run: dict[str, object]) -> dict[str, object]:
    return {
        "command": list(run["command"]),
        "exit_code": run["exit_code"],
        "stderr": run["stderr"],
        "success": run["success"],
        "stdin_source": run.get("stdin_source", "catalog:circuit"),
    }


def _tool_result(
    *,
    case: dict[str, object],
    tool_command: list[str],
    failure_status: str,
    shots: int,
    seeds: list[int],
    inject_bitflip_rate: float,
) -> dict[str, object]:
    expected_distribution = dict(case["expected_distribution"])
    expected_bits = len(next(iter(expected_distribution)))
    all_samples: list[str] = []
    runs: list[dict[str, object]] = []
    failure_reasons: list[str] = []
    circuit = str(case["circuit"])

    for seed in seeds:
        run = run_tool(build_sample_command(list(tool_command), shots=shots, seed=seed), circuit=circuit)
        runs.append(_stable_run_record(run))
        if not bool(run["success"]):
            failure_reasons.append(f"seed {seed}: {run['stderr'] or 'tool failed'}")
            continue
        try:
            parsed = parse_01_samples(
                str(run["stdout"]),
                expected_bits=expected_bits,
                expected_shots=shots,
            )
        except ValueError as error:
            failure_reasons.append(f"seed {seed}: failed to parse output: {error}")
            continue
        if inject_bitflip_rate:
            parsed = _inject_bitflip(parsed, rate=inject_bitflip_rate)
        all_samples.extend(parsed)

    comparison = compare_distribution(all_samples, expected_distribution)
    if failure_reasons:
        return {
            "status": failure_status,
            "sample_count": comparison["sample_count"],
            "observed_counts": comparison["observed_counts"],
            "observed_frequencies": comparison["observed_frequencies"],
            "outcomes": comparison["outcomes"],
            "max_delta": comparison["max_delta"],
            "max_tolerance": comparison["max_tolerance"],
            "failure_reasons": failure_reasons,
            "runs": runs,
        }

    return {
        "status": comparison["status"],
        "sample_count": comparison["sample_count"],
        "observed_counts": comparison["observed_counts"],
        "observed_frequencies": comparison["observed_frequencies"],
        "outcomes": comparison["outcomes"],
        "max_delta": comparison["max_delta"],
        "max_tolerance": comparison["max_tolerance"],
        "failure_reasons": comparison["failure_reasons"],
        "runs": runs,
    }


def verify_case(
    case: dict[str, object],
    *,
    stim_command: list[str],
    rstim_command: list[str],
    shots: int,
    seeds: list[int],
    inject_rstim_bitflip_rate: float,
) -> dict[str, object]:
    stim_result = _tool_result(
        case=case,
        tool_command=stim_command,
        failure_status=STATUS_STIM_FAILED,
        shots=shots,
        seeds=seeds,
        inject_bitflip_rate=0.0,
    )
    rstim_result = _tool_result(
        case=case,
        tool_command=rstim_command,
        failure_status=STATUS_RSTIM_FAILED,
        shots=shots,
        seeds=seeds,
        inject_bitflip_rate=inject_rstim_bitflip_rate,
    )

    failure_reasons: list[str] = []
    if stim_result["status"] == STATUS_STIM_FAILED:
        failure_reasons.extend(stim_result["failure_reasons"])
        status = STATUS_STIM_FAILED
    elif rstim_result["status"] == STATUS_RSTIM_FAILED:
        failure_reasons.extend(rstim_result["failure_reasons"])
        status = STATUS_RSTIM_FAILED
    elif stim_result["status"] == STATUS_MISMATCH or rstim_result["status"] == STATUS_MISMATCH:
        failure_reasons.extend(stim_result["failure_reasons"])
        failure_reasons.extend(rstim_result["failure_reasons"])
        status = STATUS_MISMATCH
    else:
        status = STATUS_PASS

    return {
        "case_id": case["case_id"],
        "status": status,
        "sample_count": min(int(stim_result["sample_count"]), int(rstim_result["sample_count"])),
        "failure_reasons": failure_reasons,
        "expected_distribution": dict(case["expected_distribution"]),
        "source_url": case["source_url"],
        "source_commit": case["source_commit"],
        "source_line_start": case["source_line_start"],
        "source_line_end": case["source_line_end"],
        "stim": stim_result,
        "rstim": rstim_result,
    }


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be a positive integer")
    return parsed


def _parse_seeds(raw_value: str) -> list[int]:
    seeds: list[int] = []
    for chunk in raw_value.split(","):
        stripped = chunk.strip()
        if not stripped:
            continue
        seeds.append(int(stripped))
    if not seeds:
        raise ValueError("at least one seed is required")
    return seeds


def _command_from_arg(raw_command: str | None, *, default: list[str] | None = None) -> list[str]:
    if raw_command is None:
        if default is None:
            raise ValueError("command is required")
        return list(default)
    command = shlex.split(raw_command)
    if not command:
        raise ValueError("command must not be empty")
    return command


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _run_version_command(command: list[str]) -> dict[str, object]:
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        return {
            "command": command,
            "status": "missing",
            "stdout": "",
            "stderr": str(error),
            "exit_code": None,
        }
    return {
        "command": command,
        "status": "ok" if completed.returncode == 0 else "failed",
        "stdout": completed.stdout.strip(),
        "stderr": completed.stderr.strip(),
        "exit_code": completed.returncode,
    }


def _direct_binary_path(command: list[str]) -> str | None:
    if not command:
        return None
    executable = command[0]
    if executable == "cargo":
        return None
    resolved = shutil.which(executable)
    if resolved is not None:
        return resolved
    if Path(executable).exists():
        return executable
    return None


def collect_environment_metadata(
    stim_command: list[str],
    rstim_command: list[str],
) -> dict[str, object]:
    if stim_command:
        stim_version = _run_version_command([stim_command[0], "--version"])
    else:
        stim_version = {
            "command": [],
            "status": "missing",
            "stdout": "",
            "stderr": "stim command is empty",
            "exit_code": None,
        }
    rustc_version = _run_version_command(["rustc", "--version"])
    cargo_version = _run_version_command(["cargo", "--version"])
    return {
        "stim_command": list(stim_command),
        "rstim_command": list(rstim_command),
        "rstim_binary_path": _direct_binary_path(rstim_command),
        "stim_version": stim_version["stdout"] if stim_version["status"] == "ok" else "",
        "stim_version_command": stim_version,
        "rustc_version": rustc_version["stdout"] if rustc_version["status"] == "ok" else "",
        "rustc_version_command": rustc_version,
        "cargo_version": cargo_version["stdout"] if cargo_version["status"] == "ok" else "",
        "cargo_version_command": cargo_version,
    }


def build_summary(args: argparse.Namespace) -> dict[str, object]:
    manifest = load_manifest(args.cases)
    errors = validate_manifest(manifest)
    if errors:
        raise ValueError("\n".join(errors))
    if not 0.0 <= args.inject_rstim_bitflip_rate <= 1.0:
        raise ValueError("--inject-rstim-bitflip-rate must be between 0 and 1")

    stim_command = _command_from_arg(args.stim, default=["stim"])
    rstim_command = _command_from_arg(args.rstim, default=default_rstim_command())
    seeds = _parse_seeds(args.seeds)
    cases = manifest["cases"]
    case_results = [
        verify_case(
            case,
            stim_command=stim_command,
            rstim_command=rstim_command,
            shots=args.shots,
            seeds=seeds,
            inject_rstim_bitflip_rate=args.inject_rstim_bitflip_rate,
        )
        for case in cases
    ]
    counts = {
        STATUS_PASS: sum(1 for result in case_results if result["status"] == STATUS_PASS),
        STATUS_MISMATCH: sum(1 for result in case_results if result["status"] == STATUS_MISMATCH),
        STATUS_STIM_FAILED: sum(1 for result in case_results if result["status"] == STATUS_STIM_FAILED),
        STATUS_RSTIM_FAILED: sum(1 for result in case_results if result["status"] == STATUS_RSTIM_FAILED),
    }

    if counts[STATUS_STIM_FAILED]:
        status = STATUS_STIM_FAILED
    elif counts[STATUS_RSTIM_FAILED]:
        status = STATUS_RSTIM_FAILED
    elif counts[STATUS_MISMATCH]:
        status = STATUS_MISMATCH
    else:
        status = STATUS_PASS

    return {
        "manifest_path": str(args.cases),
        "suite": manifest.get("suite"),
        "status": status,
        "case_count": len(case_results),
        "shots": args.shots,
        "seeds": seeds,
        "stim_command": stim_command,
        "rstim_command": rstim_command,
        "inject_rstim_bitflip_rate": args.inject_rstim_bitflip_rate,
        "catalog_sha256": sha256_file(args.cases),
        "environment": collect_environment_metadata(stim_command, rstim_command),
        "counts": counts,
        "cases": case_results,
    }


def write_summary(path: Path, summary: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")


def format_report(summary: dict[str, object]) -> tuple[int, str]:
    status = str(summary["status"])
    case_count = int(summary["case_count"])
    mismatch_count = int(summary["counts"][STATUS_MISMATCH])
    if status == STATUS_PASS:
        return 0, f"PASS distribution correctness cases={case_count} mismatch={mismatch_count}"
    if status == STATUS_MISMATCH:
        return 1, f"FAIL statistical mismatch cases={case_count} mismatch={mismatch_count}"
    return 1, f"FAIL tool failure cases={case_count} mismatch={mismatch_count}"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify rstim small-circuit distributions against source-grounded Stim expectations."
    )
    parser.add_argument("--cases", type=Path, required=True)
    parser.add_argument("--stim", default="stim")
    parser.add_argument("--rstim", default=None)
    parser.add_argument("--shots", type=_positive_int, required=True)
    parser.add_argument("--seeds", default="12345")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--inject-rstim-bitflip-rate", type=float, default=0.0)
    args = parser.parse_args(argv)

    try:
        summary = build_summary(args)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"{args.cases}: {error}", file=sys.stderr)
        return 1

    raw_argv = list(sys.argv[1:] if argv is None else argv)
    summary["command_line"] = [
        "python3",
        "-m",
        "benchmarks.rstim_vs_stim_simulator.verify_distributions",
        *raw_argv,
    ]

    write_summary(args.out, summary)
    exit_code, report = format_report(summary)
    print(report)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
