from __future__ import annotations

import argparse
import csv
import json
import math
import statistics
import sys
from pathlib import Path
from typing import Any

from benchmarks.qec_code_random_window.validate_cases import load_manifest, validate_manifest


CSV_FIELDS = [
    "case_id",
    "code_id",
    "distance_side",
    "baseline_key",
    "baseline_required",
    "manifest_seed",
    "manifest_iterations",
    "manifest_restarts",
    "manifest_target_weight",
    "target_upper_bound",
    "attempted_seed_rows",
    "successful_seed_rows",
    "best_upper_bound",
    "median_elapsed_s",
    "min_elapsed_s",
    "max_elapsed_s",
    "target_hit_count",
    "target_hit_rate",
    "run_seed_values",
    "run_iterations_values",
    "run_restarts_values",
    "run_target_weight_values",
    "run_status_values",
    "summary_status",
]


class SummaryError(ValueError):
    """User-facing validation error for summary inputs."""


def _is_int(value: object) -> bool:
    return type(value) is int


def _fail(location: str, message: str) -> SummaryError:
    return SummaryError(f"{location}: {message}")


def _require_int(
    row: dict[str, Any],
    field: str,
    location: str,
    *,
    positive: bool = False,
    nonnegative: bool = False,
) -> int:
    value = row.get(field)
    if not _is_int(value):
        raise _fail(location, f'field "{field}" must be an integer')
    if positive and value <= 0:
        raise _fail(location, f'field "{field}" must be a positive integer')
    if nonnegative and value < 0:
        raise _fail(location, f'field "{field}" must be a non-negative integer')
    return value


def _require_optional_int(
    row: dict[str, Any],
    field: str,
    location: str,
    *,
    positive: bool = False,
    nonnegative: bool = False,
) -> int | None:
    value = row.get(field)
    if value is None:
        return None
    if not _is_int(value):
        raise _fail(location, f'field "{field}" must be an integer or null')
    if positive and value <= 0:
        raise _fail(location, f'field "{field}" must be a positive integer or null')
    if nonnegative and value < 0:
        raise _fail(location, f'field "{field}" must be a non-negative integer or null')
    return value


def _require_status(row: dict[str, Any], location: str) -> str:
    value = row.get("status")
    if not isinstance(value, str) or not value:
        raise _fail(location, 'field "status" must be a non-empty string')
    return value


def _require_case_id(row: dict[str, Any], location: str) -> str:
    value = row.get("case_id")
    if not isinstance(value, str) or not value:
        raise _fail(location, 'field "case_id" must be a non-empty string')
    return value


def _require_elapsed(row: dict[str, Any], location: str) -> float:
    value = row.get("elapsed_s")
    if type(value) not in {int, float}:
        raise _fail(location, 'field "elapsed_s" must be numeric')
    parsed = float(value)
    if not math.isfinite(parsed):
        raise _fail(location, 'field "elapsed_s" must be finite')
    if parsed < 0:
        raise _fail(location, 'field "elapsed_s" must be non-negative')
    return parsed


def _validate_row(
    row: dict[str, Any],
    *,
    location: str,
    known_case_ids: set[str],
) -> dict[str, Any]:
    case_id = _require_case_id(row, location)
    if case_id not in known_case_ids:
        raise _fail(location, f'unknown case_id "{case_id}"')

    status = _require_status(row, location)
    seed = _require_int(row, "seed", location, nonnegative=True)
    iterations = _require_int(row, "iterations", location, positive=True)
    restarts = _require_int(row, "restarts", location, positive=True)
    target_weight = _require_optional_int(row, "target_weight", location, positive=True)
    elapsed_s = _require_elapsed(row, location)

    command = row.get("command")
    if command is not None:
        if not isinstance(command, list) or not all(isinstance(item, str) for item in command):
            raise _fail(location, 'field "command" must be a list of strings')

    upper_bound = row.get("upper_bound")
    if status == "ok":
        if "upper_bound" not in row:
            raise _fail(
                location,
                'status = "ok" rows must include positive integer field "upper_bound"',
            )
        if not _is_int(upper_bound) or upper_bound <= 0:
            raise _fail(
                location,
                'status = "ok" rows must include positive integer field "upper_bound"',
            )
    elif upper_bound is not None and not _is_int(upper_bound):
        raise _fail(location, 'field "upper_bound" must be an integer or null')

    validated = dict(row)
    validated["case_id"] = case_id
    validated["status"] = status
    validated["seed"] = seed
    validated["iterations"] = iterations
    validated["restarts"] = restarts
    validated["target_weight"] = target_weight
    validated["elapsed_s"] = elapsed_s
    return validated


