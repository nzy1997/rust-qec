#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

import tomllib


PASS_LINE = "PASS expanded rstim-vs-Stim correctness evidence"
DEFAULT_CATALOG = Path("benchmarks/rstim_vs_stim_simulator/distribution_cases.toml")
DEFAULT_DISTRIBUTION_DIR = Path("benchmarks/rstim_vs_stim_simulator/results/distributions")
DEFAULT_FULL_SUMMARY = Path("benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> Any:
    try:
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except FileNotFoundError as exc:
        raise ValueError(f"missing required file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc.msg}") from exc


def require_dict(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} is not a JSON object")
    return value


def load_catalog_case_ids(path: Path) -> list[str]:
    try:
        with path.open("rb") as handle:
            data = tomllib.load(handle)
    except FileNotFoundError as exc:
        raise ValueError(f"missing required file: {path}") from exc
    except tomllib.TOMLDecodeError as exc:
        raise ValueError(f"invalid TOML in {path}: {exc}") from exc

    cases = data.get("cases")
    if not isinstance(cases, list):
        raise ValueError("catalog cases are missing")

    case_ids: list[str] = []
    for case in cases:
        if not isinstance(case, dict):
            continue
        case_id = case.get("case_id")
        if isinstance(case_id, str):
            case_ids.append(case_id)

    if not case_ids:
        raise ValueError("catalog cases are missing")
    return case_ids


def validate_distribution_summary(summary: dict[str, Any], catalog_path: Path) -> None:
    if summary.get("status") != "pass":
        raise ValueError("distribution summary status is not pass")

    catalog_case_ids = set(load_catalog_case_ids(catalog_path))
    catalog_sha256 = summary.get("catalog_sha256")
    if catalog_sha256 != sha256_file(catalog_path):
        raise ValueError("distribution summary catalog hash mismatch")

    cases = summary.get("cases")
    if not isinstance(cases, list):
        raise ValueError("distribution summary cases are missing")

    by_case_id: dict[str, dict[str, Any]] = {}
    for case in cases:
        if not isinstance(case, dict):
            continue
        case_id = case.get("case_id")
        if isinstance(case_id, str):
            by_case_id[case_id] = case

    for case_id in sorted(by_case_id):
        if case_id not in catalog_case_ids:
            raise ValueError(f"unknown distribution evidence case {case_id}")

    for case_id in sorted(catalog_case_ids):
        case = by_case_id.get(case_id)
        if case is None:
            raise ValueError(f"missing distribution evidence for case {case_id}")
        if case.get("status") != "pass":
            raise ValueError(f"distribution evidence for case {case_id} did not pass")


def validate_rollup(rollup: dict[str, Any], summary_path: Path, full_summary_path: Path) -> None:
    if rollup.get("status") != "pass":
        raise ValueError("expanded correctness rollup status is not pass")
    if rollup.get("distribution_summary_path") != str(summary_path):
        raise ValueError("expanded rollup distribution summary path mismatch")
    if rollup.get("full_summary_path") != str(full_summary_path):
        raise ValueError("expanded rollup full summary path mismatch")
    if rollup.get("distribution_summary_sha256") != sha256_file(summary_path):
        raise ValueError("expanded rollup distribution summary hash mismatch")
    if rollup.get("full_summary_sha256") != sha256_file(full_summary_path):
        raise ValueError("expanded rollup full summary hash mismatch")


def validate_report(report_path: Path, summary_path: Path, rollup_path: Path, full_summary_path: Path) -> None:
    try:
        report_text = report_path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise ValueError(f"missing required file: {report_path}") from exc

    for ref in (str(summary_path), str(rollup_path), str(full_summary_path)):
        if ref not in report_text:
            raise ValueError(f"report is missing artifact reference {ref}")


def validate_full_summary(full_summary: dict[str, Any]) -> None:
    if full_summary.get("status") != "pass":
        raise ValueError("full correctness summary status is not pass")


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--catalog", default=str(DEFAULT_CATALOG))
    parser.add_argument("--distribution-dir", default=str(DEFAULT_DISTRIBUTION_DIR))
    parser.add_argument("--full-summary", default=str(DEFAULT_FULL_SUMMARY))
    parser.add_argument("--summary", help=argparse.SUPPRESS)
    parser.add_argument("--rollup", help=argparse.SUPPRESS)
    parser.add_argument("--report", help=argparse.SUPPRESS)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    distribution_dir = Path(args.distribution_dir)
    catalog_path = Path(args.catalog)
    summary_path = Path(args.summary) if args.summary else distribution_dir / "summary.json"
    rollup_path = (
        Path(args.rollup) if args.rollup else distribution_dir / "expanded-correctness.json"
    )
    report_path = Path(args.report) if args.report else distribution_dir / "report.md"
    full_summary_path = Path(args.full_summary)

    try:
        summary = require_dict(load_json(summary_path), "distribution summary")
        rollup = require_dict(load_json(rollup_path), "expanded rollup")
        full_summary = require_dict(load_json(full_summary_path), "full correctness summary")
        validate_distribution_summary(summary, catalog_path)
        validate_rollup(rollup, summary_path, full_summary_path)
        validate_report(report_path, summary_path, rollup_path, full_summary_path)
        validate_full_summary(full_summary)
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 1

    print(PASS_LINE)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
