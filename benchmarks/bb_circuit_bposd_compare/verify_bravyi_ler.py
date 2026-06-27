from __future__ import annotations

import argparse
import csv
import math
import sys
from dataclasses import dataclass
from pathlib import Path

ACCEPTED_STATUSES = {"ok", "partial"}
TOLERANCE = 1e-12
REQUIRED_COLUMNS = (
    "case_id",
    "runner",
    "decoder_impl",
    "p",
    "num_cycles",
    "shots_used",
    "logical_errors",
    "logical_error_rate",
    "status",
)


@dataclass(frozen=True)
class VerifiedRow:
    case_id: str
    decoder_impl: str
    shots_used: int
    logical_errors: int
    logical_error_rate: float
    bravyi_tuple: tuple[str, int, int, int]


@dataclass(frozen=True)
class VerificationError:
    message: str


def load_rows(csv_path: Path) -> list[dict[str, str]]:
    with csv_path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def verify_rows(rows: list[dict[str, str]]) -> list[VerifiedRow | VerificationError]:
    results: list[VerifiedRow | VerificationError] = []
    if not rows:
        return [VerificationError("CSV has no data rows")]

    missing_columns = [
        column for column in REQUIRED_COLUMNS if not all(column in row for row in rows)
    ]
    if missing_columns:
        return [
            VerificationError(
                "row is missing required CSV column(s): " + ", ".join(missing_columns)
            )
        ]

    for row_index, row in enumerate(rows, start=2):
        if not _is_accepted_batched_row(row):
            continue
        parsed = _parse_row(row, row_index)
        if isinstance(parsed, VerificationError):
            results.append(parsed)
            continue
        case_id, decoder_impl, p, num_cycles, shots_used, logical_errors, actual = parsed
        expected = logical_errors / shots_used
        if not math.isclose(actual, expected, rel_tol=0.0, abs_tol=TOLERANCE):
            results.append(
                VerificationError(_mismatch_message(row_index, row, actual, expected))
            )
            continue
        results.append(
            VerifiedRow(
                case_id=case_id,
                decoder_impl=decoder_impl,
                shots_used=shots_used,
                logical_errors=logical_errors,
                logical_error_rate=actual,
                bravyi_tuple=(p, num_cycles, shots_used, logical_errors),
            )
        )

    if not results:
        return [VerificationError("CSV has no completed or partial batched rows to verify")]
    return results


def _is_accepted_batched_row(row: dict[str, str]) -> bool:
    return row.get("runner") == "batched_compare" and row.get("status") in ACCEPTED_STATUSES


def _parse_row(
    row: dict[str, str],
    row_index: int,
) -> tuple[str, str, str, int, int, int, float] | VerificationError:
    context = f"row {row_index} {row.get('case_id', '<missing case_id>')}"

    def parse_int(field_name: str) -> int | VerificationError:
        try:
            return int(row[field_name])
        except ValueError as error:
            return VerificationError(
                f"{context}: failed to parse numeric field {field_name}: {error}"
            )

    def parse_float(field_name: str) -> float | VerificationError:
        try:
            return float(row[field_name])
        except ValueError as error:
            return VerificationError(
                f"{context}: failed to parse numeric field {field_name}: {error}"
            )

    num_cycles = parse_int("num_cycles")
    if isinstance(num_cycles, VerificationError):
        return num_cycles
    shots_used = parse_int("shots_used")
    if isinstance(shots_used, VerificationError):
        return shots_used
    logical_errors = parse_int("logical_errors")
    if isinstance(logical_errors, VerificationError):
        return logical_errors
    actual = parse_float("logical_error_rate")
    if isinstance(actual, VerificationError):
        return actual

    if num_cycles <= 0:
        return VerificationError(f"{context}: num_cycles must be positive")
    if shots_used <= 0:
        return VerificationError(
            f"{context}: shots_used must be positive for trial-level LER"
        )
    if logical_errors < 0:
        return VerificationError(f"{context}: logical_errors must be nonnegative")
    if logical_errors > shots_used:
        return VerificationError(f"{context}: logical_errors must be <= shots_used")
    if not math.isfinite(actual):
        return VerificationError(f"{context}: logical_error_rate must be finite")
    return (
        row["case_id"],
        row["decoder_impl"],
        row["p"],
        num_cycles,
        shots_used,
        logical_errors,
        actual,
    )


def _mismatch_message(
    row_index: int,
    row: dict[str, str],
    actual: float,
    expected: float,
) -> str:
    shots_used = int(row["shots_used"])
    logical_errors = int(row["logical_errors"])
    num_cycles = int(row["num_cycles"])
    per_cycle = logical_errors / (shots_used * num_cycles)
    context = f"row {row_index} {row.get('case_id', '<missing case_id>')} {row.get('decoder_impl', '')}"
    if math.isclose(actual, per_cycle, rel_tol=0.0, abs_tol=TOLERANCE):
        return (
            f"{context}: logical_error_rate appears per-cycle normalized; "
            f"got {actual}, expected trial-level LER {expected} "
            f"from logical_errors/shots_used"
        )
    if 0.0 < actual < expected:
        ratio = expected / actual
        nearest = round(ratio)
        if nearest >= 2 and math.isclose(ratio, nearest, rel_tol=0.0, abs_tol=1e-9):
            return (
                f"{context}: logical_error_rate appears divided by {nearest} "
                f"before plotting; got {actual}, expected trial-level LER {expected}"
            )
    return (
        f"{context}: logical_error_rate mismatched trial-level LER; "
        f"got {actual}, expected {expected} from logical_errors/shots_used"
    )


def format_table(rows: list[VerifiedRow]) -> str:
    header = (
        "case_id decoder_impl shots_used logical_errors "
        "logical_error_rate bravyi_tuple"
    )
    lines = [header]
    for row in rows:
        p, num_cycles, shots_used, logical_errors = row.bravyi_tuple
        lines.append(
            "PASS "
            f"{row.case_id} {row.decoder_impl} {row.shots_used} "
            f"{row.logical_errors} {row.logical_error_rate:.17g} "
            f"bravyi_tuple=({p}, {num_cycles}, {shots_used}, {logical_errors})"
        )
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("csv_path", type=Path)
    args = parser.parse_args(argv)

    results = verify_rows(load_rows(args.csv_path))
    errors = [item for item in results if isinstance(item, VerificationError)]
    verified_rows = [item for item in results if isinstance(item, VerifiedRow)]
    if errors:
        for error in errors:
            print(error.message, file=sys.stderr)
        return 1
    print(format_table(verified_rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
