from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import write_readiness_report

REQUIRED_HEADINGS = (
    "# BB BP-OSD Full-Campaign Readiness Report",
    "## Gate Summary",
    "## Semantic Parity Replay",
    "## BB90 Hard-Profile Counters",
    "## Setup/Run Split Evidence",
    "## Diagnostic Rust/Python Compare Rows",
    "## Small-LDPC Case Coverage",
)


def validate_report(results_dir: Path, report_path: Path) -> list[str]:
    errors: list[str] = []
    try:
        report = report_path.read_text()
    except OSError as error:
        return [f"cannot read report: {error}"]

    for heading in REQUIRED_HEADINGS:
        if heading not in report:
            errors.append(f"missing report section: {heading}")

    expected = write_readiness_report.snapshot_model(
        write_readiness_report.build_report_model(results_dir)
    )
    visible_verdict = _visible_verdict(report)
    if visible_verdict is None:
        errors.append("missing final readiness verdict")
    elif visible_verdict != expected["verdict"]:
        errors.append(
            "final readiness verdict mismatch: "
            f"report says {visible_verdict}, source gate says {expected['verdict']}"
        )

    snapshot = _report_snapshot(report, errors)
    if snapshot is not None and snapshot != expected:
        _append_snapshot_errors(snapshot, expected, errors)

    _check_visible_tokens(report, expected, errors)
    return errors


def _visible_verdict(report: str) -> str | None:
    match = re.search(
        r"^\*\*Final readiness verdict:\*\*\s+(PASS|WARN|FAIL)\s*$",
        report,
        re.MULTILINE,
    )
    if match is None:
        return None
    return match.group(1)


def _report_snapshot(report: str, errors: list[str]) -> dict[str, object] | None:
    prefix = re.escape(write_readiness_report.SNAPSHOT_PREFIX)
    match = re.search(rf"<!--\s*{prefix}\s*(\{{.*\}})\s*-->", report)
    if match is None:
        errors.append("missing snapshot: rstim-bb-readiness-snapshot comment not found")
        return None
    try:
        snapshot = json.loads(match.group(1))
    except json.JSONDecodeError as error:
        errors.append(f"malformed snapshot: {error}")
        return None
    if not isinstance(snapshot, dict):
        errors.append("malformed snapshot: expected JSON object")
        return None
    return snapshot


def _append_snapshot_errors(
    snapshot: dict[str, object], expected: dict[str, object], errors: list[str]
) -> None:
    flagged: set[str] = set()

    def flag(section: str) -> None:
        if section not in flagged:
            flagged.add(section)
            errors.append(f"stale or missing section: {section}")

    if snapshot.get("verdict") != expected.get("verdict"):
        flag("final readiness verdict")
    if snapshot.get("gate_checks") != expected.get("gate_checks"):
        flag("gate-summary")

    snapshot_hashes = snapshot.get("artifact_hashes")
    expected_hashes = expected.get("artifact_hashes")
    if not isinstance(snapshot_hashes, dict) or not isinstance(expected_hashes, dict):
        errors.append("stale or missing snapshot metadata: artifact_hashes")
    else:
        for name, value in expected_hashes.items():
            if snapshot_hashes.get(name) != value:
                flag(str(name))

    snapshot_sections = snapshot.get("sections")
    expected_sections = expected.get("sections")
    if not isinstance(snapshot_sections, dict) or not isinstance(expected_sections, dict):
        errors.append("stale or missing snapshot metadata: sections")
    else:
        for name, value in expected_sections.items():
            if snapshot_sections.get(name) != value:
                flag(str(name))

    if not flagged and snapshot != expected:
        errors.append("stale or missing snapshot metadata")


def _check_visible_tokens(
    report: str, expected: dict[str, object], errors: list[str]
) -> None:
    gate_checks = expected.get("gate_checks")
    if isinstance(gate_checks, list) and gate_checks:
        first_check = gate_checks[0]
        if isinstance(first_check, dict):
            _require_visible_tokens(
                report,
                "gate-summary",
                [first_check.get("name"), first_check.get("artifact")],
                errors,
            )

    sections = expected.get("sections")
    if not isinstance(sections, dict):
        return

    semantic_rows = sections.get("semantic-replay")
    if isinstance(semantic_rows, list) and semantic_rows:
        first_row = semantic_rows[0]
        if isinstance(first_row, dict):
            _require_visible_tokens(
                report,
                "semantic-replay",
                [first_row.get("case_id"), first_row.get("logical_prediction")],
                errors,
            )

    hard_profile = sections.get("hard-profile")
    if isinstance(hard_profile, dict) and hard_profile:
        _require_visible_tokens(
            report,
            "hard-profile",
            [
                "planned_candidate_count",
                hard_profile.get("planned_candidate_count"),
                hard_profile.get("case_id"),
            ],
            errors,
        )

    setup_run = sections.get("setup-run-separation")
    if isinstance(setup_run, dict) and setup_run:
        _require_visible_tokens(
            report,
            "setup-run-separation",
            ["decoder_build_count", setup_run.get("code_id"), setup_run.get("sample_count")],
            errors,
        )

    diagnostic_rows = sections.get("diagnostic-compare")
    if isinstance(diagnostic_rows, list) and diagnostic_rows:
        first_row = diagnostic_rows[0]
        if isinstance(first_row, dict):
            _require_visible_tokens(
                report,
                "diagnostic-compare",
                [first_row.get("case_id"), first_row.get("decoder_impl")],
                errors,
            )

    catalog_rows = sections.get("catalog-coverage")
    if isinstance(catalog_rows, list) and catalog_rows:
        first_row = catalog_rows[0]
        if isinstance(first_row, dict):
            _require_visible_tokens(
                report,
                "catalog-coverage",
                [first_row.get("code_id"), first_row.get("catalog_status")],
                errors,
            )


def _require_visible_tokens(
    report: str, section: str, tokens: list[object], errors: list[str]
) -> None:
    visible_tokens = [str(token) for token in tokens if token not in (None, "")]
    if not visible_tokens:
        return
    if any(token not in report for token in visible_tokens):
        errors.append(f"placeholder section: {section}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    errors = validate_report(args.results_dir, args.report)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("readiness report validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
