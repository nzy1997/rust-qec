#!/usr/bin/env python3
"""Read-only verification for the effective default-branch merge gate."""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.parse import quote


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RULESET = REPO_ROOT / "tools/repository_gate_ruleset.json"
GITHUB_ACTIONS_APP_ID = 15368
DISQUALIFYING_PR_LABELS = {"run-perf"}
SUCCESS = "success"
TERMINAL_APPLICABLE_CONCLUSIONS = {"success", "failure", "timed_out", "action_required"}


class UnavailableError(RuntimeError):
    """The read-only GitHub state could not be obtained reliably."""


@dataclass(frozen=True, order=True)
class CheckSpec:
    context: str
    integration_id: int | None

    @classmethod
    def from_mapping(cls, value: object) -> "CheckSpec":
        if not isinstance(value, dict) or not isinstance(value.get("context"), str):
            raise ValueError("required status check must contain a string context")
        app_id = value.get("integration_id")
        if app_id is not None and not isinstance(app_id, int):
            raise ValueError(f"{value['context']}: integration_id must be an integer or null")
        return cls(value["context"], app_id)

    def display(self) -> str:
        app = "any app" if self.integration_id is None else f"app {self.integration_id}"
        return f"{self.context} ({app})"


@dataclass(frozen=True)
class DesiredGate:
    branch_ref: str
    checks: tuple[CheckSpec, ...]
    bypass_actors: tuple[tuple[str, int | None, str], ...]


@dataclass(frozen=True)
class GateResult:
    errors: tuple[str, ...]
    details: tuple[str, ...]

    @property
    def passed(self) -> bool:
        return not self.errors


