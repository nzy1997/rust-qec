from __future__ import annotations

import argparse
import csv
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare.cases import CSV_HEADER, SMOKE_CASES

REQUIRED_OK_FIELDS = (
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
    "status",
)
PINNED_UPSTREAM_SETTINGS = {
    "bp_method": "ms",
    "max_iter": "10000",
    "osd_method": "osd_cs",
    "osd_order": "7",
    "seed": "12345",
}


def verify_rows(rows: list[dict[str, str]]) -> list[str]:
    errors: list[str] = []

    if not rows:
        return ["CSV has no data rows"]

    missing_columns = [column for column in CSV_HEADER if not all(column in row for row in rows)]
    if missing_columns:
        errors.append(
            "row is missing required CSV column(s): " + ", ".join(missing_columns)
        )

    if not any(row.get("decoder_impl") == "rbposd" for row in rows):
        errors.append("Rust rbposd comparison row is missing")
    if not any(row.get("decoder_impl") == "ldpc_bposd" for row in rows):
        errors.append("upstream ldpc/bposd comparison row is missing")

    ok_rows = [row for row in rows if row.get("status") == "ok"]
    for row in ok_rows:
        if any(not row.get(field) for field in REQUIRED_OK_FIELDS):
            errors.append(
                "completed row missing required timing/logical/status field"
            )
            break
        if row.get("decoder_impl") == "ldpc_bposd":
            for field, expected_value in PINNED_UPSTREAM_SETTINGS.items():
                if row.get(field) != expected_value:
                    errors.append(
                        "completed upstream ldpc/bposd row has mismatched pinned setting"
                    )
                    break
            if errors and errors[-1] == (
                "completed upstream ldpc/bposd row has mismatched pinned setting"
            ):
                break

    paired_case_ids = {
        row.get("case_id", "")
        for row in ok_rows
        if row.get("decoder_impl") == "rbposd"
    } & {
        row.get("case_id", "")
        for row in ok_rows
        if row.get("decoder_impl") == "ldpc_bposd"
    }
    required_case_ids = {case.case_id for case in SMOKE_CASES}
    if not paired_case_ids or not required_case_ids.issubset(paired_case_ids):
        errors.append("no paired Rust/Python diagnostic case is present")

    for case in SMOKE_CASES:
        if case.case_id not in {row.get("case_id", "") for row in rows}:
            errors.append(f"required smoke case is missing: {case.case_id}")

    return errors


def _load_rows(csv_path: Path) -> list[dict[str, str]]:
    with csv_path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("csv_path", type=Path)
    args = parser.parse_args(argv)

    errors = verify_rows(_load_rows(args.csv_path))
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
