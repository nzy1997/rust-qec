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

FINAL_VERDICT_PATTERN = re.compile(
    r"^\*\*Final readiness verdict:\*\*\s+(PASS|WARN|FAIL)\s*$"
)
GENERATED_AT_PATTERN = re.compile(r"^\*\*Generated at:\*\*\s+.*$")


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

    visible_verdict = _visible_verdict(report, errors)
    if visible_verdict is not None and visible_verdict != expected["verdict"]:
        errors.append(
            "final readiness verdict mismatch: "
            f"report says {visible_verdict}, source gate says {expected['verdict']}"
        )

    snapshot = _report_snapshot(report, errors)
    if snapshot is not None and snapshot != expected:
        _append_snapshot_errors(snapshot, expected, errors)

    _check_visible_report_body(report, expected_report, errors)
    return errors


def _visible_verdict(report: str, errors: list[str]) -> str | None:
    lines = report.splitlines()
    matches = [
        (index, match.group(1))
        for index, line in enumerate(lines)
        if (match := FINAL_VERDICT_PATTERN.fullmatch(line)) is not None
    ]
    if not matches:
        errors.append("missing final readiness verdict line in report preamble")
        return None
    if len(matches) > 1:
        errors.append(
            "duplicate final readiness verdict lines: "
            "final readiness verdict must appear exactly once in report preamble"
        )
        return None

    first_section_index = _first_visible_section_index(lines)
    verdict_index, verdict = matches[0]
    if first_section_index is not None and verdict_index >= first_section_index:
        errors.append("missing final readiness verdict line in report preamble")
        return None
    return verdict


def _report_snapshot(report: str, errors: list[str]) -> dict[str, object] | None:
    snapshot_matches = [
        match
        for line in report.splitlines()
        if (match := _snapshot_line_match(line)) is not None
    ]
    if not snapshot_matches:
        errors.append("missing snapshot: rstim-bb-readiness-snapshot comment not found")
        return None
    if len(snapshot_matches) > 1:
        errors.append(
            "malformed snapshot: rstim-bb-readiness-snapshot comment must appear exactly once"
        )
        return None
    try:
        snapshot = json.loads(snapshot_matches[0].group(1))
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

    handled_keys = {"verdict", "gate_checks", "artifact_hashes", "sections"}
    for key in sorted(set(snapshot) | set(expected)):
        if key in handled_keys:
            continue
        if snapshot.get(key) != expected.get(key):
            errors.append(f"stale or missing snapshot metadata: {key}")

    if not flagged and snapshot != expected and not any(
        "snapshot metadata" in error for error in errors
    ):
        errors.append(
            "stale or missing snapshot metadata: snapshot payload differs from source artifacts"
        )


def _check_visible_report_body(report: str, expected_report: str, errors: list[str]) -> None:
    if _normalize_visible_body(report) == _normalize_visible_body(expected_report):
        return

    structure_errors = _document_structure_errors(report)
    errors.extend(structure_errors)
    if structure_errors:
        return

    body_errors_start = len(errors)
    actual_sections = _extract_visible_sections(report, errors)
    expected_sections = _extract_visible_sections(expected_report, [])
    if errors[body_errors_start:]:
        return

    if _extract_visible_preamble(report) != _extract_visible_preamble(expected_report):
        errors.append(
            "stale or fabricated report-body: report preamble does not match source artifacts"
        )

    for section_name, heading in VISIBLE_SECTIONS:
        actual = actual_sections.get(section_name)
        expected = expected_sections.get(section_name)
        if expected is None:
            errors.append(f"validator error: expected rendered section missing: {section_name}")
            continue
        if actual is None:
            continue
        if _normalize_visible_section(actual) != _normalize_visible_section(expected):
            errors.append(f"stale or fabricated visible section: {section_name}")

    if errors[body_errors_start:]:
        return

    errors.append(
        "stale or fabricated report-body: visible report body does not match source artifacts"
    )


