from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import stim

from benchmarks.rstim_vs_stim_simulator.validate_cases import (
    load_manifest,
    validate_manifest,
)
from benchmarks.rstim_vs_stim_simulator.verify_correctness import resolve_case_input_path


MEASUREMENT_GATES = {
    "M",
    "MX",
    "MY",
    "MR",
    "MRX",
    "MRY",
    "MRZ",
    "MXX",
    "MYY",
    "MZZ",
    "MPP",
    "MPAD",
}
DEFAULT_MANIFEST = Path("benchmarks/rstim_vs_stim_simulator/cases.full.toml")


@dataclass
class RepeatStats:
    repeat_block_count: int = 0
    repeat_depth: int = 0
    repeat_expansion_count: int = 0

    def __iadd__(self, other: "RepeatStats") -> "RepeatStats":
        self.repeat_block_count += other.repeat_block_count
        self.repeat_depth = max(self.repeat_depth, other.repeat_depth)
        self.repeat_expansion_count += other.repeat_expansion_count
        return self


def _collect_repeat_stats(
    circuit: stim.Circuit, *, multiplier: int = 1, depth: int = 0
) -> RepeatStats:
    stats = RepeatStats()
    for instruction in circuit:
        if isinstance(instruction, stim.CircuitRepeatBlock):
            repeat_count = int(instruction.repeat_count)
            expanded_invocations = multiplier * repeat_count
            stats.repeat_block_count += 1
            stats.repeat_expansion_count += expanded_invocations
            stats.repeat_depth = max(stats.repeat_depth, depth + 1)
            stats += _collect_repeat_stats(
                instruction.body_copy(),
                multiplier=expanded_invocations,
                depth=depth + 1,
            )
    return stats


def summarize_circuit(circuit: stim.Circuit) -> dict[str, Any]:
    flattened = list(circuit.flattened())
    repeat_stats = _collect_repeat_stats(circuit)

    operations: dict[str, dict[str, int]] = {}
    for instruction in flattened:
        op_name = str(instruction.name)
        entry = operations.setdefault(
            op_name, {"operation_count": 0, "target_count": 0, "measurement_count": 0}
        )
        target_count = len(instruction.targets_copy())
        entry["operation_count"] += 1
        entry["target_count"] += target_count
        if op_name in MEASUREMENT_GATES:
            entry["measurement_count"] += target_count

    if repeat_stats.repeat_expansion_count:
        operations["REPEAT"] = {
            "operation_count": repeat_stats.repeat_expansion_count,
            "target_count": 0,
            "measurement_count": 0,
        }

    flattened_operation_count = len(flattened)
    expanded_operation_count = flattened_operation_count + repeat_stats.repeat_expansion_count

    return {
        "flattened_operation_count": flattened_operation_count,
        "repeat_block_count": repeat_stats.repeat_block_count,
        "repeat_depth": repeat_stats.repeat_depth,
        "repeat_expansion_count": repeat_stats.repeat_expansion_count,
        "expanded_operation_count": expanded_operation_count,
        "operations": operations,
    }


def find_case(
    manifest: dict[str, Any], case_id: str
) -> dict[str, object] | None:
    cases = manifest.get("cases")
    if not isinstance(cases, list):
        return None
    for raw_case in cases:
        if not isinstance(raw_case, dict):
            continue
        if raw_case.get("case_id") == case_id:
            return raw_case
    return None


def _coerce_path(value: Any) -> str:
    if not isinstance(value, str):
        raise ValueError("canonical_input_path must be a string")
    return value


