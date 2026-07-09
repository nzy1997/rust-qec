#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RESULTS_DIR = Path("benchmarks/rstim_vs_stim_simulator/results/release-dem-sample")
DEFAULT_CASE_LABEL = "stim-style-surface-dem-sample-d11-r100-b1024"
DEFAULT_REQUIRED_VARIANTS = ("stim-sample-dem", "rstim-sample-dem")
EXPECTED_WORKLOAD = "sample_dem"
EXPECTED_SHOTS = 1024
EXPECTED_DETECTOR_COUNT = 12000
EXPECTED_OBSERVABLE_COUNT = 1
REQUIRED_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json")

METADATA_PATH = (
    REPO_ROOT
    / "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.dem.metadata.json"
)


def _mismatch(detail: str) -> ValueError:
    return ValueError(f"DEM metadata mismatch: {detail}")


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def require_dict(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} is not a JSON object")
    return value


def require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise ValueError(f"{label} is not a JSON array")
    return value


def metadata() -> dict[str, Any]:
    payload = require_dict(load_json(METADATA_PATH), "DEM metadata")
    return payload


def resolve_metadata_path(raw_path: object) -> Path:
    path = Path(str(raw_path))
    if path.is_absolute():
        return path.resolve()
    return (METADATA_PATH.parent / path).resolve()


_METADATA = metadata()
EXPECTED_DEM_PATH = resolve_metadata_path(_METADATA["dem_path"])
EXPECTED_DEM_SHA256 = str(_METADATA["dem_sha256"])
EXPECTED_SOURCE_CIRCUIT_PATH = resolve_metadata_path(_METADATA["source_circuit_path"])
EXPECTED_SOURCE_CIRCUIT_SHA256 = str(_METADATA["source_circuit_sha256"])


def validate_required_files(results_dir: Path) -> None:
    for filename in REQUIRED_FILES:
        path = results_dir / filename
        if not path.is_file():
            raise ValueError(f"missing required release file: {filename}")


def find_case(summary: dict[str, Any], case_label: str) -> dict[str, Any]:
    for case in require_list(summary.get("cases"), "summary cases"):
        if isinstance(case, dict) and case.get("case_label") == case_label:
            return case
    raise ValueError(f"missing requested case {case_label}")


def variants_by_name(case: dict[str, Any]) -> dict[str, dict[str, Any]]:
    variants: dict[str, dict[str, Any]] = {}
    for variant in require_list(case.get("variants"), "case variants"):
        if not isinstance(variant, dict):
            continue
        name = variant.get("tool_variant")
        if isinstance(name, str):
            variants[name] = variant
    return variants


def validate_summary(summary: dict[str, Any], *, case_label: str, required_variants: list[str]) -> None:
    if summary.get("issues") != []:
        raise ValueError("summary issues must be []")

    case = find_case(summary, case_label)
    if case.get("workload") != EXPECTED_WORKLOAD:
        raise ValueError(f"case {case_label} workload must be {EXPECTED_WORKLOAD}")

    present_variants = require_list(case.get("present_variants"), "present_variants")
    variant_names = variants_by_name(case)
    for required_variant in required_variants:
        if required_variant not in present_variants or required_variant not in variant_names:
            raise ValueError(f"missing required variant {required_variant}")
        if variant_names[required_variant].get("status") != "completed":
            status = variant_names[required_variant].get("status")
            raise ValueError(f"required variant {required_variant} status is {status}")


def require_equal(actual: Any, expected: Any, detail: str) -> None:
    if actual != expected:
        raise _mismatch(detail)


def validate_environment(environment: dict[str, Any], *, case_label: str) -> None:
    require_equal(environment.get("profile"), "release", "profile does not match")
    require_equal(environment.get("case_label"), case_label, "case label does not match")
    require_equal(environment.get("dem_path"), str(EXPECTED_DEM_PATH), "dem path does not match")
    require_equal(environment.get("dem_sha256"), EXPECTED_DEM_SHA256, "dem hash does not match")
    require_equal(
        environment.get("source_circuit_path"),
        str(EXPECTED_SOURCE_CIRCUIT_PATH),
        "source circuit path does not match",
    )
    require_equal(
        environment.get("source_circuit_sha256"),
        EXPECTED_SOURCE_CIRCUIT_SHA256,
        "source circuit hash does not match",
    )
    require_equal(
        environment.get("expected_detectors"),
        EXPECTED_DETECTOR_COUNT,
        "detector count does not match",
    )
    require_equal(
        environment.get("expected_observables"),
        EXPECTED_OBSERVABLE_COUNT,
        "observable count does not match",
    )


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results-dir", default=str(DEFAULT_RESULTS_DIR))
    parser.add_argument("--case", default=DEFAULT_CASE_LABEL)
    parser.add_argument("--required-variants", nargs="+", default=list(DEFAULT_REQUIRED_VARIANTS))
    args = parser.parse_args(argv)
    args.required_variants = [
        variant.strip()
        for raw_variant in args.required_variants
        for variant in raw_variant.split(",")
        if variant.strip()
    ]
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    results_dir = Path(args.results_dir)
    try:
        validate_required_files(results_dir)
        summary = require_dict(load_json(results_dir / "summary.json"), "summary.json")
        environment = require_dict(load_json(results_dir / "environment.json"), "environment.json")
        validate_summary(summary, case_label=args.case, required_variants=list(args.required_variants))
        validate_environment(environment, case_label=args.case)
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        return 1

    print(f"PASS release DEM speed case {args.case}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
