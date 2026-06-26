from __future__ import annotations

import argparse
import csv
import math
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare.cases import CSV_HEADER, DIAGNOSTIC_CASES

REQUIRED_OK_FIELDS = (
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
    "status",
)
PAIR_FIELDS = (
    "case_id",
    "code_id",
    "p",
    "num_cycles",
    "num_trials",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
)
RUST_COUNTER_FIELDS = (
    "bp_seconds",
    "osd_seconds",
    "decode_call_count",
    "bp_iteration_count",
    "osd_use_count",
    "osd_candidate_count",
    "gf2_solve_count",
    "gf2_full_elimination_count",
)
RUST_INTEGER_COUNTER_FIELDS = (
    "decode_call_count",
    "bp_iteration_count",
    "osd_use_count",
    "osd_candidate_count",
    "gf2_solve_count",
    "gf2_full_elimination_count",
)


def verify_rows(
    rows: list[dict[str, str]],
    allow_missing_python: bool = False,
) -> list[str]:
    errors: list[str] = []
    if not rows:
        return ["CSV has no data rows"]

    missing_columns = [
        column for column in CSV_HEADER if not all(column in row for row in rows)
    ]
    if missing_columns:
        errors.append(
            "row is missing required CSV column(s): " + ", ".join(missing_columns)
        )

    _verify_no_unexpected_rows(rows, errors)

    for case in DIAGNOSTIC_CASES:
        expected = {
            "code_id": case.code_id,
            "p": str(case.p),
            "num_cycles": str(case.num_cycles),
            "num_trials": str(case.num_trials),
            "seed": str(case.seed),
            "bp_method": case.bp_method,
            "max_iter": str(case.max_iter),
            "osd_method": case.osd_method,
            "osd_order": str(case.osd_order),
        }
        case_rows = [row for row in rows if row.get("case_id") == case.case_id]
        config_rows = [
            row for row in rows if _matches_expected_config(row, expected)
        ]
        if not case_rows:
            errors.append(f"required diagnostic case is missing: {case.code_id}")
            _verify_near_pair(config_rows, errors)
            continue

        for row in case_rows:
            for field, expected_value in expected.items():
                if row.get(field) != expected_value:
                    errors.append(
                        f"diagnostic row has mismatched {field} for "
                        f"{case.code_id}: expected {expected_value}, "
                        f"got {row.get(field, '')}"
                    )

        rust_rows = [
            row for row in case_rows if row.get("decoder_impl") == "rbposd"
        ]
        python_rows = [
            row for row in case_rows if row.get("decoder_impl") == "ldpc_bposd"
        ]
        if len(rust_rows) != 1:
            errors.append(
                f"expected exactly one Rust rbposd diagnostic row for {case.case_id}"
            )
        if len(python_rows) != 1:
            errors.append(
                f"expected exactly one Python ldpc_bposd diagnostic row for "
                f"{case.case_id}"
            )

        if len(rust_rows) == 1:
            _verify_rust_counters(rust_rows[0], errors)
            _verify_ok_row(rust_rows[0], errors)
        if len(rust_rows) == 1 and len(python_rows) == 1:
            _verify_pair(rust_rows[0], python_rows[0], errors)
            python_is_skipped = _verify_completed_or_skipped_python(
                python_rows[0],
                allow_missing_python,
                errors,
            )
            if not python_is_skipped:
                _verify_ok_row(python_rows[0], errors)
        else:
            _verify_near_pair(config_rows, errors)

    return errors


def _verify_no_unexpected_rows(
    rows: list[dict[str, str]],
    errors: list[str],
) -> None:
    expected_rows = {
        (case.case_id, decoder_impl)
        for case in DIAGNOSTIC_CASES
        for decoder_impl in ("rbposd", "ldpc_bposd")
    }
    seen_unexpected: set[tuple[str, str]] = set()
    for row in rows:
        row_key = (row.get("case_id", ""), row.get("decoder_impl", ""))
        if row_key in expected_rows or row_key in seen_unexpected:
            continue
        seen_unexpected.add(row_key)
        errors.append(
            "unexpected diagnostic row is present: "
            f"{row_key[0]} {row_key[1]}"
        )


def _matches_expected_config(row: dict[str, str], expected: dict[str, str]) -> bool:
    return all(
        row.get(field) == expected_value
        for field, expected_value in expected.items()
    )


def _verify_near_pair(rows: list[dict[str, str]], errors: list[str]) -> None:
    rust_rows = [row for row in rows if row.get("decoder_impl") == "rbposd"]
    python_rows = [row for row in rows if row.get("decoder_impl") == "ldpc_bposd"]
    if len(rust_rows) == 1 and len(python_rows) == 1:
        _verify_pair(rust_rows[0], python_rows[0], errors)


def _verify_pair(
    rust: dict[str, str],
    python: dict[str, str],
    errors: list[str],
) -> None:
    for field in PAIR_FIELDS:
        if rust.get(field) != python.get(field):
            errors.append(f"Rust/Python diagnostic rows differ on {field}")


def _verify_ok_row(row: dict[str, str], errors: list[str]) -> None:
    if row.get("status") != "ok":
        errors.append(
            "diagnostic row is not completed: " + row.get("decoder_impl", "")
        )
        return
    if any(not row.get(field) for field in REQUIRED_OK_FIELDS):
        errors.append(
            "completed diagnostic row missing required timing/logical/status field"
        )


def _verify_completed_or_skipped_python(
    row: dict[str, str],
    allow_missing_python: bool,
    errors: list[str],
) -> bool:
    if row.get("status") != "skipped":
        return False

    if not row.get("error"):
        errors.append(
            "Python ldpc_bposd diagnostic row is skipped without an explicit error"
        )
    if not allow_missing_python:
        errors.append("Python ldpc_bposd diagnostic row is skipped")
    return True


def _verify_rust_counters(row: dict[str, str], errors: list[str]) -> None:
    if any(not row.get(field) for field in RUST_COUNTER_FIELDS):
        errors.append(
            "Rust rbposd diagnostic row is missing OSD/GF(2) counter fields"
        )
        return

    for field in RUST_COUNTER_FIELDS:
        _require_nonnegative_number(row, field, errors)
    for field in RUST_INTEGER_COUNTER_FIELDS:
        _require_integer(row, field, errors)


def _require_nonnegative_number(
    row: dict[str, str],
    field: str,
    errors: list[str],
) -> None:
    try:
        value = float(row[field])
    except ValueError:
        errors.append(
            f"Rust rbposd diagnostic counter/timing field is not numeric: {field}"
        )
        return
    if not math.isfinite(value):
        errors.append(
            f"Rust rbposd diagnostic counter/timing field is not numeric: {field}"
        )
        return
    if value < 0.0:
        errors.append(
            f"Rust rbposd diagnostic counter/timing field is negative: {field}"
        )


def _require_integer(row: dict[str, str], field: str, errors: list[str]) -> None:
    try:
        value = float(row[field])
    except ValueError:
        return
    if value.is_integer():
        return
    errors.append(f"Rust rbposd diagnostic counter field is not an integer: {field}")


def _load_rows(csv_path: Path) -> list[dict[str, str]]:
    with csv_path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--allow-missing-python", action="store_true")
    parser.add_argument("csv_path", type=Path)
    args = parser.parse_args(argv)

    errors = verify_rows(
        _load_rows(args.csv_path),
        allow_missing_python=args.allow_missing_python,
    )
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