def _coerce_int(value: Any, field: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError(f'field "{field}" must be an integer')
    return value


def build_report(
    case: dict[str, object], *, manifest_path: Path, base_dir: Path
) -> dict[str, object]:
    case_id = str(case["case_id"])
    raw_input_path = _coerce_path(case["canonical_input_path"])
    input_path = resolve_case_input_path(raw_input_path, base_dir)
    circuit = stim.Circuit(input_path.read_text())

    expected_measurements = _coerce_int(case["expected_measurements"], "expected_measurements")
    expected_detectors = _coerce_int(case["expected_detectors"], "expected_detectors")
    expected_observables = _coerce_int(case["expected_observables"], "expected_observables")

    actual_measurements = circuit.num_measurements
    actual_detectors = circuit.num_detectors
    actual_observables = circuit.num_observables

    if expected_measurements != actual_measurements:
        raise ValueError(
            f'case "{case_id}" expected_measurements mismatch: expected {expected_measurements}, got {actual_measurements}'
        )
    if expected_detectors != actual_detectors:
        raise ValueError(
            f'case "{case_id}" expected_detectors mismatch: expected {expected_detectors}, got {actual_detectors}'
        )
    if expected_observables != actual_observables:
        raise ValueError(
            f'case "{case_id}" expected_observables mismatch: expected {expected_observables}, got {actual_observables}'
        )

    summary = summarize_circuit(circuit)
    report: dict[str, object] = {
        "case_id": case_id,
        "manifest_path": str(manifest_path),
        "input_path": str(input_path),
        "expected_measurements": expected_measurements,
        "expected_detectors": expected_detectors,
        "expected_observables": expected_observables,
        "actual_measurements": actual_measurements,
        "actual_detectors": actual_detectors,
        "actual_observables": actual_observables,
        "status": "pass",
    }
    report.update(summary)
    return report


def summary_line(report: dict[str, object]) -> str:
    status = str(report.get("status", "pass"))
    case_id = str(report["case_id"])
    if status == "pass":
        return f"PASS fixture load {case_id}"
    if status == "warn":
        return f"WARN fixture load {case_id}"
    if status == "fail":
        return f"FAIL fixture load {case_id}"
    return f"{status.upper()} fixture load {case_id}"


def format_text_report(report: dict[str, object]) -> str:
    return (
        f"{summary_line(report)}\n"
        f"case_id={report['case_id']}\n"
        f"manifest_path={report['manifest_path']}\n"
        f"input_path={report['input_path']}\n"
        f"expected_measurements={report['expected_measurements']}\n"
        f"expected_detectors={report['expected_detectors']}\n"
        f"expected_observables={report['expected_observables']}\n"
        f"actual_measurements={report['actual_measurements']}\n"
        f"actual_detectors={report['actual_detectors']}\n"
        f"actual_observables={report['actual_observables']}\n"
        f"flattened_operation_count={report['flattened_operation_count']}\n"
        f"repeat_block_count={report['repeat_block_count']}\n"
        f"repeat_depth={report['repeat_depth']}\n"
        f"repeat_expansion_count={report['repeat_expansion_count']}\n"
        f"expanded_operation_count={report['expanded_operation_count']}\n"
        "operations:\n"
        + "\n".join(
            [
                f"  {name}: {json.dumps(value, sort_keys=True)}"
                for name, value in sorted(report["operations"].items(), key=lambda item: item[0])
            ]
        )
        + "\n"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Inspect fixture load characteristics.")
    parser.add_argument("--case", required=True, dest="case_id")
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="Path to TOML fixture manifest.",
    )
    parser.add_argument(
        "--format",
        choices=["text", "json"],
        default="text",
        dest="format",
    )
    parser.add_argument("--out", type=Path, default=None)

    args = parser.parse_args(argv)

    try:
        manifest = load_manifest(args.manifest)
    except (OSError, ValueError) as error:
        print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    errors = validate_manifest(manifest, args.manifest.parent)
    if errors:
        for error in errors:
            print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    case = find_case(manifest, args.case_id)
    if case is None:
        print(f'case "{args.case_id}" not found in manifest', file=sys.stderr)
        return 1

    try:
        report = build_report(case, manifest_path=args.manifest, base_dir=args.manifest.parent)
    except (OSError, ValueError) as error:
        print(f"{args.manifest}: {error}", file=sys.stderr)
        return 1

    if args.format == "json":
        body = json.dumps(report, indent=2, sort_keys=True) + "\n"
    else:
        body = format_text_report(report)

    if args.out is not None:
        args.out.write_text(body)
        print(summary_line(report))
    else:
        if args.format == "json":
            print(body, end="")
            print(summary_line(report), file=sys.stderr)
        else:
            print(body, end="")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
