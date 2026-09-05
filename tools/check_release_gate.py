#!/usr/bin/env python3
"""Read-only gate for creating a GitHub Release from a tested immutable tag."""

from __future__ import annotations

import argparse
import base64
import fnmatch
import json
import re
import sys
import time
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.parse import quote


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools.check_repository_gate import (
    CheckSpec,
    GITHUB_ACTIONS_APP_ID,
    UnavailableError,
    gh_api_json,
    load_desired_gate,
    require_array,
    require_object,
)


DEFAULT_POLICY = REPO_ROOT / "tools/release_version_policy.json"
DEFAULT_BRANCH_RULESET = REPO_ROOT / "tools/repository_gate_ruleset.json"
DEFAULT_TAG_RULESET = REPO_ROOT / "tools/release_tag_ruleset.json"
FULL_SHA = re.compile(r"^[0-9a-f]{40}$")


@dataclass(frozen=True)
class PackageSpec:
    name: str
    path: str
    synchronized: bool


@dataclass(frozen=True)
class ReleasePolicy:
    tag_pattern: str
    prerelease_policy: str
    ci_workflow_path: str
    packages: tuple[PackageSpec, ...]


@dataclass(frozen=True)
class TagProtectionPolicy:
    include: tuple[str, ...]
    exclude: tuple[str, ...]
    required_rules: frozenset[str]
    bypass_actors: frozenset[tuple[str, int | None, str]]


@dataclass(frozen=True)
class ReleaseResult:
    tag: str
    version: str
    commit: str
    checks: tuple[tuple[str, int], ...]
    synchronized: tuple[tuple[str, str], ...]
    independent: tuple[tuple[str, str], ...]
    protection_sources: tuple[int, ...]
    errors: tuple[str, ...]

    @property
    def passed(self) -> bool:
        return not self.errors


def _read_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {label} {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} root must be an object")
    return value


def load_release_policy(path: Path) -> ReleasePolicy:
    value = _read_json_object(path, "release policy")
    if value.get("schema_version") != 1:
        raise ValueError("release policy schema_version must be 1")
    tag_pattern = value.get("tag_pattern")
    prerelease = value.get("prerelease_policy")
    workflow = value.get("ci_workflow_path")
    if not all(isinstance(item, str) for item in (tag_pattern, prerelease, workflow)):
        raise ValueError("release policy tag_pattern, prerelease_policy, and ci_workflow_path are required")
    if prerelease != "reject":
        raise ValueError("only the explicit prerelease_policy=reject policy is supported")
    try:
        compiled = re.compile(tag_pattern)
    except re.error as error:
        raise ValueError(f"invalid release tag_pattern: {error}") from error
    if "version" not in compiled.groupindex:
        raise ValueError("release tag_pattern must define a named version group")

    packages: list[PackageSpec] = []
    for key, synchronized in (("synchronized", True), ("independent", False)):
        entries = value.get(key)
        if not isinstance(entries, list):
            raise ValueError(f"release policy {key} must be an array")
        for entry in entries:
            if not isinstance(entry, dict):
                raise ValueError(f"release policy {key} entry must be an object")
            name, package_path = entry.get("name"), entry.get("path")
            if not isinstance(name, str) or not isinstance(package_path, str):
                raise ValueError(f"release policy {key} entry needs string name and path")
            packages.append(PackageSpec(name, package_path, synchronized))
    if not packages:
        raise ValueError("release policy must classify workspace packages")
    if len({item.name for item in packages}) != len(packages):
        raise ValueError("release policy contains duplicate package names")
    if len({item.path for item in packages}) != len(packages):
        raise ValueError("release policy contains duplicate package paths")
    return ReleasePolicy(tag_pattern, prerelease, workflow, tuple(packages))


def _actor_tuple(actor: object) -> tuple[str, int | None, str] | None:
    if not isinstance(actor, dict):
        return None
    actor_type, actor_id, mode = actor.get("actor_type"), actor.get("actor_id"), actor.get("bypass_mode")
    if not isinstance(actor_type, str) or not isinstance(mode, str):
        return None
    return actor_type, actor_id if isinstance(actor_id, int) else None, mode


