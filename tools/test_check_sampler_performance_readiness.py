#!/usr/bin/env python3
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_sampler_performance_readiness.py"
CATALOG = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
COMMITTED_JSON = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json"
COMMITTED_MD = REPO_ROOT / "sampler-performance-readiness.md"
PASS_LINE = "PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05\n"


class SamplerPerformanceReadinessCheckerTest(unittest.TestCase):
    def run_checker(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), *args],
            cwd=REPO_ROOT,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_help_imports_without_side_effects(self) -> None:
        result = self.run_checker("--help")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--catalog", result.stdout)
        self.assertIn("--verify-github", result.stdout)

    def test_cli_accepts_committed_catalog_and_writes_derived_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "readiness.json"
            markdown = Path(tmp) / "readiness.md"

            result = self.run_checker("--catalog", str(CATALOG), "--out", str(out), "--markdown-out", str(markdown))

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, PASS_LINE)
            self.assertEqual(result.stderr, "")
            readiness = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(readiness["status"], "ready")
            self.assertEqual(readiness["bundle_count"], 4)
            self.assertGreaterEqual(readiness["reference_build"]["direct_speedup"], 2.0)
            self.assertEqual(readiness["reference_build"]["direct_canonical_materializations"], 0)
            self.assertEqual(readiness["reference_build"]["direct_executed_repeat_iterations"], 1)
            self.assertLessEqual(readiness["frame_noise"]["candidate_over_baseline"], 1.05)
            self.assertEqual(readiness["frame_noise"]["correctness_status"], "pass")
            self.assertEqual(readiness["distribution_correctness"]["status"], "pass")
            self.assertEqual(readiness["historical_406"]["status"], "preserved")
            self.assertIn("#379", "\n".join(readiness["claim_limits"]))
            text = markdown.read_text(encoding="utf-8")
            for required in (
                "fair-cli-release",
                "compiled-steady-release",
                "reference-build-release",
                "frame-instruction-wide-release",
                "#38",
                "#406",
                "#379",
            ):
                self.assertIn(required, text)

    def test_committed_markdown_is_derived_from_committed_json(self) -> None:
        checker = __import__("tools.check_sampler_performance_readiness", fromlist=["render_markdown"])
        readiness = json.loads(COMMITTED_JSON.read_text(encoding="utf-8"))

        self.assertEqual(COMMITTED_MD.read_text(encoding="utf-8"), checker.render_markdown(readiness))

    def test_absolute_catalog_provenance_reports_not_ready(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            temp_repo = Path(tmp) / "repo"
            shutil.copytree(REPO_ROOT / "benchmarks", temp_repo / "benchmarks")
            catalog = temp_repo / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
            text = catalog.read_text(encoding="utf-8")
            text = text.replace(
                'value = { case_id = "stim_surface_d11_r100", profile = "release"',
                'host_path = "/tmp/provenance.json"\nvalue = { case_id = "stim_surface_d11_r100", profile = "release"',
                1,
            )
            catalog.write_text(text, encoding="utf-8")
            out = Path(tmp) / "readiness.json"

            result = self.run_checker("--catalog", str(catalog), "--out", str(out))

            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("not ready", result.stderr)
            self.assertIn("checked provenance contains host-absolute path", result.stderr)

    def test_reference_speedup_below_two_reports_not_ready(self) -> None:
        checker = __import__("tools.check_sampler_performance_readiness", fromlist=["ReadinessError", "build_readiness"])
        with mock.patch.object(checker.reference_build, "validate_bundle", return_value={"direct_speedup": 1.99}):
            with self.assertRaisesRegex(checker.ReadinessError, "not ready: reference direct/canonical speedup"):
                checker.build_readiness(CATALOG)

    def test_frame_ratio_above_limit_reports_not_ready(self) -> None:
        checker = __import__("tools.check_sampler_performance_readiness", fromlist=["ReadinessError", "build_readiness"])
        replacement = {
            "builds": 803,
            "attempts": 82_290_688,
            "legacy_setups": 80_362,
            "candidate_over_baseline": 1.06,
            "outcome": "regressed",
        }
        with mock.patch.object(checker.instruction_wide, "validate_bundle", return_value=replacement):
            with self.assertRaisesRegex(checker.ReadinessError, "not ready: frame candidate/baseline ratio"):
                checker.build_readiness(CATALOG)

    def test_mocked_open_github_milestone_fails_with_title(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            github_json = Path(tmp) / "issues.json"
            github_json.write_text(
                json.dumps(
                    [
                        {
                            "number": 999,
                            "title": "Operational sampler-performance milestone closure",
                            "state": "OPEN",
                            "milestone": {"title": "M4: Measured Optimization Closure"},
                        }
                    ]
                ),
                encoding="utf-8",
            )
            out = Path(tmp) / "readiness.json"

            result = self.run_checker(
                "--catalog",
                str(CATALOG),
                "--out",
                str(out),
                "--verify-github",
                "nzy1997/rstim",
                "--github-json",
                str(github_json),
            )

            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("not ready", result.stderr)
            self.assertIn("Operational sampler-performance milestone closure", result.stderr)

    def test_mocked_closed_github_milestone_succeeds(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            github_json = Path(tmp) / "issues.json"
            github_json.write_text(
                json.dumps(
                    [
                        {
                            "number": 999,
                            "title": "Completed sampler-performance milestone closure",
                            "state": "CLOSED",
                            "milestone": {"title": "M4: Measured Optimization Closure"},
                        }
                    ]
                ),
                encoding="utf-8",
            )
            out = Path(tmp) / "readiness.json"

            result = self.run_checker(
                "--catalog",
                str(CATALOG),
                "--out",
                str(out),
                "--verify-github",
                "nzy1997/rstim",
                "--github-json",
                str(github_json),
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, PASS_LINE)
            self.assertEqual(json.loads(out.read_text(encoding="utf-8"))["issues"]["milestone"]["status"], "closed")


if __name__ == "__main__":
    unittest.main()
