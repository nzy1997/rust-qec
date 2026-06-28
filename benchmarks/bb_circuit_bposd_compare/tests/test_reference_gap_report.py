from __future__ import annotations

from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import (
    validate_reference_gap_report,
    write_reference_gap_report,
)
from benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract import UPSTREAM_COMMIT


ROOT = Path(__file__).resolve().parents[3]
RESULTS = (
    ROOT
    / "benchmarks"
    / "bb_circuit_bposd_compare"
    / "results"
    / "full"
    / "results.csv"
)
CONTRACT = (
    ROOT
    / "benchmarks"
    / "bb_circuit_bposd_compare"
    / "reference"
    / "bravyi_contract.json"
)


def write_report(tmp_path: Path, *, full_csv_treatment: str | None = None) -> Path:
    report = tmp_path / "reference_gap_report.md"
    args = [
        "--results",
        str(RESULTS),
        "--contract",
        str(CONTRACT),
        "--out",
        str(report),
        "--controlled-results",
        str(RESULTS),
        "--python-env",
        "/private/tmp/rstim-ldpc-venv/bin/python (ldpc 2.4.1, bposd 2.1, numpy 2.5.0)",
        "--rust-binary",
        "target/release/rsinter",
        "--rust-commit",
        "6e3d5a9c66e69c5c210c84bad298ca7593db0867",
        "--controlled-command",
        "python -m benchmarks.bb_circuit_bposd_compare.run_compare --tier bb72-bb144-plot-smoke",
    ]
    if full_csv_treatment is not None:
        args.extend(["--full-csv-treatment", full_csv_treatment])
    status = write_reference_gap_report.main(args)
    assert status == 0
    return report


def test_write_reference_gap_report_includes_required_sections(tmp_path: Path) -> None:
    report = write_report(tmp_path)
    text = report.read_text()

    assert text.startswith("# BB72/BB144 Circuit BP-OSD Reference-Gap Report\n")
    assert UPSTREAM_COMMIT in text
    assert "## Source Contract" in text
    assert "## Audit Status" in text
    assert "Bravyi contract audit | PASS" in text
    assert "Bravyi LER audit | PASS" in text
    assert "Batched accounting audit | PASS" in text
    assert "Bravyi model audit | PASS" in text
    assert "Hard replay parity | PASS" in text
    assert "Full results rows: 16" in text
    assert "Paired comparison groups: 8" in text
    assert "| bb72 | 0.003 | 6 | rbposd | 8000 | 216 | 0.027 | ok | errors_budget_reached |" in text
    assert "| bb72 | 0.003 | 6 | 0.027 | 0.027125 | -0.000125 |" in text
    assert "**Final verdict for #303:**" in text
    assert "not directly comparable" in text


def test_write_reference_gap_report_can_record_fresh_full_rerun(
    tmp_path: Path,
) -> None:
    treatment = (
        "fresh full paired rerun completed for the checked-in benchmark evidence."
    )
    report = write_report(tmp_path, full_csv_treatment=treatment)
    text = report.read_text()

    assert f"- Full CSV treatment: {treatment}" in text
    assert "preserved because the full paired rerun is too expensive" not in text
    assert "preserved BB72/BB144 full run" not in text


def test_validate_reference_gap_report_accepts_generated_report(
    tmp_path: Path, capsys
) -> None:
    report = write_report(tmp_path)

    status = validate_reference_gap_report.main(
        ["--results", str(RESULTS), "--report", str(report)]
    )

    captured = capsys.readouterr()
    assert status == 0
    assert "PASS reference gap report validated" in captured.out
    assert "rows=16" in captured.out
    assert "pairs=8" in captured.out


def test_validate_reference_gap_report_rejects_missing_contract_commit(
    tmp_path: Path, capsys
) -> None:
    report = write_report(tmp_path)
    report.write_text(report.read_text().replace(UPSTREAM_COMMIT, "", 1))

    status = validate_reference_gap_report.main(
        ["--results", str(RESULTS), "--report", str(report)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "Bravyi contract commit" in captured.err


def test_validate_reference_gap_report_rejects_missing_final_verdict(
    tmp_path: Path, capsys
) -> None:
    report = write_report(tmp_path)
    text = report.read_text()
    verdict_line = next(
        line for line in text.splitlines() if line.startswith("**Final verdict for #303:**")
    )
    report.write_text(text.replace(verdict_line + "\n", ""))

    status = validate_reference_gap_report.main(
        ["--results", str(RESULTS), "--report", str(report)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "final verdict" in captured.err


def test_validate_reference_gap_report_rejects_missing_report_file(
    tmp_path: Path, capsys
) -> None:
    missing_report = tmp_path / "missing_reference_gap_report.md"

    status = validate_reference_gap_report.main(
        ["--results", str(RESULTS), "--report", str(missing_report)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "failed to read report" in captured.err
    assert str(missing_report) in captured.err


def test_validate_reference_gap_report_rejects_tampered_ler_row(
    tmp_path: Path, capsys
) -> None:
    report = write_report(tmp_path)
    report.write_text(
        report.read_text().replace(
            "| bb72 | 0.003 | 6 | rbposd | 8000 | 216 | 0.027 | ok | errors_budget_reached |",
            "| bb72 | 0.003 | 6 | rbposd | 8000 | 216 | 0.001 | ok | errors_budget_reached |",
            1,
        )
    )

    status = validate_reference_gap_report.main(
        ["--results", str(RESULTS), "--report", str(report)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "per-row LER table" in captured.err
