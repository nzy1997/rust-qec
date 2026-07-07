from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import shlex
import subprocess
import sys
import time
from collections.abc import Sequence
from pathlib import Path
import tomllib

from benchmarks.rstim_vs_stim_simulator.validate_cases import load_manifest, validate_manifest


STATUS_PASS = "pass"
STATUS_WARN = "warn"
STATUS_MISMATCH = "statistical_mismatch"
STATUS_STIM_FAILED = "stim_failed"
STATUS_RSTIM_FAILED = "rstim_failed"
STATUS_SKIPPED = "skipped"
PACKAGE_DIR = Path(__file__).resolve().parent


def resolve_case_input_path(raw_path: str, base_dir: Path) -> Path:
    candidate = (base_dir / raw_path).resolve()
    if candidate.is_relative_to(PACKAGE_DIR):
        return candidate
    return (PACKAGE_DIR / raw_path).resolve()


def default_rstim_command() -> list[str]:
    for binary in (Path("target/release/rstim"), Path("target/debug/rstim")):
        if binary.exists():
            return [str(binary)]
    return ["cargo", "run", "--offline", "--quiet", "-p", "rstim", "--bin", "rstim", "--"]


def build_sample_command(
    tool_command: list[str], *, mode: str, shots: int, seed: int, input_path: Path
) -> list[str]:
    command = [
        *tool_command,
        mode,
        "--shots",
        str(shots),
        "--seed",
        str(seed),
        "--out_format",
        "01",
        "--in",
        str(input_path),
    ]
    if mode == "detect":
        command.insert(len(tool_command) + 1, "--append_observables")
    return command


def run_tool(command: list[str], *, input_path: Path) -> dict[str, object]:
    start = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        elapsed_s = time.perf_counter() - start
        return {
            "command": command,
            "input_path": str(input_path),
            "exit_code": None,
            "stdout": "",
            "stderr": str(error),
            "elapsed_s": elapsed_s,
            "success": False,
        }

    elapsed_s = time.perf_counter() - start
    return {
        "command": command,
        "input_path": str(input_path),
        "exit_code": completed.returncode,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
        "elapsed_s": elapsed_s,
        "success": completed.returncode == 0,
    }


def _deterministic_bitflip_seed(case_id: str, seed: int) -> int:
    digest = hashlib.sha256(f"{case_id}:{seed}".encode("utf-8")).digest()
    return int.from_bytes(digest[:8], "big")


def _failure_result(
    *,
    case: dict[str, object],
    status: str,
    mode: str,
    input_path: Path,
    expected_bits: int,
    shots: int,
    seeds: list[int],
    selected_columns: list[int],
    selected_pairs: list[tuple[int, int]],
    failure_reasons: list[str],
    stim_runs: list[dict[str, object]],
    rstim_runs: list[dict[str, object]],
    stim_samples: list[list[int]],
    rstim_samples: list[list[int]],
) -> dict[str, object]:
    comparison = compare_sample_sets(
        stim_samples,
        rstim_samples,
        columns=selected_columns,
        pairs=selected_pairs,
    )
    return {
        "case_id": case["case_id"],
        "tier": case["tier"],
        "status": status,
        "mode": mode,
        "input_path": str(input_path),
        "expected_bits": expected_bits,
        "shots_per_seed": shots,
        "seeds": list(seeds),
        "sample_count": comparison["sample_count"],
        "selected_columns": selected_columns,
        "selected_pairs": [list(pair) for pair in selected_pairs],
        "max_delta": comparison["max_delta"],
        "max_tolerance": comparison["max_tolerance"],
        "failure_reasons": failure_reasons,
        "marginals": comparison["marginals"],
        "pairs": comparison["pairs"],
        "stim_runs": stim_runs,
        "rstim_runs": rstim_runs,
    }