def load_tag_protection_policy(path: Path) -> TagProtectionPolicy:
    value = _read_json_object(path, "tag ruleset")
    if value.get("target") != "tag" or value.get("enforcement") != "active":
        raise ValueError("tag ruleset payload must be active and target tags")
    ref_name = value.get("conditions", {}).get("ref_name")
    if not isinstance(ref_name, dict):
        raise ValueError("tag ruleset payload needs ref_name conditions")
    include, exclude = ref_name.get("include"), ref_name.get("exclude")
    if not isinstance(include, list) or not all(isinstance(item, str) for item in include):
        raise ValueError("tag ruleset include patterns must be strings")
    if not isinstance(exclude, list) or not all(isinstance(item, str) for item in exclude):
        raise ValueError("tag ruleset exclude patterns must be strings")
    rules = value.get("rules")
    if not isinstance(rules, list):
        raise ValueError("tag ruleset rules must be an array")
    required = frozenset(rule.get("type") for rule in rules if isinstance(rule, dict))
    if required != {"update", "deletion"}:
        raise ValueError("tag ruleset must protect exactly tag update and deletion")
    raw_bypass = value.get("bypass_actors")
    if not isinstance(raw_bypass, list):
        raise ValueError("tag ruleset bypass_actors must be an array")
    bypass = frozenset(actor for item in raw_bypass if (actor := _actor_tuple(item)) is not None)
    if len(bypass) != len(raw_bypass):
        raise ValueError("tag ruleset contains an invalid bypass actor")
    return TagProtectionPolicy(tuple(include), tuple(exclude), required, bypass)


def parse_release_version(tag: str, policy: ReleasePolicy) -> str:
    match = re.fullmatch(policy.tag_pattern, tag)
    if match is None:
        raise ValueError(
            f"tag {tag!r} is not a stable vMAJOR.MINOR.PATCH release; prereleases are rejected"
        )
    return match.group("version")


def _decode_content(response: object, label: str) -> str:
    value = require_object(response, label)
    if value.get("encoding") != "base64" or not isinstance(value.get("content"), str):
        raise UnavailableError(f"{label}: expected base64 file content")
    try:
        encoded = "".join(value["content"].split())
        return base64.b64decode(encoded, validate=True).decode("utf-8")
    except (ValueError, UnicodeDecodeError) as error:
        raise UnavailableError(f"{label}: invalid base64 UTF-8 content: {error}") from error


def read_repository_file(repo: str, path: str, commit: str) -> str:
    encoded_path = quote(path, safe="/")
    return _decode_content(
        gh_api_json(f"repos/{repo}/contents/{encoded_path}?ref={commit}"),
        f"{path}@{commit}",
    )


def resolve_tag_commit(repo: str, tag: str) -> tuple[str, str]:
    """Return (tag object/ref SHA, fully peeled commit SHA)."""
    encoded_tag = quote(tag, safe="")
    ref = require_object(gh_api_json(f"repos/{repo}/git/ref/tags/{encoded_tag}"), f"tag {tag}")
    obj = require_object(ref.get("object"), f"tag {tag} object")
    tag_object_sha = obj.get("sha")
    object_type = obj.get("type")
    if not isinstance(tag_object_sha, str) or not isinstance(object_type, str):
        raise UnavailableError(f"tag {tag}: object identity is unavailable")
    current_sha = tag_object_sha
    seen: set[str] = set()
    for _ in range(8):
        if current_sha in seen:
            raise UnavailableError(f"tag {tag}: tag object cycle detected")
        seen.add(current_sha)
        if object_type == "commit":
            if not FULL_SHA.fullmatch(current_sha):
                raise UnavailableError(f"tag {tag}: resolved commit is not a full SHA")
            return tag_object_sha, current_sha
        if object_type != "tag":
            raise ValueError(f"tag {tag} points to unsupported object type {object_type!r}")
        annotated = require_object(
            gh_api_json(f"repos/{repo}/git/tags/{current_sha}"),
            f"annotated tag object {current_sha}",
        )
        obj = require_object(annotated.get("object"), f"annotated tag {current_sha} target")
        current_sha, object_type = obj.get("sha"), obj.get("type")
        if not isinstance(current_sha, str) or not isinstance(object_type, str):
            raise UnavailableError(f"annotated tag {tag}: target identity is unavailable")
    raise UnavailableError(f"tag {tag}: exceeded maximum annotated-tag depth")