def load_run_rows(paths: list[Path], known_case_ids: set[str]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for path in paths:
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            raise _fail(str(path), str(error)) from error

        for line_number, line in enumerate(text.splitlines(), start=1):
            if not line.strip():
                continue
            location = f"{path}:{line_number}"
            try:
                raw = json.loads(line)
            except json.JSONDecodeError as error:
                raise _fail(location, f"invalid JSON: {error.msg}") from error
            if not isinstance(raw, dict):
                raise _fail(location, "row must be a JSON object")
            rows.append(
                _validate_row(
                    raw,
                    location=location,
                    known_case_ids=known_case_ids,
                )
            )
    return rows


def _format_csv_value(value: object) -> str:
    if value is None:
        return ""
    if type(value) is bool:
        return "true" if value else "false"
    return str(value)


def _join_sorted(values: set[int | str]) -> str:
    if not values:
        return ""
    sample = next(iter(values))
    if isinstance(sample, int):
        ordered = sorted(value for value in values if isinstance(value, int))
        return ";".join(str(value) for value in ordered)
    ordered = sorted(str(value) for value in values)
    return ";".join(ordered)


def _summarize_case(case: dict[str, Any], rows: list[dict[str, Any]]) -> dict[str, object]:
    successful = [row for row in rows if row["status"] == "ok"]
    target_upper_bound = case.get("target_upper_bound")
    best_upper_bound = min((row["upper_bound"] for row in successful), default=None)
    elapsed_values = [row["elapsed_s"] for row in successful]

    target_hit_count: int | None
    target_hit_rate: str | None
    if target_upper_bound is None:
        target_hit_count = None
        target_hit_rate = None
    else:
        target_hit_count = sum(1 for row in successful if row["upper_bound"] <= target_upper_bound)
        target_hit_rate = (
            f"{target_hit_count / len(successful):.6f}" if successful else None
        )

    return {
        "case_id": case["case_id"],
        "code_id": case["code_id"],
        "distance_side": case["distance_side"],
        "baseline_key": case["baseline_key"],
        "baseline_required": case["baseline_required"],
        "manifest_seed": case["seed"],
        "manifest_iterations": case["iterations"],
        "manifest_restarts": case["restarts"],
        "manifest_target_weight": case.get("target_weight"),
        "target_upper_bound": target_upper_bound,
        "attempted_seed_rows": len(rows),
        "successful_seed_rows": len(successful),
        "best_upper_bound": best_upper_bound,
        "median_elapsed_s": statistics.median(elapsed_values) if elapsed_values else None,
        "min_elapsed_s": min(elapsed_values) if elapsed_values else None,
        "max_elapsed_s": max(elapsed_values) if elapsed_values else None,
        "target_hit_count": target_hit_count,
        "target_hit_rate": target_hit_rate,
        "run_seed_values": _join_sorted({row["seed"] for row in rows}),
        "run_iterations_values": _join_sorted({row["iterations"] for row in rows}),
        "run_restarts_values": _join_sorted({row["restarts"] for row in rows}),
        "run_target_weight_values": _join_sorted(
            {row["target_weight"] for row in rows if row["target_weight"] is not None}
        ),
        "run_status_values": _join_sorted({row["status"] for row in rows}),
        "summary_status": "ok" if successful else "no_success",
    }


def summarize_cases(
    cases: list[dict[str, Any]],
    rows: list[dict[str, Any]],
) -> tuple[dict[str, list[dict[str, Any]]], list[dict[str, object]]]:
    rows_by_case = {case["case_id"]: [] for case in cases if isinstance(case, dict)}
    for row in rows:
        rows_by_case[row["case_id"]].append(row)
    summaries = [_summarize_case(case, rows_by_case[case["case_id"]]) for case in cases]
    return rows_by_case, summaries


def write_summary_csv(path: Path, summaries: list[dict[str, object]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_FIELDS)
        writer.writeheader()
        for summary in summaries:
            writer.writerow({field: _format_csv_value(summary[field]) for field in CSV_FIELDS})


def _manifest_settings_lines(cases: list[dict[str, Any]]) -> list[str]:
    return [
        (
            f"- `{case['case_id']}`: seed={case['seed']}, iterations={case['iterations']}, "
            f"restarts={case['restarts']}, target_weight={case.get('target_weight') or 'none'}"
        )
        for case in cases
    ]


def _observed_settings_lines(
    manifest_case_ids: list[str],
    rows_by_case: dict[str, list[dict[str, Any]]],
) -> list[str]:
    lines: list[str] = []
    for case_id in manifest_case_ids:
        rows = rows_by_case[case_id]
        if not rows:
            lines.append(f"- `{case_id}`: none")
            continue
        seeds = _join_sorted({row["seed"] for row in rows})
        iterations = _join_sorted({row["iterations"] for row in rows})
        restarts = _join_sorted({row["restarts"] for row in rows})
        target_weights = _join_sorted(
            {row["target_weight"] for row in rows if row["target_weight"] is not None}
        )
        commands = sorted(
            {" ".join(row["command"]) for row in rows if isinstance(row.get("command"), list)}
        )
        command_text = " | ".join(commands) if commands else "none"
        lines.append(
            (
                f"- `{case_id}`: seeds={seeds or 'none'}, iterations={iterations or 'none'}, "
                f"restarts={restarts or 'none'}, target_weight={target_weights or 'none'}, "
                f"commands={command_text}"
            )
        )
    return lines


def write_summary_md(
    path: Path,
    *,
    manifest_path: Path,
    run_paths: list[Path],
    argv: list[str],
    manifest: dict[str, Any],
    cases: list[dict[str, Any]],
    rows_by_case: dict[str, list[dict[str, Any]]],
    summaries: list[dict[str, object]],
) -> None:
    lines = [
        "# QEC Code Random-Window Summary",
        "",
        "## Provenance",
        f"- Manifest: `{manifest_path}`",
        f"- Run files: {', '.join(f'`{path}`' for path in run_paths)}",
        f"- Manifest suite/version: `{manifest.get('suite')}` / `{manifest.get('manifest_version')}`",
        f"- Summarizer argv: `{json.dumps(argv)}`",
        "- Manifest command settings:",
        *_manifest_settings_lines(cases),
        "- Observed run command settings:",
        *_observed_settings_lines([case["case_id"] for case in cases], rows_by_case),
        "",
        "## Case Summary",
        "",
        "| case_id | code_id | status | attempted | successful | best_upper_bound | target_upper_bound | elapsed_s | note |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]

    for summary in summaries:
        target_upper_bound = summary["target_upper_bound"]
        target_upper_bound_text = (
            "-" if target_upper_bound in {None, ""} else str(target_upper_bound)
        )
        if summary["successful_seed_rows"]:
            elapsed_text = (
                f"median={summary['median_elapsed_s']}, min={summary['min_elapsed_s']}, "
                f"max={summary['max_elapsed_s']}"
            )
            note = ""
        else:
            elapsed_text = "-"
            note = "NO SUCCESSFUL ROWS"

        best_upper_bound = summary["best_upper_bound"]
        best_text = "-" if best_upper_bound in {None, ""} else str(best_upper_bound)
        lines.append(
            "| {case_id} | {code_id} | {summary_status} | {attempted_seed_rows} | "
            "{successful_seed_rows} | {best} | {target_upper_bound} | {elapsed} | {note} |".format(
                case_id=summary["case_id"],
                code_id=summary["code_id"],
                summary_status=summary["summary_status"],
                attempted_seed_rows=summary["attempted_seed_rows"],
                successful_seed_rows=summary["successful_seed_rows"],
                best=best_text,
                target_upper_bound=target_upper_bound_text,
                elapsed=elapsed_text,
                note=note,
            )
        )

    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _validated_cases(manifest_path: Path) -> list[dict[str, Any]]:
    manifest = load_manifest(manifest_path)
    errors = validate_manifest(manifest)
    if errors:
        raise SummaryError("\n".join(f"{manifest_path}: {error}" for error in errors))
    cases = manifest["cases"]
    assert isinstance(cases, list)
    return [case for case in cases if isinstance(case, dict)]


def run(args: argparse.Namespace, argv: list[str] | None = None) -> int:
    try:
        manifest = load_manifest(args.cases)
        cases = _validated_cases(args.cases)
    except SummaryError as error:
        print(error, file=sys.stderr)
        return 2
    except Exception as error:
        print(f"{args.cases}: {error}", file=sys.stderr)
        return 2

    known_case_ids = {case["case_id"] for case in cases}

    try:
        rows = load_run_rows(args.runs, known_case_ids)
    except SummaryError as error:
        print(error, file=sys.stderr)
        return 1

    rows_by_case, summaries = summarize_cases(cases, rows)

    try:
        args.out_dir.mkdir(parents=True, exist_ok=True)
        write_summary_csv(args.out_dir / "summary.csv", summaries)
        write_summary_md(
            args.out_dir / "summary.md",
            manifest_path=args.cases,
            run_paths=args.runs,
            argv=sys.argv[1:] if argv is None else argv,
            manifest=manifest,
            cases=cases,
            rows_by_case=rows_by_case,
            summaries=summaries,
        )
    except OSError as error:
        print(f"{args.out_dir}: {error}", file=sys.stderr)
        return 1

    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Summarize qec-code random-window benchmark JSONL runs."
    )
    parser.add_argument("--cases", type=Path, required=True)
    parser.add_argument("--runs", type=Path, nargs="+", required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return run(args, argv)


if __name__ == "__main__":
    raise SystemExit(main())
