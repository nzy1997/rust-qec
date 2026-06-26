from benchmarks.bb_circuit_bposd_compare import (
    validate_readiness_report,
    write_readiness_report,
)
from benchmarks.bb_circuit_bposd_compare.tests.test_ready_for_full import (
    _write_json,
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


def test_validate_readiness_report_rejects_visible_hard_profile_tampering(
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
    report = report_path.read_text()
    assert "| planned_candidate_count | 4100 |" in report
    report_path.write_text(
        report.replace(
            "| planned_candidate_count | 4100 |",
            "| planned_candidate_count | 9999 |",
        )
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "hard-profile" in captured.err


def test_validate_readiness_report_rejects_duplicate_spoofed_hard_profile_section(
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

    report = report_path.read_text()
    hard_profile_heading = "## BB90 Hard-Profile Counters"
    next_heading = "## Setup/Run Split Evidence"
    start = report.index(hard_profile_heading)
    end = report.index(next_heading)
    hard_profile_section = report[start:end]
    tampered_section = hard_profile_section.replace(
        "| planned_candidate_count | 4100 |",
        "| planned_candidate_count | 9999 |",
    )
    assert tampered_section != hard_profile_section
    report_path.write_text(report[:start] + hard_profile_section + tampered_section + report[end:])

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "hard-profile" in captured.err


def test_validate_readiness_report_rejects_duplicate_final_verdict_lines(
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

    report_path.write_text(
        report_path.read_text().replace(
            "**Final readiness verdict:** PASS",
            "**Final readiness verdict:** PASS\n**Final readiness verdict:** FAIL",
            1,
        )
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "final readiness verdict" in captured.err


def test_validate_readiness_report_requires_visible_final_verdict_line(
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
    report_path.write_text(
        report_path.read_text().replace("**Final readiness verdict:** PASS\n", "")
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "final readiness verdict line in report preamble" in captured.err


def test_validate_readiness_report_rejects_visible_prose_before_title(
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
    report_path.write_text(
        "Reviewer note: manual approval pending.\n\n" + report_path.read_text()
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "document-structure" in captured.err


def test_validate_readiness_report_rejects_visible_section_after_snapshot(
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
    report_path.write_text(
        report_path.read_text()
        + "\n## Reviewer Notes\n\nManual summary: everything looks good here.\n"
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "report-body" in captured.err


def test_validate_readiness_report_rejects_visible_content_on_snapshot_line(
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

    snapshot_line = next(
        line
        for line in report_path.read_text().splitlines()
        if write_readiness_report.SNAPSHOT_PREFIX in line
    )
    report_path.write_text(
        report_path.read_text().replace(
            snapshot_line,
            snapshot_line + " **visible tamper after snapshot**",
            1,
        )
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert (
        "report-body" in captured.err
        or "snapshot" in captured.err
        or "document-structure" in captured.err
    )


def test_validate_readiness_report_rejects_visible_source_results_dir_tampering(
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

    report_path.write_text(
        report_path.read_text().replace(
            f"**Source results directory:** {results_dir}",
            "**Source results directory:** /tmp/forged-results-dir",
            1,
        )
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "report preamble" in captured.err or "Source results directory" in captured.err


def test_write_readiness_report_escapes_markdown_table_cells(tmp_path) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir, provenance=False)
    _write_json(
        results_dir / "provenance.json",
        {
            "artifact_hash": "sha256:example",
            "command": "collect | verify\nclose --> reopen",
            "timestamp": "2026-06-27T00:00:00+08:00",
        },
    )

    status = write_readiness_report.main(
        ["--results-dir", str(results_dir), "--out", str(report_path)]
    )

    report = report_path.read_text()
    assert status == 0
    assert "collect \\| verify<br>close" in report
    assert "-->" not in "\n".join(
        line
        for line in report.splitlines()
        if write_readiness_report.SNAPSHOT_PREFIX not in line
    )


def test_write_readiness_report_handles_unreadable_detail_artifacts(tmp_path) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)

    hard_replay_path = results_dir / "hard-replay" / "results.csv"
    hard_replay_path.unlink()
    hard_replay_path.mkdir()

    setup_profile_path = results_dir / "setup-run" / "profile.json"
    setup_profile_path.unlink()
    setup_profile_path.mkdir()

    status = write_readiness_report.main(
        ["--results-dir", str(results_dir), "--out", str(report_path)]
    )

    report = report_path.read_text()
    assert status == 0
    assert "**Final readiness verdict:** FAIL" in report
    assert "semantic-replay" in report
    assert "setup-run-separation" in report
    assert "cannot read hard-replay/results.csv" in report
    assert "cannot read setup-run/profile.json" in report


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
