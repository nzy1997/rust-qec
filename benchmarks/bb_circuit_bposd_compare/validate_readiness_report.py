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

VISIBLE_SECTIONS = (
    ("gate-summary", "## Gate Summary"),
    ("semantic-replay", "## Semantic Parity Replay"),
    ("hard-profile", "## BB90 Hard-Profile Counters"),
    ("setup-run-separation", "## Setup/Run Split Evidence"),
    ("diagnostic-compare", "## Diagnostic Rust/Python Compare Rows"),
    ("catalog-coverage", "## Small-LDPC Case Coverage"),
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

    expected_model = write_readiness_report.build_report_model(results_dir)
    expected = write_readiness_report.snapshot_model(expected_model)
    expected_report = write_readiness_report.render_markdown(expected_model)

    visible_verdict = _visible_verdict(report)
    if visible_verdict is None:
        errors.append("missing final readiness verdict line in report preamble")
    elif visible_verdict != expected["verdict"]:
        errors.append(
            "final readiness verdict mismatch: "
            f"report says {visible_verdict}, source gate says {expected['verdict']}"
        )

    snapshot = _report_snapshot(report, errors)
    if snapshot is not None and snapshot != expected:
        _append_snapshot_errors(snapshot, expected, errors)

    _check_visible_sections(report, expected_report, errors)
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


def _check_visible_sections(report: str, expected_report: str, errors: list[str]) -> None:
    for section_name, heading in VISIBLE_SECTIONS:
        actual = _extract_visible_section(report, heading)
        expected = _extract_visible_section(expected_report, heading)
        if expected is None:
            errors.append(f"validator error: expected rendered section missing: {section_name}")
            continue
        if actual is None:
            errors.append(f"missing visible section: {section_name}")
            continue
        if _normalize_visible_section(actual) != _normalize_visible_section(expected):
            errors.append(f"stale or fabricated visible section: {section_name}")


def _extract_visible_section(report: str, heading: str) -> str | None:
    lines = report.splitlines()
    start: int | None = None
    for index, line in enumerate(lines):
        if line == heading:
            start = index
            break
    if start is None:
        return None

    end = len(lines)
    snapshot_prefix = f"<!-- {write_readiness_report.SNAPSHOT_PREFIX}"
    for index in range(start + 1, len(lines)):
        line = lines[index]
        if line.startswith("## ") or line.startswith(snapshot_prefix):
            end = index
            break
    return "\n".join(lines[start:end])


def _normalize_visible_section(section: str) -> str:
    normalized_lines = [line.rstrip() for line in section.splitlines()]
    while normalized_lines and normalized_lines[-1] == "":
        normalized_lines.pop()
    return "\n".join(normalized_lines)


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