def gh_api_json(endpoint: str, *, allow_not_found: bool = False) -> object | None:
    """Read one GitHub REST endpoint through gh and parse its JSON response."""
    result = subprocess.run(
        ["gh", "api", endpoint],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        if allow_not_found and "HTTP 404" in result.stderr:
            return None
        message = result.stderr.strip().splitlines()
        reason = message[-1] if message else f"gh exited {result.returncode}"
        raise UnavailableError(f"GET {endpoint}: {reason}")
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise UnavailableError(f"GET {endpoint}: invalid JSON: {error}") from error


def require_object(value: object, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise UnavailableError(f"{label}: expected a JSON object")
    return value


def require_array(value: object, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise UnavailableError(f"{label}: expected a JSON array")
    return value


def load_desired_gate(path: Path) -> DesiredGate:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read ruleset payload {path}: {error}") from error
    if not isinstance(payload, dict):
        raise ValueError("ruleset payload root must be an object")
    if payload.get("target") != "branch" or payload.get("enforcement") != "active":
        raise ValueError("ruleset payload must be an active branch ruleset")

    ref_name = payload.get("conditions", {}).get("ref_name", {})
    includes = ref_name.get("include") if isinstance(ref_name, dict) else None
    if not isinstance(includes, list) or len(includes) != 1 or not isinstance(includes[0], str):
        raise ValueError("ruleset payload must include exactly one branch ref")

    status_rules = [
        rule
        for rule in payload.get("rules", [])
        if isinstance(rule, dict) and rule.get("type") == "required_status_checks"
    ]
    if len(status_rules) != 1:
        raise ValueError("ruleset payload must contain one required_status_checks rule")
    raw_checks = status_rules[0].get("parameters", {}).get("required_status_checks")
    if not isinstance(raw_checks, list) or not raw_checks:
        raise ValueError("ruleset payload must contain at least one required status check")
    checks = tuple(CheckSpec.from_mapping(value) for value in raw_checks)
    if len(set(checks)) != len(checks):
        raise ValueError("ruleset payload contains duplicate required status checks")

    raw_bypass = payload.get("bypass_actors")
    if not isinstance(raw_bypass, list):
        raise ValueError("ruleset payload bypass_actors must be an array")
    bypass: list[tuple[str, int | None, str]] = []
    for actor in raw_bypass:
        if not isinstance(actor, dict):
            raise ValueError("ruleset bypass actor must be an object")
        actor_type = actor.get("actor_type")
        actor_id = actor.get("actor_id")
        mode = actor.get("bypass_mode")
        if not isinstance(actor_type, str) or not isinstance(mode, str):
            raise ValueError("ruleset bypass actor needs actor_type and bypass_mode")
        if actor_id is not None and not isinstance(actor_id, int):
            raise ValueError("ruleset bypass actor_id must be an integer or null")
        bypass.append((actor_type, actor_id, mode))
    return DesiredGate(includes[0], checks, tuple(sorted(bypass)))


def _run_id(details_url: object) -> int | None:
    if not isinstance(details_url, str):
        return None
    match = re.search(r"/actions/runs/(\d+)(?:/|$)", details_url)
    return int(match.group(1)) if match else None


def _app_id(check: dict[str, Any]) -> int | None:
    app = check.get("app")
    if isinstance(app, dict) and isinstance(app.get("id"), int):
        return app["id"]
    value = check.get("integration_id")
    return value if isinstance(value, int) else None


def _latest_matching_check(checks: list[Any], spec: CheckSpec, head_sha: str) -> dict[str, Any] | None:
    matches = [
        check
        for check in checks
        if (
            isinstance(check, dict)
            and check.get("name") == spec.context
            and (spec.integration_id is None or _app_id(check) == spec.integration_id)
            and check.get("head_sha", head_sha) == head_sha
        )
    ]
    if not matches:
        return None
    # Check-run IDs increase as reruns are created. A newer queued/in-progress
    # rerun has completed_at=null, so sorting by completed_at would incorrectly
    # let an older successful attempt hide it.
    return max(matches, key=lambda item: int(item.get("id", 0)))


def collect_live_snapshot(repo: str, branch: str, desired: DesiredGate, minimum_samples: int) -> dict[str, Any]:
    """Collect only the REST state needed by evaluate_snapshot."""
    encoded_branch = quote(branch, safe="")
    repo_path = f"repos/{repo}"
    repository = require_object(gh_api_json(repo_path), repo_path)
    branch_data = require_object(gh_api_json(f"{repo_path}/branches/{encoded_branch}"), "branch")
    commit = require_object(branch_data.get("commit"), "branch.commit")
    head_sha = commit.get("sha")
    if not isinstance(head_sha, str) or not head_sha:
        raise UnavailableError("branch.commit.sha is unavailable")

    protection = gh_api_json(
        f"{repo_path}/branches/{encoded_branch}/protection",
        allow_not_found=True,
    )
    ruleset_summaries = require_array(
        gh_api_json(f"{repo_path}/rulesets?includes_parents=true&per_page=100"),
        "rulesets",
    )
    rulesets: list[dict[str, Any]] = []
    for summary in ruleset_summaries:
        if not isinstance(summary, dict) or not isinstance(summary.get("id"), int):
            raise UnavailableError("ruleset summary is missing an integer id")
        rulesets.append(
            require_object(
                gh_api_json(f"{repo_path}/rulesets/{summary['id']}"),
                f"ruleset {summary['id']}",
            )
        )
    effective_rules = require_array(
        gh_api_json(f"{repo_path}/rules/branches/{encoded_branch}"),
        "effective branch rules",
    )

    check_response = require_object(
        gh_api_json(f"{repo_path}/commits/{head_sha}/check-runs?per_page=100"),
        "default head check runs",
    )
    head_checks = require_array(check_response.get("check_runs"), "default head check_runs")

    workflow_for_check: dict[CheckSpec, int] = {}
    run_cache: dict[int, dict[str, Any]] = {}
    for spec in desired.checks:
        check = _latest_matching_check(head_checks, spec, head_sha)
        if check is None:
            continue
        check["head_sha"] = head_sha
        run_id = _run_id(check.get("details_url"))
        if run_id is None:
            continue
        if run_id not in run_cache:
            run_cache[run_id] = require_object(
                gh_api_json(f"{repo_path}/actions/runs/{run_id}"),
                f"Actions run {run_id}",
            )
        run = run_cache[run_id]
        workflow_id = run.get("workflow_id")
        if run.get("head_sha") == head_sha and isinstance(workflow_id, int):
            check["workflow_id"] = workflow_id
            workflow_for_check[spec] = workflow_id

    samples: list[dict[str, Any]] = []
    for workflow_id in sorted(set(workflow_for_check.values())):
        runs_response = require_object(
            gh_api_json(
                f"{repo_path}/actions/workflows/{workflow_id}/runs"
                "?event=pull_request&status=completed&per_page=30"
            ),
            f"workflow {workflow_id} pull-request runs",
        )
        runs = require_array(runs_response.get("workflow_runs"), "workflow_runs")
        collected = 0
        seen_prs: set[int] = set()
        for run in runs:
            if collected >= minimum_samples:
                break
            if not isinstance(run, dict) or not isinstance(run.get("id"), int):
                continue
            sha = run.get("head_sha")
            if not isinstance(sha, str):
                continue
            pulls = require_array(
                gh_api_json(f"{repo_path}/commits/{sha}/pulls?per_page=20"),
                f"pull requests for {sha}",
            )
            pull = next(
                (
                    item
                    for item in pulls
                    if isinstance(item, dict)
                    and item.get("base", {}).get("ref") == branch
                    and item.get("head", {}).get("sha") == sha
                    and isinstance(item.get("number"), int)
                ),
                None,
            )
            if pull is None or pull["number"] in seen_prs:
                continue
            labels = {
                label.get("name")
                for label in pull.get("labels", [])
                if isinstance(label, dict) and isinstance(label.get("name"), str)
            }
            if labels & DISQUALIFYING_PR_LABELS:
                continue
            jobs_response = require_object(
                gh_api_json(f"{repo_path}/actions/runs/{run['id']}/jobs?per_page=100"),
                f"Actions jobs for run {run['id']}",
            )
            jobs = require_array(jobs_response.get("jobs"), "Actions jobs")
            samples.append(
                {
                    "workflow_id": workflow_id,
                    "run_id": run["id"],
                    "head_sha": sha,
                    "pull_request": pull["number"],
                    "labels": sorted(labels),
                    "jobs": jobs,
                }
            )
            seen_prs.add(pull["number"])
            collected += 1

    return {
        "repository": repo,
        "repository_default_branch": repository.get("default_branch"),
        "branch": branch,
        "head_sha": head_sha,
        "branch_protection": protection,
        "rulesets": rulesets,
        "effective_rules": effective_rules,
        "default_head_checks": head_checks,
        "pull_request_samples": samples,
        "unavailable": [],
    }


def _matches_ref_pattern(pattern: str, ref: str, default_ref: str) -> bool:
    if pattern == "~ALL":
        return True
    if pattern == "~DEFAULT_BRANCH":
        return ref == default_ref
    return fnmatch.fnmatchcase(ref, pattern)


def ruleset_targets_branch(ruleset: dict[str, Any], branch: str, default_branch: str) -> bool:
    if ruleset.get("target") != "branch" or ruleset.get("enforcement") != "active":
        return False
    raw_ref = ruleset.get("conditions", {}).get("ref_name")
    if not isinstance(raw_ref, dict):
        return False
    include = raw_ref.get("include")
    exclude = raw_ref.get("exclude")
    if not isinstance(include, list) or not isinstance(exclude, list):
        return False
    ref = f"refs/heads/{branch}"
    default_ref = f"refs/heads/{default_branch}"
    return (
        any(isinstance(pattern, str) and _matches_ref_pattern(pattern, ref, default_ref) for pattern in include)
        and not any(
            isinstance(pattern, str) and _matches_ref_pattern(pattern, ref, default_ref)
            for pattern in exclude
        )
    )


def _protection_policy(protection: object) -> tuple[bool, set[CheckSpec], list[bool], bool]:
    if not isinstance(protection, dict):
        return False, set(), [], False
    has_pr = isinstance(protection.get("required_pull_request_reviews"), dict)
    raw_status = protection.get("required_status_checks")
    checks: set[CheckSpec] = set()
    strict: list[bool] = []
    if isinstance(raw_status, dict):
        strict.append(raw_status.get("strict") is True)
        raw_checks = raw_status.get("checks")
        if isinstance(raw_checks, list):
            for check in raw_checks:
                if isinstance(check, dict) and isinstance(check.get("context"), str):
                    app_id = check.get("app_id")
                    checks.add(CheckSpec(check["context"], app_id if isinstance(app_id, int) else None))
        raw_contexts = raw_status.get("contexts")
        if not checks and isinstance(raw_contexts, list):
            checks.update(CheckSpec(context, None) for context in raw_contexts if isinstance(context, str))
    enforce_admins = protection.get("enforce_admins")
    admin_bypass = isinstance(enforce_admins, dict) and enforce_admins.get("enabled") is False
    return has_pr, checks, strict, admin_bypass


def _ruleset_policy(
    snapshot: dict[str, Any], branch: str, default_branch: str
) -> tuple[bool, set[CheckSpec], list[bool], list[dict[str, Any]], list[str]]:
    errors: list[str] = []
    rulesets = {
        item["id"]: item
        for item in snapshot.get("rulesets", [])
        if isinstance(item, dict) and isinstance(item.get("id"), int)
    }
    effective = snapshot.get("effective_rules")
    if not isinstance(effective, list):
        return False, set(), [], [], ["effective branch rules are unavailable"]

    has_pr = False
    checks: set[CheckSpec] = set()
    strict: list[bool] = []
    sources: dict[int, dict[str, Any]] = {}
    for rule in effective:
        if not isinstance(rule, dict):
            continue
        ruleset_id = rule.get("ruleset_id")
        if not isinstance(ruleset_id, int):
            errors.append("effective rule is missing its ruleset_id")
            continue
        source = rulesets.get(ruleset_id)
        if source is None:
            errors.append(f"effective ruleset {ruleset_id} details are unavailable")
            continue
        if not ruleset_targets_branch(source, branch, default_branch):
            errors.append(f"ruleset {ruleset_id} is disabled or does not target refs/heads/{branch}")
            continue
        sources[ruleset_id] = source
        if rule.get("type") == "pull_request":
            has_pr = True
        if rule.get("type") == "required_status_checks":
            parameters = rule.get("parameters")
            if not isinstance(parameters, dict):
                errors.append(f"ruleset {ruleset_id} status-check parameters are unavailable")
                continue
            strict.append(parameters.get("strict_required_status_checks_policy") is True)
            raw_checks = parameters.get("required_status_checks")
            if not isinstance(raw_checks, list):
                errors.append(f"ruleset {ruleset_id} required status checks are unavailable")
                continue
            for raw_check in raw_checks:
                try:
                    checks.add(CheckSpec.from_mapping(raw_check))
                except ValueError as error:
                    errors.append(f"ruleset {ruleset_id}: {error}")
    return has_pr, checks, strict, list(sources.values()), errors


def _bypass_actor(actor: object) -> tuple[str, int | None, str] | None:
    if not isinstance(actor, dict):
        return None
    actor_type = actor.get("actor_type")
    actor_id = actor.get("actor_id")
    mode = actor.get("bypass_mode")
    if not isinstance(actor_type, str) or not isinstance(mode, str):
        return None
    return actor_type, actor_id if isinstance(actor_id, int) else None, mode


def evaluate_snapshot(
    snapshot: dict[str, Any], desired: DesiredGate, minimum_samples: int = 3
) -> GateResult:
    errors: list[str] = []
    details: list[str] = []
    unavailable = snapshot.get("unavailable", [])
    if unavailable:
        raise UnavailableError("; ".join(str(item) for item in unavailable))

    branch = snapshot.get("branch")
    head_sha = snapshot.get("head_sha")
    default_branch = snapshot.get("repository_default_branch", branch)
    if not isinstance(branch, str) or not isinstance(head_sha, str):
        raise UnavailableError("snapshot branch or exact head SHA is unavailable")
    if not isinstance(default_branch, str):
        raise UnavailableError("repository default branch is unavailable")
    expected_ref = f"refs/heads/{branch}"
    if desired.branch_ref != expected_ref:
        errors.append(f"reviewed payload targets {desired.branch_ref}, not {expected_ref}")

    bp_pr, bp_checks, bp_strict, bp_admin_bypass = _protection_policy(
        snapshot.get("branch_protection")
    )
    rs_pr, rs_checks, rs_strict, rs_sources, rs_errors = _ruleset_policy(
        snapshot, branch, default_branch
    )
    errors.extend(rs_errors)
    required_checks = bp_checks | rs_checks
    if not (bp_pr or rs_pr):
        errors.append("no active pull-request rule applies to the branch")
    if not required_checks:
        errors.append("the effective required-check list is empty")
    expected_checks = set(desired.checks)
    missing = expected_checks - required_checks
    unexpected = required_checks - expected_checks
    if missing:
        errors.append("missing required checks: " + ", ".join(item.display() for item in sorted(missing)))
    if unexpected:
        errors.append(
            "unexpected required checks (possibly conditional): "
            + ", ".join(item.display() for item in sorted(unexpected))
        )
    if any(strict is False for strict in bp_strict + rs_strict):
        errors.append("required checks do not require testing the latest target-branch code")

    if rs_sources and any("bypass_actors" not in source for source in rs_sources):
        raise UnavailableError(
            "ruleset bypass actors are unavailable; authenticated write access is required"
        )
    active_bypasses = {
        actor
        for source in rs_sources
        for actor in (_bypass_actor(item) for item in source.get("bypass_actors", []))
        if actor is not None
    }
    if rs_sources:
        desired_bypass = set(desired.bypass_actors)
        if not desired_bypass <= active_bypasses:
            errors.append("the reviewed pull-request-only repository-admin emergency bypass is missing")
        broad = active_bypasses - desired_bypass
        if broad:
            errors.append("ruleset has broader bypass actors than the reviewed emergency path")
    elif snapshot.get("branch_protection") is not None and not bp_admin_bypass:
        errors.append("legacy branch protection has no emergency repository-admin bypass")

    raw_head_checks = snapshot.get("default_head_checks")
    if not isinstance(raw_head_checks, list):
        raise UnavailableError("default-head check runs are unavailable")
    workflow_for_check: dict[CheckSpec, int] = {}
    for spec in desired.checks:
        check = _latest_matching_check(raw_head_checks, spec, head_sha)
        if check is None:
            errors.append(f"exact head {head_sha[:12]} has no {spec.display()} check")
            continue
        if check.get("status") != "completed" or check.get("conclusion") != SUCCESS:
            errors.append(
                f"exact head {head_sha[:12]} latest {spec.display()} is "
                f"{check.get('status')}/{check.get('conclusion')}"
            )
        workflow_id = check.get("workflow_id")
        if not isinstance(workflow_id, int):
            errors.append(f"exact head {spec.display()} is not tied to its current Actions workflow run")
            continue
        workflow_for_check[spec] = workflow_id

    raw_samples = snapshot.get("pull_request_samples")
    if not isinstance(raw_samples, list):
        raise UnavailableError("ordinary pull-request samples are unavailable")
    for workflow_id in sorted(set(workflow_for_check.values())):
        samples = [
            item
            for item in raw_samples
            if isinstance(item, dict) and item.get("workflow_id") == workflow_id
        ]
        if len(samples) < minimum_samples:
            errors.append(
                f"workflow {workflow_id} has {len(samples)} ordinary pull-request samples; "
                f"need {minimum_samples}"
            )
            continue
        supplied = [spec for spec, source_id in workflow_for_check.items() if source_id == workflow_id]
        for sample in samples[:minimum_samples]:
            labels = set(sample.get("labels", [])) if isinstance(sample.get("labels"), list) else set()
            if labels & DISQUALIFYING_PR_LABELS:
                errors.append(f"PR #{sample.get('pull_request')} evidence has opt-in labels")
            jobs = sample.get("jobs")
            if not isinstance(jobs, list):
                errors.append(f"PR #{sample.get('pull_request')} job evidence is unavailable")
                continue
            for spec in supplied:
                matches = [
                    job for job in jobs if isinstance(job, dict) and job.get("name") == spec.context
                ]
                job = max(matches, key=lambda item: int(item.get("id", 0))) if matches else None
                if (
                    job is None
                    or job.get("status") != "completed"
                    or job.get("conclusion") not in TERMINAL_APPLICABLE_CONCLUSIONS
                ):
                    conclusion = "missing" if job is None else str(job.get("conclusion"))
                    errors.append(
                        f"PR #{sample.get('pull_request')} does not prove {spec.context} is "
                        f"always applicable ({conclusion})"
                    )

    details.append(f"branch={branch} head={head_sha}")
    details.append("checks=" + ",".join(spec.context for spec in desired.checks))
    details.append(f"ordinary_pr_samples={minimum_samples}")
    return GateResult(tuple(errors), tuple(details))


def load_snapshot(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise UnavailableError(f"cannot read fixture {path}: {error}") from error
    snapshot = require_object(value, f"fixture {path}")
    base = snapshot.pop("$base", None)
    if base is None:
        return snapshot
    if not isinstance(base, str) or not base or Path(base).is_absolute():
        raise UnavailableError(f"fixture {path}: $base must be a relative path")
    inherited = load_snapshot(path.parent / base)
    inherited.update(snapshot)
    return inherited


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default="nzy1997/rust-qec", help="OWNER/REPO")
    parser.add_argument("--branch", default="master")
    parser.add_argument("--ruleset", type=Path, default=DEFAULT_RULESET)
    parser.add_argument("--fixture", type=Path, help="evaluate an offline snapshot instead of GitHub")
    parser.add_argument("--minimum-pr-samples", type=int, default=3)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.minimum_pr_samples < 1:
        print("FAIL default branch gate: --minimum-pr-samples must be positive", file=sys.stderr)
        return 1
    try:
        desired = load_desired_gate(args.ruleset)
        snapshot = (
            load_snapshot(args.fixture)
            if args.fixture
            else collect_live_snapshot(args.repo, args.branch, desired, args.minimum_pr_samples)
        )
        result = evaluate_snapshot(snapshot, desired, args.minimum_pr_samples)
    except UnavailableError as error:
        print(f"UNAVAILABLE default branch gate: {error}", file=sys.stderr)
        return 2
    except ValueError as error:
        print(f"FAIL default branch gate: {error}", file=sys.stderr)
        return 1

    if not result.passed:
        for error in result.errors:
            print(f"FAIL default branch gate: {error}", file=sys.stderr)
        return 1
    print("PASS default branch gate " + " ".join(result.details))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