def _document_structure_errors(report: str) -> list[str]:
    errors: list[str] = []
    lines = report.splitlines()

    title_indexes = [index for index, line in enumerate(lines) if line == REQUIRED_HEADINGS[0]]
    if title_indexes:
        title_index = title_indexes[0]
        if any(line.strip() for line in lines[:title_index]):
            errors.append(
                "document-structure mismatch: unexpected visible content before report title"
            )

    malformed_snapshot_indexes = [
        index
        for index, line in enumerate(lines)
        if write_readiness_report.SNAPSHOT_PREFIX in line
        and _snapshot_line_match(line) is None
    ]
    if malformed_snapshot_indexes:
        errors.append(
            "document-structure mismatch: snapshot metadata comment must occupy its own line"
        )

    snapshot_index = _snapshot_start_index(lines)
    if snapshot_index < len(lines) and any(
        line.strip() for line in lines[snapshot_index + 1 :]
    ):
        errors.append(
            "report-body mismatch: unexpected visible content after snapshot metadata"
        )

    return errors


def _extract_visible_sections(report: str, errors: list[str]) -> dict[str, str]:
    lines = report.splitlines()
    heading_positions: dict[str, int] = {}
    for section_name, heading in VISIBLE_SECTIONS:
        matches = [index for index, line in enumerate(lines) if line == heading]
        if not matches:
            errors.append(f"missing visible section: {section_name}")
            continue
        if len(matches) > 1:
            errors.append(f"duplicate visible section heading: {section_name}")
            continue
        heading_positions[section_name] = matches[0]

    previous_position = -1
    previous_section: str | None = None
    for section_name, _heading in VISIBLE_SECTIONS:
        position = heading_positions.get(section_name)
        if position is None:
            continue
        if position <= previous_position:
            errors.append(
                "out-of-order visible section: "
                f"{section_name} appears before {previous_section}"
            )
        previous_position = position
        previous_section = section_name

    snapshot_start = _snapshot_start_index(lines)
    extracted: dict[str, str] = {}
    for index, (section_name, _heading) in enumerate(VISIBLE_SECTIONS):
        start = heading_positions.get(section_name)
        if start is None:
            continue

        end = snapshot_start
        for later_section, _later_heading in VISIBLE_SECTIONS[index + 1 :]:
            later_start = heading_positions.get(later_section)
            if later_start is not None:
                end = later_start
                break
        extracted[section_name] = "\n".join(lines[start:end])
    return extracted


def _first_visible_section_index(lines: list[str]) -> int | None:
    for index, line in enumerate(lines):
        if any(line == heading for _section_name, heading in VISIBLE_SECTIONS):
            return index
    return None


def _snapshot_start_index(lines: list[str]) -> int:
    for index, line in enumerate(lines):
        if _snapshot_line_match(line) is not None:
            return index
    return len(lines)


def _extract_visible_preamble(report: str) -> str:
    lines = report.splitlines()
    first_section_index = _first_visible_section_index(lines)
    if first_section_index is None:
        return _normalize_visible_lines(lines)
    return _normalize_visible_lines(lines[:first_section_index])


def _normalize_visible_body(report: str) -> str:
    return _normalize_visible_lines(report.splitlines())


def _normalize_visible_lines(lines: list[str]) -> str:
    normalized_lines: list[str] = []
    for line in lines:
        if _snapshot_line_match(line) is not None:
            continue
        if GENERATED_AT_PATTERN.fullmatch(line):
            normalized_lines.append("**Generated at:** <volatile>")
            continue
        normalized_lines.append(line.rstrip())
    while normalized_lines and normalized_lines[-1] == "":
        normalized_lines.pop()
    return "\n".join(normalized_lines)


def _normalize_visible_section(section: str) -> str:
    return _normalize_visible_lines(section.splitlines())


def _snapshot_line_match(line: str) -> re.Match[str] | None:
    prefix = re.escape(write_readiness_report.SNAPSHOT_PREFIX)
    return re.fullmatch(rf"<!--\s*{prefix}\s*(\{{.*\}})\s*-->", line)


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
