#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools import check_rstim_vs_stim_expanded_correctness as correctness_checker
from tools import check_rstim_vs_stim_release_dem_speed_case as dem_checker
from tools import check_rstim_vs_stim_release_speed_case as speed_checker

PASS_LINE = "PASS expanded rstim-vs-Stim evidence"
DEFAULT_CATALOG = Path("benchmarks/rstim_vs_stim_simulator/distribution_cases.toml")
OLD_DEBUG_SUMMARY = Path(
    "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
)
DEM_CASE = "stim-style-surface-dem-sample-d11-r100-b1024"
DEM_VARIANTS = ("stim-sample-dem", "rstim-sample-dem")


class EvidenceArgumentParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise ValueError(message)


@dataclass(frozen=True)
class SpeedEvidenceSpec:
    case_label: str
    workload: str
    required_variants: tuple[str, ...]
    source_issue: int
    evidence_kind_fragment: str


STANDARD_VARIANTS = ("stim-cli", "rstim-interpreted", "rstim-compiled")
SPEED_EVIDENCE_SPECS = (
    SpeedEvidenceSpec(
        "stim-style-surface-sample-d11-r100-b1024",
        "sample",
        STANDARD_VARIANTS,
        416,
        "post-optimization",
    ),
    SpeedEvidenceSpec(
        "rep-sample-d13-r13",
        "sample",
        STANDARD_VARIANTS,
        434,
        "repetition",
    ),
    SpeedEvidenceSpec(
        "surface-detect-d13-r13",
        "detect",
        STANDARD_VARIANTS,
        435,
        "surface detect",
    ),
)


def load_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        with path.open(encoding="utf-8") as handle:
            value = json.load(handle)
    except FileNotFoundError as exc:
        raise ValueError(f"missing required file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc.msg}") from exc
    if not isinstance(value, dict):
        raise ValueError(f"{label} is not a JSON object")
    return value


def summary_case_labels(summary: dict[str, Any], label: str) -> list[str]:
    cases = summary.get("cases")
    if not isinstance(cases, list):
        raise ValueError(f"{label} missing cases")
    return [
        case_label
        for case in cases
        if isinstance(case, dict)
        and isinstance((case_label := case.get("case_label")), str)
    ]


def repository_relative_path(path: Path) -> Path:
    try:
        return path.resolve().relative_to(REPO_ROOT.resolve())
    except ValueError:
        return path


def index_speed_evidence(
    speed_dirs: list[Path],
) -> dict[str, tuple[Path, dict[str, Any]]]:
    expected = {spec.case_label for spec in SPEED_EVIDENCE_SPECS}
    indexed: dict[str, tuple[Path, dict[str, Any]]] = {}
    unmatched_dirs: list[Path] = []
    for results_dir in speed_dirs:
        summary = load_json_object(results_dir / "summary.json", "summary.json")
        matched = False
        for case_label in summary_case_labels(summary, "summary.json"):
            if case_label not in expected:
                continue
            matched = True
            if case_label in indexed:
                raise ValueError(f"duplicate required evidence case {case_label}")
            indexed[case_label] = (results_dir, summary)
        if not matched:
            unmatched_dirs.append(results_dir)
    for spec in SPEED_EVIDENCE_SPECS:
        if spec.case_label not in indexed:
            raise ValueError(f"missing required evidence case {spec.case_label}")
    if unmatched_dirs:
        raise ValueError(
            f"speed directory contains no required evidence case: {unmatched_dirs[0]}"
        )
    return indexed


def validate_correctness(
    catalog_path: Path,
    correctness_dir: Path,
    full_correctness_path: Path,
) -> None:
    summary_path = correctness_dir / "summary.json"
    rollup_path = correctness_dir / "expanded-correctness.json"
    summary = load_json_object(summary_path, "distribution summary")
    rollup = load_json_object(rollup_path, "expanded rollup")
    full_summary = load_json_object(full_correctness_path, "full correctness summary")
    summary_reference = repository_relative_path(summary_path)
    rollup_reference = repository_relative_path(rollup_path)
    full_summary_reference = repository_relative_path(full_correctness_path)
    validated_rollup = dict(rollup)
    for field, reference, artifact_path in (
        ("distribution_summary_path", summary_reference, summary_path),
        ("full_summary_path", full_summary_reference, full_correctness_path),
    ):
        if reference != artifact_path:
            if validated_rollup.get(field) != str(reference):
                field_label = field.removesuffix("_path").replace("_", " ")
                raise ValueError(f"expanded rollup {field_label} path mismatch")
            validated_rollup[field] = str(artifact_path)
    correctness_checker.validate_distribution_summary(summary, catalog_path)
    correctness_checker.validate_rollup(
        validated_rollup,
        summary,
        catalog_path=catalog_path,
        summary_path=summary_path,
        full_summary_path=full_correctness_path,
    )
    correctness_checker.validate_report(
        correctness_dir / "report.md",
        summary_reference,
        rollup_reference,
        full_summary_reference,
    )
    correctness_checker.validate_full_summary(full_summary)


