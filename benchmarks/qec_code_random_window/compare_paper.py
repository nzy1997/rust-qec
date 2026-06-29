from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from pathlib import Path
from typing import Any

from benchmarks.qec_code_random_window.validate_cases import load_manifest, validate_manifest


NA = "NA"

CSV_FIELDS = [
    "case_id",
    "code_id",
    "distance_side",
    "baseline_key",
    "baseline_required",
    "local_best_upper_bound",
    "local_median_elapsed_s",
    "paper_method",
    "paper_upper_bound",
    "paper_elapsed_s",
    "upper_bound_delta",
    "elapsed_time_ratio",
    "baseline_provenance",
    "baseline_source_file",
    "baseline_source_sheet",
    "baseline_source_row",
    "comparison_status",
]


class CompareError(ValueError):
    """User-facing validation error for comparison inputs."""


def _fail(location: str, message: str) -> CompareError:
    return CompareError(f"{location}: {message}")


def _is_blank(value: object) -> bool:
    return value is None or (isinstance(value, str) and value.strip() == "")


def _stringify(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value)


def _validated_cases(path: Path) -> list[dict[str, Any]]:
    try:
        manifest = load_manifest(path)
    except Exception as error:
        raise CompareError(f"{path}: {error}") from error

    errors = validate_manifest(manifest)
    if errors:
        raise CompareError("\n".join(f"{path}: {error}" for error in errors))

    cases = manifest.get("cases")
    assert isinstance(cases, list)
    return [case for case in cases if isinstance(case, dict)]


def _require_columns(
    path: Path,
    rows: list[dict[str, str]],
    required: list[str] | tuple[str, ...],
) -> None:
    missing = [column for column in required if any(column not in row for row in rows) or not rows]
    if missing:
        raise CompareError(f"{path}: missing required column(s): {', '.join(missing)}")


def _load_csv_rows(path: Path) -> list[dict[str, str]]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            return list(csv.DictReader(handle))
    except OSError as error:
        raise CompareError(f"{path}: {error}") from error


def _require_case_id(path: Path, row: dict[str, str], index: int) -> str:
    case_id = row.get("case_id", "")
    if not case_id.strip():
        raise _fail(f"{path}:{index}", 'field "case_id" must be a non-empty string')
    return case_id


def load_local_summaries(path: Path, known_case_ids: set[str]) -> dict[str, dict[str, str]]:
    rows = _load_csv_rows(path)
    _require_columns(path, rows, ("case_id", "best_upper_bound", "median_elapsed_s"))

    summaries: dict[str, dict[str, str]] = {}
    for index, row in enumerate(rows, start=2):
        case_id = _require_case_id(path, row, index)
        if case_id not in known_case_ids:
            raise _fail(f"{path}:{index}", f'unknown case_id "{case_id}"')
        summaries[case_id] = {
            "best_upper_bound": row.get("best_upper_bound", ""),
            "median_elapsed_s": row.get("median_elapsed_s", ""),
        }
    return summaries


def load_paper_baselines(path: Path, known_case_ids: set[str]) -> dict[str, dict[str, str]]:
    rows = _load_csv_rows(path)
    _require_columns(
        path,
        rows,
        (
            "case_id",
            "paper_case",
            "baseline_method",
            "baseline_upper_bound",
            "baseline_elapsed_s",
            "source_file",
            "source_sheet",
            "source_row",
        ),
    )

    baselines: dict[str, dict[str, str]] = {}
    for index, row in enumerate(rows, start=2):
        case_id = _require_case_id(path, row, index)
        if case_id not in known_case_ids:
            raise _fail(f"{path}:{index}", f'unknown case_id "{case_id}"')
        if case_id in baselines:
            continue
        baselines[case_id] = {
            "paper_case": row.get("paper_case", ""),
            "baseline_method": row.get("baseline_method", ""),
            "baseline_upper_bound": row.get("baseline_upper_bound", ""),
            "baseline_elapsed_s": row.get("baseline_elapsed_s", ""),
            "source_file": row.get("source_file", ""),
            "source_sheet": row.get("source_sheet", ""),
            "source_row": row.get("source_row", ""),
        }
    return baselines


