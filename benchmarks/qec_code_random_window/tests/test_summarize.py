from __future__ import annotations

import csv
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "benchmarks" / "qec_code_random_window" / "tests" / "fixtures"


def read_csv_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


class SummarizeTest(unittest.TestCase):
    def run_summarizer(
        self,
        out_dir: Path,
        *runs: Path,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                "-m",
                "benchmarks.qec_code_random_window.summarize",
                "--cases",
                str(FIXTURES / "summary_cases.toml"),
                "--runs",
                *(str(run) for run in runs),
                "--out-dir",
                str(out_dir),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_fixture_runs_write_exact_summary_csv_values(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            result = self.run_summarizer(out_dir, FIXTURES / "summary_runs.jsonl")

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, "")
            self.assertEqual(
                read_csv_rows(out_dir / "summary.csv"),
                [
                    {
                        "case_id": "target_case",
                        "code_id": "bb72",
                        "distance_side": "any",
                        "baseline_key": "codeDistancePYPI:bivariate_bicycle:bb72",
                        "baseline_required": "true",
                        "manifest_seed": "11",
                        "manifest_iterations": "10",
                        "manifest_restarts": "2",
                        "manifest_target_weight": "5",
                        "target_upper_bound": "5",
                        "attempted_seed_rows": "3",
                        "successful_seed_rows": "2",
                        "best_upper_bound": "5",
                        "median_elapsed_s": "2.0",
                        "min_elapsed_s": "1.0",
                        "max_elapsed_s": "3.0",
                        "target_hit_count": "1",
                        "target_hit_rate": "0.500000",
                        "run_seed_values": "11;12;13",
                        "run_iterations_values": "10",
                        "run_restarts_values": "2",
                        "run_target_weight_values": "5",
                        "run_status_values": "cli_error;ok",
                        "summary_status": "ok",
                    },
                    {
                        "case_id": "no_success_case",
                        "code_id": "steane",
                        "distance_side": "any",
                        "baseline_key": "unmapped:steane",
                        "baseline_required": "false",
                        "manifest_seed": "21",
                        "manifest_iterations": "20",
                        "manifest_restarts": "1",
                        "manifest_target_weight": "3",
                        "target_upper_bound": "",
                        "attempted_seed_rows": "1",
                        "successful_seed_rows": "0",
                        "best_upper_bound": "",
                        "median_elapsed_s": "",
                        "min_elapsed_s": "",
                        "max_elapsed_s": "",
                        "target_hit_count": "",
                        "target_hit_rate": "",
                        "run_seed_values": "21",
                        "run_iterations_values": "20",
                        "run_restarts_values": "1",
                        "run_target_weight_values": "3",
                        "run_status_values": "cli_error",
                        "summary_status": "no_success",
                    },
                    {
                        "case_id": "unattempted_case",
                        "code_id": "toric:d=3",
                        "distance_side": "any",
                        "baseline_key": "unmapped:toric_d3",
                        "baseline_required": "false",
                        "manifest_seed": "31",
                        "manifest_iterations": "30",
                        "manifest_restarts": "3",
                        "manifest_target_weight": "3",
                        "target_upper_bound": "3",
                        "attempted_seed_rows": "0",
                        "successful_seed_rows": "0",
                        "best_upper_bound": "",
                        "median_elapsed_s": "",
                        "min_elapsed_s": "",
                        "max_elapsed_s": "",
                        "target_hit_count": "0",
                        "target_hit_rate": "",
                        "run_seed_values": "",
                        "run_iterations_values": "",
                        "run_restarts_values": "",
                        "run_target_weight_values": "",
                        "run_status_values": "",
                        "summary_status": "no_success",
                    },
                ],
            )

    def test_summary_markdown_has_manifest_rows_and_zero_success_marker(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            result = self.run_summarizer(out_dir, FIXTURES / "summary_runs.jsonl")

            self.assertEqual(result.returncode, 0, result.stderr)
            markdown = (out_dir / "summary.md").read_text(encoding="utf-8")
            self.assertIn("Manifest:", markdown)
            self.assertIn("Run files:", markdown)
            self.assertEqual(markdown.count("| target_case |"), 1)
            self.assertEqual(markdown.count("| no_success_case |"), 1)
            self.assertEqual(markdown.count("| unattempted_case |"), 1)
            self.assertIn("NO SUCCESSFUL ROWS", markdown)

    def test_success_row_missing_upper_bound_exits_nonzero_with_context(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = self.run_summarizer(
                Path(tmp),
                FIXTURES / "missing_upper_bound_success.jsonl",
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing_upper_bound_success.jsonl:1", result.stderr)
            self.assertIn("upper_bound", result.stderr)
            self.assertIn('status = "ok"', result.stderr)

    def test_help_exits_zero(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                "-m",
                "benchmarks.qec_code_random_window.summarize",
                "--help",
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(result.returncode, 0)
        self.assertIn("--cases", result.stdout)
        self.assertIn("--runs", result.stdout)
        self.assertIn("--out-dir", result.stdout)


if __name__ == "__main__":
    unittest.main()
