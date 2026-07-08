#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any


DEFAULT_SUMMARY_PATH = Path("benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json")
DEFAULT_MANIFEST_PATH = Path("site/benchmark-site.json")
SELECTED_CASE_LABEL = "stim-style-surface-sample-d11-r100-b1024"
EXPECTED_WORKLOAD = "sample"
EXPECTED_TIER = "report_only"
EXPECTED_PRESENT_VARIANTS = ["rstim-compiled", "rstim-interpreted", "stim-cli"]
EXPECTED_RATES = {
    "stim-cli": 5690.64878525516,
    "rstim-compiled": 21.774891038227285,
}
EXPECTED_SAMPLE_COUNTS = {
    "stim-cli": 1,
    "rstim-compiled": 1,
}
RATIO_MIN = 200.0
RATIO_MAX = 300.0
RATE_REL_TOL = 1e-12
RATE_ABS_TOL = 1e-9


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def find_selected_case(summary: dict[str, Any]) -> dict[str, Any]:
    cases = summary.get("cases")
    if not isinstance(cases, list):
        raise ValueError("missing selected case")
    for case in cases:
        if isinstance(case, dict) and case.get("case_label") == SELECTED_CASE_LABEL:
            return case
    raise ValueError("missing selected case")


def variants_by_name(case: dict[str, Any]) -> dict[str, dict[str, Any]]:
    variants: dict[str, dict[str, Any]] = {}
    for variant in case.get("variants", []):
        if not isinstance(variant, dict):
            continue
        name = variant.get("tool_variant")
        if isinstance(name, str):
            variants[name] = variant
    return variants


def validate_case(summary: dict[str, Any]) -> float:
    case = find_selected_case(summary)

    if case.get("workload") != EXPECTED_WORKLOAD:
        raise ValueError("selected-case workload changed")
    if case.get("tier") != EXPECTED_TIER:
        raise ValueError("selected-case tier changed")

    variants = variants_by_name(case)

    stim = variants.get("stim-cli")
    if stim is None:
        raise ValueError("missing stim-cli")
    rstim = variants.get("rstim-compiled")
    if rstim is None:
        raise ValueError("missing rstim-compiled")

    if stim.get("status") != "completed":
        raise ValueError("stim-cli status is not completed")
    if rstim.get("status") != "completed":
        raise ValueError("rstim-compiled status is not completed")

    if stim.get("sample_count") != EXPECTED_SAMPLE_COUNTS["stim-cli"]:
        raise ValueError("stim-cli sample count changed")
    if rstim.get("sample_count") != EXPECTED_SAMPLE_COUNTS["rstim-compiled"]:
        raise ValueError("rstim-compiled sample count changed")

    stim_rate = stim.get("median_shots_per_second")
    rstim_rate = rstim.get("median_shots_per_second")
    if not isinstance(stim_rate, (int, float)) or not isinstance(rstim_rate, (int, float)):
        raise ValueError("selected-case rate changed")

    ratio = float(stim_rate) / float(rstim_rate)
    if not (RATIO_MIN <= ratio <= RATIO_MAX):
        raise ValueError("ratio outside 200-300")

    if not math.isclose(
        float(stim_rate),
        EXPECTED_RATES["stim-cli"],
        rel_tol=RATE_REL_TOL,
        abs_tol=RATE_ABS_TOL,
    ):
        raise ValueError("selected-case rate changed")
    if not math.isclose(
        float(rstim_rate),
        EXPECTED_RATES["rstim-compiled"],
        rel_tol=RATE_REL_TOL,
        abs_tol=RATE_ABS_TOL,
    ):
        raise ValueError("selected-case rate changed")

    if case.get("present_variants") != EXPECTED_PRESENT_VARIANTS:
        raise ValueError("selected-case present variants changed")
    return ratio


def recorded_manifest_sha256(manifest: dict[str, Any], artifact_path: str) -> str | None:
    families = manifest.get("families")
    if not isinstance(families, list):
        return None
    for family in families:
        if not isinstance(family, dict):
            continue
        items = family.get("evidence_items")
        if not isinstance(items, list):
            continue
        for item in items:
            if not isinstance(item, dict):
                continue
            artifacts = item.get("artifacts")
            if not isinstance(artifacts, list):
                continue
            for artifact in artifacts:
                if not isinstance(artifact, dict) or artifact.get("path") != artifact_path:
                    continue
                provenance = item.get("provenance")
                if not isinstance(provenance, dict):
                    continue
                artifact_hashes = provenance.get("artifact_hashes")
                if not isinstance(artifact_hashes, dict) or artifact_hashes.get("status") != "recorded":
                    return None
                value = artifact_hashes.get("value")
                if not isinstance(value, dict):
                    return None
                entry = value.get(artifact_path)
                if not isinstance(entry, dict):
                    return None
                digest = entry.get("sha256")
                return digest if isinstance(digest, str) else None
    return None


def validate_default_hash(summary_path: Path) -> None:
    resolved_summary = summary_path.resolve()
    default_summary = (Path.cwd() / DEFAULT_SUMMARY_PATH).resolve()
    if resolved_summary != default_summary:
        return

    manifest_path = DEFAULT_MANIFEST_PATH
    if not manifest_path.exists():
        return

    manifest = load_json(manifest_path)
    if not isinstance(manifest, dict):
        return

    recorded = recorded_manifest_sha256(manifest, str(DEFAULT_SUMMARY_PATH))
    if recorded is None:
        return
    if sha256_file(summary_path) != recorded:
        raise ValueError("checked artifact hash differs from site manifest")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("summary_path", nargs="?", default=str(DEFAULT_SUMMARY_PATH))
    args = parser.parse_args(argv)

    summary_path = Path(args.summary_path)
    try:
        summary = load_json(summary_path)
        if not isinstance(summary, dict):
            raise ValueError("missing selected case")
        validate_default_hash(summary_path)
        ratio = validate_case(summary)
    except Exception as exc:
        print(f"ERROR checked #406 gap is not preserved: {exc}", file=sys.stderr)
        return 1

    print(f"PASS checked #406 gap is preserved: stim-cli is {ratio:.2f}x faster than rstim-compiled")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
