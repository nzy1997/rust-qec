#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


REQUIRED_RELEASE_FILES = ("summary.json", "report.md", "environment.json")
FORBIDDEN_BROAD_REPORT_PATTERNS = (
    "broad speed superiority",
    "broad rstim/stim performance parity",
    "all-workload parity",
    "all workloads",
)
REQUIRED_ENVIRONMENT_FIELDS = (
    "rstim_binary_path",
    "rustc_version",
    "cargo_version",
    "stim_cli_status",
)


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def require_dict(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} is not a JSON object")
    return value


def parse_required_variants(raw: str) -> list[str]:
    variants = [variant.strip() for variant in raw.split(",") if variant.strip()]
    if not variants:
        raise ValueError("no required variants requested")
    return variants


def validate_release_files(results_dir: Path) -> tuple[Path, Path, Path]:
    allowed_filenames = set(REQUIRED_RELEASE_FILES)
    for path in sorted(results_dir.iterdir(), key=lambda entry: entry.name):
        if path.is_file() and path.name not in allowed_filenames:
            raise ValueError(f"unexpected release file: {path.name}")

    paths: list[Path] = []
    for filename in REQUIRED_RELEASE_FILES:
        path = results_dir / filename
        if not path.is_file():
            raise ValueError(f"missing required release file: {filename}")
        paths.append(path)
    return paths[0], paths[1], paths[2]


def variants_by_name(case: dict[str, Any]) -> dict[str, dict[str, Any]]:
    variants: dict[str, dict[str, Any]] = {}
    raw_variants = case.get("variants")
    if not isinstance(raw_variants, list):
        return variants
    for variant in raw_variants:
        if not isinstance(variant, dict):
            continue
        name = variant.get("tool_variant")
        if isinstance(name, str):
            variants[name] = variant
    return variants


def validate_case(summary: dict[str, Any], case_label: str, workload: str, required_variants: list[str]) -> None:
    cases = summary.get("cases")
    if not isinstance(cases, list):
        raise ValueError("summary.json missing cases")

    matches = [case for case in cases if isinstance(case, dict) and case.get("case_label") == case_label]
    if len(matches) != 1:
        raise ValueError(f"case {case_label} must be present exactly once")

    case = matches[0]
    if case.get("workload") != workload:
        raise ValueError(f"case {case_label} workload must be {workload}")

    present_variants = case.get("present_variants")
    if not isinstance(present_variants, list):
        raise ValueError("summary.json case missing present_variants")
    present_variant_set = {variant for variant in present_variants if isinstance(variant, str)}

    variants = variants_by_name(case)
    for required in required_variants:
        if required not in present_variant_set or required not in variants:
            raise ValueError(f"missing required variant {required}")
        if variants[required].get("status") != "completed":
            raise ValueError(f"required variant {required} status is not completed")


def validate_environment(environment: dict[str, Any], case_label: str) -> None:
    if environment.get("profile") != "release":
        raise ValueError("environment.json profile must be release")

    for field in REQUIRED_ENVIRONMENT_FIELDS:
        value = environment.get(field)
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"environment.json missing {field}")

    case_labels = environment.get("case_labels")
    case_label_value = environment.get("case_label")
    if isinstance(case_labels, list):
        if case_label not in case_labels:
            raise ValueError(f"environment.json missing case label {case_label}")
    elif case_label_value != case_label:
        raise ValueError(f"environment.json missing case label {case_label}")


def validate_report(report_path: Path, case_label: str) -> None:
    report = report_path.read_text(encoding="utf-8")
    if case_label not in report:
        raise ValueError(f"report.md missing case label {case_label}")
    report_lower = report.lower()
    for pattern in FORBIDDEN_BROAD_REPORT_PATTERNS:
        if pattern in report_lower:
            raise ValueError("report.md contains forbidden broad performance claim")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results-dir", type=Path, required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--workload", required=True)
    parser.add_argument("--required-variants", required=True)
    args = parser.parse_args(argv)

    try:
        summary_path, report_path, environment_path = validate_release_files(args.results_dir)
        required_variants = parse_required_variants(args.required_variants)
        summary = require_dict(load_json(summary_path), "summary.json")
        environment = require_dict(load_json(environment_path), "environment.json")
        validate_case(summary, args.case, args.workload, required_variants)
        validate_environment(environment, args.case)
        validate_report(report_path, args.case)
    except Exception as exc:
        print(f"ERROR release speed case check failed: {exc}", file=sys.stderr)
        return 1

    print(f"PASS release speed case {args.case}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
