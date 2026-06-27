from __future__ import annotations

import csv
import subprocess
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import verify_batched_accounting
from benchmarks.bb_circuit_bposd_compare.cases import BATCHED_CSV_HEADER


FULL_RESULTS = (
    Path(__file__).resolve().parents[1] / "results" / "full" / "results.csv"
)


def make_row(decoder_impl: str = "rbposd", **overrides: str) -> dict[str, str]:
    logical_errors = "200" if decoder_impl == "rbposd" else "120"
    shots_used = "500"
    row = {column: "" for column in BATCHED_CSV_HEADER}
    row.update(
        {
            "case_id": "bb72-p0030-c6-t1000000-seed12345",
            "runner": "batched_compare",
            "decoder_impl": decoder_impl,
            "code_id": "bb72",
            "p": "0.003",
            "num_cycles": "6",
            "shots_budget": "1000000",
            "errors_budget": "200",
            "shots_used": shots_used,
            "seed": "12345",
            "bp_method": "ms",
            "max_iter": "10000",
            "osd_method": "osd_cs",
            "osd_order": "7",
            "batch_size": "500",
            "batches_completed": "1",
            "logical_errors": logical_errors,
            "logical_error_rate": str(int(logical_errors) / int(shots_used)),
            "status": "ok",
            "stop_reason": "errors_budget_reached",
            "error": "",
        }
    )
    row.update(overrides)
    if "logical_errors" in overrides and "logical_error_rate" not in overrides:
        row["logical_error_rate"] = str(
            int(row["logical_errors"]) / int(row["shots_used"])
        )
    if "shots_used" in overrides and "logical_error_rate" not in overrides:
        row["logical_error_rate"] = str(
            int(row["logical_errors"]) / int(row["shots_used"])
        )
    return row


def make_pair(
    rust_overrides: dict[str, str] | None = None,
    python_overrides: dict[str, str] | None = None,
) -> list[dict[str, str]]:
    return [
        make_row("rbposd", **(rust_overrides or {})),
        make_row("ldpc_bposd", **(python_overrides or {})),
    ]


def write_csv(path: Path, rows: list[dict[str, str]]) -> None:
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=BATCHED_CSV_HEADER)
        writer.writeheader()
        writer.writerows(rows)


def partition(
    result: list[
        verify_batched_accounting.VerifiedPair
        | verify_batched_accounting.VerificationError
    ],
) -> tuple[
    list[verify_batched_accounting.VerifiedPair],
    list[verify_batched_accounting.VerificationError],
]:
    verified = [
        item
        for item in result
        if isinstance(item, verify_batched_accounting.VerifiedPair)
    ]
    errors = [
        item
        for item in result
        if isinstance(item, verify_batched_accounting.VerificationError)
    ]
    return verified, errors


def test_accepts_comparable_error_budget_pair() -> None:
    verified, errors = partition(verify_batched_accounting.verify_rows(make_pair()))
    assert errors == []
    assert len(verified) == 1
    assert verified[0].case_id == "bb72-p0030-c6-t1000000-seed12345"
    assert verified[0].stop_reason == "errors_budget_reached"
    assert verified[0].shots_used == 500
    assert verified[0].batches_completed == 1
    assert verified[0].rbposd_logical_errors == 200
    assert verified[0].ldpc_bposd_logical_errors == 120


def test_checked_in_full_results_have_comparable_error_budget_pairs() -> None:
    result = verify_batched_accounting.verify_rows(
        verify_batched_accounting.load_rows(FULL_RESULTS)
    )
    verified, errors = partition(result)
    assert errors == []
    assert {pair.code_id for pair in verified} == {"bb72", "bb144"}
    assert all(pair.stop_reason == "errors_budget_reached" for pair in verified)
    assert any(pair.code_id == "bb72" for pair in verified)
    assert any(pair.code_id == "bb144" for pair in verified)
    assert all(pair.shots_used > 0 for pair in verified)
    assert all(pair.batches_completed > 0 for pair in verified)


