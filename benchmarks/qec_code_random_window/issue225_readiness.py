from __future__ import annotations

import argparse
import csv
import json
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Sequence


REQUIRED_ISSUES = [337, 338, 339, 343, 344, 345, 346, 351, 352, 353]
EXPECTED_LADDER = {
    "surface_rotated_d5": 5,
    "toric_d5": 5,
    "bb72": 6,
    "bb144": 12,
}
EXPECTED_MULTISEED_CASES = {
    "bb72_no_target_smoke": {7, 11, 17},
    "bb144_no_target_smoke": {7, 11, 17},
}
REQUIRED_COUNTERS = [
    "permutations_sampled",
    "kernel_basis_generations",
    "component_candidates_generated",
    "zero_candidates_rejected",
    "weight_pruned_candidates",
    "stabilizer_span_candidates_rejected",
    "witness_validation_candidates_rejected",
    "valid_witnesses_found",
    "best_witness_updates",
]
REQUIRED_TIMINGS = [
    "permutation_time_ns",
    "kernel_basis_time_ns",
    "span_filter_time_ns",
    "witness_validation_time_ns",
    "best_update_time_ns",
    "total_search_time_ns",
]


def _md_cell(value: object) -> str:
    return str(value).replace("|", r"\|").replace("\n", " ")


class Issue225ReadinessError(ValueError):
    """Validation error for issue-225 readiness inputs."""


@dataclass(frozen=True)
class EvidenceEntry:
    milestone: str
    issue: int
    issue_url: str
    title: str
    pr: int
    pr_url: str
    merged_at: str
    evidence: str


@dataclass(frozen=True)
class LadderRow:
    case_id: str
    code_id: str
    best_upper_bound: int
    build_profile: str
    target_weight: None
    target_reached: bool


@dataclass(frozen=True)
class MultiseedRow:
    case_id: str
    code_id: str
    seeds: tuple[int, ...]
    best_upper_bound: int
    build_profile: str
    target_weight: None
    target_reached: bool


@dataclass(frozen=True)
class SearchMetricRow:
    case_id: str
    values: dict[str, int | bool]


