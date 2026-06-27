from __future__ import annotations

import argparse
import csv
import math
import sys
from dataclasses import dataclass
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare.cases import BATCHED_CSV_HEADER


TOLERANCE = 1e-12
COMPLETED_STATUSES = {"ok", "partial"}
PARTIAL_STOP_REASONS = {"wall_budget_exhausted", "python_dependency_missing"}
PAIR_METADATA_FIELDS = (
    "shots_used",
    "batch_size",
    "batches_completed",
    "stop_reason",
    "seed",
    "bp_method",
    "max_iter",
    "osd_method",
    "osd_order",
)
REQUIRED_COLUMNS = tuple(BATCHED_CSV_HEADER)


@dataclass(frozen=True)
class CompletedRow:
    row_index: int
    raw: dict[str, str]
    logical_errors: int
    logical_error_rate: float
    shots_used: int
    batch_size: int
    batches_completed: int
    errors_budget: int | None


@dataclass(frozen=True)
class VerifiedPair:
    case_id: str
    code_id: str
    p: str
    num_cycles: str
    status: str
    stop_reason: str
    shots_used: int
    batch_size: int
    batches_completed: int
    rbposd_logical_errors: int
    ldpc_bposd_logical_errors: int | None


@dataclass(frozen=True)
class VerificationError:
    message: str


def load_rows(csv_path: Path) -> list[dict[str, str]]:
    with csv_path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def verify_rows(rows: list[dict[str, str]]) -> list[VerifiedPair | VerificationError]:
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

    groups: dict[tuple[str, str, str, str], list[tuple[int, dict[str, str]]]] = {}
    results: list[VerifiedPair | VerificationError] = []
    for row_index, row in enumerate(rows, start=2):
        if row.get("runner") != "batched_compare":
            continue
        key = (row["case_id"], row["code_id"], row["p"], row["num_cycles"])
        groups.setdefault(key, []).append((row_index, row))

    if not groups:
        return [VerificationError("CSV has no batched_compare rows to verify")]

    for key in sorted(groups):
        results.extend(_verify_group(key, groups[key]))
    return results


def _verify_group(
    key: tuple[str, str, str, str],
    group_rows: list[tuple[int, dict[str, str]]],
) -> list[VerifiedPair | VerificationError]:
    errors: list[VerificationError] = []
    case_id, code_id, p, num_cycles = key
    rust_rows = [item for item in group_rows if item[1].get("decoder_impl") == "rbposd"]
    python_rows = [
        item for item in group_rows if item[1].get("decoder_impl") == "ldpc_bposd"
    ]
    other_rows = [
        item
        for item in group_rows
        if item[1].get("decoder_impl") not in {"rbposd", "ldpc_bposd"}
    ]

    context = f"group {case_id} ({code_id}, p={p}, num_cycles={num_cycles})"
    for row_index, row in other_rows:
        errors.append(
            VerificationError(
                f"row {row_index} {context}: unsupported decoder_impl "
                f"{row.get('decoder_impl', '')!r}"
            )
        )

    if len(rust_rows) != 1:
        errors.append(
            VerificationError(
                f"{context}: expected exactly one rbposd row, found {len(rust_rows)}"
            )
        )
    if len(python_rows) != 1:
        errors.append(
            VerificationError(
                f"{context}: expected exactly one ldpc_bposd row unless explicitly "
                f"skipped for python_dependency_missing; found {len(python_rows)}"
            )
        )
    if errors:
        return errors

    rust_parsed = _parse_completed_row(*rust_rows[0])
    if isinstance(rust_parsed, VerificationError):
        errors.append(rust_parsed)
    python_row_index, python_row = python_rows[0]
    python_status = python_row.get("status", "")
    python_stop_reason = python_row.get("stop_reason", "")
    rust_stop_reason = rust_rows[0][1].get("stop_reason", "")

    if python_status == "skipped":
        skip_error = _validate_python_dependency_skip(
            key=key,
            rust_parsed=rust_parsed,
            rust_row=rust_rows[0][1],
            python_row_index=python_row_index,
            python_row=python_row,
        )
        if isinstance(skip_error, VerificationError):
            errors.append(skip_error)
            return errors
        assert isinstance(rust_parsed, CompletedRow)
        accounting_error = _validate_logical_error_rate(rust_parsed)
        if accounting_error is not None:
            return [accounting_error]
        return [
            VerifiedPair(
                case_id=case_id,
                code_id=code_id,
                p=p,
                num_cycles=num_cycles,
                status=rust_parsed.raw["status"],
                stop_reason=rust_parsed.raw["stop_reason"],
                shots_used=rust_parsed.shots_used,
                batch_size=rust_parsed.batch_size,
                batches_completed=rust_parsed.batches_completed,
                rbposd_logical_errors=rust_parsed.logical_errors,
                ldpc_bposd_logical_errors=None,
            )
        ]

    if (
        python_stop_reason == "python_dependency_missing"
        or rust_stop_reason == "python_dependency_missing"
    ):
        errors.append(
            VerificationError(
                f"row {python_row_index} {context}: only a skipped ldpc_bposd row may use "
                f"stop_reason='python_dependency_missing'"
            )
        )

    python_parsed = _parse_completed_row(python_row_index, python_row)
    if isinstance(python_parsed, VerificationError):
        errors.append(python_parsed)

    if errors:
        return errors

    assert isinstance(rust_parsed, CompletedRow)
    assert isinstance(python_parsed, CompletedRow)

    for field_name in PAIR_METADATA_FIELDS:
        if rust_parsed.raw[field_name] != python_parsed.raw[field_name]:
            errors.append(
                VerificationError(
                    f"{context}: Rust/Python pair is no longer comparable because "
                    f"{field_name} differs ({rust_parsed.raw[field_name]!r} != "
                    f"{python_parsed.raw[field_name]!r})"
                )
            )

    if rust_parsed.raw["status"] == "partial" or python_parsed.raw["status"] == "partial":
        for parsed in (rust_parsed, python_parsed):
            if parsed.raw["status"] == "partial" and parsed.raw["stop_reason"] not in PARTIAL_STOP_REASONS:
                errors.append(
                    VerificationError(
                        f"{context}: partial row must use explicit stop_reason "
                        f"wall_budget_exhausted or python_dependency_missing; got "
                        f"{parsed.raw['stop_reason']!r}"
                    )
                )

    if rust_parsed.raw["stop_reason"] == "errors_budget_reached":
        budget_error = _validate_errors_budget_reached(
            key=key,
            rust_parsed=rust_parsed,
            python_parsed=python_parsed,
        )
        if budget_error is not None:
            errors.append(budget_error)

    for parsed in (rust_parsed, python_parsed):
        accounting_error = _validate_logical_error_rate(parsed)
        if accounting_error is not None:
            errors.append(accounting_error)

    if errors:
        return errors

    status = "partial" if "partial" in {rust_parsed.raw["status"], python_parsed.raw["status"]} else "ok"
    return [
        VerifiedPair(
            case_id=case_id,
            code_id=code_id,
            p=p,
            num_cycles=num_cycles,
            status=status,
            stop_reason=rust_parsed.raw["stop_reason"],
            shots_used=rust_parsed.shots_used,
            batch_size=rust_parsed.batch_size,
            batches_completed=rust_parsed.batches_completed,
            rbposd_logical_errors=rust_parsed.logical_errors,
            ldpc_bposd_logical_errors=python_parsed.logical_errors,
        )
    ]