def test_rejects_mismatched_python_shots_used_as_uncomparable() -> None:
    rows = make_pair(python_overrides={"shots_used": "501"})
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "no longer comparable" in errors[0].message
    assert "shots_used" in errors[0].message


def test_rejects_mismatched_python_batches_completed_as_uncomparable() -> None:
    rows = make_pair(python_overrides={"batches_completed": "2"})
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "no longer comparable" in errors[0].message
    assert "batches_completed" in errors[0].message


def test_errors_budget_stop_requires_one_decoder_to_reach_budget() -> None:
    rows = make_pair(
        rust_overrides={"logical_errors": "199"},
        python_overrides={"logical_errors": "198"},
    )
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "errors_budget_reached" in errors[0].message
    assert "logical_errors >= errors_budget" in errors[0].message


def test_accepts_wall_budget_partial_pair() -> None:
    rows = make_pair(
        rust_overrides={
            "status": "partial",
            "stop_reason": "wall_budget_exhausted",
            "logical_errors": "2",
        },
        python_overrides={
            "status": "partial",
            "stop_reason": "wall_budget_exhausted",
            "logical_errors": "1",
        },
    )
    verified, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors == []
    assert verified[0].status == "partial"
    assert verified[0].stop_reason == "wall_budget_exhausted"


def test_rejects_partial_row_without_explicit_partial_reason() -> None:
    rows = make_pair(
        rust_overrides={"status": "partial", "stop_reason": "completed"},
        python_overrides={"status": "partial", "stop_reason": "completed"},
    )
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "partial" in errors[0].message
    assert "wall_budget_exhausted" in errors[0].message


def test_accepts_explicit_python_dependency_missing_skip() -> None:
    rows = [
        make_row(
            "rbposd",
            status="partial",
            stop_reason="python_dependency_missing",
            logical_errors="3",
        ),
        make_row(
            "ldpc_bposd",
            status="skipped",
            stop_reason="python_dependency_missing",
            logical_errors="0",
            logical_error_rate="0.0",
            error="python dependency unavailable for ldpc_bposd replay: ldpc",
        ),
    ]
    verified, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors == []
    assert verified[0].status == "partial"
    assert verified[0].stop_reason == "python_dependency_missing"
    assert verified[0].ldpc_bposd_logical_errors is None


def test_rejects_logical_error_rate_not_computed_from_shots_used() -> None:
    rows = make_pair(python_overrides={"logical_error_rate": "0.999"})
    _, errors = partition(verify_batched_accounting.verify_rows(rows))
    assert errors
    assert "logical_error_rate" in errors[0].message
    assert "logical_errors / shots_used" in errors[0].message


def test_rejects_missing_official_batched_csv_header_column(tmp_path: Path) -> None:
    csv_path = tmp_path / "missing_shots_budget.csv"
    header = [column for column in BATCHED_CSV_HEADER if column != "shots_budget"]
    row = {column: value for column, value in make_row().items() if column in header}
    with csv_path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=header)
        writer.writeheader()
        writer.writerow(row)

    result = verify_batched_accounting.verify_rows(
        verify_batched_accounting.load_rows(csv_path)
    )
    _, errors = partition(result)
    assert errors
    assert "row is missing required CSV column(s)" in errors[0].message
    assert "shots_budget" in errors[0].message


def test_cli_prints_pass_lines_for_full_results() -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_batched_accounting",
            str(FULL_RESULTS),
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    assert "PASS BB batched paired accounting" in result.stdout
    assert "PASS bb72-p0030-c6-t1000000-seed12345" in result.stdout
    assert "PASS bb144-p0030-c12-t1000000-seed12345" in result.stdout
    assert "stop_reason=errors_budget_reached" in result.stdout


def test_cli_negative_control_exits_nonzero_for_unpaired_rows(tmp_path: Path) -> None:
    csv_path = tmp_path / "bb_batched_unpaired_bad.csv"
    write_csv(csv_path, make_pair(python_overrides={"shots_used": "501"}))
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_batched_accounting",
            str(csv_path),
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0
    assert "no longer comparable" in result.stderr