def verify_case(
    case: dict[str, object],
    *,
    base_dir: Path,
    stim_command: list[str],
    rstim_command: list[str],
    shots: int,
    seeds: list[int],
    inject_rstim_bitflip_rate: float,
) -> dict[str, object]:
    tier = str(case["tier"])
    case_id = str(case["case_id"])
    input_path = resolve_case_input_path(str(case["canonical_input_path"]), base_dir)
    mode = "detect" if int(case["expected_detectors"]) > 0 else "sample"
    expected_bits = (
        int(case["expected_detectors"]) + int(case["expected_observables"])
        if mode == "detect"
        else int(case["expected_measurements"])
    )
    selected_columns = select_columns(
        expected_bits,
        observable_count=int(case["expected_observables"]),
    )
    selected_pairs = select_pairs(
        selected_columns,
        bit_count=expected_bits,
        observable_count=int(case["expected_observables"]),
    )

    if tier == "documentation-only":
        return {
            "case_id": case_id,
            "tier": tier,
            "status": STATUS_SKIPPED,
            "mode": mode,
            "input_path": str(input_path),
            "expected_bits": expected_bits,
            "shots_per_seed": shots,
            "seeds": list(seeds),
            "sample_count": 0,
            "selected_columns": selected_columns,
            "selected_pairs": [list(pair) for pair in selected_pairs],
            "failure_reasons": ["documentation-only"],
            "stim_runs": [],
            "rstim_runs": [],
        }

    stim_runs: list[dict[str, object]] = []
    rstim_runs: list[dict[str, object]] = []
    stim_samples: list[list[int]] = []
    rstim_samples: list[list[int]] = []
    stim_failure_reasons: list[str] = []
    rstim_failure_reasons: list[str] = []

    for seed in seeds:
        stim_seed_samples: list[list[int]] | None = None
        stim_run = run_tool(
            build_sample_command(
                list(stim_command),
                mode=mode,
                shots=shots,
                seed=seed,
                input_path=input_path,
            ),
            input_path=input_path,
        )
        stim_runs.append(stim_run)
        if not bool(stim_run["success"]):
            stim_failure_reasons.append(f"seed {seed}: {stim_run['stderr'] or 'stim failed'}")
        else:
            try:
                stim_seed_samples = parse_01_samples(
                    str(stim_run["stdout"]),
                    expected_bits=expected_bits,
                    expected_shots=shots,
                )
            except ValueError as error:
                stim_failure_reasons.append(f"seed {seed}: failed to parse stim output: {error}")

        rstim_seed_samples: list[list[int]] | None = None
        rstim_run = run_tool(
            build_sample_command(
                list(rstim_command),
                mode=mode,
                shots=shots,
                seed=seed,
                input_path=input_path,
            ),
            input_path=input_path,
        )
        rstim_runs.append(rstim_run)
        if not bool(rstim_run["success"]):
            rstim_failure_reasons.append(f"seed {seed}: {rstim_run['stderr'] or 'rstim failed'}")
        else:
            try:
                rstim_seed_samples = parse_01_samples(
                    str(rstim_run["stdout"]),
                    expected_bits=expected_bits,
                    expected_shots=shots,
                )
            except ValueError as error:
                rstim_failure_reasons.append(f"seed {seed}: failed to parse rstim output: {error}")

        if rstim_seed_samples is not None and inject_rstim_bitflip_rate:
            rstim_seed_samples = inject_bitflip(
                rstim_seed_samples,
                rate=inject_rstim_bitflip_rate,
                seed=_deterministic_bitflip_seed(case_id, seed),
            )

        if stim_seed_samples is not None and rstim_seed_samples is not None:
            stim_samples.extend(stim_seed_samples)
            rstim_samples.extend(rstim_seed_samples)

    if stim_failure_reasons:
        return _failure_result(
            case=case,
            status=STATUS_STIM_FAILED,
            mode=mode,
            input_path=input_path,
            expected_bits=expected_bits,
            shots=shots,
            seeds=seeds,
            selected_columns=selected_columns,
            selected_pairs=selected_pairs,
            failure_reasons=stim_failure_reasons,
            stim_runs=stim_runs,
            rstim_runs=rstim_runs,
            stim_samples=stim_samples,
            rstim_samples=rstim_samples,
        )

    if rstim_failure_reasons:
        return _failure_result(
            case=case,
            status=STATUS_RSTIM_FAILED,
            mode=mode,
            input_path=input_path,
            expected_bits=expected_bits,
            shots=shots,
            seeds=seeds,
            selected_columns=selected_columns,
            selected_pairs=selected_pairs,
            failure_reasons=rstim_failure_reasons,
            stim_runs=stim_runs,
            rstim_runs=rstim_runs,
            stim_samples=stim_samples,
            rstim_samples=rstim_samples,
        )

    comparison = compare_sample_sets(
        stim_samples,
        rstim_samples,
        columns=selected_columns,
        pairs=selected_pairs,
    )
    return {
        "case_id": case_id,
        "tier": tier,
        "status": comparison["status"],
        "mode": mode,
        "input_path": str(input_path),
        "expected_bits": expected_bits,
        "shots_per_seed": shots,
        "seeds": list(seeds),
        "sample_count": comparison["sample_count"],
        "selected_columns": selected_columns,
        "selected_pairs": [list(pair) for pair in selected_pairs],
        "max_delta": comparison["max_delta"],
        "max_tolerance": comparison["max_tolerance"],
        "failure_reasons": comparison["failure_reasons"],
        "marginals": comparison["marginals"],
        "pairs": comparison["pairs"],
        "stim_runs": stim_runs,
        "rstim_runs": rstim_runs,
    }


