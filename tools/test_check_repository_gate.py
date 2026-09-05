from __future__ import annotations

import copy
import importlib
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER_PATH = REPO_ROOT / "tools/check_repository_gate.py"
FIXTURES = REPO_ROOT / "tools/fixtures/repository_gate"
RULESET = REPO_ROOT / "tools/repository_gate_ruleset.json"
checker = importlib.import_module("tools.check_repository_gate")


class RepositoryGateCheckerTest(unittest.TestCase):
    def run_fixture(self, name: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(CHECKER_PATH),
                "--repo",
                "nzy1997/rust-qec",
                "--branch",
                "master",
                "--fixture",
                str(FIXTURES / name),
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_valid_active_ruleset_and_exact_evidence_pass(self) -> None:
        result = self.run_fixture("valid.json")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("PASS default branch gate", result.stdout)
        self.assertIn("ordinary_pr_samples=3", result.stdout)
        self.assertEqual(result.stderr, "")

    def test_no_rule_is_rejected(self) -> None:
        result = self.run_fixture("no_rule.json")

        self.assertEqual(result.returncode, 1)
        self.assertNotIn("PASS", result.stdout)
        self.assertIn("no active pull-request rule", result.stderr)

    def test_disabled_enforcement_is_rejected_even_if_effective_fixture_claims_rules(self) -> None:
        result = self.run_fixture("disabled_enforcement.json")

        self.assertEqual(result.returncode, 1)
        self.assertNotIn("PASS", result.stdout)
        self.assertIn("disabled or does not target", result.stderr)

    def test_empty_required_check_list_is_rejected(self) -> None:
        result = self.run_fixture("empty_required_checks.json")

        self.assertEqual(result.returncode, 1)
        self.assertNotIn("PASS", result.stdout)
        self.assertIn("effective required-check list is empty", result.stderr)

    def test_conditionally_absent_job_cannot_supply_required_check(self) -> None:
        snapshot = checker.load_snapshot(FIXTURES / "conditional_only.json")
        desired = checker.DesiredGate(
            "refs/heads/master",
            (checker.CheckSpec("perf-gate", checker.GITHUB_ACTIONS_APP_ID),),
            (("RepositoryRole", 5, "pull_request"),),
        )

        result = checker.evaluate_snapshot(snapshot, desired)

        self.assertFalse(result.passed)
        self.assertTrue(
            all("does not prove perf-gate is always applicable (skipped)" in error for error in result.errors),
            result.errors,
        )

    def test_committed_policy_rejects_conditional_only_ruleset(self) -> None:
        result = self.run_fixture("conditional_only.json")

        self.assertEqual(result.returncode, 1)
        self.assertNotIn("PASS", result.stdout)
        self.assertIn("unexpected required checks (possibly conditional): perf-gate", result.stderr)

    def test_stale_checks_from_another_sha_are_rejected(self) -> None:
        result = self.run_fixture("stale_unrelated_run.json")

        self.assertEqual(result.returncode, 1)
        self.assertNotIn("PASS", result.stdout)
        self.assertIn("exact head aaaaaaaaaaaa has no test", result.stderr)

    def test_latest_rerun_failure_cannot_hide_behind_earlier_success(self) -> None:
        snapshot = checker.load_snapshot(FIXTURES / "valid.json")
        snapshot["default_head_checks"][0]["completed_at"] = "2026-09-05T05:46:00Z"
        snapshot["default_head_checks"].append(
            {
                "id": 999,
                "name": "test",
                "head_sha": snapshot["head_sha"],
                "status": "in_progress",
                "conclusion": None,
                "started_at": "2026-09-05T06:00:00Z",
                "completed_at": None,
                "app": {"id": checker.GITHUB_ACTIONS_APP_ID},
                "workflow_id": 231,
            }
        )

        result = checker.evaluate_snapshot(snapshot, checker.load_desired_gate(RULESET))

        self.assertFalse(result.passed)
        self.assertIn("latest test (app 15368) is in_progress/None", "\n".join(result.errors))

    def test_wrong_app_identity_is_rejected(self) -> None:
        snapshot = checker.load_snapshot(FIXTURES / "valid.json")
        snapshot["default_head_checks"][0]["app"]["id"] = 1

        result = checker.evaluate_snapshot(snapshot, checker.load_desired_gate(RULESET))

        self.assertFalse(result.passed)
        self.assertIn("has no test (app 15368) check", "\n".join(result.errors))

    def test_broader_bypass_is_rejected(self) -> None:
        snapshot = checker.load_snapshot(FIXTURES / "valid.json")
        snapshot["rulesets"][0]["bypass_actors"].append(
            {"actor_id": 2, "actor_type": "RepositoryRole", "bypass_mode": "always"}
        )

        result = checker.evaluate_snapshot(snapshot, checker.load_desired_gate(RULESET))

        self.assertFalse(result.passed)
        self.assertIn("broader bypass actors", "\n".join(result.errors))

    def test_hidden_ruleset_bypass_data_is_unavailable(self) -> None:
        snapshot = checker.load_snapshot(FIXTURES / "valid.json")
        del snapshot["rulesets"][0]["bypass_actors"]

        with self.assertRaisesRegex(checker.UnavailableError, "authenticated write access"):
            checker.evaluate_snapshot(snapshot, checker.load_desired_gate(RULESET))

    def test_modern_branch_protection_does_not_double_count_contexts(self) -> None:
        protection = {
            "required_pull_request_reviews": {"required_approving_review_count": 0},
            "required_status_checks": {
                "strict": True,
                "contexts": ["test"],
                "checks": [{"context": "test", "app_id": checker.GITHUB_ACTIONS_APP_ID}],
            },
            "enforce_admins": {"enabled": False},
        }

        has_pr, checks, strict, admin_bypass = checker._protection_policy(protection)

        self.assertTrue(has_pr)
        self.assertEqual(checks, {checker.CheckSpec("test", checker.GITHUB_ACTIONS_APP_ID)})
        self.assertEqual(strict, [True])
        self.assertTrue(admin_bypass)

    def test_permissions_failure_is_unavailable_never_pass(self) -> None:
        result = self.run_fixture("permissions_unavailable.json")

        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")
        self.assertIn("UNAVAILABLE default branch gate", result.stderr)
        self.assertNotIn("PASS", result.stderr)

    def test_gh_api_allows_only_explicit_not_found(self) -> None:
        not_found = subprocess.CompletedProcess(
            ["gh", "api"], 1, stdout="", stderr="gh: Branch not protected (HTTP 404)\n"
        )
        forbidden = subprocess.CompletedProcess(
            ["gh", "api"], 1, stdout="", stderr="gh: Resource not accessible (HTTP 403)\n"
        )
        with mock.patch.object(checker.subprocess, "run", return_value=not_found):
            self.assertIsNone(checker.gh_api_json("repos/o/r/branches/main/protection", allow_not_found=True))
        with mock.patch.object(checker.subprocess, "run", return_value=forbidden):
            with self.assertRaises(checker.UnavailableError):
                checker.gh_api_json("repos/o/r/rulesets")

    def test_committed_ruleset_has_only_five_always_on_actions_checks(self) -> None:
        desired = checker.load_desired_gate(RULESET)

        self.assertEqual(desired.branch_ref, "refs/heads/master")
        self.assertEqual(
            {item.context for item in desired.checks},
            {
                "test",
                "rsmp-v1-readiness",
                "checked-evidence-portability",
                "publication-evidence-check",
                "coverage",
            },
        )
        self.assertTrue(all(item.integration_id == checker.GITHUB_ACTIONS_APP_ID for item in desired.checks))
        self.assertNotIn("perf-gate", {item.context for item in desired.checks})
        self.assertEqual(desired.bypass_actors, (("RepositoryRole", 5, "pull_request"),))

    def test_ruleset_target_matching_requires_active_matching_ref(self) -> None:
        snapshot = checker.load_snapshot(FIXTURES / "valid.json")
        ruleset = snapshot["rulesets"][0]

        self.assertTrue(checker.ruleset_targets_branch(ruleset, "master", "master"))
        disabled = copy.deepcopy(ruleset)
        disabled["enforcement"] = "evaluate"
        self.assertFalse(checker.ruleset_targets_branch(disabled, "master", "master"))
        wrong_ref = copy.deepcopy(ruleset)
        wrong_ref["conditions"]["ref_name"]["include"] = ["refs/heads/release/*"]
        self.assertFalse(checker.ruleset_targets_branch(wrong_ref, "master", "master"))


if __name__ == "__main__":
    unittest.main()