def validate_speed_provenance(
    environment: dict[str, Any],
    spec: SpeedEvidenceSpec,
) -> None:
    if environment.get("published_artifact") is not True:
        raise ValueError("environment.json published_artifact must be true")
    if environment.get("source_issue") != spec.source_issue:
        raise ValueError(f"environment.json source_issue must be {spec.source_issue}")
    evidence_kind = environment.get("evidence_kind")
    if (
        not isinstance(evidence_kind, str)
        or spec.evidence_kind_fragment not in evidence_kind.lower()
    ):
        raise ValueError(
            "environment.json evidence_kind missing "
            f"{spec.evidence_kind_fragment}"
        )


def validate_speed_evidence(speed_dirs: list[Path]) -> None:
    if not speed_dirs:
        raise ValueError("no speed directories requested")
    indexed = index_speed_evidence(speed_dirs)
    old_debug_summary = load_json_object(
        REPO_ROOT / OLD_DEBUG_SUMMARY, "old #406 summary"
    )
    for spec in SPEED_EVIDENCE_SPECS:
        results_dir, summary = indexed[spec.case_label]
        _, report_path, environment_path = speed_checker.validate_release_files(
            results_dir
        )
        environment = load_json_object(environment_path, "environment.json")
        speed_checker.validate_case(
            summary,
            spec.case_label,
            spec.workload,
            list(spec.required_variants),
        )
        speed_checker.validate_environment(environment, spec.case_label)
        speed_checker.validate_report(report_path, spec.case_label)
        validate_speed_provenance(environment, spec)
        if spec.source_issue == 416 and summary == old_debug_summary:
            raise ValueError("release evidence reuses old #406 debug summary")


def validate_dem_evidence(results_dir: Path) -> None:
    summary_path = results_dir / "summary.json"
    if not summary_path.is_file():
        raise ValueError(f"missing required evidence case {DEM_CASE}")
    summary = load_json_object(summary_path, "DEM summary.json")
    matches = [
        label
        for label in summary_case_labels(summary, "DEM summary.json")
        if label == DEM_CASE
    ]
    if not matches:
        raise ValueError(f"missing required evidence case {DEM_CASE}")
    if len(matches) != 1:
        raise ValueError(f"duplicate required evidence case {DEM_CASE}")

    dem_checker.validate_pinned_metadata()
    dem_checker.validate_required_files(results_dir)
    dem_checker.validate_raw_records(
        results_dir,
        case_label=DEM_CASE,
        required_variants=list(DEM_VARIANTS),
    )
    environment = load_json_object(
        results_dir / "environment.json", "environment.json"
    )
    dem_checker.validate_summary(
        summary,
        case_label=DEM_CASE,
        required_variants=list(DEM_VARIANTS),
    )
    dem_checker.validate_environment(environment, case_label=DEM_CASE)
    report = (results_dir / "report.md").read_text(encoding="utf-8")
    if DEM_CASE not in report:
        raise ValueError(f"report.md missing case label {DEM_CASE}")
    for field in speed_checker.REQUIRED_ENVIRONMENT_FIELDS:
        value = environment.get(field)
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"environment.json missing {field}")


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = EvidenceArgumentParser()
    parser.add_argument("--catalog", default=str(DEFAULT_CATALOG))
    parser.add_argument("--correctness-dir", required=True)
    parser.add_argument("--full-correctness", required=True)
    parser.add_argument("--speed-dirs", required=True)
    parser.add_argument("--dem-speed-dir", required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
        speed_dirs = [
            Path(raw.strip()) for raw in args.speed_dirs.split(",") if raw.strip()
        ]
        validate_correctness(
            Path(args.catalog),
            Path(args.correctness_dir),
            Path(args.full_correctness),
        )
        validate_speed_evidence(speed_dirs)
        validate_dem_evidence(Path(args.dem_speed_dir))
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        return 1
    print(PASS_LINE)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
