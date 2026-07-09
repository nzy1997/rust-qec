#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools.check_rstim_vs_stim_gap_artifact import (
    load_json,
    recorded_manifest_sha256,
    sha256_file,
    validate_case,
)


DEFAULT_OLD_SUMMARY = Path("benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json")
DEFAULT_NEW_DIR = Path("benchmarks/rstim_vs_stim_simulator/results/release")
DEFAULT_DOCS_PATH = Path("docs/showcases/rstim-vs-stim-simulator.md")
DEFAULT_MANIFEST_PATH = Path("site/benchmark-site.json")
SELECTED_CASE_LABEL = "stim-style-surface-sample-d11-r100-b1024"
REQUIRED_RELEASE_FILES = ("summary.json", "report.md", "environment.json")
BROAD_CLAIM_PATTERNS = (
    "broad rstim/stim performance parity",
    "all-workload parity",
    "all workloads",
)
FAMILY_ID = "rstim-vs-stim-simulator"
FULL_ITEM_ID = "rstim-vs-stim-full"
RELEASE_ITEM_ID = "rstim-vs-stim-release"


def require_dict(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} is not a JSON object")
    return value


def validate_release_files(new_dir: Path) -> tuple[Path, Path, Path]:
    paths: list[Path] = []
    for filename in REQUIRED_RELEASE_FILES:
        path = new_dir / filename
        if not path.is_file():
            raise ValueError(f"missing required release file: {filename}")
        paths.append(path)
    return paths[0], paths[1], paths[2]


def validate_new_summary(summary: dict[str, Any]) -> None:
    cases = summary.get("cases")
    if not isinstance(cases, list) or len(cases) != 1:
        raise ValueError("new summary must contain exactly one case")
    case = require_dict(cases[0], "new summary case")
    if case.get("case_label") != SELECTED_CASE_LABEL:
        raise ValueError("new summary selected case label changed")
    if case.get("workload") != "sample":
        raise ValueError("new summary workload changed")
    if case.get("tier") != "report_only":
        raise ValueError("new summary tier changed")
    present_variants = case.get("present_variants")
    if not isinstance(present_variants, list) or "rstim-compiled" not in present_variants:
        raise ValueError("new summary missing rstim-compiled present variant")


def validate_environment(environment: dict[str, Any]) -> None:
    if environment.get("profile") != "release":
        raise ValueError("environment.json profile must be release")

    evidence_kind = environment.get("evidence_kind")
    if not isinstance(evidence_kind, str) or "post-optimization" not in evidence_kind:
        raise ValueError("environment.json evidence_kind must contain post-optimization")

    required_fields = (
        "rstim_binary_path",
        "rustc_version",
        "cargo_version",
        "stim_cli_status",
    )
    for field in required_fields:
        if field not in environment:
            raise ValueError(f"environment.json missing {field}")
        if not isinstance(environment[field], str) or environment[field].strip() == "":
            raise ValueError(f"environment.json missing {field}")

    stim_cli_status = environment["stim_cli_status"]
    stim_cli_version = environment.get("stim_cli_version")
    has_stim_version = isinstance(stim_cli_version, str) and stim_cli_version.strip() != ""
    if stim_cli_status == "ok" and not has_stim_version:
        raise ValueError("environment.json stim_cli_version is empty for ok Stim CLI")

    stim_stderr = environment.get("stim_cli.stderr")
    has_stim_stderr = isinstance(stim_stderr, str) and stim_stderr != ""
    stim_cli = environment.get("stim_cli")
    if not has_stim_stderr and isinstance(stim_cli, dict):
        nested_stderr = stim_cli.get("stderr")
        has_stim_stderr = isinstance(nested_stderr, str) and nested_stderr != ""
    if not has_stim_version and not has_stim_stderr:
        raise ValueError("environment.json missing stim_cli_version or stim_cli.stderr")


def validate_report(report_path: Path) -> None:
    report_text = report_path.read_text(encoding="utf-8")
    if "report-only Stim comparison" not in report_text:
        raise ValueError("report.md missing report-only Stim comparison")
    if SELECTED_CASE_LABEL not in report_text:
        raise ValueError("report.md missing selected case label")


def validate_docs(docs_path: Path) -> None:
    docs_text = docs_path.read_text(encoding="utf-8")
    required_refs = (
        str(DEFAULT_OLD_SUMMARY),
        str(DEFAULT_NEW_DIR / "summary.json"),
    )
    for ref in required_refs:
        if ref not in docs_text:
            raise ValueError(f"docs missing artifact link {ref}")

    lower_docs = docs_text.lower()
    for pattern in BROAD_CLAIM_PATTERNS:
        if pattern in lower_docs:
            raise ValueError("docs contain forbidden broad parity wording")


def validate_old_summary_hash(old_summary_path: Path, manifest: dict[str, Any], repo_root: Path) -> None:
    candidates: list[str] = []
    try:
        candidates.append(str(old_summary_path.resolve().relative_to(repo_root.resolve())))
    except ValueError:
        pass
    candidates.extend([str(old_summary_path), str(DEFAULT_OLD_SUMMARY)])

    recorded: str | None = None
    for artifact_path in dict.fromkeys(candidates):
        recorded = recorded_manifest_sha256(manifest, artifact_path)
        if recorded is not None:
            break
    if recorded is None:
        raise ValueError("site manifest missing recorded hash for old #406 summary")
    if sha256_file(old_summary_path) != recorded:
        raise ValueError("checked artifact hash differs from site manifest")


