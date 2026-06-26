from benchmarks.bb_circuit_bposd_compare import (
    validate_readiness_report,
    write_readiness_report,
)
from benchmarks.bb_circuit_bposd_compare.tests.test_ready_for_full import (
    write_ready_tree,
)


def test_write_readiness_report_includes_required_reviewer_sections(tmp_path) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)

    status = write_readiness_report.main(
        ["--results-dir", str(results_dir), "--out", str(report_path)]
    )

    assert status == 0
    report = report_path.read_text()
    assert "# BB BP-OSD Full-Campaign Readiness Report" in report
    assert "**Final readiness verdict:** PASS" in report
    assert "## Gate Summary" in report
    assert "## Semantic Parity Replay" in report
    assert "bb90-p006-c10-seed12345-order7-hard-syndrome" in report
    assert "## BB90 Hard-Profile Counters" in report
    assert "planned_candidate_count" in report
    assert "4100" in report
    assert "## Setup/Run Split Evidence" in report
    assert "decoder_build_count" in report
    assert "## Diagnostic Rust/Python Compare Rows" in report
    assert "bb144-p0060-c12-t1-seed12345" in report
    assert "## Small-LDPC Case Coverage" in report
    assert "bb288" in report
    assert "unsupported_rust_constructor" in report
    assert "rstim-bb-readiness-snapshot" in report


def test_validate_readiness_report_accepts_generated_report(
    tmp_path, capsys
) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)
    assert (
        write_readiness_report.main(
            ["--results-dir", str(results_dir), "--out", str(report_path)]
        )
        == 0
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 0
    assert "readiness report validated" in captured.out


def test_validate_readiness_report_rejects_stale_catalog_section(
    tmp_path, capsys
) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)
    assert (
        write_readiness_report.main(
            ["--results-dir", str(results_dir), "--out", str(report_path)]
        )
        == 0
    )
    (results_dir / "small-ldpc-catalog" / "manifest.csv").unlink()

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "catalog-coverage" in captured.err


def test_validate_readiness_report_rejects_visible_pass_when_gate_fails(
    tmp_path, capsys
) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)
    (results_dir / "hard-profile" / "profile.json").unlink()
    assert (
        write_readiness_report.main(
            ["--results-dir", str(results_dir), "--out", str(report_path)]
        )
        == 0
    )
    report_path.write_text(
        report_path.read_text().replace(
            "**Final readiness verdict:** FAIL",
            "**Final readiness verdict:** PASS",
        )
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "final readiness verdict" in captured.err


def test_validate_readiness_report_rejects_placeholder_report(
    tmp_path, capsys
) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)
    report_path.write_text(
        "# BB BP-OSD Full-Campaign Readiness Report\n\n"
        "**Final readiness verdict:** PASS\n\n"
        "## Gate Summary\n\n"
        "## Semantic Parity Replay\n\n"
        "## BB90 Hard-Profile Counters\n\n"
        "## Setup/Run Split Evidence\n\n"
        "## Diagnostic Rust/Python Compare Rows\n\n"
        "## Small-LDPC Case Coverage\n"
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "snapshot" in captured.err
