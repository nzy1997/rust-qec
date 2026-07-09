#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_release_speed_case.py"
CASE_LABEL = "rep-sample-d13-r13"
REQUIRED_VARIANTS = "stim-cli,rstim-interpreted,rstim-compiled"


def valid_summary() -> dict[str, object]:
    return {
        "cases": [
            {
                "case_label": CASE_LABEL,
                "workload": "sample",
                "tier": "gating",
                "present_variants": ["rstim-compiled", "rstim-interpreted", "stim-cli"],
                "variants": [
                    {"tool_variant": "rstim-compiled", "status": "completed"},
                    {"tool_variant": "rstim-interpreted", "status": "completed"},
                    {"tool_variant": "stim-cli", "status": "completed"},
                ],
            }
        ],
        "issues": [],
    }


def valid_environment() -> dict[str, object]:
    return {
        "profile": "release",
        "case_labels": [CASE_LABEL],
        "case_count": 1,
        "rstim_binary_path": "/tmp/target/release/rstim",
        "rustc_version": "rustc 1.93.1",
        "cargo_version": "cargo 1.93.1",
        "stim_cli_status": "ok",
    }


class ReleaseSpeedCaseCheckerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmpdir.cleanup)
        self.results_dir = Path(self.tmpdir.name) / "results"
        self.results_dir.mkdir()
        self.write_bundle(valid_summary(), valid_environment())

    def write_bundle(self, summary: dict[str, object], environment: dict[str, object]) -> None:
        (self.results_dir / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
        (self.results_dir / "environment.json").write_text(json.dumps(environment), encoding="utf-8")
        (self.results_dir / "report.md").write_text(f"# Report\n\n### {CASE_LABEL}\n", encoding="utf-8")

    def write_report(self, report: str) -> None:
        (self.results_dir / "report.md").write_text(report, encoding="utf-8")

    def run_checker(
        self,
        *,
        case_label: str = CASE_LABEL,
        workload: str = "sample",
        required_variants: str = REQUIRED_VARIANTS,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "python3",
                str(CHECKER),
                "--results-dir",
                str(self.results_dir),
                "--case",
                case_label,
                "--workload",
                workload,
                "--required-variants",
                required_variants,
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_accepts_valid_release_case(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, f"PASS release speed case {CASE_LABEL}\n")

    def test_rejects_missing_required_variant(self) -> None:
        summary = valid_summary()
        case = summary["cases"][0]  # type: ignore[index]
        assert isinstance(case, dict)
        case["present_variants"] = ["rstim-compiled", "rstim-interpreted"]
        case["variants"] = [
            {"tool_variant": "rstim-compiled", "status": "completed"},
            {"tool_variant": "rstim-interpreted", "status": "completed"},
        ]
        self.write_bundle(summary, valid_environment())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required variant stim-cli", result.stderr)

    def test_rejects_duplicate_requested_case(self) -> None:
        summary = valid_summary()
        summary["cases"].append(copy.deepcopy(summary["cases"][0]))  # type: ignore[attr-defined,index]
        self.write_bundle(summary, valid_environment())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("case rep-sample-d13-r13 must be present exactly once", result.stderr)

    def test_rejects_wrong_workload(self) -> None:
        summary = valid_summary()
        case = summary["cases"][0]  # type: ignore[index]
        assert isinstance(case, dict)
        case["workload"] = "detect"
        self.write_bundle(summary, valid_environment())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"workload mismatch for {CASE_LABEL}", result.stderr)

    def test_rejects_surface_detect_labeled_as_sample(self) -> None:
        case_label = "surface-detect-d13-r13"
        summary: dict[str, object] = {
            "cases": [
                {
                    "case_label": case_label,
                    "workload": "sample",
                    "tier": "gating",
                    "present_variants": ["rstim-compiled", "rstim-interpreted", "stim-cli"],
                    "variants": [
                        {
                            "tool_variant": "rstim-compiled",
                            "status": "completed",
                            "median_wall_time_ns": 10,
                        },
                        {
                            "tool_variant": "rstim-interpreted",
                            "status": "completed",
                            "median_wall_time_ns": 20,
                        },
                        {
                            "tool_variant": "stim-cli",
                            "status": "completed",
                            "median_wall_time_ns": 30,
                        },
                    ],
                }
            ],
            "issues": [],
        }
        environment = valid_environment()
        environment["case_labels"] = [case_label]
        self.write_bundle(summary, environment)
        self.write_report(f"# Report\n\n### {case_label}\n")
        result = self.run_checker(case_label=case_label, workload="detect")
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("workload mismatch for surface-detect-d13-r13", result.stderr)

    def test_rejects_required_variant_not_completed(self) -> None:
        summary = valid_summary()
        case = summary["cases"][0]  # type: ignore[index]
        assert isinstance(case, dict)
        variants = case["variants"]
        assert isinstance(variants, list)
        variants[0]["status"] = "tool_failed"  # type: ignore[index]
        self.write_bundle(summary, valid_environment())
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("required variant rstim-compiled status is not completed", result.stderr)

    def test_rejects_bad_release_profile(self) -> None:
        environment = valid_environment()
        environment["profile"] = "debug"
        self.write_bundle(valid_summary(), environment)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment.json profile must be release", result.stderr)

    def test_rejects_missing_case_labels(self) -> None:
        environment = valid_environment()
        del environment["case_labels"]
        self.write_bundle(valid_summary(), environment)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"environment.json missing case label {CASE_LABEL}", result.stderr)

    def test_rejects_wrong_case_label(self) -> None:
        environment = valid_environment()
        environment["case_labels"] = ["rep-sample-d13-r14"]
        self.write_bundle(valid_summary(), environment)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"environment.json missing case label {CASE_LABEL}", result.stderr)

    def test_rejects_missing_rustc_version(self) -> None:
        environment = valid_environment()
        del environment["rustc_version"]
        self.write_bundle(valid_summary(), environment)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment.json missing rustc_version", result.stderr)

    def test_rejects_missing_cargo_version(self) -> None:
        environment = valid_environment()
        del environment["cargo_version"]
        self.write_bundle(valid_summary(), environment)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment.json missing cargo_version", result.stderr)

    def test_rejects_missing_stim_cli_status(self) -> None:
        environment = valid_environment()
        del environment["stim_cli_status"]
        self.write_bundle(valid_summary(), environment)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment.json missing stim_cli_status", result.stderr)

    def test_rejects_missing_environment_metadata(self) -> None:
        environment = valid_environment()
        del environment["rstim_binary_path"]
        self.write_bundle(valid_summary(), environment)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment.json missing rstim_binary_path", result.stderr)

    def test_rejects_missing_report_file(self) -> None:
        (self.results_dir / "report.md").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required release file: report.md", result.stderr)

    def test_rejects_report_missing_case_label(self) -> None:
        self.write_report("# Report\n\n### unrelated-case\n")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"report.md missing case label {CASE_LABEL}", result.stderr)

    def test_rejects_report_broad_performance_claim(self) -> None:
        self.write_report(
            "# Report\n\n### "
            f"{CASE_LABEL}\n\nThe results show broad speed superiority across all workloads.\n",
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("report.md contains forbidden broad performance claim", result.stderr)

    def test_rejects_unexpected_release_file(self) -> None:
        (self.results_dir / "raw.jsonl").write_text("{}", encoding="utf-8")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("unexpected release file: raw.jsonl", result.stderr)

    def test_rejects_unexpected_release_directory(self) -> None:
        (self.results_dir / "scratch").mkdir()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("unexpected release file: scratch", result.stderr)


if __name__ == "__main__":
    unittest.main()