def _parse_positive_or_none(value: object) -> float | None:
    if _is_blank(value) or value == NA:
        return None
    if isinstance(value, bool):
        raise CompareError(f"invalid numeric value: {value!r}")
    try:
        parsed = float(str(value).strip())
    except ValueError as error:
        raise CompareError(f"invalid numeric value: {value!r}") from error
    if not math.isfinite(parsed) or parsed < 0:
        raise CompareError(f"invalid numeric value: {value!r}")
    return parsed


def _parse_int_or_none(value: object) -> int | None:
    if _is_blank(value) or value == NA:
        return None
    if isinstance(value, bool):
        raise CompareError(f"invalid integer value: {value!r}")
    text = str(value).strip()
    try:
        return int(text)
    except ValueError as error:
        raise CompareError(f"invalid integer value: {value!r}") from error


def _format_delta(local: int | None, paper: int | None) -> str:
    if local is None or paper is None:
        return NA
    return str(local - paper)


def _format_ratio(local_elapsed: float | None, paper_elapsed: float | None) -> str:
    if local_elapsed is None or paper_elapsed is None:
        return NA
    if not math.isfinite(local_elapsed) or not math.isfinite(paper_elapsed):
        return NA
    if local_elapsed <= 0 or paper_elapsed <= 0:
        return NA
    return f"{local_elapsed / paper_elapsed:.6f}"


def _baseline_provenance(row: dict[str, str]) -> str:
    return f"{row['source_file']}#{row['source_sheet']}:{row['source_row']}"


