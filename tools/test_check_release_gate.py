from __future__ import annotations

import contextlib
import copy
import importlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
FIXTURES = REPO_ROOT / "tools/fixtures/release_gate"
POLICY_PATH = REPO_ROOT / "tools/release_version_policy.json"
BRANCH_RULESET = REPO_ROOT / "tools/repository_gate_ruleset.json"
TAG_RULESET = REPO_ROOT / "tools/release_tag_ruleset.json"
WORKFLOW = REPO_ROOT / ".github/workflows/release.yml"
checker = importlib.import_module("tools.check_release_gate")


class ReleaseGateCheckerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.policy = checker.load_release_policy(POLICY_PATH)
        cls.required_checks = checker.load_desired_gate(BRANCH_RULESET).checks
        cls.tag_policy = checker.load_tag_protection_policy(TAG_RULESET)

    def evaluate(self, fixture: str) -> checker.ReleaseResult:
        return checker.evaluate_snapshot(
            checker.load_snapshot(FIXTURES / fixture),
            self.policy,
            self.required_checks,
            self.tag_policy,
        )

    def run_main(self, fixture: str, output: Path | None = None) -> tuple[int, str, str]:
        argv = ["--tag", "v0.2.1", "--dry-run", "--fixture", str(FIXTURES / fixture)]
        if output is not None:
            argv.extend(["--github-output", str(output)])
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            result = checker.main(argv)
        return result, stdout.getvalue(), stderr.getvalue()

    def test_valid_v021_snapshot_passes_and_exports_stable_interface(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "github-output"
            status, stdout, stderr = self.run_main("valid.json", output)

            self.assertEqual(status, 0, stderr)
            self.assertIn(
                "PASS release gate v0.2.1 commit=cfc935fc13e73469f413a08e08d0c19ecad0e42a",
                stdout,
            )
            self.assertIn("independent crates: qec-ilp-core=0.1.0", stdout)
            self.assertEqual(
                output.read_text(encoding="utf-8"),
                "commit=cfc935fc13e73469f413a08e08d0c19ecad0e42a\n"
                "version=0.2.1\n"
                "prerelease=false\n",
            )

    def test_successful_parent_commit_checks_are_rejected(self) -> None:
        result = self.evaluate("parent_commit_checks.json")

        self.assertFalse(result.passed)
        self.assertIn("tagged commit cfc935fc13e7 is missing test", "\n".join(result.errors))

    def test_failed_cancelled_and_missing_required_checks_are_rejected(self) -> None:
        result = self.evaluate("bad_required_checks.json")
        errors = "\n".join(result.errors)

        self.assertFalse(result.passed)
        self.assertIn("test (app 15368) is completed/failure", errors)
        self.assertIn("rsmp-v1-readiness (app 15368) is completed/cancelled", errors)
        self.assertIn("missing coverage (app 15368)", errors)

    def test_changed_synchronized_crate_version_is_rejected(self) -> None:
        result = self.evaluate("changed_synchronized_version.json")

        self.assertFalse(result.passed)
        self.assertIn("synchronized crate rstim is 0.2.0, expected 0.2.1", result.errors)

    def test_independent_crates_do_not_have_to_match_release_version(self) -> None:
        result = self.evaluate("valid.json")

        self.assertTrue(result.passed, result.errors)
        self.assertIn(("qec-code", "0.1.0"), result.independent)
        self.assertIn(("rstim-shot-web", "0.1.0"), result.independent)

    def test_unprotected_tag_pattern_is_rejected(self) -> None:
        result = self.evaluate("unprotected_tag.json")

        self.assertFalse(result.passed)
        self.assertIn("no active update-and-deletion ruleset protects refs/tags/v0.2.1", result.errors)

    def test_prerelease_is_explicitly_rejected(self) -> None:
        snapshot = checker.load_snapshot(FIXTURES / "prerelease.json")

        with self.assertRaisesRegex(ValueError, "prereleases are rejected"):
            checker.evaluate_snapshot(
                snapshot,
                self.policy,
                self.required_checks,
                self.tag_policy,
            )

    def test_newer_pending_check_hides_older_success(self) -> None:
        snapshot = checker.load_snapshot(FIXTURES / "valid.json")
        older = next(item for item in snapshot["checks"] if item["name"] == "test")
        older["completed_at"] = "2026-09-05T05:46:02Z"
        pending = copy.deepcopy(older)
        pending.update(
            {
                "id": 999,
                "status": "in_progress",
                "conclusion": None,
                "started_at": "2026-09-05T06:00:00Z",
                "completed_at": None,
            }
        )
        snapshot["checks"].append(pending)

        result = checker.evaluate_snapshot(
            snapshot, self.policy, self.required_checks, self.tag_policy
        )

        self.assertFalse(result.passed)
        self.assertIn("latest test (app 15368) is in_progress/None", "\n".join(result.errors))

    def test_unclassified_workspace_member_is_rejected(self) -> None:
        snapshot = checker.load_snapshot(FIXTURES / "valid.json")
        snapshot["workspace_members"].append("new-crate")

        result = checker.evaluate_snapshot(
            snapshot, self.policy, self.required_checks, self.tag_policy
        )

        self.assertFalse(result.passed)
        self.assertIn("workspace packages unclassified by release policy: new-crate", result.errors)

    def test_rejections_never_publish_or_export_success_outputs(self) -> None:
        rejected = (
            "parent_commit_checks.json",
            "bad_required_checks.json",
            "changed_synchronized_version.json",
            "unprotected_tag.json",
        )
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(
            checker, "gh_api_json", side_effect=AssertionError("fixture mode must not call GitHub")
        ):
            for fixture in rejected:
                output = Path(tmp) / f"{fixture}.out"
                status, stdout, _ = self.run_main(fixture, output)
                self.assertEqual(status, 1, fixture)
                self.assertNotIn("PASS release gate", stdout)
                self.assertFalse(output.exists(), fixture)

    def test_release_workflow_keeps_every_write_downstream_of_gate(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        gate = text.index("  release-gate:")
        release = text.index("  create-release:")
        action = text.index("softprops/action-gh-release@v2")

        self.assertLess(gate, release)
        self.assertLess(release, action)
        self.assertIn("    needs: release-gate", text[release:action])
        self.assertIn("--dry-run", text[gate:release])
        self.assertIn("--github-output \"$GITHUB_OUTPUT\"", text[gate:release])
        self.assertNotIn("gh release create", (REPO_ROOT / "tools/check_release_gate.py").read_text())
        self.assertNotIn("gh release upload", (REPO_ROOT / "tools/check_release_gate.py").read_text())

    def test_annotated_and_lightweight_tags_resolve_to_full_commit(self) -> None:
        annotated_ref = {"object": {"sha": "1" * 40, "type": "tag"}}
        annotated = {"object": {"sha": "2" * 40, "type": "commit"}}
        with mock.patch.object(checker, "gh_api_json", side_effect=[annotated_ref, annotated]):
            self.assertEqual(checker.resolve_tag_commit("o/r", "v1.0.0"), ("1" * 40, "2" * 40))

        lightweight = {"object": {"sha": "3" * 40, "type": "commit"}}
        with mock.patch.object(checker, "gh_api_json", return_value=lightweight):
            self.assertEqual(checker.resolve_tag_commit("o/r", "v1.0.0"), ("3" * 40, "3" * 40))

    def test_github_content_base64_accepts_api_line_wrapping(self) -> None:
        self.assertEqual(
            checker._decode_content({"encoding": "base64", "content": "aGVs\nbG8=\n"}, "fixture"),
            "hello",
        )

    def test_tag_ruleset_must_be_active_and_match_exact_ref(self) -> None:
        ruleset = checker.load_snapshot(FIXTURES / "valid.json")["rulesets"][0]
        self.assertTrue(checker.tag_policy_targets_tag(self.tag_policy, "v0.2.1"))
        self.assertTrue(checker.ruleset_targets_tag(ruleset, "v0.2.1"))
        disabled = copy.deepcopy(ruleset)
        disabled["enforcement"] = "disabled"
        self.assertFalse(checker.ruleset_targets_tag(disabled, "v0.2.1"))
        excluded = copy.deepcopy(ruleset)
        excluded["conditions"]["ref_name"]["exclude"] = ["refs/tags/v0.2.1"]
        self.assertFalse(checker.ruleset_targets_tag(excluded, "v0.2.1"))


if __name__ == "__main__":
    unittest.main()