def _parse_completed_row(
    row_index: int,
    row: dict[str, str],
) -> CompletedRow | VerificationError:
    context = f"row {row_index} {row.get('case_id', '<missing case_id>')} {row.get('decoder_impl', '')}"
    status = row.get("status", "")
    if status not in COMPLETED_STATUSES:
        return VerificationError(
            f"{context}: expected completed row status in {sorted(COMPLETED_STATUSES)}, got {status!r}"
        )

    num_cycles = _parse_int(row, "num_cycles", context)
    if isinstance(num_cycles, VerificationError):
        return num_cycles
    shots_used = _parse_int(row, "shots_used", context)
    if isinstance(shots_used, VerificationError):
        return shots_used
    batch_size = _parse_int(row, "batch_size", context)
    if isinstance(batch_size, VerificationError):
        return batch_size
    batches_completed = _parse_int(row, "batches_completed", context)
    if isinstance(batches_completed, VerificationError):
        return batches_completed
    logical_errors = _parse_int(row, "logical_errors", context)
    if isinstance(logical_errors, VerificationError):
        return logical_errors
    logical_error_rate = _parse_float(row, "logical_error_rate", context)
    if isinstance(logical_error_rate, VerificationError):
        return logical_error_rate
    errors_budget = _parse_optional_int(row, "errors_budget", context)
    if isinstance(errors_budget, VerificationError):
        return errors_budget

    if num_cycles <= 0:
        return VerificationError(f"{context}: num_cycles must be positive")
    if shots_used <= 0:
        return VerificationError(f"{context}: shots_used must be positive")
    if batch_size <= 0:
        return VerificationError(f"{context}: batch_size must be positive")
    if batches_completed <= 0:
        return VerificationError(f"{context}: batches_completed must be positive")
    if logical_errors < 0:
        return VerificationError(f"{context}: logical_errors must be nonnegative")
    if logical_errors > shots_used:
        return VerificationError(f"{context}: logical_errors must be <= shots_used")
    if not math.isfinite(logical_error_rate):
        return VerificationError(f"{context}: logical_error_rate must be finite")

    return CompletedRow(
        row_index=row_index,
        raw=row,
        logical_errors=logical_errors,
        logical_error_rate=logical_error_rate,
        shots_used=shots_used,
        batch_size=batch_size,
        batches_completed=batches_completed,
        errors_budget=errors_budget,
    )