def validate_no_broad_claim_text(value: Any, label: str) -> None:
    text = json_dumps_lower(value)
    for pattern in BROAD_CLAIM_PATTERNS:
        if pattern in text:
            raise ValueError(f"{label} contains forbidden broad parity wording")


def json_dumps_lower(value: Any) -> str:
    import json

    return json.dumps(value, sort_keys=True).lower()


def find_family(manifest: dict[str, Any], family_id: str) -> dict[str, Any]:
    families = manifest.get("families")
    if not isinstance(families, list):
        raise ValueError("site manifest missing families")
    for family in families:
        if isinstance(family, dict) and family.get("id") == family_id:
            return family
    raise ValueError(f"site manifest missing family {family_id}")


def find_evidence_item(family: dict[str, Any], item_id: str) -> dict[str, Any]:
    items = family.get("evidence_items")
    if not isinstance(items, list):
        raise ValueError(f"site manifest missing evidence item {item_id}")
    for item in items:
        if isinstance(item, dict) and item.get("id") == item_id:
            return item
    raise ValueError(f"site manifest missing evidence item {item_id}")


def artifact_paths(item: dict[str, Any]) -> set[str]:
    artifacts = item.get("artifacts")
    if not isinstance(artifacts, list):
        return set()
    paths: set[str] = set()
    for artifact in artifacts:
        if isinstance(artifact, dict):
            path = artifact.get("path")
            if isinstance(path, str):
                paths.add(path)
    return paths


def recorded_hashes(item: dict[str, Any]) -> dict[str, Any]:
    provenance = item.get("provenance")
    if not isinstance(provenance, dict):
        raise ValueError("site manifest release item missing provenance")
    artifact_hashes = provenance.get("artifact_hashes")
    if not isinstance(artifact_hashes, dict) or artifact_hashes.get("status") != "recorded":
        raise ValueError("site manifest release item missing recorded artifact hashes")
    value = artifact_hashes.get("value")
    if not isinstance(value, dict):
        raise ValueError("site manifest release item missing recorded artifact hashes")
    return value


def validate_manifest(manifest: dict[str, Any], repo_root: Path) -> None:
    validate_no_broad_claim_text(manifest, "site manifest")
    family = find_family(manifest, FAMILY_ID)
    find_evidence_item(family, FULL_ITEM_ID)
    release_item = find_evidence_item(family, RELEASE_ITEM_ID)

    required_release_artifacts = {
        str(DEFAULT_NEW_DIR / "summary.json"),
        str(DEFAULT_NEW_DIR / "report.md"),
        str(DEFAULT_NEW_DIR / "environment.json"),
    }
    paths = artifact_paths(release_item)
    missing_artifacts = sorted(required_release_artifacts - paths)
    if missing_artifacts:
        raise ValueError(f"site manifest release item missing artifact {missing_artifacts[0]}")

    hashes = recorded_hashes(release_item)
    for artifact in sorted(required_release_artifacts):
        entry = hashes.get(artifact)
        if not isinstance(entry, dict):
            raise ValueError(f"site manifest missing recorded hash for {artifact}")
        recorded_sha = entry.get("sha256")
        if not isinstance(recorded_sha, str):
            raise ValueError(f"site manifest missing recorded hash for {artifact}")
        actual_sha = sha256_file(repo_root / artifact)
        if actual_sha != recorded_sha:
            raise ValueError(f"site manifest hash mismatch for {artifact}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--old", default=str(DEFAULT_OLD_SUMMARY))
    parser.add_argument("--new-dir", default=str(DEFAULT_NEW_DIR))
    parser.add_argument("--docs", default=str(DEFAULT_DOCS_PATH))
    parser.add_argument("--manifest", default=str(DEFAULT_MANIFEST_PATH))
    args = parser.parse_args(argv)

    old_summary_path = Path(args.old)
    new_dir = Path(args.new_dir)
    docs_path = Path(args.docs)
    manifest_path = Path(args.manifest)

    try:
        summary_path, report_path, environment_path = validate_release_files(new_dir)
        if sha256_file(old_summary_path) == sha256_file(summary_path):
            raise ValueError("new summary reuses the checked #406 summary")

        old_summary = require_dict(load_json(old_summary_path), "old summary")
        validate_case(old_summary)
        manifest = require_dict(load_json(manifest_path), "site manifest")
        validate_old_summary_hash(old_summary_path, manifest, Path.cwd())

        new_summary = require_dict(load_json(summary_path), "new summary")
        validate_new_summary(new_summary)
        environment = require_dict(load_json(environment_path), "environment.json")
        validate_environment(environment)
        validate_report(report_path)
        validate_docs(docs_path)
        validate_manifest(manifest, Path.cwd())
    except Exception as exc:
        print(f"ERROR post-optimization evidence check failed: {exc}", file=sys.stderr)
        return 1

    print("PASS post-optimization evidence is separate from the checked #406 artifact")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
