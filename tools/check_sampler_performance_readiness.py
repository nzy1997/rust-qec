#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tomllib
from pathlib import Path, PurePosixPath
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.rstim_vs_stim_simulator.portable_provenance import EXPECTED_BUNDLE_IDS, load_catalog, validate_catalog
from tools import check_all_portable_evidence as portable
from tools import check_rstim_vs_stim_expanded_correctness as expanded_correctness
from tools import check_rstim_vs_stim_fair_cli_evidence as fair_cli
from tools import check_rstim_vs_stim_gap_artifact as gap_artifact
from tools import check_rstim_vs_stim_instruction_wide_noise_evidence as instruction_wide
from tools import check_rstim_vs_stim_reference_build_evidence as reference_build


PASS_LINE = "PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05"
ISSUE_BASE_URL = "https://github.com/nzy1997/rstim/issues"


class ReadinessError(RuntimeError):
    """Raised when committed evidence does not meet the readiness contract."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json_object(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def load_jsonl_records(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        value = json.loads(line)
        if not isinstance(value, dict):
            raise ValueError(f"{path}:{line_number} must contain a JSON object")
        records.append(value)
    return records


def bundle_path(catalog_path: Path, bundle: dict[str, Any]) -> Path:
    raw_path = bundle.get("bundle_path")
    if not isinstance(raw_path, str):
        raise ValueError("catalog bundle path is missing")
    return catalog_path.resolve().parents[2] / PurePosixPath(raw_path)


def repository_relative_path(path: Path) -> str:
    return path.resolve().relative_to(REPO_ROOT).as_posix()


def contains_host_absolute_path(value: object) -> bool:
    if isinstance(value, str):
        return value.startswith("/") or (len(value) >= 3 and value[1:3] == ":\\")
    if isinstance(value, dict):
        return any(contains_host_absolute_path(item) for item in value.values())
    if isinstance(value, list):
        return any(contains_host_absolute_path(item) for item in value)
    return False


def validate_checked_provenance_is_portable(catalog: dict[str, Any]) -> None:
    bundles = catalog.get("bundles")
    if not isinstance(bundles, list):
        return
    for bundle in bundles:
        if not isinstance(bundle, dict):
            continue
        provenance = bundle.get("checked_provenance")
        if contains_host_absolute_path(provenance):
            raise ReadinessError("not ready: checked provenance contains host-absolute path")


def direct_phase_counters(raw_path: Path) -> dict[str, int]:
    records = load_jsonl_records(raw_path)
    measured = [
        record
        for record in records
        if record.get("variant") == "rstim-direct-repeat-reference-b8" and record.get("phase") == "measured"
    ]
    if not measured:
        raise ValueError("direct reference measured records are missing")
    counters = measured[0].get("phase_counters")
    if not isinstance(counters, dict):
        raise ValueError("direct reference phase counters are missing")
    if any(record.get("phase_counters") != counters for record in measured):
        raise ValueError("direct reference phase counters differ across measured records")
    required = ("canonical_materializations", "executed_repeat_iterations", "skipped_repeat_iterations")
    result: dict[str, int] = {}
    for key in required:
        value = counters.get(key)
        if not isinstance(value, int):
            raise ValueError(f"direct reference phase counter {key} is missing")
        result[key] = value
    return result


def validate_expanded_correctness() -> dict[str, Any]:
    catalog_path = Path("benchmarks/rstim_vs_stim_simulator/distribution_cases.toml")
    distribution_dir = Path("benchmarks/rstim_vs_stim_simulator/results/distributions")
    summary_path = distribution_dir / "summary.json"
    rollup_path = distribution_dir / "expanded-correctness.json"
    report_path = distribution_dir / "report.md"
    full_summary_path = Path("benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json")
    summary = expanded_correctness.require_dict(expanded_correctness.load_json(summary_path), "distribution summary")
    rollup = expanded_correctness.require_dict(expanded_correctness.load_json(rollup_path), "expanded rollup")
    full_summary = expanded_correctness.require_dict(expanded_correctness.load_json(full_summary_path), "full correctness summary")
    expanded_correctness.validate_distribution_summary(summary, catalog_path)
    expanded_correctness.validate_rollup(
        rollup,
        summary,
        catalog_path=catalog_path,
        summary_path=summary_path,
        full_summary_path=full_summary_path,
    )
    expanded_correctness.validate_report(report_path, summary_path, rollup_path, full_summary_path)
    expanded_correctness.validate_full_summary(full_summary)
    cases = summary.get("cases")
    return {
        "status": "pass",
        "case_count": len(cases) if isinstance(cases, list) else 0,
        "summary_path": str(summary_path),
        "rollup_path": str(rollup_path),
        "report_path": str(report_path),
        "full_summary_path": str(full_summary_path),
    }


def validate_historical_406() -> dict[str, Any]:
    summary_path = Path(gap_artifact.DEFAULT_SUMMARY_PATH)
    summary = load_json_object(summary_path)
    gap_artifact.validate_default_hash(summary_path)
    ratio = gap_artifact.validate_case(summary)
    return {
        "status": "preserved",
        "case_label": gap_artifact.SELECTED_CASE_LABEL,
        "summary_path": str(summary_path),
        "summary_sha256": sha256_file(summary_path),
        "stim_cli_over_rstim_compiled": ratio,
    }


def read_github_issues(repo: str, github_json: Path | None) -> list[dict[str, Any]]:
    if github_json is not None:
        with github_json.open(encoding="utf-8") as handle:
            value = json.load(handle)
    else:
        completed = subprocess.run(
            [
                "gh", "issue", "list", "--repo", repo, "--state", "open", "--milestone",
                "M4: Measured Optimization Closure", "--json", "number,title,state,milestone", "--limit", "100",
            ],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if completed.returncode != 0:
            raise ValueError(f"GitHub milestone query failed: {completed.stderr.strip()}")
        value = json.loads(completed.stdout)
    if not isinstance(value, list) or not all(isinstance(issue, dict) for issue in value):
        raise ValueError("GitHub issue response must be a JSON array of objects")
    return value


def build_readiness(catalog_path: Path, verify_github: str | None = None, github_json: Path | None = None) -> dict[str, object]:
    try:
        catalog = load_catalog(catalog_path)
        validate_checked_provenance_is_portable(catalog)
        catalog_errors = validate_catalog(catalog, catalog_path)
        if catalog_errors:
            raise ReadinessError(f"not ready: {catalog_errors[0]}")
        bundles = catalog.get("bundles")
        if not isinstance(bundles, list):
            raise ValueError("catalog bundles are missing")

        portable_bundles: list[dict[str, object]] = []
        paths: dict[str, Path] = {}
        for bundle in bundles:
            if not isinstance(bundle, dict):
                raise ValueError("catalog bundle is invalid")
            bundle_id = bundle.get("id")
            if not isinstance(bundle_id, str):
                raise ValueError("catalog bundle id is missing")
            checker = portable.CHECKERS.get(bundle_id)
            if checker is None:
                raise ValueError(f"no portable checker is registered for {bundle_id}")
            root = bundle_path(catalog_path, bundle)
            result = checker.validate(root)
            paths[bundle_id] = root
            portable_bundles.append({
                "id": bundle_id,
                "bundle_path": bundle["bundle_path"],
                "checker": checker.validate.__module__,
                "pass_line": checker.pass_line(result),
            })

        reference_result = reference_build.validate_bundle(paths["reference-build-release"])
        if reference_result["direct_speedup"] < 2.0:
            raise ReadinessError("not ready: reference direct/canonical speedup below 2.0")
        counters = direct_phase_counters(paths["reference-build-release"] / "raw.jsonl")
        direct_canonical_materializations = counters["canonical_materializations"]
        direct_executed_repeat_iterations = counters["executed_repeat_iterations"]
        if direct_canonical_materializations != 0:
            raise ReadinessError("not ready: direct reference path recorded production canonical materializations")
        if direct_executed_repeat_iterations != 1:
            raise ReadinessError("not ready: direct reference path did not execute exactly one d11 repeat iteration")

        fair_result = fair_cli.validate_bundle(paths["fair-cli-release"])
        frame_result = instruction_wide.validate_bundle(paths["frame-instruction-wide-release"])
        if frame_result["candidate_over_baseline"] > 1.05:
            raise ReadinessError("not ready: frame candidate/baseline ratio exceeds 1.05")
        frame_correctness = load_json_object(paths["frame-instruction-wide-release"] / "correctness-summary.json")
        if frame_correctness.get("status") != "pass":
            raise ReadinessError("not ready: frame-noise correctness status is not pass")

        issues: dict[str, object] = {"status": "not_checked", "open": []}
        if verify_github is not None:
            open_issues = [
                issue for issue in read_github_issues(verify_github, github_json)
                if str(issue.get("state", "")).upper() == "OPEN"
            ]
            if open_issues:
                titles = ", ".join(str(issue.get("title", issue.get("number", "unknown"))) for issue in open_issues)
                raise ReadinessError(f"not ready: open sampler-performance milestone issues: {titles}")
            issues = {"status": "closed", "repo": verify_github, "open": []}

        distribution_correctness = validate_expanded_correctness()
        historical_406 = validate_historical_406()
        return {
            "status": "ready",
            "catalog_path": str(catalog_path),
            "catalog_sha256": sha256_file(catalog_path),
            "bundle_count": len(bundles),
            "bundle_ids": list(EXPECTED_BUNDLE_IDS),
            "portable_bundles": portable_bundles,
            "reference_build": {
                "direct_speedup": reference_result["direct_speedup"],
                "direct_canonical_materializations": direct_canonical_materializations,
                "direct_executed_repeat_iterations": direct_executed_repeat_iterations,
                "direct_skipped_repeat_iterations": counters["skipped_repeat_iterations"],
                "canonical_variant": "rstim-canonical-reference-b8",
                "direct_variant": "rstim-direct-repeat-reference-b8",
                "summary_path": repository_relative_path(paths["reference-build-release"] / "summary.json"),
                "raw_path": repository_relative_path(paths["reference-build-release"] / "raw.jsonl"),
                "report_path": repository_relative_path(paths["reference-build-release"] / "report.md"),
            },
            "fair_cli": {
                **fair_result,
                "ratio_delta": fair_result["candidate_rstim_over_stim"] - fair_result["baseline_rstim_over_stim"],
                "comparison_path": repository_relative_path(paths["fair-cli-release"] / "comparison.json"),
                "summary_path": repository_relative_path(paths["fair-cli-release"] / "summary.json"),
                "report_path": repository_relative_path(paths["fair-cli-release"] / "report.md"),
            },
            "frame_noise": {
                **frame_result,
                "correctness_status": frame_correctness["status"],
                "paired_summary_path": repository_relative_path(paths["frame-instruction-wide-release"] / "paired-summary.json"),
                "correctness_path": repository_relative_path(paths["frame-instruction-wide-release"] / "correctness-summary.json"),
                "report_path": repository_relative_path(paths["frame-instruction-wide-release"] / "report.md"),
            },
            "distribution_correctness": distribution_correctness,
            "historical_406": historical_406,
            "focused_rust_tests": ["cargo", "test", "-p", "rstim", "--test", "reusable_compiled_measurement_sampler", "--test", "packed_inverse_tableau_storage", "--test", "packed_inverse_tableau_clifford", "--test", "packed_inverse_tableau_measurement", "--test", "packed_inverse_direct_collapse", "--test", "packed_reference_routing", "--test", "reference_sample_tree", "--test", "repeat_aware_reference_sample", "--test", "rare_error_iterator", "--test", "frame_instruction_wide_one_qubit_noise", "--test", "frame_instruction_wide_depolarize2"],
            "claim_limits": [
                "Readiness is limited to the committed evidence bundles and focused Rust tests.",
                "This is not a broad Stim parity claim and does not close #406.",
                "Site-facing #379 remains separate; this readiness artifact does not update the site or close #379.",
            ],
            "issues": {str(number): f"{ISSUE_BASE_URL}/{number}" for number in (38, 406, 379)} | {"milestone": issues},
        }
    except ReadinessError:
        raise
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
        raise ReadinessError(f"not ready: {error}") from error


def render_markdown(readiness: dict[str, object]) -> str:
    bundles = readiness["portable_bundles"]
    reference = readiness["reference_build"]
    frame = readiness["frame_noise"]
    distribution = readiness["distribution_correctness"]
    historical = readiness["historical_406"]
    issues = readiness["issues"]
    lines = ["# Sampler Performance Readiness", "", f"Status: **{readiness['status']}**", "", "## Evidence Bundles"]
    for bundle in bundles:  # type: ignore[union-attr]
        lines.append(f"- [{bundle['id']}]({bundle['bundle_path']}/): {bundle['pass_line']}")
    lines.extend([
        "", "## Readiness Checks",
        f"- Reference direct/canonical speedup: `{reference['direct_speedup']}`x (minimum `2.0x`).",  # type: ignore[index]
        f"- Direct reference canonical materializations: `{reference['direct_canonical_materializations']}`; executed repeat iterations: `{reference['direct_executed_repeat_iterations']}`.",  # type: ignore[index]
        f"- Frame candidate/baseline ratio: `{frame['candidate_over_baseline']}` (maximum `1.05`); correctness: `{frame['correctness_status']}`.",  # type: ignore[index]
        f"- Distribution correctness: `{distribution['status']}` across `{distribution['case_count']}` cases.",  # type: ignore[index]
        f"- Historical #406 evidence: `{historical['status']}` (`{historical['stim_cli_over_rstim_compiled']:.2f}`x stim-cli/rstim-compiled).",  # type: ignore[index]
        "", "## Claim Limits",
    ])
    lines.extend(f"- {limit}" for limit in readiness["claim_limits"])  # type: ignore[union-attr]
    lines.extend(["", "## Issue Links"])
    for number in (38, 406, 379):
        lines.append(f"- [#{number}]({issues[str(number)]})")  # type: ignore[index]
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check sampler performance readiness evidence.")
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--markdown-out", type=Path, default=REPO_ROOT / "sampler-performance-readiness.md")
    parser.add_argument("--verify-github")
    parser.add_argument("--github-json", type=Path)
    args = parser.parse_args(argv)
    try:
        readiness = build_readiness(args.catalog, args.verify_github, args.github_json)
        args.out.write_text(json.dumps(readiness, indent=2) + "\n", encoding="utf-8")
        args.markdown_out.write_text(render_markdown(readiness), encoding="utf-8")
    except ReadinessError as error:
        print(error, file=sys.stderr)
        return 1
    print(PASS_LINE)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
