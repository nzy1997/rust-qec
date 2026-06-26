from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare.cases import CSV_HEADER, HARD_REPLAY_CASES
from benchmarks.bb_circuit_bposd_compare.run_compare import HARD_REPLAY_FIXTURE_PATH

REQUIRED_OK_FIELDS = (
    "basis",
    "syndrome_weight",
    "syndrome_support",
    "logical_prediction",
    "expected_logical",
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
    "status",
)
PINNED_REPLAY_SETTINGS = {
    "bp_method": "ms",
    "max_iter": "10000",
    "osd_order": "7",
    "seed": "12345",
    "basis": "Z",
}
ACCEPTED_OSD_METHODS = {"osd_cs", "ldpc_cs", "ldpc_osd_cs"}
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
    fixture: dict[str, object] | None = None,
) -> list[str]:
    errors: list[str] = []
    if not rows:
        return ["CSV has no data rows"]
    if fixture is None:
        fixture = _load_hard_replay_fixture()

    missing_columns = [
        column for column in CSV_HEADER if not all(column in row for row in rows)
    ]
    if missing_columns:
        errors.append(
            "row is missing required CSV column(s): " + ", ".join(missing_columns)
        )

    case_id = HARD_REPLAY_CASES[0].case_id
    case_rows = [row for row in rows if row.get("case_id") == case_id]
    rust_rows = [row for row in case_rows if row.get("decoder_impl") == "rbposd"]
    python_rows = [
        row for row in case_rows if row.get("decoder_impl") == "ldpc_bposd"
    ]
    if len(rust_rows) != 1:
        errors.append("expected exactly one Rust rbposd hard replay row")
    if len(python_rows) != 1:
        errors.append("expected exactly one Python ldpc_bposd hard replay row")
    if len(rust_rows) != 1 or len(python_rows) != 1:
        return errors

    rust = rust_rows[0]
    python = python_rows[0]
    _verify_fixture_metadata(rust, fixture, errors)
    _verify_fixture_metadata(python, fixture, errors)
    for row in (rust, python):
        for field, expected_value in PINNED_REPLAY_SETTINGS.items():
            if row.get(field) != expected_value:
                errors.append(
                    f"hard replay row has mismatched {field}: "
                    f"expected {expected_value}, got {row.get(field, '')}"
                )
        if row.get("osd_method") not in ACCEPTED_OSD_METHODS:
            errors.append(
                "hard replay row has mismatched osd_method: "
                "expected osd_cs/ldpc_cs equivalent"
            )

    _verify_rust_counters(rust, errors)

    python_status = python.get("status")
    if python_status == "skipped":
        if not python.get("error"):
            errors.append(
                "Python ldpc_bposd replay row is skipped without an explicit error"
            )
        if not allow_missing_python:
            errors.append("Python ldpc_bposd replay row is skipped")
        return errors

    ok_rows = [rust, python]
    for row in ok_rows:
        if row.get("status") != "ok":
            errors.append("hard replay row is not completed: " + row.get("decoder_impl", ""))
            continue
        if any(not row.get(field) for field in REQUIRED_OK_FIELDS):
            errors.append(
                "completed hard replay row missing required timing/logical/status field"
            )
            break

    pair_fields = (
        "case_id",
        "basis",
        "syndrome_weight",
        "syndrome_support",
        "expected_logical",
    )
    if any(rust.get(field) != python.get(field) for field in pair_fields):
        errors.append("Rust/Python replay is no longer paired")

    if _json_list(rust.get("logical_prediction", "")) != _json_list(
        python.get("logical_prediction", "")
    ):
        errors.append("Rust/Python logical predictions do not match")

    return errors


def _verify_rust_counters(row: dict[str, str], errors: list[str]) -> None:
    if any(not row.get(field) for field in RUST_COUNTER_FIELDS):
        errors.append("Rust rbposd replay row is missing OSD/GF(2) counter fields")
        return

    for field in RUST_COUNTER_FIELDS:
        _require_nonnegative_number(row, field, errors)
    for field in RUST_INTEGER_COUNTER_FIELDS:
        _require_integer(row, field, errors)
    if _as_int(row, "osd_use_count") <= 0:
        errors.append("Rust rbposd replay row did not record OSD use")
    if _as_int(row, "osd_candidate_count") <= 0:
        errors.append("Rust rbposd replay row did not record OSD candidates")
    if _as_int(row, "gf2_solve_count") <= 0:
        errors.append("Rust rbposd replay row did not record GF(2) solves")


def _verify_fixture_metadata(
    row: dict[str, str],
    fixture: dict[str, object],
    errors: list[str],
) -> None:
    expected_support = fixture.get("syndrome_support")
    expected_logical = fixture.get("expected_sampled_logical")
    if not isinstance(expected_support, list) or not isinstance(expected_logical, list):
        errors.append("checked-in hard replay fixture is missing expected metadata")
        return

    if row.get("basis") != fixture.get("basis"):
        errors.append("hard replay row no longer matches checked-in fixture basis")
    if row.get("syndrome_weight") != str(len(expected_support)):
        errors.append("hard replay row no longer matches checked-in fixture syndrome weight")
    if _json_list(row.get("syndrome_support", "")) != expected_support:
        errors.append("hard replay row no longer matches checked-in fixture syndrome")
    if row.get("expected_logical") and _json_list(row["expected_logical"]) != expected_logical:
        errors.append("hard replay row no longer matches checked-in fixture logical")


def _json_list(value: str) -> list[object]:
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError:
        return ["<invalid-json-list>", value]
    return parsed if isinstance(parsed, list) else ["<not-a-list>", parsed]


def _as_int(row: dict[str, str], field: str) -> int:
    try:
        return int(row.get(field, "0"))
    except ValueError:
        return -1


def _require_nonnegative_number(
    row: dict[str, str],
    field: str,
    errors: list[str],
) -> None:
    try:
        value = float(row[field])
    except ValueError:
        errors.append(f"Rust rbposd replay counter/timing field is not numeric: {field}")
        return
    if value < 0.0:
        errors.append(f"Rust rbposd replay counter/timing field is negative: {field}")


def _require_integer(row: dict[str, str], field: str, errors: list[str]) -> None:
    try:
        value = float(row[field])
    except ValueError:
        return
    if value.is_integer():
        return
    errors.append(f"Rust rbposd replay counter field is not an integer: {field}")


def _load_rows(csv_path: Path) -> list[dict[str, str]]:
    with csv_path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def _load_hard_replay_fixture() -> dict[str, object]:
    return json.loads(HARD_REPLAY_FIXTURE_PATH.read_text())


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
