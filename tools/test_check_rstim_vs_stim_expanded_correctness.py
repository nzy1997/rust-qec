#!/usr/bin/env python3
from __future__ import annotations

import contextlib
import hashlib
import io
import json
import tempfile
import unittest
from pathlib import Path
 
from tools.check_rstim_vs_stim_expanded_correctness import PASS_LINE, main


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
        self.summary = self.root / "summary.json"
        self.rollup = self.root / "expanded-correctness.json"
        self.report = self.root / "report.md"
        self.full_summary = self.root / "correctness-summary.json"

    def run_checker(self) -> int:
        return main(
            [
                "--catalog",
                str(self.catalog),
                "--summary",
                str(self.summary),
                "--rollup",
                str(self.rollup),
                "--report",
                str(self.report),
                "--full-summary",
                str(self.full_summary),
            ]
        )

    def write_text(self, path: Path, text: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def write_json(self, path: Path, value: object) -> None:
        self.write_text(path, json.dumps(value, indent=2, sort_keys=True) + "\n")

    def write_valid_fixture(self) -> None:
        self.write_text(
            self.catalog,
            (
                "manifest_version = 1\n"
                "suite = \"rstim_vs_stim_simulator\"\n"
                "\n"
                "[[cases]]\n"
                "case_id = \"case_alpha\"\n"
                "\n"
                "[[cases]]\n"
                "case_id = \"case_beta\"\n"
            ),
        )
        self.write_json(
            self.summary,
            {
                "suite": "rstim_vs_stim_simulator",
                "status": "pass",
                "catalog_sha256": sha256_text(self.catalog),
                "cases": [
                    {"case_id": "case_alpha", "status": "pass"},
                    {"case_id": "case_beta", "status": "pass"},
                ],
            },
        )
        self.write_json(
            self.full_summary,
            {
                "suite": "rstim_vs_stim_simulator",
                "status": "pass",
                "case_count": 1,
            },
        )
        self.write_json(
            self.rollup,
            {
                "status": "pass",
                "distribution_summary_path": str(self.summary),
                "distribution_summary_sha256": sha256_text(self.summary),
                "full_summary_path": str(self.full_summary),
                "full_summary_sha256": sha256_text(self.full_summary),
            },
        )
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
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            code = self.run_checker()

        self.assertEqual(code, 0)
        self.assertEqual(stdout.getvalue().strip(), PASS_LINE)

    def test_rejects_missing_distribution_catalog_case(self) -> None:
        self.write_valid_fixture()
        summary = json.loads(self.summary.read_text(encoding="utf-8"))
        summary["cases"] = [summary["cases"][0]]
        self.write_json(self.summary, summary)
        rollup = json.loads(self.rollup.read_text(encoding="utf-8"))
        rollup["distribution_summary_sha256"] = sha256_text(self.summary)
        self.write_json(self.rollup, rollup)

        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            code = self.run_checker()

        self.assertEqual(code, 1)
        self.assertIn("missing distribution evidence for case", stderr.getvalue())

    def test_rejects_distribution_case_that_did_not_pass(self) -> None:
        self.write_valid_fixture()
        summary = json.loads(self.summary.read_text(encoding="utf-8"))
        summary["cases"][1]["status"] = "fail"
        self.write_json(self.summary, summary)
        rollup = json.loads(self.rollup.read_text(encoding="utf-8"))
        rollup["distribution_summary_sha256"] = sha256_text(self.summary)
        self.write_json(self.rollup, rollup)

        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            code = self.run_checker()

        self.assertEqual(code, 1)
        self.assertIn("distribution evidence for case case_beta did not pass", stderr.getvalue())

    def test_rejects_stale_catalog_hash(self) -> None:
        self.write_valid_fixture()
        summary = json.loads(self.summary.read_text(encoding="utf-8"))
        summary["catalog_sha256"] = "0" * 64
        self.write_json(self.summary, summary)
        rollup = json.loads(self.rollup.read_text(encoding="utf-8"))
        rollup["distribution_summary_sha256"] = sha256_text(self.summary)
        self.write_json(self.rollup, rollup)

        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            code = self.run_checker()

        self.assertEqual(code, 1)
        self.assertIn("distribution summary catalog hash mismatch", stderr.getvalue())

    def test_rejects_full_summary_without_pass_status(self) -> None:
        self.write_valid_fixture()
        full_summary = json.loads(self.full_summary.read_text(encoding="utf-8"))
        full_summary["status"] = "fail"
        self.write_json(self.full_summary, full_summary)
        rollup = json.loads(self.rollup.read_text(encoding="utf-8"))
        rollup["full_summary_sha256"] = sha256_text(self.full_summary)
        self.write_json(self.rollup, rollup)

        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            code = self.run_checker()

        self.assertEqual(code, 1)
        self.assertIn("full correctness summary status is not pass", stderr.getvalue())

    def test_rejects_rollup_summary_hash_mismatch(self) -> None:
        self.write_valid_fixture()
        rollup = json.loads(self.rollup.read_text(encoding="utf-8"))
        rollup["distribution_summary_sha256"] = "f" * 64
        self.write_json(self.rollup, rollup)

        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            code = self.run_checker()

        self.assertEqual(code, 1)
        self.assertIn("expanded rollup distribution summary hash mismatch", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