def parse_01_samples(stdout: str, *, expected_bits: int, expected_shots: int) -> list[list[int]]:
    lines = [line.strip() for line in stdout.splitlines() if line.strip()]
    if len(lines) != expected_shots:
        raise ValueError(f"expected {expected_shots} shots, got {len(lines)}")

    samples: list[list[int]] = []
    for shot_index, line in enumerate(lines):
        if len(line) != expected_bits:
            raise ValueError(f"shot {shot_index}: expected {expected_bits} bits, got {len(line)}")
        if any(ch not in "01" for ch in line):
            raise ValueError(f"shot {shot_index}: output contains non-01 data")
        samples.append([1 if ch == "1" else 0 for ch in line])
    return samples


def inject_bitflip(samples: list[list[int]], *, rate: float, seed: int) -> list[list[int]]:
    if not 0.0 <= rate <= 1.0:
        raise ValueError("rate must be between 0 and 1")

    rng = random.Random(seed)
    mutated: list[list[int]] = [row.copy() for row in samples]
    for row in mutated:
        for index, bit in enumerate(row):
            if rng.random() < rate:
                row[index] = 1 - bit
    return mutated


def select_columns(bit_count: int, *, observable_count: int, limit: int = 16) -> list[int]:
    if bit_count <= 0:
        return []
    if observable_count < 0:
        raise ValueError("observable_count must be non-negative")
    if limit <= 0:
        return []

    observable_start = max(0, bit_count - observable_count)
    selected: set[int] = set()

    def add(index: int) -> None:
        if 0 <= index < bit_count:
            selected.add(index)

    add(0)
    for index in range(observable_start, bit_count):
        add(index)

    middle_stop = observable_start if observable_count else bit_count
    middle_count = max(0, limit - len(selected))
    if middle_count:
        span = max(0, middle_stop - 1)
        if span > 0:
            for step in range(1, middle_count + 1):
                index = round(step * span / (middle_count + 1))
                add(index)
        else:
            add(0)

    if len(selected) < limit:
        for index in range(bit_count):
            add(index)
            if len(selected) >= limit:
                break

    return sorted(selected)


