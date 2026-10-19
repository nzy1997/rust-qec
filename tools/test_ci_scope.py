"""Regression controls for CI selection, including deleted/renamed source files."""

from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest

from tools.ci_scope import changed_paths, select_scope


class ScopeTests(unittest.TestCase):
    def test_site_only_pr_does_not_compile_rust_or_collect_coverage(self):
        self.assertEqual(select_scope("pull_request", ["site/templates/benchmark-simulation.html"]),
                         {"rust": False, "coverage": False})

    def test_repository_documentation_stays_light(self):
        self.assertFalse(select_scope("pull_request", ["docs/maintainer-reference.md", "rsinter/README.md"])["rust"])

    def test_package_contract_documentation_keeps_rust_tests(self):
        for path in ["qec-code/doc/hyperbolic_5_5_contract.md",
                     "qec-code/doc/perturbed_hgp_contract.md",
                     "qec-code/doc/css_distance.md", "qec-code/docs/future_contract.md",
                     "rstim/doc/QP101-ZY.md"]:
            with self.subTest(path=path):
                self.assertTrue(select_scope("pull_request", [path])["rust"])

    def test_unknown_paths_and_runtime_inputs_fail_towards_full_tests(self):
        for path in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml",
                     "Makefile", "tools/ci_scope.py", "rstim/tests/fixtures/input.json",
                     "rstim/assets/shot-viewer/pkg/module.wasm", "new-package/config.json",
                     "docs/example.rs", "docs/example/Cargo.toml", "docs/envelope-support.json",
                     "rstim/doc/examples/load_training_data.py",
                     "rstim/doc/examples/load_blinded_training_data.py",
                     "site/static/install.sh", ".github/workflows/ci.yml"]:
            with self.subTest(path=path):
                self.assertTrue(select_scope("pull_request", [path])["rust"])

    def test_mixed_documentation_and_source_keeps_rust_tests(self):
        self.assertTrue(select_scope("pull_request", ["README.md", "rstim/src/parser.rs"])["rust"])

    def test_full_runs_keep_tests_and_coverage_even_for_docs(self):
        for event in ["push", "schedule", "workflow_dispatch"]:
            self.assertEqual(select_scope(event, ["README.md"]), {"rust": True, "coverage": True})

    def test_missing_base_cannot_report_a_light_pr(self):
        with self.assertRaisesRegex(ValueError, "base commit"):
            changed_paths("")


class GitSelectionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.git("init", "-q")
        self.git("config", "user.name", "CI test")
        self.git("config", "user.email", "ci@example.invalid")
        (self.root / "src").mkdir()
        (self.root / "src/lib.rs").write_text("fn example() {}\n")
        self.commit()
        self.base = self.git("rev-parse", "HEAD").strip()

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, text=True, stderr=subprocess.PIPE)

    def commit(self):
        self.git("add", "-A")
        self.git("commit", "-qm", "Test input")

    def test_source_renamed_into_docs_still_runs_tests(self):
        (self.root / "docs").mkdir()
        self.git("mv", "src/lib.rs", "docs/archive.txt")
        self.commit()
        changed = changed_paths(self.base, cwd=self.root)
        self.assertEqual(set(changed), {"src/lib.rs", "docs/archive.txt"})
        self.assertTrue(select_scope("pull_request", changed)["rust"])

    def test_deleted_fixture_still_runs_tests(self):
        (self.root / "src/lib.rs").unlink()
        self.commit()
        self.assertTrue(select_scope("pull_request", changed_paths(self.base, cwd=self.root))["rust"])

    def prepare_contract(self, name):
        path = self.root / "qec-code/doc" / name
        path.parent.mkdir(parents=True)
        path.write_text("# Contract\ncontract_version: 1\n")
        self.commit()
        return path, self.git("rev-parse", "HEAD").strip()

    def test_modified_contract_document_runs_rust_tests(self):
        path, base = self.prepare_contract("hyperbolic_5_5_contract.md")
        path.write_text("# Incomplete contract\n")
        self.commit()
        changed = changed_paths(base, cwd=self.root)
        self.assertEqual(changed, ["qec-code/doc/hyperbolic_5_5_contract.md"])
        self.assertTrue(select_scope("pull_request", changed)["rust"])

    def test_deleted_contract_document_runs_rust_tests(self):
        path, base = self.prepare_contract("perturbed_hgp_contract.md")
        path.unlink()
        self.commit()
        changed = changed_paths(base, cwd=self.root)
        self.assertEqual(changed, ["qec-code/doc/perturbed_hgp_contract.md"])
        self.assertFalse(path.exists())
        self.assertTrue(select_scope("pull_request", changed)["rust"])

    def test_base_branch_only_changes_do_not_expand_pr_scope(self):
        self.git("checkout", "-qb", "base-update")
        (self.root / "src/lib.rs").write_text("fn changed_base() {}\n")
        self.commit()
        updated_base = self.git("rev-parse", "HEAD").strip()
        self.git("checkout", "-qb", "pr", self.base)
        (self.root / "README.md").write_text("Documentation update\n")
        self.commit()
        self.assertEqual(changed_paths(updated_base, cwd=self.root), ["README.md"])

    def test_newline_in_path_cannot_split_into_false_documentation(self):
        (self.root / "src/unusual\nREADME.md").write_text("runtime fixture\n")
        self.commit()
        changed = changed_paths(self.base, cwd=self.root)
        self.assertEqual(changed, ["src/unusual\nREADME.md"])
        self.assertTrue(select_scope("pull_request", changed)["rust"])

    def test_cli_reports_outputs_and_does_not_hide_diff_failures(self):
        script = Path(__file__).with_name("ci_scope.py").resolve()
        output = self.root / "outputs"
        env = {**os.environ, "GITHUB_OUTPUT": str(output)}
        good = subprocess.run([sys.executable, str(script), "--event", "pull_request", "--base", self.base],
                              cwd=self.root, env=env, capture_output=True, text=True)
        self.assertEqual(good.returncode, 0, good.stderr)
        self.assertEqual(output.read_text(), "rust=false\ncoverage=false\n")
        output.unlink()
        bad = subprocess.run([sys.executable, str(script), "--event", "pull_request", "--base", "missing-ref"],
                             cwd=self.root, env=env, capture_output=True, text=True)
        self.assertNotEqual(bad.returncode, 0)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
