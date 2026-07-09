#!/usr/bin/env python3
from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_expanded_correctness.py"


def sha256_text(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


class ExpandedCorrectnessCheckerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmpdir.cleanup)
        self.root = Path(self.tmpdir.name)
        self.catalog = self.root / "distribution_cases.toml"
        self.distribution_dir = self.root / "distribution"
        self.distribution_dir.mkdir()
        self.summary = self.distribution_dir / "summary.json"
        self.rollup = self.distribution_dir / "expanded-correctness.json"
        self.report = self.distribution_dir / "report.md"
        self.full_summary = self.root / "correctness-summary.json"

    def run_checker(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "python3",
                str(CHECKER),
                "--catalog",
                str(self.catalog),
                "--distribution-dir",
                str(self.distribution_dir),
                "--full-summary",
                str(self.full_summary),
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def write_text(self, path: Path, text: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def write_json(self, path: Path, value: object) -> None:
        self.write_text(path, json.dumps(value, indent=2, sort_keys=True) + "\n")

    def valid_catalog_text(self) -> str:
        return (
            "manifest_version = 1\n"
            "suite = \"rstim_vs_stim_simulator\"\n"
            "\n"
            "[[cases]]\n"
            "case_id = \"case_alpha\"\n"
            "\n"
            "[[cases]]\n"
            "case_id = \"case_beta\"\n"
        )

    def valid_summary(self) -> dict[str, object]:
        return {
            "suite": "rstim_vs_stim_simulator",
            "status": "pass",
            "catalog_sha256": sha256_text(self.catalog),
            "cases": [
                {"case_id": "case_alpha", "status": "pass"},
                {"case_id": "case_beta", "status": "pass"},
            ],
        }

    def valid_full_summary(self) -> dict[str, object]:
        return {
            "suite": "rstim_vs_stim_simulator",
            "status": "pass",
            "case_count": 1,
        }

    def valid_rollup(self) -> dict[str, object]:
        return {
            "status": "pass",
            "distribution_summary_path": str(self.summary),
            "distribution_summary_sha256": sha256_text(self.summary),
            "full_summary_path": str(self.full_summary),
            "full_summary_sha256": sha256_text(self.full_summary),
        }

    def write_valid_fixture(self) -> None:
        self.write_text(self.catalog, self.valid_catalog_text())
        self.write_json(self.summary, self.valid_summary())
        self.write_json(self.full_summary, self.valid_full_summary())
        self.write_json(self.rollup, self.valid_rollup())
        self.write_text(
            self.report,
            (
                "# Expanded correctness report\n"
                f"- Distribution summary: `{self.summary}`\n"
                f"- Rollup manifest: `{self.rollup}`\n"
                f"- Full correctness summary: `{self.full_summary}`\n"
            ),
        )

    def test_accepts_complete_expanded_evidence(self) -> None:
        self.write_valid_fixture()
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, PASS_LINE := "PASS expanded rstim-vs-Stim correctness evidence\n")

    def test_rejects_missing_distribution_catalog_case(self) -> None:
        self.write_valid_fixture()
        summary = copy.deepcopy(json.loads(self.summary.read_text(encoding="utf-8")))
        assert isinstance(summary, dict)
        summary["cases"] = [summary["cases"][0]]  # type: ignore[index]
        self.write_json(self.summary, summary)
        self.write_json(self.rollup, self.valid_rollup())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing distribution evidence for case", result.stderr)

    def test_rejects_distribution_case_that_did_not_pass(self) -> None:
        self.write_valid_fixture()
        summary = copy.deepcopy(json.loads(self.summary.read_text(encoding="utf-8")))
        assert isinstance(summary, dict)
        cases = summary["cases"]
        assert isinstance(cases, list)
        cases[1]["status"] = "fail"  # type: ignore[index]
        self.write_json(self.summary, summary)
        self.write_json(self.rollup, self.valid_rollup())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("distribution evidence for case case_beta did not pass", result.stderr)

    def test_rejects_stale_catalog_hash(self) -> None:
        self.write_valid_fixture()
        summary = copy.deepcopy(json.loads(self.summary.read_text(encoding="utf-8")))
        assert isinstance(summary, dict)
        summary["catalog_sha256"] = "0" * 64
        self.write_json(self.summary, summary)
        self.write_json(self.rollup, self.valid_rollup())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("distribution summary catalog hash mismatch", result.stderr)

    def test_rejects_full_summary_without_pass_status(self) -> None:
        self.write_valid_fixture()
        full_summary = self.valid_full_summary()
        full_summary["status"] = "fail"
        self.write_json(self.full_summary, full_summary)
        self.write_json(self.rollup, self.valid_rollup())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("full correctness summary status is not pass", result.stderr)

    def test_rejects_rollup_summary_hash_mismatch(self) -> None:
        self.write_valid_fixture()
        rollup = self.valid_rollup()
        rollup["distribution_summary_sha256"] = "f" * 64
        self.write_json(self.rollup, rollup)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("expanded rollup distribution summary hash mismatch", result.stderr)

    def test_rejects_distribution_summary_without_pass_status(self) -> None:
        self.write_valid_fixture()
        summary = self.valid_summary()
        summary["status"] = "fail"
        self.write_json(self.summary, summary)
        self.write_json(self.rollup, self.valid_rollup())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("distribution summary status is not pass", result.stderr)

    def test_rejects_distribution_summary_with_unknown_case_id(self) -> None:
        self.write_valid_fixture()
        summary = self.valid_summary()
        cases = summary["cases"]
        assert isinstance(cases, list)
        cases.append({"case_id": "case_gamma", "status": "pass"})
        self.write_json(self.summary, summary)
        self.write_json(self.rollup, self.valid_rollup())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("unknown distribution evidence case case_gamma", result.stderr)

    def test_rejects_rollup_without_pass_status(self) -> None:
        self.write_valid_fixture()
        rollup = self.valid_rollup()
        rollup["status"] = "fail"
        self.write_json(self.rollup, rollup)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("expanded correctness rollup status is not pass", result.stderr)

    def test_rejects_rollup_with_wrong_distribution_summary_path(self) -> None:
        self.write_valid_fixture()
        rollup = self.valid_rollup()
        rollup["distribution_summary_path"] = str(self.root / "wrong-summary.json")
        self.write_json(self.rollup, rollup)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("expanded rollup distribution summary path mismatch", result.stderr)

    def test_rejects_rollup_with_wrong_full_summary_path(self) -> None:
        self.write_valid_fixture()
        rollup = self.valid_rollup()
        rollup["full_summary_path"] = str(self.root / "wrong-full-summary.json")
        self.write_json(self.rollup, rollup)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("expanded rollup full summary path mismatch", result.stderr)

    def test_rejects_missing_distribution_file_cleanly(self) -> None:
        self.write_valid_fixture()
        self.summary.unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"missing required file: {self.summary}", result.stderr)

    def test_rejects_malformed_distribution_json_cleanly(self) -> None:
        self.write_valid_fixture()
        self.write_text(self.summary, "{bad json\n")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"invalid JSON in {self.summary}", result.stderr)

    def test_rejects_malformed_catalog_toml_cleanly(self) -> None:
        self.write_valid_fixture()
        self.write_text(self.catalog, "cases = [\n")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"invalid TOML in {self.catalog}", result.stderr)


if __name__ == "__main__":
    unittest.main()
