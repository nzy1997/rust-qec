from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import stim

from benchmarks.rstim_vs_stim_simulator.inspect_fixture_load import (
    find_case,
    build_report,
    summarize_circuit,
)
from benchmarks.rstim_vs_stim_simulator.validate_cases import load_manifest


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "rstim_vs_stim_simulator"
FULL_MANIFEST = PACKAGE_DIR / "cases.full.toml"


def run_inspector(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.inspect_fixture_load",
            *args,
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


class InspectFixtureLoadReportTest(unittest.TestCase):
    def test_full_fixture_report_matches_issue_contract(self) -> None:
        manifest = load_manifest(FULL_MANIFEST)
        case = find_case(manifest, "stim_surface_d11_r100")
        if case is None:
            self.fail("case stim_surface_d11_r100 not found")

        report = build_report(case, manifest_path=FULL_MANIFEST, base_dir=FULL_MANIFEST.parent)

        self.assertEqual(report["case_id"], "stim_surface_d11_r100")
        self.assertEqual(report["expected_measurements"], 12121)
        self.assertEqual(report["expected_detectors"], 12000)
        self.assertEqual(report["expected_observables"], 1)
        self.assertEqual(report["actual_measurements"], 12121)
        self.assertEqual(report["actual_detectors"], 12000)
        self.assertEqual(report["actual_observables"], 1)
        self.assertEqual(report["flattened_operation_count"], 14448)
        self.assertEqual(report["repeat_depth"], 1)
        self.assertEqual(report["repeat_expansion_count"], 99)
        self.assertEqual(report["expanded_operation_count"], 14547)
        self.assertEqual(report["operations"]["DEPOLARIZE2"]["target_count"], 88000)
        self.assertEqual(report["operations"]["DETECTOR"]["operation_count"], 12000)
        self.assertEqual(report["operations"]["REPEAT"]["operation_count"], 99)

    def test_cli_text_report_includes_human_counts(self) -> None:
        result = run_inspector(
            "--case",
            "stim_surface_d11_r100",
            "--manifest",
            str(FULL_MANIFEST),
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("expected_measurements=12121", result.stdout)
        self.assertIn("expected_detectors=12000", result.stdout)
        self.assertIn("expected_observables=1", result.stdout)
        self.assertIn("expanded_operation_count=14547", result.stdout)
        self.assertIn("actual_measurements=12121", result.stdout)
        self.assertIn("actual_detectors=12000", result.stdout)
        self.assertIn("actual_observables=1", result.stdout)
        self.assertIn("flattened_operation_count=14448", result.stdout)
        self.assertIn("repeat_block_count=1", result.stdout)
        self.assertIn("repeat_depth=1", result.stdout)
        self.assertIn("repeat_expansion_count=99", result.stdout)

        found_depola = False
        found_detector = False
        for line in result.stdout.splitlines():
            if line.startswith("  DEPOLARIZE2: "):
                value = json.loads(line.split(": ", 1)[1])
                self.assertEqual(value["target_count"], 88000)
                found_depola = True
            elif line.startswith("  DETECTOR: "):
                value = json.loads(line.split(": ", 1)[1])
                self.assertEqual(value["operation_count"], 12000)
                found_detector = True
        self.assertTrue(found_depola, "DEPOLARIZE2 entry missing from text report")
        self.assertTrue(found_detector, "DETECTOR entry missing from text report")

    def test_cli_writes_json_report_and_prints_summary(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "load.json"

            result = run_inspector(
                "--case",
                "stim_surface_d11_r100",
                "--manifest",
                str(FULL_MANIFEST),
                "--format",
                "json",
                "--out",
                str(out),
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("PASS fixture load stim_surface_d11_r100", result.stdout)
            self.assertEqual(result.stderr, "")
            report = json.loads(out.read_text())
            self.assertEqual(report["expanded_operation_count"], 14547)
            self.assertEqual(report["operations"]["DEPOLARIZE2"]["target_count"], 88000)

    def test_missing_case_is_rejected_with_requested_id(self) -> None:
        result = run_inspector(
            "--case",
            "no_such_case",
            "--manifest",
            str(FULL_MANIFEST),
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no_such_case", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_nested_repeat_summary_counts_depth_and_expansion_markers(self) -> None:
        circuit = stim.Circuit(
            """
            REPEAT 2 {
                M 0
                REPEAT 3 {
                    DETECTOR rec[-1]
                }
            }
            """
        )

        summary = summarize_circuit(circuit)

        self.assertEqual(summary["flattened_operation_count"], 8)
        self.assertEqual(summary["repeat_block_count"], 2)
        self.assertEqual(summary["repeat_depth"], 2)
        self.assertEqual(summary["repeat_expansion_count"], 8)
        self.assertEqual(summary["expanded_operation_count"], 16)
        self.assertEqual(summary["operations"]["M"]["operation_count"], 2)
        self.assertEqual(summary["operations"]["DETECTOR"]["operation_count"], 6)
        self.assertEqual(summary["operations"]["REPEAT"]["operation_count"], 8)


if __name__ == "__main__":
    unittest.main()