def _check_app_id(check: dict[str, Any]) -> int | None:
    app = check.get("app")
    return app.get("id") if isinstance(app, dict) and isinstance(app.get("id"), int) else None


def _latest_check(checks: list[Any], spec: CheckSpec, commit: str) -> dict[str, Any] | None:
    matches = [
        item
        for item in checks
        if isinstance(item, dict)
        and item.get("name") == spec.context
        and item.get("head_sha") == commit
        and (spec.integration_id is None or _check_app_id(item) == spec.integration_id)
    ]
    return max(matches, key=lambda item: int(item.get("id", 0))) if matches else None


def _actions_run_id(details_url: object) -> int | None:
    if not isinstance(details_url, str):
        return None
    match = re.search(r"/actions/runs/(\d+)(?:/|$)", details_url)
    return int(match.group(1)) if match else None


def _checks_ready(checks: list[Any], specs: tuple[CheckSpec, ...], commit: str) -> bool:
    for spec in specs:
        check = _latest_check(checks, spec, commit)
        if check is None:
            return False
        if check.get("status") != "completed":
            return False
    return True


def collect_exact_checks(
    repo: str,
    commit: str,
    specs: tuple[CheckSpec, ...],
    wait_seconds: int,
    poll_seconds: int,
) -> list[Any]:
    deadline = time.monotonic() + wait_seconds
    while True:
        response = require_object(
            gh_api_json(f"repos/{repo}/commits/{commit}/check-runs?per_page=100"),
            f"check runs for {commit}",
        )
        checks = require_array(response.get("check_runs"), f"check runs for {commit}")
        if _checks_ready(checks, specs, commit) or time.monotonic() >= deadline:
            return checks
        remaining = max(0, round(deadline - time.monotonic()))
        print(f"WAIT release gate exact-commit CI remaining_seconds={remaining}", file=sys.stderr)
        time.sleep(min(poll_seconds, max(0, deadline - time.monotonic())))


def _parse_toml(text: str, label: str) -> dict[str, Any]:
    try:
        value = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        raise ValueError(f"{label}: invalid TOML: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label}: TOML root must be a table")
    return value


def collect_version_snapshot(repo: str, commit: str, policy: ReleasePolicy) -> dict[str, Any]:
    workspace = _parse_toml(read_repository_file(repo, "Cargo.toml", commit), "Cargo.toml")
    members = workspace.get("workspace", {}).get("members")
    if not isinstance(members, list) or not all(isinstance(item, str) for item in members):
        raise ValueError("Cargo.toml workspace.members must be an array of strings")
    packages: dict[str, dict[str, str]] = {}
    for spec in policy.packages:
        manifest_path = f"{spec.path}/Cargo.toml"
        manifest = _parse_toml(read_repository_file(repo, manifest_path, commit), manifest_path)
        package = manifest.get("package")
        if not isinstance(package, dict):
            raise ValueError(f"{manifest_path}: missing [package]")
        name, version = package.get("name"), package.get("version")
        if not isinstance(name, str) or not isinstance(version, str):
            raise ValueError(f"{manifest_path}: package name/version must be strings")
        packages[spec.path] = {"name": name, "version": version}
    lock = _parse_toml(read_repository_file(repo, "Cargo.lock", commit), "Cargo.lock")
    lock_versions: dict[str, list[str]] = {}
    for package in lock.get("package", []):
        if (
            isinstance(package, dict)
            and isinstance(package.get("name"), str)
            and isinstance(package.get("version"), str)
        ):
            lock_versions.setdefault(package["name"], []).append(package["version"])
    return {"workspace_members": members, "packages": packages, "lock_versions": lock_versions}


