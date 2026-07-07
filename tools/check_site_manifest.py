#!/usr/bin/env python3
"""Validate the benchmark site manifest and checked-artifact policy."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ALLOWED_STATUSES = {"existing", "partial", "future", "local-only"}
REQUIRED_FAMILY_IDS = {
    "surface-decoder-comparison",
    "bb-circuit-bposd-comparison",
    "qec-code-random-window",
    "rstim-vs-stim-simulator",
    "internal-regression-evidence",
}
FAMILY_REQUIRED_FIELDS = {"id", "title", "status", "source_docs", "claims_limit", "evidence_items"}
ITEM_REQUIRED_FIELDS = {
    "id",
    "title",
    "status",
    "tier",
    "artifacts",
    "commands",
    "provenance_requirements",
    "provenance_sources",
    "claims_limit",
}
CHECKED_ARTIFACT_REFERENCE_RE = re.compile(
    r"benchmarks/(?:surface_decoder_compare|bb_circuit_bposd_compare)/results/full/[A-Za-z0-9._/-]+"
)
PROVENANCE_SCHEMA_VERSION = 1
PROVENANCE_REQUIRED_FIELDS = (
    "schema_version",
    "artifact_date",
    "source_commit",
    "commands",
    "os",
    "cpu_model",
    "rust_version",
    "python_version",
    "dependency_versions",
    "external_repository_commits",
    "seed_policy",
    "build_profile",
    "shots_or_error_budget",
    "artifact_hashes",
)


def git_ok(repo_root: Path, args: list[str]) -> bool:
    result = subprocess.run(
        ["git", *args],
        cwd=repo_root,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def path_is_tracked(repo_root: Path, relative: str) -> bool:
    return git_ok(repo_root, ["ls-files", "--error-unmatch", "--", relative])


def path_is_ignored(repo_root: Path, relative: str) -> bool:
    return git_ok(repo_root, ["check-ignore", "--no-index", "-q", "--", relative])


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def add_error(errors: list[str], scope: str, message: str) -> None:
    errors.append(f"{scope}: {message}")


def is_non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value)


def validate_non_empty_string(scope: str, field: str, value: Any, errors: list[str]) -> str | None:
    if not is_non_empty_string(value):
        add_error(errors, scope, f"{field} must be a non-empty string")
        return None
    return value


def validate_repo_path(repo_root: Path, relative: str, scope: str, label: str, errors: list[str]) -> None:
    candidate = repo_root / relative
    if not candidate.exists():
        add_error(errors, scope, f"{label} {relative} does not exist")
        return
    if path_is_ignored(repo_root, relative):
        add_error(errors, scope, f"{label} {relative} is ignored")
        return
    if not path_is_tracked(repo_root, relative):
        add_error(errors, scope, f"{label} {relative} is not tracked by git")


def iter_checked_artifact_paths(manifest: dict[str, Any]) -> list[tuple[str, str]]:
    paths: list[tuple[str, str]] = []
    for family in manifest.get("families", []):
        if not isinstance(family, dict):
            continue
        for item in family.get("evidence_items", []):
            if not isinstance(item, dict):
                continue
            item_id = item.get("id", "<missing>")
            for artifact in item.get("artifacts", []):
                if not isinstance(artifact, dict):
                    continue
                if artifact.get("checked") is True and isinstance(artifact.get("path"), str):
                    paths.append((item_id, artifact["path"]))
    return paths


def validate_site_paths(site_root: Path, manifest: dict[str, Any], errors: list[str]) -> None:
    for item_id, artifact_path in iter_checked_artifact_paths(manifest):
        copied = site_root / artifact_path
        if not copied.is_file():
            add_error(
                errors,
                f"evidence item {item_id}",
                f"checked artifact {artifact_path} was not copied to {site_root}",
            )


def validate_artifact(repo_root: Path, family_id: str, item_id: str, artifact: dict[str, Any], errors: list[str]) -> None:
    scope = f"evidence item {item_id}"
    missing = [field for field in ("path", "kind", "checked") if field not in artifact]
    if missing:
        add_error(errors, scope, f"artifact missing required field {missing[0]}")
        return

    artifact_path = validate_non_empty_string(scope, "artifact path", artifact["path"], errors)
    validate_non_empty_string(scope, "artifact kind", artifact["kind"], errors)
    if artifact["checked"] is not True:
        add_error(errors, scope, f"artifact {artifact.get('path')} must set checked=True")
        return
    if artifact_path is None:
        return

    validate_repo_path(repo_root, artifact_path, scope, "artifact", errors)


def item_has_checked_artifacts(item: dict[str, Any]) -> bool:
    artifacts = item.get("artifacts")
    if not isinstance(artifacts, list):
        return False
    return any(isinstance(artifact, dict) and artifact.get("checked") is True for artifact in artifacts)


def validate_provenance_status_field(scope: str, provenance: dict[str, Any], field: str, errors: list[str]) -> None:
    if field not in provenance:
        add_error(errors, scope, f"provenance missing required field {field}")
        return

    entry = provenance[field]
    if not isinstance(entry, dict):
        add_error(errors, scope, f"provenance.{field} must be an object")
        return

    status = entry.get("status")
    if status == "recorded":
        if "value" not in entry:
            add_error(errors, scope, f"provenance.{field} recorded entry must include value")
        return

    if status == "not_recorded":
        if not is_non_empty_string(entry.get("reason")):
            add_error(errors, scope, f"provenance.{field} not_recorded entry must include non-empty reason")
        return

    add_error(errors, scope, f"provenance.{field} status must be 'recorded' or 'not_recorded'")


def validate_checked_item_provenance(scope: str, item: dict[str, Any], errors: list[str]) -> None:
    if not item_has_checked_artifacts(item):
        return

    provenance = item.get("provenance")
    if not isinstance(provenance, dict):
        add_error(errors, scope, "provenance must be an object")
        return

    schema_version = provenance.get("schema_version")
    if type(schema_version) is not int or schema_version != PROVENANCE_SCHEMA_VERSION:
        add_error(errors, scope, f"provenance.schema_version must be {PROVENANCE_SCHEMA_VERSION}")

    for field in PROVENANCE_REQUIRED_FIELDS:
        if field == "schema_version":
            continue
        validate_provenance_status_field(scope, provenance, field, errors)


def validate_string_list(scope: str, label: str, values: Any, errors: list[str], *, allow_empty: bool) -> None:
    if not isinstance(values, list):
        add_error(errors, scope, f"{label} must be a list")
        return
    if not values and not allow_empty:
        add_error(errors, scope, f"{label} must not be empty")
        return
    for value in values:
        if not isinstance(value, str):
            add_error(errors, scope, f"{label} entries must be strings")


def validate_path_list(
    repo_root: Path,
    scope: str,
    label: str,
    paths: Any,
    errors: list[str],
    *,
    allow_empty: bool = False,
) -> None:
    if not isinstance(paths, list):
        add_error(errors, scope, f"{label} must be a list")
        return
    if not paths and not allow_empty:
        add_error(errors, scope, f"{label} must not be empty")
        return
    for path in paths:
        if not isinstance(path, str):
            add_error(errors, scope, f"{label} entries must be strings")
            continue
        validate_repo_path(repo_root, path, scope, label, errors)


def validate_item(repo_root: Path, family_id: str, item: Any, errors: list[str]) -> None:
    if not isinstance(item, dict):
        add_error(errors, f"family {family_id}", "evidence item must be an object")
        return

    item_id = item.get("id", "<missing>")
    scope = f"evidence item {item_id}"

    for field in ITEM_REQUIRED_FIELDS:
        if field not in item:
            add_error(errors, scope, f"missing required field {field}")
    validate_non_empty_string(scope, "id", item.get("id"), errors)
    validate_non_empty_string(scope, "title", item.get("title"), errors)
    validate_non_empty_string(scope, "claims_limit", item.get("claims_limit"), errors)
    if not isinstance(item.get("status"), str) or item["status"] not in ALLOWED_STATUSES:
        add_error(errors, scope, f"invalid status {item.get('status')!r}")
    if not isinstance(item.get("tier"), str) or not item["tier"]:
        add_error(errors, scope, "tier must be a non-empty string")
    validate_string_list(scope, "commands", item.get("commands"), errors, allow_empty=True)
    validate_string_list(
        scope,
        "provenance_requirements",
        item.get("provenance_requirements"),
        errors,
        allow_empty=False,
    )
    if "caveats" in item:
        validate_string_list(scope, "caveats", item.get("caveats"), errors, allow_empty=False)

    if item.get("status") in {"local-only", "future"} and item.get("artifacts"):
        add_error(errors, scope, f"{item['status']} evidence item must not list checked artifacts")

    validate_path_list(repo_root, scope, "provenance_sources", item.get("provenance_sources"), errors)

    artifacts = item.get("artifacts")
    if isinstance(artifacts, list):
        for artifact in artifacts:
            if isinstance(artifact, dict):
                validate_artifact(repo_root, family_id, item_id, artifact, errors)
            else:
                add_error(errors, scope, "artifact entries must be objects")
    else:
        add_error(errors, scope, "artifacts must be a list")
    validate_checked_item_provenance(scope, item, errors)


def validate_family(repo_root: Path, family: Any, errors: list[str]) -> str | None:
    if not isinstance(family, dict):
        add_error(errors, "manifest", "family entry must be an object")
        return None

    family_id = family.get("id", "<missing>")
    scope = f"family {family_id}"

    for field in FAMILY_REQUIRED_FIELDS:
        if field not in family:
            add_error(errors, scope, f"missing required field {field}")
    family_id_value = validate_non_empty_string(scope, "id", family.get("id"), errors)
    validate_non_empty_string(scope, "title", family.get("title"), errors)
    validate_non_empty_string(scope, "claims_limit", family.get("claims_limit"), errors)

    if not isinstance(family.get("status"), str) or family["status"] not in ALLOWED_STATUSES:
        add_error(errors, scope, f"invalid status {family.get('status')!r}")

    validate_path_list(repo_root, scope, "source_docs", family.get("source_docs"), errors)

    evidence_items = family.get("evidence_items")
    if isinstance(evidence_items, list):
        seen_items: set[str] = set()
        for item in evidence_items:
            if isinstance(item, dict) and isinstance(item.get("id"), str):
                item_id = item["id"]
                if item_id in seen_items:
                    add_error(errors, scope, f"duplicate evidence item id {item_id}")
                seen_items.add(item_id)
            validate_item(repo_root, family_id, item, errors)
    else:
        add_error(errors, scope, "evidence_items must be a list")

    return family_id_value


def validate_manifest(repo_root: Path, manifest_path: Path, site_root: Path | None = None) -> list[str]:
    errors: list[str] = []
    try:
        manifest = load_json(manifest_path)
    except FileNotFoundError:
        return [f"manifest does not exist: {manifest_path}"]
    except json.JSONDecodeError as exc:
        return [f"manifest JSON parse error: {exc.msg}"]

    if not isinstance(manifest, dict):
        return ["manifest must be a JSON object"]

    if manifest.get("schema_version") != 1:
        add_error(errors, "manifest", "schema_version must be 1")

    families = manifest.get("families")
    if not isinstance(families, list):
        add_error(errors, "manifest", "families must be a list")
        return errors

    seen: set[str] = set()
    seen_items: set[str] = set()
    family_ids: list[str] = []
    for family in families:
        family_id = validate_family(repo_root, family, errors)
        if family_id is None:
            continue
        family_ids.append(family_id)
        if family_id in seen:
            add_error(errors, "manifest", f"duplicate family id {family_id}")
        seen.add(family_id)
        if isinstance(family, dict) and isinstance(family.get("evidence_items"), list):
            for item in family["evidence_items"]:
                if not isinstance(item, dict) or not isinstance(item.get("id"), str):
                    continue
                item_id = item["id"]
                if item_id in seen_items:
                    add_error(errors, "manifest", f"duplicate evidence item id {item_id}")
                seen_items.add(item_id)

    missing = sorted(REQUIRED_FAMILY_IDS - set(family_ids))
    for family_id in missing:
        add_error(errors, "manifest", f"missing required family {family_id}")

    extra = sorted(set(family_ids) - REQUIRED_FAMILY_IDS)
    for family_id in extra:
        add_error(errors, "manifest", f"unexpected family {family_id}")

    if site_root is not None and not errors:
        validate_site_paths(site_root, manifest, errors)

    return errors


def validate_site_root(site_root: Path, manifest_path: Path) -> list[str]:
    errors: list[str] = []
    scope = "site root"
    index_path = site_root / "index.html"
    app_path = site_root / "app.js"
    expected_manifest = site_root / "data/benchmark-site.json"

    for path, label in [
        (index_path, "index.html"),
        (app_path, "app.js"),
        (expected_manifest, "data/benchmark-site.json"),
    ]:
        if not path.is_file():
            add_error(errors, scope, f"missing built site file {label}")

    if manifest_path.resolve() != expected_manifest.resolve():
        add_error(errors, scope, "manifest path must be _site/data/benchmark-site.json when --site-root is used")

    index = index_path.read_text(encoding="utf-8") if index_path.is_file() else ""
    app = app_path.read_text(encoding="utf-8") if app_path.is_file() else ""

    for marker in [
        'id="benchmarks"',
        'id="benchmark-manifest"',
        'id="checked-benchmark-results"',
        'id="checked-benchmark-result-cards"',
        "Benchmark Methodology",
        "Claims Policy",
    ]:
        if marker not in index:
            add_error(errors, scope, f"index.html missing benchmark marker {marker}")

    required_app_markers = [
        'fetch("data/benchmark-site.json")',
        "renderBenchmarkManifest",
        "family.status",
        "family.claims_limit",
        "item.status",
        "item.claims_limit",
    ]
    missing_app_markers = [marker for marker in required_app_markers if marker not in app]
    if missing_app_markers:
        add_error(errors, scope, f"app.js missing manifest status and claims_limit wiring: {missing_app_markers}")

    checked_result_markers = [
        "checkedBenchmarkItems",
        "renderCheckedBenchmarkResults",
        "item.artifacts",
        "item.commands",
        "item.caveats",
        "artifact.checked",
        'artifact.kind === "image"',
    ]
    missing_checked_markers = [marker for marker in checked_result_markers if marker not in app]
    if missing_checked_markers:
        add_error(errors, scope, f"app.js missing checked result rendering: {missing_checked_markers}")

    try:
        manifest = load_json(manifest_path)
    except (FileNotFoundError, json.JSONDecodeError):
        manifest = None
    if isinstance(manifest, dict):
        validate_site_artifact_references(site_root, manifest, errors)

    return errors


def validate_site_artifact_references(site_root: Path, manifest: dict[str, Any], errors: list[str]) -> None:
    checked_paths = {artifact_path for _, artifact_path in iter_checked_artifact_paths(manifest)}
    for relative in ("index.html", "app.js"):
        path = site_root / relative
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for match in sorted(set(CHECKED_ARTIFACT_REFERENCE_RE.findall(text))):
            if match not in checked_paths:
                add_error(
                    errors,
                    "site root",
                    f"{relative} references checked artifact {match} that is not listed as a checked manifest artifact",
                )


def make_fixture_repo() -> tuple[tempfile.TemporaryDirectory[str], Path, Path]:
    tmpdir = tempfile.TemporaryDirectory()
    root = Path(tmpdir.name)

    (root / ".gitignore").write_text("/benchmarks/out/\n", encoding="utf-8")
    (root / "docs/showcases").mkdir(parents=True)
    (root / "benchmarks/surface_decoder_compare/results/full").mkdir(parents=True)
    (root / "benchmarks/qec_code_random_window").mkdir(parents=True)
    (root / ".github/workflows").mkdir(parents=True)
    (root / "site").mkdir(parents=True)
    (root / "benchmarks/out").mkdir(parents=True)

    (root / "docs/showcases/benchmark-evidence.md").write_text("# Benchmark Evidence\n", encoding="utf-8")
    (root / "benchmarks/surface_decoder_compare/results/full/results.csv").write_text("distance,shots\n", encoding="utf-8")
    (root / "benchmarks/qec_code_random_window/README.md").write_text("# Random Window\n", encoding="utf-8")
    (root / ".github/workflows/ci.yml").write_text("name: ci\n", encoding="utf-8")
    (root / "benchmarks/out/ignored.csv").write_text("ignored\n", encoding="utf-8")

    def fixture_provenance(commands: list[str]) -> dict[str, Any]:
        return {
            "schema_version": 1,
            "artifact_date": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "source_commit": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "commands": {"status": "recorded", "value": commands},
            "os": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "cpu_model": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "rust_version": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "python_version": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "dependency_versions": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "external_repository_commits": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "seed_policy": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "build_profile": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "shots_or_error_budget": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
            "artifact_hashes": {"status": "not_recorded", "reason": "historical fixture predates canonical provenance capture"},
        }

    manifest = {
        "schema_version": 1,
        "families": [
            {
                "id": "surface-decoder-comparison",
                "title": "Surface Decoder Comparison",
                "status": "existing",
                "source_docs": ["docs/showcases/benchmark-evidence.md"],
                "claims_limit": "Checked full artifacts are committed-run evidence, not a general decoder ordering claim.",
                "evidence_items": [
                    {
                        "id": "surface-decoder-full",
                        "title": "Checked surface-decoder full artifacts",
                        "status": "existing",
                        "tier": "full",
                        "artifacts": [
                            {
                                "path": "benchmarks/surface_decoder_compare/results/full/results.csv",
                                "kind": "csv",
                                "checked": True,
                            }
                        ],
                        "commands": ["make surface-decoder-compare-full"],
                        "provenance": fixture_provenance(["make surface-decoder-compare-full"]),
                        "provenance_requirements": ["command line", "date"],
                        "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                        "claims_limit": "Fixture claim limit.",
                    }
                ],
            },
            {
                "id": "bb-circuit-bposd-comparison",
                "title": "BB Circuit BP-OSD Comparison",
                "status": "partial",
                "source_docs": ["docs/showcases/benchmark-evidence.md"],
                "claims_limit": "BB72/BB144 only.",
                "evidence_items": [
                    {
                        "id": "bb-circuit-full",
                        "title": "Checked BB full artifacts",
                        "status": "existing",
                        "tier": "full",
                        "artifacts": [],
                        "commands": ["make bb-circuit-bposd-compare-full"],
                        "provenance": fixture_provenance(["make bb-circuit-bposd-compare-full"]),
                        "provenance_requirements": ["command line", "date"],
                        "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                        "claims_limit": "Fixture claim limit.",
                    }
                ],
            },
            {
                "id": "qec-code-random-window",
                "title": "qec-code Random Window",
                "status": "local-only",
                "source_docs": ["benchmarks/qec_code_random_window/README.md"],
                "claims_limit": "Generated outputs are ignored local evidence.",
                "evidence_items": [
                    {
                        "id": "qec-code-smoke",
                        "title": "Local smoke command",
                        "status": "local-only",
                        "tier": "smoke",
                        "artifacts": [],
                        "commands": ["make qec-code-random-window-bench-smoke"],
                        "provenance_requirements": ["command line", "date"],
                        "provenance_sources": ["benchmarks/qec_code_random_window/README.md"],
                        "claims_limit": "Local wiring check only.",
                    }
                ],
            },
            {
                "id": "rstim-vs-stim-simulator",
                "title": "rstim versus Stim Simulator",
                "status": "future",
                "source_docs": ["docs/showcases/benchmark-evidence.md"],
                "claims_limit": "No current site-facing benchmark artifacts.",
                "evidence_items": [
                    {
                        "id": "rstim-stim-future",
                        "title": "Future simulator benchmark",
                        "status": "future",
                        "tier": "future",
                        "artifacts": [],
                        "commands": [],
                        "provenance_requirements": ["command line", "date"],
                        "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                        "claims_limit": "Planning entry only.",
                    }
                ],
            },
            {
                "id": "internal-regression-evidence",
                "title": "Internal Regression Evidence",
                "status": "partial",
                "source_docs": [".github/workflows/ci.yml"],
                "claims_limit": "Regression gate evidence only.",
                "evidence_items": [
                    {
                        "id": "rstim-perf-ci",
                        "title": "rstim perf CI",
                        "status": "partial",
                        "tier": "regression-gate",
                        "artifacts": [],
                        "commands": ["cargo run -p rstim --bin rstim -- perf ci --out-dir perf-artifacts"],
                        "provenance_requirements": ["command line", "date"],
                        "provenance_sources": [".github/workflows/ci.yml"],
                        "claims_limit": "Regression gate evidence only.",
                    }
                ],
            },
        ],
    }

    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    manifest_path = root / "site/benchmark-site.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    subprocess.run(
        [
            "git",
            "add",
            ".gitignore",
            "docs/showcases/benchmark-evidence.md",
            "benchmarks/surface_decoder_compare/results/full/results.csv",
            "benchmarks/qec_code_random_window/README.md",
            ".github/workflows/ci.yml",
            "site/benchmark-site.json",
        ],
        cwd=root,
        check=True,
    )
    return tmpdir, root, manifest_path


def run_self_test() -> list[str]:
    tmpdir, repo_root, manifest_path = make_fixture_repo()
    try:
        valid_errors = validate_manifest(repo_root, manifest_path)
        if valid_errors:
            return [f"self-test: unexpected errors for valid fixture: {valid_errors}"]

        base_manifest = load_json(manifest_path)
        mutations = [
            ("missing_artifact", "surface-decoder-full", "does not exist"),
            ("missing_claims_limit", "surface-decoder-full", "claims_limit"),
            ("ignored_artifact", "surface-decoder-full", "ignored"),
            ("missing_provenance", "surface-decoder-full", "provenance"),
            ("missing_provenance_cpu_model", "surface-decoder-full", "cpu_model"),
            ("provenance_cpu_model_missing_reason", "surface-decoder-full", "reason"),
            ("bad_provenance_schema_version", "surface-decoder-full", "schema_version"),
        ]

        for mutation, entry_id, rule in mutations:
            manifest = json.loads(json.dumps(base_manifest))
            if mutation == "missing_artifact":
                manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = "benchmarks/missing/results.csv"
            elif mutation == "missing_claims_limit":
                del manifest["families"][0]["evidence_items"][0]["claims_limit"]
            elif mutation == "ignored_artifact":
                manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = "benchmarks/out/ignored.csv"
            elif mutation == "missing_provenance":
                del manifest["families"][0]["evidence_items"][0]["provenance"]
            elif mutation == "missing_provenance_cpu_model":
                del manifest["families"][0]["evidence_items"][0]["provenance"]["cpu_model"]
            elif mutation == "provenance_cpu_model_missing_reason":
                manifest["families"][0]["evidence_items"][0]["provenance"]["cpu_model"] = {"status": "not_recorded"}
            elif mutation == "bad_provenance_schema_version":
                manifest["families"][0]["evidence_items"][0]["provenance"]["schema_version"] = 2

            manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
            mutated_errors = validate_manifest(repo_root, manifest_path)
            if not any(entry_id in error and rule in error for error in mutated_errors):
                return [f"self-test: mutation {mutation} did not reject {entry_id} with {rule}: {mutated_errors}"]

        return []
    finally:
        tmpdir.cleanup()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate the benchmark site manifest.")
    parser.add_argument("--self-test", action="store_true", help="Run the validator self-test")
    parser.add_argument("--repo-root", type=Path, default=Path("."), help="Repository root for git checks")
    parser.add_argument(
        "--site-root",
        type=Path,
        help="Built site root for copied artifact and status/claims-limit wiring checks",
    )
    parser.add_argument("manifest", nargs="?", type=Path, help="Path to site/benchmark-site.json")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        if args.manifest is not None:
            print("error: --self-test does not accept a manifest path", file=sys.stderr)
            return 1
        errors = run_self_test()
        if errors:
            for error in errors:
                print(f"error: {error}", file=sys.stderr)
            return 1
        print("ok: self-test")
        return 0

    if args.manifest is None:
        print("error: manifest path is required unless --self-test is used", file=sys.stderr)
        return 1

    errors = validate_manifest(args.repo_root, args.manifest, site_root=args.site_root)
    if args.site_root is not None:
        errors.extend(validate_site_root(args.site_root, args.manifest))
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1

    manifest = load_json(args.manifest)
    for family in manifest.get("families", []):
        print(f"ok: family {family['id']} status={family['status']} items={len(family['evidence_items'])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