@dataclass(frozen=True)
class ReadinessReport:
    decision: str
    issue_225: dict[str, object]
    evidence_chain: tuple[EvidenceEntry, ...]
    ladder_rows: tuple[LadderRow, ...]
    multiseed_rows: tuple[MultiseedRow, ...]
    counter_rows: tuple[SearchMetricRow, ...]
    timing_rows: tuple[SearchMetricRow, ...]

    def to_markdown(self) -> str:
        lines = [
            "# Issue 225 Readiness Report",
            "",
            f"issue_225_readiness: {self.decision}",
            "",
            (
                f"Issue [#{self.issue_225['issue']}]({self.issue_225['url']}): "
                f"{self.issue_225['summary']}"
            ),
            "",
            "## Evidence Chain",
            "",
        ]
        grouped: dict[str, list[EvidenceEntry]] = defaultdict(list)
        for entry in self.evidence_chain:
            grouped[entry.milestone].append(entry)
        for milestone in sorted(grouped):
            lines.extend(
                [
                    f"### {milestone}",
                    "",
                    "| issue | pr | merged_at | title | evidence |",
                    "| --- | --- | --- | --- | --- |",
                ]
            )
            for entry in grouped[milestone]:
                lines.append(
                    f"| [#{entry.issue}]({entry.issue_url}) | "
                    f"[#{entry.pr}]({entry.pr_url}) | {entry.merged_at} | "
                    f"{_md_cell(entry.title)} | {_md_cell(entry.evidence)} |"
                )
            lines.append("")

        lines.extend(
            [
                "## No-Target Ladder",
                "",
                "| case_id | code_id | best_upper_bound | semantics |",
                "| --- | --- | --- | --- |",
            ]
        )
        for row in self.ladder_rows:
            lines.append(
                "| {case_id} | {code_id} | {best_upper_bound} | target_weight = null; "
                "target_reached = false; build_profile = release |".format(
                    case_id=_md_cell(row.case_id),
                    code_id=_md_cell(row.code_id),
                    best_upper_bound=row.best_upper_bound,
                )
            )
        lines.extend(
            [
                "",
                "## Multi-Seed Stability",
                "",
                "| case_id | code_id | observed_seeds | best_upper_bound | semantics |",
                "| --- | --- | --- | --- | --- |",
            ]
        )
        for row in self.multiseed_rows:
            seed_text = ";".join(str(seed) for seed in row.seeds)
            lines.append(
                "| {case_id} | {code_id} | {seed_text} | {best_upper_bound} | target_weight = null; "
                "target_reached = false; build_profile = release |".format(
                    case_id=_md_cell(row.case_id),
                    code_id=_md_cell(row.code_id),
                    seed_text=seed_text,
                    best_upper_bound=row.best_upper_bound,
                )
            )

        lines.extend(
            [
                "",
                "## Search Counters",
                "",
                "| case_id | " + " | ".join(REQUIRED_COUNTERS) + " | target_reached |",
                "| " + " | ".join(["---"] * (len(REQUIRED_COUNTERS) + 2)) + " |",
            ]
        )
        for row in self.counter_rows:
            values = [str(row.values[field]) for field in REQUIRED_COUNTERS]
            target_reached = "true" if row.values["target_reached"] else "false"
            lines.append(f"| {row.case_id} | " + " | ".join(values) + f" | {target_reached} |")

        lines.extend(
            [
                "",
                "## Timing Buckets",
                "",
                "| case_id | " + " | ".join(REQUIRED_TIMINGS) + " |",
                "| " + " | ".join(["---"] * (len(REQUIRED_TIMINGS) + 1)) + " |",
            ]
        )
        for row in self.timing_rows:
            values = [str(row.values[field]) for field in REQUIRED_TIMINGS]
            lines.append(f"| {row.case_id} | " + " | ".join(values) + " |")
        lines.append("")
        return "\n".join(lines)

    def write_outputs(self, out_dir: Path) -> None:
        out_dir.mkdir(parents=True, exist_ok=True)
        report_path = out_dir / "report.md"
        summary_path = out_dir / "summary.txt"
        report_path.write_text(self.to_markdown(), encoding="utf-8")
        summary_path.write_text(
            "issue_225_readiness: PASS\n"
            f"ladder_cases: {len(self.ladder_rows)}\n"
            f"multiseed_cases: {len(self.multiseed_rows)}\n",
            encoding="utf-8",
        )