def collect_rulesets(repo: str) -> list[dict[str, Any]]:
    summaries = require_array(
        gh_api_json(f"repos/{repo}/rulesets?includes_parents=true&targets=tag&per_page=100"),
        "tag rulesets",
    )
    details: list[dict[str, Any]] = []
    for summary in summaries:
        if not isinstance(summary, dict) or not isinstance(summary.get("id"), int):
            raise UnavailableError("tag ruleset summary is missing an integer id")
        details.append(
            require_object(
                gh_api_json(f"repos/{repo}/rulesets/{summary['id']}?includes_parents=true"),
                f"tag ruleset {summary['id']}",
            )
        )
    return details


def collect_live_snapshot(
    repo: str,
    tag: str,
    policy: ReleasePolicy,
    checks: tuple[CheckSpec, ...],
    wait_seconds: int,
    poll_seconds: int,
) -> dict[str, Any]:
    tag_object_sha, commit = resolve_tag_commit(repo, tag)
    check_runs = collect_exact_checks(repo, commit, checks, wait_seconds, poll_seconds)
    run_cache: dict[int, dict[str, Any]] = {}
    for spec in checks:
        check = _latest_check(check_runs, spec, commit)
        if check is None:
            continue
        run_id = _actions_run_id(check.get("details_url"))
        if run_id is None:
            continue
        if run_id not in run_cache:
            run_cache[run_id] = require_object(
                gh_api_json(f"repos/{repo}/actions/runs/{run_id}"),
                f"Actions run {run_id}",
            )
        run = run_cache[run_id]
        check["workflow_run"] = {
            "id": run_id,
            "head_sha": run.get("head_sha"),
            "event": run.get("event"),
            "status": run.get("status"),
            "conclusion": run.get("conclusion"),
            "path": run.get("path"),
        }
    versions = collect_version_snapshot(repo, commit, policy)
    return {
        "repository": repo,
        "tag": tag,
        "tag_object_sha": tag_object_sha,
        "commit": commit,
        "checks": check_runs,
        "rulesets": collect_rulesets(repo),
        **versions,
        "unavailable": [],
    }


def _ref_matches(pattern: str, ref: str) -> bool:
    return pattern == "~ALL" or fnmatch.fnmatchcase(ref, pattern)


def ruleset_targets_tag(ruleset: dict[str, Any], tag: str) -> bool:
    if ruleset.get("target") != "tag" or ruleset.get("enforcement") != "active":
        return False
    ref_name = ruleset.get("conditions", {}).get("ref_name")
    if not isinstance(ref_name, dict):
        return False
    include, exclude = ref_name.get("include"), ref_name.get("exclude")
    if not isinstance(include, list) or not isinstance(exclude, list):
        return False
    ref = f"refs/tags/{tag}"
    return any(isinstance(item, str) and _ref_matches(item, ref) for item in include) and not any(
        isinstance(item, str) and _ref_matches(item, ref) for item in exclude
    )


def tag_policy_targets_tag(policy: TagProtectionPolicy, tag: str) -> bool:
    ref = f"refs/tags/{tag}"
    return any(_ref_matches(item, ref) for item in policy.include) and not any(
        _ref_matches(item, ref) for item in policy.exclude
    )


