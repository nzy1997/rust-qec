from __future__ import annotations

import argparse
import csv
import inspect
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from benchmarks.qec_code_random_window import summarize


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
            with (out_dir / "summary.csv").open(newline="", encoding="utf-8") as handle:
                self.assertEqual(csv.reader(handle).__next__(), summarize.CSV_FIELDS)
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
                        "run_build_profile_values": "",
                        "run_status_values": "cli_error;ok",
                        "search_stats_rows": "",
                        "search_stats_total_permutations_sampled": "",
                        "search_stats_total_kernel_basis_generations": "",
                        "search_stats_total_component_candidates_generated": "",
                        "search_stats_total_zero_candidates_rejected": "",
                        "search_stats_total_stabilizer_span_candidates_rejected": "",
                        "search_stats_total_witness_validation_candidates_rejected": "",
                        "search_stats_total_valid_witnesses_found": "",
                        "search_stats_total_best_witness_updates": "",
                        "search_stats_target_reached_count": "",
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
                        "run_build_profile_values": "",
                        "run_status_values": "cli_error",
                        "search_stats_rows": "",
                        "search_stats_total_permutations_sampled": "",
                        "search_stats_total_kernel_basis_generations": "",
                        "search_stats_total_component_candidates_generated": "",
                        "search_stats_total_zero_candidates_rejected": "",
                        "search_stats_total_stabilizer_span_candidates_rejected": "",
                        "search_stats_total_witness_validation_candidates_rejected": "",
                        "search_stats_total_valid_witnesses_found": "",
                        "search_stats_total_best_witness_updates": "",
                        "search_stats_target_reached_count": "",
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
                        "run_build_profile_values": "",
                        "run_status_values": "",
                        "search_stats_rows": "",
                        "search_stats_total_permutations_sampled": "",
                        "search_stats_total_kernel_basis_generations": "",
                        "search_stats_total_component_candidates_generated": "",
                        "search_stats_total_zero_candidates_rejected": "",
                        "search_stats_total_stabilizer_span_candidates_rejected": "",
                        "search_stats_total_witness_validation_candidates_rejected": "",
                        "search_stats_total_valid_witnesses_found": "",
                        "search_stats_total_best_witness_updates": "",
                        "search_stats_target_reached_count": "",
                        "summary_status": "no_success",
                    },
                ],
            )

    def test_multiple_run_files_aggregate_into_one_manifest_ordered_summary(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            result = self.run_summarizer(
                out_dir,
                FIXTURES / "summary_runs_part1.jsonl",
                FIXTURES / "summary_runs_part2.jsonl",
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                [row["case_id"] for row in read_csv_rows(out_dir / "summary.csv")],
                ["target_case", "no_success_case", "unattempted_case"],
            )
            target_row = read_csv_rows(out_dir / "summary.csv")[0]
            self.assertEqual(target_row["attempted_seed_rows"], "3")
            self.assertEqual(target_row["successful_seed_rows"], "2")
            self.assertEqual(target_row["run_seed_values"], "11;12;13")
            self.assertEqual(target_row["run_status_values"], "cli_error;ok")

    def test_no_target_run_rows_summarize_with_blank_target_weight(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            manifest.write_text(
                """
manifest_version = 1
suite = "qec_code_random_window"

[[cases]]
case_id = "bb144_no_target_smoke"
code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
distance_side = "any"
iterations = 500
restarts = 1
seed = 7
target_upper_bound = 12
baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
baseline_required = true
""".lstrip(),
                encoding="utf-8",
            )
            runs.write_text(
                json.dumps(
                    {
                        "case_id": "bb144_no_target_smoke",
                        "status": "ok",
                        "seed": 7,
                        "iterations": 500,
                        "restarts": 1,
                        "target_weight": None,
                        "upper_bound": 12,
                        "elapsed_s": 3.76,
                        "build_profile": "release",
                        "command": [
                            "target/release/qec-code",
                            "code",
                            "css-distance",
                            "random-window-upper-bound",
                            "--code-id",
                            "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0",
                        ],
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "benchmarks.qec_code_random_window.summarize",
                    "--cases",
                    str(manifest),
                    "--runs",
                    str(runs),
                    "--out-dir",
                    str(out_dir),
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            row = read_csv_rows(out_dir / "summary.csv")[0]
            self.assertEqual(row["manifest_target_weight"], "")
            self.assertEqual(row["run_target_weight_values"], "")
            self.assertEqual(row["run_build_profile_values"], "release")
            self.assertEqual(row["best_upper_bound"], "12")
            markdown = (out_dir / "summary.md").read_text(encoding="utf-8")
            self.assertIn(
                "| case_id | code_id | status | attempted | successful | observed_seeds | best_upper_bound | target_upper_bound | target_hits | elapsed_s | target_weight | build_profile | search_stats | note |",
                markdown,
            )

    def test_duplicate_seed_rows_are_rejected_for_targeted_case(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            manifest.write_text(
                """
manifest_version = 1
suite = "qec_code_random_window"

[[cases]]
case_id = "target_case"
code_id = "surface_rotated:d=5"
distance_side = "any"
iterations = 20
restarts = 2
seed = 11
target_weight = 5
target_upper_bound = 5
baseline_key = "unmapped:target"
baseline_required = false
""".lstrip(),
                encoding="utf-8",
            )
            runs.write_text(
                "".join(
                    json.dumps(
                        {
                            "case_id": "target_case",
                            "status": "ok",
                            "seed": 11,
                            "iterations": 20,
                            "restarts": 2,
                            "target_weight": 5,
                            "upper_bound": 5,
                            "elapsed_s": 1.0,
                        }
                    )
                    + "\n"
                    for _ in range(2)
                ),
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "benchmarks.qec_code_random_window.summarize",
                    "--cases",
                    str(manifest),
                    "--runs",
                    str(runs),
                    "--out-dir",
                    str(out_dir),
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("target_case", result.stderr)
            self.assertIn('field "seed"', result.stderr)
            self.assertIn("11", result.stderr)

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

    def test_success_row_nonfinite_elapsed_exits_nonzero_with_context(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = self.run_summarizer(
                Path(tmp),
                FIXTURES / "nonfinite_elapsed_success.jsonl",
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("nonfinite_elapsed_success.jsonl:1", result.stderr)
            self.assertIn("elapsed_s", result.stderr)
            self.assertIn("finite", result.stderr)

    def test_module_exports_requested_api_names(self) -> None:
        self.assertTrue(issubclass(summarize.SummaryError, Exception))
        self.assertTrue(callable(summarize.load_run_rows))
        self.assertTrue(callable(summarize.summarize_cases))
        self.assertTrue(callable(summarize.write_summary_csv))
        self.assertTrue(callable(summarize.write_summary_md))
        self.assertTrue(callable(summarize.run))
        self.assertEqual(
            list(inspect.signature(summarize.run).parameters),
            ["args", "argv"],
        )

    def test_load_run_rows_raises_summary_error_for_invalid_row(self) -> None:
        manifest = summarize.load_manifest(FIXTURES / "summary_cases.toml")
        with self.assertRaises(summarize.SummaryError) as context:
            summarize.load_run_rows(
                [FIXTURES / "nonfinite_elapsed_success.jsonl"],
                {case["case_id"] for case in manifest["cases"]},
            )

        self.assertIn("nonfinite_elapsed_success.jsonl:1", str(context.exception))
        self.assertIn("elapsed_s", str(context.exception))

    def test_run_supports_direct_args_and_custom_argv(self) -> None:
        manifest = summarize.load_manifest(FIXTURES / "summary_cases.toml")
        with tempfile.TemporaryDirectory() as tmp:
            args = argparse.Namespace(
                cases=FIXTURES / "summary_cases.toml",
                runs=[FIXTURES / "summary_runs.jsonl"],
                out_dir=Path(tmp),
            )
            argv = ["--cases", "fixture path", "--label", "token with spaces"]

            exit_code = summarize.run(args, argv)

            self.assertEqual(exit_code, 0)
            markdown = (Path(tmp) / "summary.md").read_text(encoding="utf-8")
            self.assertIn(f"Summarizer argv: `{json.dumps(argv)}`", markdown)
            self.assertNotIn("Summarizer argv: `--cases fixture path --label token with spaces`", markdown)
            self.assertEqual(len(manifest["cases"]), 3)

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