def _load_json(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise Issue225ReadinessError(f"{path}: {error}") from error


def _load_jsonl(path: Path) -> list[dict[str, object]]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise Issue225ReadinessError(f"{path}: {error}") from error
    rows: list[dict[str, object]] = []
    for line_number, line in enumerate(text.splitlines(), start=1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            raise Issue225ReadinessError(f"{path}:{line_number}: {error.msg}") from error
        if not isinstance(value, dict):
            raise Issue225ReadinessError(f"{path}:{line_number}: row must be a JSON object")
        rows.append(value)
    return rows


def _load_csv(path: Path) -> list[dict[str, str]]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            return list(csv.DictReader(handle))
    except (OSError, UnicodeDecodeError, csv.Error) as error:
        raise Issue225ReadinessError(f"{path}: {error}") from error


def _require_nonempty_str(value: object) -> str | None:
    if isinstance(value, str) and value:
        return value
    return None


def _parse_int(value: object) -> int | None:
    if type(value) is int:
        return value
    if isinstance(value, str) and value:
        try:
            return int(value)
        except ValueError:
            return None
    return None


def _validate_command(command: object, case_id: str, errors: list[str]) -> None:
    if not isinstance(command, list) or not command:
        errors.append(f"{case_id}: command must be a non-empty list")
        return
    if not all(isinstance(item, str) for item in command):
        errors.append(f"{case_id}: command must contain only strings")
        return
    if any(item == "--target-weight" or item.startswith("--target-weight=") for item in command):
        errors.append(f"{case_id}: command must omit --target-weight")


def _validate_required_run_rows(
    *,
    rows_by_case: dict[str, list[dict[str, object]]],
    required_cases: dict[str, int],
    errors: list[str],
) -> None:
    for case_id, expected_best_upper_bound in required_cases.items():
        rows = rows_by_case.get(case_id, [])
        if not rows:
            errors.append(f'{case_id}: missing required run row')
            continue
        for row in rows:
            if row.get("status") != "ok":
                errors.append(f'{case_id}: status must be "ok"')
            if row.get("build_profile") != "release":
                errors.append(f'{case_id}: build_profile must be release')
            if "target_weight" not in row:
                errors.append(f"{case_id}: target_weight must be present and null")
            elif row.get("target_weight") is not None:
                errors.append(f'{case_id}: target_weight must be null')
            _validate_command(row.get("command"), case_id, errors)
            if _parse_int(row.get("upper_bound")) != expected_best_upper_bound:
                errors.append(
                    f"{case_id}: best_upper_bound run value must equal {expected_best_upper_bound}"
                )
            raw_cli_json = row.get("raw_cli_json")
            if not isinstance(raw_cli_json, dict):
                errors.append(f'{case_id}: raw_cli_json must be an object')
                continue
            search_stats = raw_cli_json.get("search_stats")
            if not isinstance(search_stats, dict):
                errors.append(f'{case_id}: raw_cli_json.search_stats must be an object')
                continue
            if search_stats.get("target_reached") is not False:
                errors.append(f'{case_id}: search_stats.target_reached must be false')
            for field in REQUIRED_COUNTERS:
                value = search_stats.get(field)
                if type(value) is not int or value < 0:
                    errors.append(f"{case_id}: search_stats.{field} must be a non-negative integer")
            for field in REQUIRED_TIMINGS:
                value = search_stats.get(field)
                if type(value) is not int or value < 0:
                    errors.append(f"{case_id}: search_stats.{field} must be a non-negative integer")


def _validate_ladder_summary(
    rows: Sequence[dict[str, str]],
    errors: list[str],
) -> list[LadderRow]:
    rows_by_case = {row.get("case_id", ""): row for row in rows}
    ladder_rows: list[LadderRow] = []
    for case_id, expected_best_upper_bound in EXPECTED_LADDER.items():
        row = rows_by_case.get(case_id)
        if row is None:
            errors.append(f"{case_id}: missing required ladder summary row")
            continue
        _validate_summary_semantics(case_id, row, errors)
        best_upper_bound = _parse_int(row.get("best_upper_bound"))
        if best_upper_bound != expected_best_upper_bound:
            errors.append(
                f"{case_id}: best_upper_bound must equal {expected_best_upper_bound}"
            )
            continue
        ladder_rows.append(
            LadderRow(
                case_id=case_id,
                code_id=row.get("code_id", ""),
                best_upper_bound=best_upper_bound,
                build_profile="release",
                target_weight=None,
                target_reached=False,
            )
        )
    return sorted(ladder_rows, key=lambda row: list(EXPECTED_LADDER).index(row.case_id))


def _parse_seed_values(value: str | None) -> set[int] | None:
    if value is None:
        return None
    text = value.strip()
    if not text:
        return set()
    try:
        return {int(part) for part in text.split(";")}
    except ValueError:
        return None


def _validate_summary_semantics(
    case_id: str,
    row: dict[str, str],
    errors: list[str],
) -> None:
    if row.get("summary_status") != "ok":
        errors.append(f'{case_id}: summary_status must be "ok"')
    if row.get("run_target_weight_values", "") != "":
        errors.append(f"{case_id}: run_target_weight_values must be empty for no-target runs")
    if row.get("run_build_profile_values") != "release":
        errors.append(f"{case_id}: run_build_profile_values must be release")
    target_reached_count = _parse_int(row.get("search_stats_target_reached_count"))
    if target_reached_count != 0:
        errors.append(f"{case_id}: search_stats_target_reached_count must be 0")


def _validate_multiseed_summary(
    rows: Sequence[dict[str, str]],
    errors: list[str],
) -> list[MultiseedRow]:
    rows_by_case = {row.get("case_id", ""): row for row in rows}
    multiseed_rows: list[MultiseedRow] = []
    for case_id, expected_seeds in EXPECTED_MULTISEED_CASES.items():
        row = rows_by_case.get(case_id)
        if row is None:
            errors.append(f"{case_id}: missing required multiseed summary row")
            continue
        _validate_summary_semantics(case_id, row, errors)
        actual_seeds = _parse_seed_values(row.get("run_seed_values"))
        if actual_seeds != expected_seeds:
            errors.append(
                f"{case_id}: run_seed_values must equal {';'.join(str(seed) for seed in sorted(expected_seeds))}"
            )
        best_upper_bound = _parse_int(row.get("best_upper_bound"))
        expected_best_upper_bound = EXPECTED_LADDER["bb72" if "bb72" in case_id else "bb144"]
        if best_upper_bound != expected_best_upper_bound:
            errors.append(
                f"{case_id}: best_upper_bound must equal {expected_best_upper_bound}"
            )
            continue
        multiseed_rows.append(
            MultiseedRow(
                case_id=case_id,
                code_id=row.get("code_id", ""),
                seeds=tuple(sorted(expected_seeds)),
                best_upper_bound=best_upper_bound,
                build_profile="release",
                target_weight=None,
                target_reached=False,
            )
        )
    return sorted(multiseed_rows, key=lambda row: row.case_id)


def _validate_evidence(payload: object, errors: list[str]) -> tuple[dict[str, object], list[EvidenceEntry]]:
    if not isinstance(payload, dict):
        errors.append("evidence: root must be a JSON object")
        return {}, []
    issue_225 = payload.get("issue_225")
    if not isinstance(issue_225, dict):
        errors.append("evidence: issue_225 must be an object")
        issue_225 = {}
    if _parse_int(issue_225.get("issue")) != 225:
        errors.append("evidence: issue_225.issue must equal 225")
    if _require_nonempty_str(issue_225.get("url")) is None:
        errors.append("evidence: issue_225.url must be non-empty")
    if _require_nonempty_str(issue_225.get("summary")) is None:
        errors.append("evidence: issue_225.summary must be non-empty")

    chain = payload.get("chain")
    if not isinstance(chain, list):
        errors.append("evidence: chain must be a list")
        return dict(issue_225), []

    seen_issues: dict[int, int] = defaultdict(int)
    entries: list[EvidenceEntry] = []
    for index, item in enumerate(chain):
        location = f"evidence.chain[{index}]"
        if not isinstance(item, dict):
            errors.append(f"{location}: entry must be an object")
            continue
        issue = _parse_int(item.get("issue"))
        pr = _parse_int(item.get("pr"))
        milestone = _require_nonempty_str(item.get("milestone"))
        issue_url = _require_nonempty_str(item.get("issue_url"))
        title = _require_nonempty_str(item.get("title"))
        pr_url = _require_nonempty_str(item.get("pr_url"))
        merged_at = _require_nonempty_str(item.get("merged_at"))
        evidence = _require_nonempty_str(item.get("evidence"))
        if issue is None:
            errors.append(f"{location}: issue must be a positive integer")
            continue
        seen_issues[issue] += 1
        if pr is None or pr <= 0:
            errors.append(f"{location}: pr must be a positive integer")
        if milestone is None:
            errors.append(f"{location}: milestone must be non-empty")
        if issue_url is None:
            errors.append(f"{location}: issue_url must be non-empty")
        if pr_url is None:
            errors.append(f"{location}: pr_url must be non-empty")
        if title is None:
            errors.append(f"{location}: title must be non-empty")
        if merged_at is None:
            errors.append(f"{location}: merged_at must be non-empty")
        if evidence is None:
            errors.append(f"{location}: evidence must be non-empty")
        if (
            pr is not None
            and milestone is not None
            and issue_url is not None
            and title is not None
            and pr_url is not None
            and merged_at is not None
            and evidence is not None
        ):
            entries.append(
                EvidenceEntry(
                    milestone=milestone,
                    issue=issue,
                    issue_url=issue_url,
                    title=title,
                    pr=pr,
                    pr_url=pr_url,
                    merged_at=merged_at,
                    evidence=evidence,
                )
            )
    for issue in REQUIRED_ISSUES:
        count = seen_issues.get(issue, 0)
        if count == 0:
            errors.append(f"evidence: missing issue {issue}")
        elif count != 1:
            errors.append(f"evidence: issue {issue} must appear exactly once")
    return dict(issue_225), sorted(entries, key=lambda entry: REQUIRED_ISSUES.index(entry.issue))


def _build_metric_rows(
    rows_by_case: dict[str, list[dict[str, object]]],
    field_names: Sequence[str],
) -> list[SearchMetricRow]:
    metric_rows: list[SearchMetricRow] = []
    for case_id in EXPECTED_LADDER:
        row = rows_by_case[case_id][0]
        search_stats = row["raw_cli_json"]["search_stats"]  # type: ignore[index]
        values = {field: search_stats[field] for field in field_names}
        if "target_reached" in search_stats:
            values["target_reached"] = search_stats["target_reached"]
        metric_rows.append(SearchMetricRow(case_id=case_id, values=values))
    return metric_rows


def evaluate_readiness(
    evidence_path: Path,
    ladder_runs_path: Path,
    ladder_summary_path: Path,
    multiseed_runs_path: Path,
    multiseed_summary_path: Path,
) -> ReadinessReport:
    errors: list[str] = []
    issue_225, evidence_entries = _validate_evidence(_load_json(evidence_path), errors)

    ladder_run_rows = _load_jsonl(ladder_runs_path)
    ladder_runs_by_case: dict[str, list[dict[str, object]]] = defaultdict(list)
    for row in ladder_run_rows:
        case_id = row.get("case_id")
        if isinstance(case_id, str):
            ladder_runs_by_case[case_id].append(row)
    _validate_required_run_rows(
        rows_by_case=ladder_runs_by_case,
        required_cases=EXPECTED_LADDER,
        errors=errors,
    )

    multiseed_run_rows = _load_jsonl(multiseed_runs_path)
    multiseed_runs_by_case: dict[str, list[dict[str, object]]] = defaultdict(list)
    for row in multiseed_run_rows:
        case_id = row.get("case_id")
        if isinstance(case_id, str):
            multiseed_runs_by_case[case_id].append(row)
    for case_id, expected_seeds in EXPECTED_MULTISEED_CASES.items():
        rows = multiseed_runs_by_case.get(case_id, [])
        if not rows:
            errors.append(f"{case_id}: missing required run row")
            continue
        actual_seeds = {
            seed for row in rows if (seed := _parse_int(row.get("seed"))) is not None
        }
        if actual_seeds != expected_seeds:
            errors.append(
                f"{case_id}: observed seeds must equal {';'.join(str(seed) for seed in sorted(expected_seeds))}"
            )
    _validate_required_run_rows(
        rows_by_case=multiseed_runs_by_case,
        required_cases={
            "bb72_no_target_smoke": EXPECTED_LADDER["bb72"],
            "bb144_no_target_smoke": EXPECTED_LADDER["bb144"],
        },
        errors=errors,
    )

    ladder_rows = _validate_ladder_summary(_load_csv(ladder_summary_path), errors)
    multiseed_rows = _validate_multiseed_summary(_load_csv(multiseed_summary_path), errors)

    if errors:
        raise Issue225ReadinessError("\n".join(errors))

    return ReadinessReport(
        decision="PASS",
        issue_225=issue_225,
        evidence_chain=tuple(evidence_entries),
        ladder_rows=tuple(ladder_rows),
        multiseed_rows=tuple(multiseed_rows),
        counter_rows=tuple(_build_metric_rows(ladder_runs_by_case, [*REQUIRED_COUNTERS, "target_reached"])),
        timing_rows=tuple(_build_metric_rows(ladder_runs_by_case, REQUIRED_TIMINGS)),
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate issue-225 readiness evidence.")
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--ladder-runs", type=Path, required=True)
    parser.add_argument("--ladder-summary", type=Path, required=True)
    parser.add_argument("--multiseed-runs", type=Path, required=True)
    parser.add_argument("--multiseed-summary", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        report = evaluate_readiness(
            evidence_path=args.evidence,
            ladder_runs_path=args.ladder_runs,
            ladder_summary_path=args.ladder_summary,
            multiseed_runs_path=args.multiseed_runs,
            multiseed_summary_path=args.multiseed_summary,
        )
    except Issue225ReadinessError as error:
        print(str(error), file=sys.stderr)
        return 1
    report.write_outputs(args.out_dir)
    print("issue_225_readiness: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