def evaluate_snapshot(
    snapshot: dict[str, Any],
    policy: ReleasePolicy,
    required_checks: tuple[CheckSpec, ...],
    tag_policy: TagProtectionPolicy,
) -> ReleaseResult:
    unavailable = snapshot.get("unavailable", [])
    if unavailable:
        raise UnavailableError("; ".join(str(item) for item in unavailable))
    tag, commit = snapshot.get("tag"), snapshot.get("commit")
    if not isinstance(tag, str) or not isinstance(commit, str) or not FULL_SHA.fullmatch(commit):
        raise UnavailableError("snapshot tag or full resolved commit is unavailable")
    version = parse_release_version(tag, policy)
    errors: list[str] = []
    if not tag_policy_targets_tag(tag_policy, tag):
        errors.append(f"reviewed tag ruleset payload does not target refs/tags/{tag}")

    checks = snapshot.get("checks")
    if not isinstance(checks, list):
        raise UnavailableError("exact-commit check runs are unavailable")
    checked: list[tuple[str, int]] = []
    for spec in required_checks:
        check = _latest_check(checks, spec, commit)
        if check is None:
            errors.append(f"tagged commit {commit[:12]} is missing {spec.display()}")
            continue
        if check.get("status") != "completed" or check.get("conclusion") != "success":
            errors.append(
                f"tagged commit {commit[:12]} latest {spec.display()} is "
                f"{check.get('status')}/{check.get('conclusion')}"
            )
        run = check.get("workflow_run")
        if not isinstance(run, dict):
            errors.append(f"{spec.display()} is not tied to a GitHub Actions workflow run")
            continue
        if run.get("head_sha") != commit:
            errors.append(f"{spec.display()} workflow run is for a different commit")
        if run.get("path") != policy.ci_workflow_path or run.get("event") != "push":
            errors.append(
                f"{spec.display()} came from {run.get('path')}/{run.get('event')}, not exact-commit push CI"
            )
        if run.get("status") != "completed" or run.get("conclusion") != "success":
            errors.append(f"{spec.display()} workflow run did not complete successfully")
        run_id = run.get("id")
        if isinstance(run_id, int):
            checked.append((spec.context, run_id))

    members = snapshot.get("workspace_members")
    packages = snapshot.get("packages")
    lock_versions = snapshot.get("lock_versions")
    if not isinstance(members, list) or not isinstance(packages, dict) or not isinstance(lock_versions, dict):
        raise UnavailableError("exact-commit crate version metadata is unavailable")
    expected_paths = {item.path for item in policy.packages}
    actual_paths = {item for item in members if isinstance(item, str)}
    if actual_paths != expected_paths:
        missing, extra = expected_paths - actual_paths, actual_paths - expected_paths
        if missing:
            errors.append("workspace packages missing from tagged commit: " + ", ".join(sorted(missing)))
        if extra:
            errors.append("workspace packages unclassified by release policy: " + ", ".join(sorted(extra)))

    synchronized: list[tuple[str, str]] = []
    independent: list[tuple[str, str]] = []
    for spec in policy.packages:
        package = packages.get(spec.path)
        if not isinstance(package, dict):
            errors.append(f"{spec.path}/Cargo.toml version metadata is missing")
            continue
        actual_name, actual_version = package.get("name"), package.get("version")
        if actual_name != spec.name or not isinstance(actual_version, str):
            errors.append(f"{spec.path}/Cargo.toml does not declare expected package {spec.name}")
            continue
        if spec.synchronized and actual_version != version:
            errors.append(f"synchronized crate {spec.name} is {actual_version}, expected {version}")
        locked = lock_versions.get(spec.name)
        if not isinstance(locked, list) or actual_version not in locked:
            errors.append(f"Cargo.lock does not contain {spec.name} {actual_version}")
        target = synchronized if spec.synchronized else independent
        target.append((spec.name, actual_version))

    raw_rulesets = snapshot.get("rulesets")
    if not isinstance(raw_rulesets, list):
        raise UnavailableError("tag ruleset metadata is unavailable")
    protection_sources: list[int] = []
    for ruleset in raw_rulesets:
        if not isinstance(ruleset, dict) or not ruleset_targets_tag(ruleset, tag):
            continue
        rules = ruleset.get("rules")
        if not isinstance(rules, list):
            continue
        rule_types = {rule.get("type") for rule in rules if isinstance(rule, dict)}
        if not tag_policy.required_rules <= rule_types:
            continue
        if "bypass_actors" not in ruleset:
            raise UnavailableError(
                "tag ruleset bypass actors are unavailable; authenticated write access is required"
            )
        bypass = frozenset(
            actor for item in ruleset.get("bypass_actors", []) if (actor := _actor_tuple(item)) is not None
        )
        if bypass != tag_policy.bypass_actors:
            errors.append(f"tag ruleset {ruleset.get('id')} does not use the reviewed emergency bypass")
            continue
        ruleset_id = ruleset.get("id")
        if isinstance(ruleset_id, int):
            protection_sources.append(ruleset_id)
    if not protection_sources:
        errors.append(f"no active update-and-deletion ruleset protects refs/tags/{tag}")

    return ReleaseResult(
        tag,
        version,
        commit,
        tuple(checked),
        tuple(synchronized),
        tuple(independent),
        tuple(sorted(protection_sources)),
        tuple(errors),
    )