def _case_lookup(cases: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    lookup: dict[str, dict[str, Any]] = {}
    for case in cases:
        lookup[case["case_id"]] = case
    return lookup


def _value_or_na(row: dict[str, str] | None, field: str) -> str:
    if row is None:
        return NA
    value = row.get(field, "")
    return value if not _is_blank(value) else NA


def compare_cases(
    cases: list[dict[str, Any]],
    local_summaries: dict[str, dict[str, str]],
    paper_baselines: dict[str, dict[str, str]],
) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for case in cases:
        case_id = case["case_id"]
        local = local_summaries.get(case_id)
        paper = paper_baselines.get(case_id)

        local_best = _parse_int_or_none(_value_or_na(local, "best_upper_bound"))
        local_elapsed = _parse_positive_or_none(_value_or_na(local, "median_elapsed_s"))
        paper_upper = _parse_int_or_none(_value_or_na(paper, "baseline_upper_bound"))
        paper_elapsed = _parse_positive_or_none(_value_or_na(paper, "baseline_elapsed_s"))

        baseline_required = case["baseline_required"]
        has_paper = paper is not None
        comparison_status = "paper_matched" if has_paper else "no_paper_baseline"
        provenance = NA
        source_file = NA
        source_sheet = NA
        source_row = NA
        paper_method = NA
        paper_upper_text = NA
        paper_elapsed_text = NA
        if paper is not None:
            provenance = _baseline_provenance(paper)
            source_file = _value_or_na(paper, "source_file")
            source_sheet = _value_or_na(paper, "source_sheet")
            source_row = _value_or_na(paper, "source_row")
            paper_method = _value_or_na(paper, "baseline_method")
            paper_upper_text = _value_or_na(paper, "baseline_upper_bound")
            paper_elapsed_text = _value_or_na(paper, "baseline_elapsed_s")

        rows.append(
            {
                "case_id": case_id,
                "code_id": str(case["code_id"]),
                "distance_side": str(case["distance_side"]),
                "baseline_key": str(case["baseline_key"]),
                "baseline_required": _stringify(baseline_required),
                "local_best_upper_bound": str(local_best) if local_best is not None else NA,
                "local_median_elapsed_s": (
                    _stringify(local_elapsed) if local_elapsed is not None else NA
                ),
                "paper_method": paper_method,
                "paper_upper_bound": paper_upper_text,
                "paper_elapsed_s": paper_elapsed_text,
                "upper_bound_delta": _format_delta(local_best, paper_upper),
                "elapsed_time_ratio": _format_ratio(local_elapsed, paper_elapsed),
                "baseline_provenance": provenance,
                "baseline_source_file": source_file,
                "baseline_source_sheet": source_sheet,
                "baseline_source_row": source_row,
                "comparison_status": comparison_status,
            }
        )
    return rows


def write_comparison_csv(path: Path, rows: list[dict[str, str]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_FIELDS)
        writer.writeheader()
        for row in rows:
            writer.writerow({field: row.get(field, NA) for field in CSV_FIELDS})


def write_comparison_md(
    path: Path,
    *,
    manifest_path: Path,
    local_summary_path: Path,
    paper_baselines_path: Path,
    argv: list[str],
    manifest: dict[str, Any],
    rows: list[dict[str, str]],
) -> None:
    lines = [
        "# QEC Code Random-Window Comparison",
        "",
        "## Provenance",
        f"- Manifest: `{manifest_path}`",
        f"- Local summary: `{local_summary_path}`",
        f"- Paper baselines: `{paper_baselines_path}`",
        f"- Manifest suite/version: `{manifest.get('suite')}` / `{manifest.get('manifest_version')}`",
        f"- Comparison argv: `{json.dumps(argv)}`",
        "",
        "## Comparison Table",
        "",
        "| case_id | local_best_upper_bound | paper_method | paper_upper_bound | upper_bound_delta | elapsed_time_ratio | baseline_provenance | source_file | source_sheet | source_row | status |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]

    for row in rows:
        lines.append(
            (
                f"| {row['case_id']} | {row['local_best_upper_bound']} | {row['paper_method']} | "
                f"{row['paper_upper_bound']} | {row['upper_bound_delta']} | "
                f"{row['elapsed_time_ratio']} | {row['baseline_provenance']} | "
                f"{row['baseline_source_file']} | {row['baseline_source_sheet']} | "
                f"{row['baseline_source_row']} | {row['comparison_status']} |"
            )
        )

    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run(args: argparse.Namespace, argv: list[str] | None = None) -> int:
    try:
        manifest = load_manifest(args.cases)
        errors = validate_manifest(manifest)
        if errors:
            raise CompareError("\n".join(f"{args.cases}: {error}" for error in errors))
    except CompareError as error:
        print(error, file=sys.stderr)
        return 2
    except Exception as error:
        print(f"{args.cases}: {error}", file=sys.stderr)
        return 2

    cases = manifest["cases"]
    assert isinstance(cases, list)
    case_ids = {case["case_id"] for case in cases if isinstance(case, dict)}

    try:
        local_summaries = load_local_summaries(args.local_summary, case_ids)
        paper_baselines = load_paper_baselines(args.paper_baselines, case_ids)
        rows = compare_cases(cases, local_summaries, paper_baselines)
    except CompareError as error:
        print(error, file=sys.stderr)
        return 1

    args.out_dir.mkdir(parents=True, exist_ok=True)
    try:
        write_comparison_csv(args.out_dir / "comparison.csv", rows)
        write_comparison_md(
            args.out_dir / "comparison.md",
            manifest_path=args.cases,
            local_summary_path=args.local_summary,
            paper_baselines_path=args.paper_baselines,
            argv=sys.argv[1:] if argv is None else argv,
            manifest=manifest,
            rows=rows,
        )
    except OSError as error:
        print(f"{args.out_dir}: {error}", file=sys.stderr)
        return 1

    if args.strict_baselines:
        missing_required = [
            case["case_id"]
            for case in cases
            if case.get("baseline_required") is True and case["case_id"] not in paper_baselines
        ]
        if missing_required:
            print(
                "missing required paper baseline rows: " + ", ".join(missing_required),
                file=sys.stderr,
            )
            return 1

    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Compare qec-code random-window local summaries against paper baselines."
    )
    parser.add_argument("--cases", type=Path, required=True)
    parser.add_argument("--local-summary", type=Path, required=True)
    parser.add_argument("--paper-baselines", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--strict-baselines", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return run(args, argv)


if __name__ == "__main__":
    raise SystemExit(main())