def select_pairs(
    columns: list[int], *, bit_count: int, observable_count: int, limit: int = 16
) -> list[tuple[int, int]]:
    if limit <= 0 or bit_count <= 1 or len(columns) < 2:
        return []
    if observable_count < 0:
        raise ValueError("observable_count must be non-negative")

    selected = sorted({index for index in columns if 0 <= index < bit_count})
    observable_start = max(0, bit_count - observable_count)
    pairs: list[tuple[int, int]] = []
    seen: set[tuple[int, int]] = set()

    def add(left: int, right: int) -> None:
        if left == right:
            return
        pair = (left, right) if left < right else (right, left)
        if pair in seen:
            return
        seen.add(pair)
        pairs.append(pair)

    for left, right in zip(selected, selected[1:]):
        add(left, right)
        if len(pairs) >= limit:
            return pairs[:limit]

    first_detector = next((index for index in selected if index < observable_start), None)
    if first_detector is not None:
        for observable in selected:
            if observable >= observable_start:
                add(first_detector, observable)
                if len(pairs) >= limit:
                    return pairs[:limit]

    return pairs[:limit]


def _validate_rectangular(samples: Sequence[Sequence[int]], label: str) -> int:
    if not samples:
        return 0
    width = len(samples[0])
    for row_index, row in enumerate(samples):
        if len(row) != width:
            raise ValueError(f"{label} row {row_index} has width {len(row)}; expected {width}")
    return width


def _bit_rate(samples: Sequence[Sequence[int]], column: int) -> tuple[int, int, float]:
    hits = sum(1 for row in samples if row[column])
    total = len(samples)
    return hits, total, (hits / total if total else 0.0)


def _pair_rate(samples: Sequence[Sequence[int]], left: int, right: int) -> tuple[int, int, float]:
    hits = sum(1 for row in samples if row[left] and row[right])
    total = len(samples)
    return hits, total, (hits / total if total else 0.0)


def _tolerance(
    stim_hits: int,
    stim_total: int,
    rstim_hits: int,
    rstim_total: int,
    *,
    z_score: float,
    floor: float,
) -> float:
    if stim_total <= 0 or rstim_total <= 0:
        return float("inf")
    pooled = (stim_hits + rstim_hits) / (stim_total + rstim_total)
    variance = pooled * (1 - pooled) * (1 / stim_total + 1 / rstim_total)
    return z_score * math.sqrt(max(0.0, variance)) + floor