def load_snapshot(path: Path) -> dict[str, Any]:
    value = _read_json_object(path, "release fixture")
    base = value.pop("$base", None)
    if base is None:
        return value
    if not isinstance(base, str) or Path(base).is_absolute():
        raise UnavailableError(f"fixture {path}: $base must be a relative path")
    inherited = load_snapshot(path.parent / base)
    inherited.update(value)
    return inherited


def _write_github_output(path: Path, result: ReleaseResult) -> None:
    with path.open("a", encoding="utf-8") as output:
        output.write(f"commit={result.commit}\n")
        output.write(f"version={result.version}\n")
        output.write("prerelease=false\n")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default="nzy1997/rust-qec", help="OWNER/REPO")
    parser.add_argument("--tag", required=True)
    parser.add_argument("--dry-run", action="store_true", required=True)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--branch-ruleset", type=Path, default=DEFAULT_BRANCH_RULESET)
    parser.add_argument("--tag-ruleset", type=Path, default=DEFAULT_TAG_RULESET)
    parser.add_argument("--fixture", type=Path)
    parser.add_argument("--wait-seconds", type=int, default=0)
    parser.add_argument("--poll-seconds", type=int, default=20)
    parser.add_argument("--github-output", type=Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.wait_seconds < 0 or args.poll_seconds < 1:
        print("FAIL release gate: wait seconds must be nonnegative and poll seconds positive", file=sys.stderr)
        return 1
    try:
        policy = load_release_policy(args.policy)
        version = parse_release_version(args.tag, policy)
        del version
        required_checks = load_desired_gate(args.branch_ruleset).checks
        tag_policy = load_tag_protection_policy(args.tag_ruleset)
        snapshot = (
            load_snapshot(args.fixture)
            if args.fixture
            else collect_live_snapshot(
                args.repo,
                args.tag,
                policy,
                required_checks,
                args.wait_seconds,
                args.poll_seconds,
            )
        )
        result = evaluate_snapshot(snapshot, policy, required_checks, tag_policy)
    except UnavailableError as error:
        print(f"UNAVAILABLE release gate {args.tag}: {error}", file=sys.stderr)
        return 2
    except ValueError as error:
        print(f"FAIL release gate {args.tag}: {error}", file=sys.stderr)
        return 1

    if not result.passed:
        for error in result.errors:
            print(f"FAIL release gate {args.tag}: {error}", file=sys.stderr)
        return 1
    for name, run_id in result.checks:
        print(f"CI {name} app={GITHUB_ACTIONS_APP_ID} run={run_id} commit={result.commit}")
    print("synchronized crates: " + ", ".join(f"{name}={version}" for name, version in result.synchronized))
    print("independent crates: " + ", ".join(f"{name}={version}" for name, version in result.independent))
    print("tag protection rulesets: " + ",".join(str(item) for item in result.protection_sources))
    print(f"PASS release gate {result.tag} commit={result.commit}")
    if args.github_output:
        _write_github_output(args.github_output, result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
