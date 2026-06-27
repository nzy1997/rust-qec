from __future__ import annotations

import csv
import subprocess
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import verify_bravyi_ler
from benchmarks.bb_circuit_bposd_compare.cases import BATCHED_CSV_HEADER


FULL_RESULTS = (
    Path(__file__).resolve().parents[1] / "results" / "full" / "results.csv"
)


def make_row(**overrides: str) -> dict[str, str]:
    row = {column: "" for column in BATCHED_CSV_HEADER}
    row.update(
        {
            "case_id": "bb144-p0030-c12-t1000000-seed12345",
            "runner": "batched_compare",
            "decoder_impl": "rbposd",
            "code_id": "bb144",
            "p": "0.003",
            "num_cycles": "12",
            "shots_budget": "1000000",
            "errors_budget": "200",
            "shots_used": "40000",
            "logical_errors": "200",
            "logical_error_rate": "0.005",
            "status": "ok",
            "stop_reason": "errors_budget_reached",
        }
    )
    row.update(overrides)
    return row


def write_csv(path: Path, rows: list[dict[str, str]]) -> None:
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=BATCHED_CSV_HEADER)
        writer.writeheader()
        writer.writerows(rows)


def test_verify_rows_accepts_ok_and_partial_trial_level_rows() -> None:
    rows = [
        make_row(),
        make_row(
            case_id="bb72-p0040-c6-t1000000-seed12345",
            decoder_impl="ldpc_bposd",
            code_id="bb72",
            p="0.004",
            num_cycles="6",
            shots_used="1000",
            logical_errors="25",
            logical_error_rate="0.025",
            status="partial",
            stop_reason="wall_budget_exhausted",
        ),
    ]

    result = verify_bravyi_ler.verify_rows(rows)

    verified_rows = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerifiedRow)
    ]
    verification_errors = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerificationError)
    ]

    assert verification_errors == []
    assert [row.bravyi_tuple for row in verified_rows] == [
        ("0.003", 12, 40000, 200),
        ("0.004", 6, 1000, 25),
    ]


def test_verify_rows_rejects_per_cycle_normalized_row() -> None:
    row = make_row(logical_error_rate=str(200 / (40000 * 12)))

    result = verify_bravyi_ler.verify_rows([row])

    verification_errors = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerificationError)
    ]

    assert verification_errors
    assert "appears per-cycle normalized" in verification_errors[0].message
    assert "trial-level LER" in verification_errors[0].message


def test_verify_rows_returns_verification_error_for_bad_numeric_field() -> None:
    row = make_row(shots_used="bad")

    result = verify_bravyi_ler.verify_rows([row])

    verification_errors = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerificationError)
    ]

    assert verification_errors
    assert "parse" in verification_errors[0].message.lower()
    assert "shots_used" in verification_errors[0].message


def test_verify_rows_returns_verification_error_for_missing_numeric_cell() -> None:
    row = make_row()
    row["shots_used"] = None  # type: ignore[assignment]

    result = verify_bravyi_ler.verify_rows([row])

    verification_errors = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerificationError)
    ]

    assert verification_errors
    assert "parse" in verification_errors[0].message.lower()
    assert "shots_used" in verification_errors[0].message


def test_checked_in_full_results_are_trial_level_normalized() -> None:
    rows = verify_bravyi_ler.load_rows(FULL_RESULTS)

    result = verify_bravyi_ler.verify_rows(rows)

    verified_rows = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerifiedRow)
    ]

    assert verified_rows
    bb144_rows = [row for row in verified_rows if "bb144" in row.case_id]
    assert bb144_rows
    assert any(
        row.bravyi_tuple == ("0.003", 12, 40000, 200)
        for row in bb144_rows
    )


def test_verify_rows_returns_partitionable_items() -> None:
    result = verify_bravyi_ler.verify_rows(
        [
            make_row(),
            make_row(
                case_id="bb72-p0040-c6-t1000000-seed12345",
                decoder_impl="ldpc_bposd",
                code_id="bb72",
                p="0.004",
                num_cycles="6",
                shots_used="1000",
                logical_errors="25",
                logical_error_rate="0.025",
                status="partial",
                stop_reason="wall_budget_exhausted",
            ),
        ]
    )

    verified_rows = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerifiedRow)
    ]
    verification_errors = [
        item for item in result if isinstance(item, verify_bravyi_ler.VerificationError)
    ]

    assert len(verified_rows) == 2
    assert verification_errors == []


def test_cli_prints_pass_table_for_valid_csv(tmp_path: Path) -> None:
    csv_path = tmp_path / "results.csv"
    write_csv(csv_path, [make_row()])

    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler",
            str(csv_path),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert "PASS" in result.stdout
    assert "bravyi_tuple=(0.003, 12, 40000, 200)" in result.stdout


def test_cli_table_uses_exact_review_columns(tmp_path: Path) -> None:
    csv_path = tmp_path / "results.csv"
    write_csv(csv_path, [make_row()])

    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler",
            str(csv_path),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    lines = result.stdout.strip().splitlines()
    assert lines[0].split() == [
        "case_id",
        "decoder_impl",
        "shots_used",
        "logical_errors",
        "logical_error_rate",
        "bravyi_tuple",
    ]
    assert "status" not in lines[0]


def test_cli_negative_control_exits_nonzero_for_per_cycle_csv(tmp_path: Path) -> None:
    csv_path = tmp_path / "bb_ler_per_cycle_bad.csv"
    write_csv(csv_path, [make_row(logical_error_rate=str(200 / (40000 * 12)))])

    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler",
            str(csv_path),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "appears per-cycle normalized" in result.stderr
