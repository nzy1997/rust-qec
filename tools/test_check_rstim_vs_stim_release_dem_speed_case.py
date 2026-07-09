#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from tools import check_rstim_vs_stim_release_dem_speed_case as checker


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_release_dem_speed_case.py"


class ReleaseDemSpeedCaseCheckerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmpdir.cleanup)
        self.results_dir = Path(self.tmpdir.name) / "results"
        self.results_dir.mkdir(parents=True)

    def run_checker(
        self,
        *,
        results_dir: Path | None = None,
        case: str = checker.DEFAULT_CASE_LABEL,
        required_variants: list[str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        args = [
            "python3",
            str(CHECKER),
            "--results-dir",
            str(results_dir or self.results_dir),
            "--case",
            case,
            "--required-variants",
            *(required_variants or list(checker.DEFAULT_REQUIRED_VARIANTS)),
        ]
        return subprocess.run(
            args,
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def write_valid_fixture(self) -> None:
        raw_records = [
            {
                "case_label": checker.DEFAULT_CASE_LABEL,
                "workload": checker.EXPECTED_WORKLOAD,
                "tool_variant": "stim-sample-dem",
                "phase": "measure",
                "round_index": 0,
                "shots": checker.EXPECTED_SHOTS,
                "status": "completed",
                "elapsed_ns": 1000,
                "exit_code": 0,
                "stderr": None,
                "command": ["stim", "sample_dem", "--shots", str(checker.EXPECTED_SHOTS)],
            },
            {
                "case_label": checker.DEFAULT_CASE_LABEL,
                "workload": checker.EXPECTED_WORKLOAD,
                "tool_variant": "rstim-sample-dem",
                "phase": "measure",
                "round_index": 0,
                "shots": checker.EXPECTED_SHOTS,
                "status": "completed",
                "elapsed_ns": 1200,
                "exit_code": 0,
                "stderr": None,
                "command": ["rstim", "sample_dem", "--shots", str(checker.EXPECTED_SHOTS)],
            },
        ]
        (self.results_dir / "raw.jsonl").write_text(
            "".join(json.dumps(record, sort_keys=True) + "\n" for record in raw_records),
            encoding="utf-8",
        )

        summary = {
            "cases": [
                {
                    "case_label": checker.DEFAULT_CASE_LABEL,
                    "workload": checker.EXPECTED_WORKLOAD,
                    "tier": "report_only",
                    "expected_variants": list(checker.DEFAULT_REQUIRED_VARIANTS),
                    "present_variants": sorted(checker.DEFAULT_REQUIRED_VARIANTS),
                    "variants": [
                        {
                            "tool_variant": "stim-sample-dem",
                            "sample_count": 1,
                            "median_wall_time_ns": 1000,
                            "median_shots_per_second": 1024.0,
                            "status": "completed",
                            "failure_reason": None,
                            "stderr": None,
                        },
                        {
                            "tool_variant": "rstim-sample-dem",
                            "sample_count": 1,
                            "median_wall_time_ns": 1200,
                            "median_shots_per_second": 853.3333333333334,
                            "status": "completed",
                            "failure_reason": None,
                            "stderr": None,
                        },
                    ],
                }
            ],
            "issues": [],
        }
        (self.results_dir / "summary.json").write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (self.results_dir / "report.md").write_text(
            "# DEM Sampling Report\n\n"
            f"## {checker.DEFAULT_CASE_LABEL}\n\n"
            "- workload: sample_dem\n",
            encoding="utf-8",
        )
        environment = {
            "profile": "release",
            "case_label": checker.DEFAULT_CASE_LABEL,
            "case_labels": [checker.DEFAULT_CASE_LABEL],
            "case_count": 1,
            "command_line": ["run-dem-speed-case", "--profile", "release"],
            "dem_path": str(checker.EXPECTED_DEM_PATH),
            "dem_sha256": checker.EXPECTED_DEM_SHA256,
            "source_circuit_path": str(checker.EXPECTED_SOURCE_CIRCUIT_PATH),
            "source_circuit_sha256": checker.EXPECTED_SOURCE_CIRCUIT_SHA256,
            "expected_detectors": checker.EXPECTED_DETECTOR_COUNT,
            "expected_observables": checker.EXPECTED_OBSERVABLE_COUNT,
        }
        (self.results_dir / "environment.json").write_text(
            json.dumps(environment, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def test_accepts_valid_release_dem_speed_fixture(self) -> None:
        self.write_valid_fixture()
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"PASS release DEM speed case {checker.DEFAULT_CASE_LABEL}", result.stdout)

    def test_rejects_missing_required_variant(self) -> None:
        self.write_valid_fixture()
        summary_path = self.results_dir / "summary.json"
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        case_summary = summary["cases"][0]
        case_summary["present_variants"] = ["stim-sample-dem"]
        case_summary["variants"] = [
            variant
            for variant in case_summary["variants"]
            if variant["tool_variant"] != "rstim-sample-dem"
        ]
        summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required variant rstim-sample-dem", result.stderr)

    def test_rejects_summary_with_issues(self) -> None:
        self.write_valid_fixture()
        summary_path = self.results_dir / "summary.json"
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        summary["issues"] = [{"tool_variant": "stim-sample-dem", "status": "tool_failed"}]
        summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("summary issues must be []", result.stderr)

    def test_rejects_bad_dem_metadata(self) -> None:
        self.write_valid_fixture()
        environment_path = self.results_dir / "environment.json"
        environment = json.loads(environment_path.read_text(encoding="utf-8"))
        bad_environment = copy.deepcopy(environment)
        bad_environment["expected_detectors"] = 999
        environment_path.write_text(
            json.dumps(bad_environment, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("DEM metadata mismatch", result.stderr)

    def test_rejects_missing_raw_jsonl(self) -> None:
        self.write_valid_fixture()
        (self.results_dir / "raw.jsonl").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required release file: raw.jsonl", result.stderr)


if __name__ == "__main__":
    unittest.main()