def compare_sample_sets(
    stim_samples: list[list[int]],
    rstim_samples: list[list[int]],
    *,
    columns: list[int],
    pairs: list[tuple[int, int]],
    z_score: float = 6.0,
    floor: float = 0.01,
) -> dict[str, object]:
    stim_width = _validate_rectangular(stim_samples, "stim_samples")
    rstim_width = _validate_rectangular(rstim_samples, "rstim_samples")
    failure_reasons: list[str] = []

    if stim_samples and rstim_samples and stim_width != rstim_width:
        failure_reasons.append(
            f"sample widths differ: stim={stim_width}, rstim={rstim_width}"
        )

    sample_count = min(len(stim_samples), len(rstim_samples))
    if len(stim_samples) != len(rstim_samples):
        failure_reasons.append(
            f"sample counts differ: stim={len(stim_samples)}, rstim={len(rstim_samples)}"
        )

    marginals: list[dict[str, object]] = []
    pair_stats: list[dict[str, object]] = []
    max_delta = 0.0
    max_tolerance = 0.0

    for column in columns:
        if column < 0:
            continue
        if column >= stim_width or column >= rstim_width:
            failure_reasons.append(f"column {column} is out of range for one of the sample sets")
            continue
        stim_hits, stim_total, stim_rate = _bit_rate(stim_samples[:sample_count], column)
        rstim_hits, rstim_total, rstim_rate = _bit_rate(rstim_samples[:sample_count], column)
        delta = abs(stim_rate - rstim_rate)
        tolerance = _tolerance(
            stim_hits,
            stim_total,
            rstim_hits,
            rstim_total,
            z_score=z_score,
            floor=floor,
        )
        max_delta = max(max_delta, delta)
        max_tolerance = max(max_tolerance, tolerance)
        if delta > tolerance:
            failure_reasons.append(
                f"column {column} exceeds tolerance: delta={delta:.6f}, tolerance={tolerance:.6f}"
            )
        marginals.append(
            {
                "column": column,
                "stim_rate": stim_rate,
                "rstim_rate": rstim_rate,
                "delta": delta,
                "tolerance": tolerance,
                "stim_hits": stim_hits,
                "rstim_hits": rstim_hits,
                "sample_count": sample_count,
            }
        )

    for left, right in pairs:
        if left < 0 or right < 0:
            continue
        if left >= stim_width or right >= stim_width or left >= rstim_width or right >= rstim_width:
            failure_reasons.append(
                f"pair ({left}, {right}) is out of range for one of the sample sets"
            )
            continue
        stim_hits, stim_total, stim_rate = _pair_rate(stim_samples[:sample_count], left, right)
        rstim_hits, rstim_total, rstim_rate = _pair_rate(rstim_samples[:sample_count], left, right)
        delta = abs(stim_rate - rstim_rate)
        tolerance = _tolerance(
            stim_hits,
            stim_total,
            rstim_hits,
            rstim_total,
            z_score=z_score,
            floor=floor,
        )
        max_delta = max(max_delta, delta)
        max_tolerance = max(max_tolerance, tolerance)
        if delta > tolerance:
            failure_reasons.append(
                f"pair ({left}, {right}) exceeds tolerance: delta={delta:.6f}, tolerance={tolerance:.6f}"
            )
        pair_stats.append(
            {
                "pair": [left, right],
                "stim_rate": stim_rate,
                "rstim_rate": rstim_rate,
                "delta": delta,
                "tolerance": tolerance,
                "stim_hits": stim_hits,
                "rstim_hits": rstim_hits,
                "sample_count": sample_count,
            }
        )

    status = STATUS_PASS if not failure_reasons else STATUS_MISMATCH
    return {
        "status": status,
        "sample_count": sample_count,
        "marginals": marginals,
        "pairs": pair_stats,
        "max_delta": max_delta,
        "max_tolerance": max_tolerance,
        "failure_reasons": failure_reasons,
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
        try:
            seed = int(stripped)
        except ValueError as error:
            raise ValueError(f"invalid seed {stripped!r}") from error
        seeds.append(seed)
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


def _overall_status(case_results: Sequence[dict[str, object]]) -> str:
    statuses = [str(result["status"]) for result in case_results]
    if STATUS_STIM_FAILED in statuses:
        return STATUS_STIM_FAILED
    if STATUS_RSTIM_FAILED in statuses:
        return STATUS_RSTIM_FAILED
    if STATUS_MISMATCH in statuses:
        return STATUS_MISMATCH
    if STATUS_WARN in statuses:
        return STATUS_WARN
    if not any(status == STATUS_PASS for status in statuses):
        return STATUS_WARN
    return STATUS_PASS


def _validate_probability(value: float, name: str) -> None:
    if not 0.0 <= value <= 1.0:
        raise ValueError(f"{name} must be between 0 and 1")


def _suite_label(summary: dict[str, object]) -> str:
    manifest_path = Path(str(summary.get("manifest_path", "")))
    stem = manifest_path.stem
    if stem.endswith(".smoke"):
        return "smoke"
    if stem.endswith(".full"):
        return "full"
    return "correctness"


def _successful_run_count(runs: Sequence[dict[str, object]]) -> int:
    return sum(1 for run in runs if bool(run.get("success")))


def _format_stat(label: str, entry: dict[str, object]) -> str:
    return (
        f"{label} "
        f"stim={float(entry['stim_rate']):.6f} rstim={float(entry['rstim_rate']):.6f} "
        f"delta={float(entry['delta']):.6f} tol={float(entry['tolerance']):.6f}"
    )


def _case_rate_lines(case: dict[str, object]) -> list[str]:
    lines: list[str] = []
    marginals = list(case.get("marginals", []))
    pairs = list(case.get("pairs", []))
    if marginals:
        worst_marginal = max(marginals, key=lambda entry: float(entry["delta"]))
        marginal_label = f"marginal c{worst_marginal['column']}"
        lines.append(f"  rates {_format_stat(marginal_label, worst_marginal)}")
    if pairs:
        worst_pair = max(pairs, key=lambda entry: float(entry["delta"]))
        pair = worst_pair["pair"]
        pair_label = f"pair {pair[0]},{pair[1]}"
        lines.append(f"  rates {_format_stat(pair_label, worst_pair)}")
    return lines


def build_summary(args: argparse.Namespace) -> dict[str, object]:
    manifest = load_manifest(args.cases)
    errors = validate_manifest(manifest, args.cases.parent)
    if errors:
        raise ValueError("\n".join(errors))
    _validate_probability(args.inject_rstim_bitflip_rate, "--inject-rstim-bitflip-rate")

    stim_command = _command_from_arg(args.stim)
    rstim_command = _command_from_arg(args.rstim, default=default_rstim_command())
    seeds = _parse_seeds(args.seeds)
    cases = manifest["cases"]
    case_results = [
        verify_case(
            case,
            base_dir=args.cases.parent,
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
        STATUS_SKIPPED: sum(1 for result in case_results if result["status"] == STATUS_SKIPPED),
    }
    return {
        "manifest_path": str(args.cases),
        "suite": manifest.get("suite"),
        "status": _overall_status(case_results),
        "case_count": len(case_results),
        "shots": args.shots,
        "seeds": seeds,
        "stim_command": stim_command,
        "rstim_command": rstim_command,
        "inject_rstim_bitflip_rate": args.inject_rstim_bitflip_rate,
        "counts": counts,
        "cases": case_results,
    }


def write_summary(path: Path, summary: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")


def format_report(summary: dict[str, object]) -> tuple[int, str]:
    status = str(summary["status"])
    suite_label = _suite_label(summary)
    if status in {STATUS_STIM_FAILED, STATUS_RSTIM_FAILED}:
        first_line = "FAIL tool failure"
        exit_code = 1
    elif status == STATUS_MISMATCH:
        first_line = "FAIL statistical mismatch"
        exit_code = 1
    elif status == STATUS_WARN:
        first_line = f"WARN correctness {suite_label}"
        exit_code = 0
    else:
        first_line = f"PASS correctness {suite_label}"
        exit_code = 0

    lines = [first_line]
    lines.append(
        "summary "
        f"status={status} "
        f"cases={summary['case_count']} "
        f"shots={summary['shots']} "
        f"seeds={len(summary['seeds'])} "
        f"pass={summary['counts'][STATUS_PASS]} "
        f"mismatch={summary['counts'][STATUS_MISMATCH]} "
        f"stim_failed={summary['counts'][STATUS_STIM_FAILED]} "
        f"rstim_failed={summary['counts'][STATUS_RSTIM_FAILED]} "
        f"skipped={summary['counts'][STATUS_SKIPPED]}"
    )
    for case in summary["cases"]:
        failure_reasons = case.get("failure_reasons", [])
        reason_suffix = ""
        if failure_reasons:
            reason_suffix = f" reason={failure_reasons[0]}"
        max_delta = case.get("max_delta")
        max_tolerance = case.get("max_tolerance")
        selected_columns = case.get("selected_columns", [])
        selected_pairs = case.get("selected_pairs", [])
        stim_runs = case.get("stim_runs", [])
        rstim_runs = case.get("rstim_runs", [])
        lines.append(
            f"{case['case_id']} "
            f"status={case['status']} "
            f"samples={case['sample_count']} "
            f"marginals={len(selected_columns)} "
            f"pairs={len(selected_pairs)} "
            f"max_delta={max_delta if max_delta is not None else 'n/a'} "
            f"tolerance={max_tolerance if max_tolerance is not None else 'n/a'}"
            f"{reason_suffix}"
        )
        lines.append(
            "  tools "
            f"stim_runs={_successful_run_count(stim_runs)}/{len(stim_runs)}_ok "
            f"rstim_runs={_successful_run_count(rstim_runs)}/{len(rstim_runs)}_ok"
        )
        lines.extend(_case_rate_lines(case))
    return exit_code, "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify rstim sampling correctness against canonical Stim fixtures."
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

    write_summary(args.out, summary)
    exit_code, report = format_report(summary)
    print(report)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