def _validate_python_dependency_skip(
    key: tuple[str, str, str, str],
    rust_parsed: CompletedRow | VerificationError,
    rust_row: dict[str, str],
    python_row_index: int,
    python_row: dict[str, str],
) -> VerificationError | None:
    case_id, code_id, p, num_cycles = key
    context = f"group {case_id} ({code_id}, p={p}, num_cycles={num_cycles})"
    if isinstance(rust_parsed, VerificationError):
        return rust_parsed
    if python_row.get("stop_reason") != "python_dependency_missing":
        return VerificationError(
            f"row {python_row_index} {context}: skipped ldpc_bposd row must use "
            f"stop_reason='python_dependency_missing'"
        )
    if rust_row.get("status") != "partial" or rust_row.get("stop_reason") != "python_dependency_missing":
        return VerificationError(
            f"{context}: python_dependency_missing skip requires the rbposd row to be "
            f"status='partial' with stop_reason='python_dependency_missing'"
        )
    for field_name in (
        "shots_used",
        "batch_size",
        "batches_completed",
        "seed",
        "bp_method",
        "max_iter",
        "osd_method",
        "osd_order",
    ):
        if rust_row.get(field_name, "") != python_row.get(field_name, ""):
            return VerificationError(
                f"{context}: Rust/Python pair is no longer comparable because "
                f"{field_name} differs ({rust_row.get(field_name)!r} != "
                f"{python_row.get(field_name)!r})"
            )
    return None


def _validate_logical_error_rate(parsed: CompletedRow) -> VerificationError | None:
    expected = parsed.logical_errors / parsed.shots_used
    if math.isclose(
        parsed.logical_error_rate,
        expected,
        rel_tol=0.0,
        abs_tol=TOLERANCE,
    ):
        return None
    context = (
        f"row {parsed.row_index} {parsed.raw.get('case_id', '<missing case_id>')} "
        f"{parsed.raw.get('decoder_impl', '')}"
    )
    return VerificationError(
        f"{context}: logical_error_rate mismatched row accounting; got "
        f"{parsed.logical_error_rate}, expected {expected} from logical_errors / shots_used"
    )


def _validate_errors_budget_reached(
    key: tuple[str, str, str, str],
    rust_parsed: CompletedRow,
    python_parsed: CompletedRow,
) -> VerificationError | None:
    case_id, code_id, p, num_cycles = key
    context = f"group {case_id} ({code_id}, p={p}, num_cycles={num_cycles})"
    budget = rust_parsed.errors_budget
    if budget is None or python_parsed.errors_budget is None:
        return VerificationError(f"{context}: errors_budget_reached requires a numeric errors_budget")
    if budget <= 0:
        return VerificationError(f"{context}: errors_budget_reached requires errors_budget > 0")
    if python_parsed.errors_budget != budget:
        return VerificationError(
            f"{context}: Rust/Python pair is no longer comparable because errors_budget differs "
            f"({budget!r} != {python_parsed.errors_budget!r})"
        )
    if (
        rust_parsed.logical_errors < budget
        and python_parsed.logical_errors < budget
    ):
        return VerificationError(
            f"{context}: stop_reason='errors_budget_reached' requires at least one "
            f"decoder to satisfy logical_errors >= errors_budget"
        )
    return None


def _parse_int(
    row: dict[str, str],
    field_name: str,
    context: str,
) -> int | VerificationError:
    try:
        return int(row[field_name])
    except (TypeError, ValueError) as error:
        return VerificationError(
            f"{context}: failed to parse numeric field {field_name}: {error}"
        )


def _parse_optional_int(
    row: dict[str, str],
    field_name: str,
    context: str,
) -> int | None | VerificationError:
    value = row.get(field_name, "")
    if value == "":
        return None
    try:
        return int(value)
    except (TypeError, ValueError) as error:
        return VerificationError(
            f"{context}: failed to parse numeric field {field_name}: {error}"
        )


def _parse_float(
    row: dict[str, str],
    field_name: str,
    context: str,
) -> float | VerificationError:
    try:
        return float(row[field_name])
    except (TypeError, ValueError) as error:
        return VerificationError(
            f"{context}: failed to parse numeric field {field_name}: {error}"
        )


def format_table(rows: list[VerifiedPair]) -> str:
    lines: list[str] = []
    for row in rows:
        python_errors = (
            "python_dependency_missing"
            if row.ldpc_bposd_logical_errors is None
            else str(row.ldpc_bposd_logical_errors)
        )
        lines.append(
            f"PASS {row.case_id} code_id={row.code_id} p={row.p} "
            f"num_cycles={row.num_cycles} status={row.status} "
            f"stop_reason={row.stop_reason} shots_used={row.shots_used} "
            f"batches_completed={row.batches_completed} batch_size={row.batch_size} "
            f"rbposd_logical_errors={row.rbposd_logical_errors} "
            f"ldpc_bposd_logical_errors={python_errors}"
        )
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("csv_path", type=Path)
    args = parser.parse_args(argv)

    results = verify_rows(load_rows(args.csv_path))
    errors = [item for item in results if isinstance(item, VerificationError)]
    verified_pairs = [item for item in results if isinstance(item, VerifiedPair)]
    if errors:
        for error in errors:
            print(error.message, file=sys.stderr)
        return 1
    print("PASS BB batched paired accounting")
    print(format_table(verified_pairs))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
