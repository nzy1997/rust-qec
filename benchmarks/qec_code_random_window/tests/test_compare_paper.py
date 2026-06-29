from __future__ import annotations

import argparse
import csv
import inspect
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from benchmarks.qec_code_random_window import compare_paper


ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "benchmarks" / "qec_code_random_window" / "tests" / "fixtures"


def read_csv_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


class ComparePaperTest(unittest.TestCase):
    def run_compare(
        self,
        out_dir: Path,
        *,
        paper_baselines: Path | None = None,
        strict: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        command = [
            sys.executable,
            "-m",
            "benchmarks.qec_code_random_window.compare_paper",
            "--cases",
            str(FIXTURES / "compare_cases.toml"),
            "--local-summary",
            str(FIXTURES / "compare_summary.csv"),
            "--paper-baselines",
            str(paper_baselines or FIXTURES / "compare_paper_baselines.csv"),
            "--out-dir",
            str(out_dir),
        ]
        if strict:
            command.append("--strict-baselines")
        return subprocess.run(
            command,
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_fixture_inputs_write_exact_comparison_csv_values(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            result = self.run_compare(out_dir)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, "")
            with (out_dir / "comparison.csv").open(newline="", encoding="utf-8") as handle:
                self.assertEqual(csv.reader(handle).__next__(), compare_paper.CSV_FIELDS)
            self.assertEqual(
                read_csv_rows(out_dir / "comparison.csv"),
                [
                    {
                        "case_id": "matched_case",
                        "code_id": "bb72",
                        "distance_side": "any",
                        "baseline_key": "codeDistancePYPI:bivariate_bicycle:bb72",
                        "baseline_required": "true",
                        "local_best_upper_bound": "5",
                        "local_median_elapsed_s": "2.5",
                        "paper_method": "QDistRndMW",
                        "paper_upper_bound": "6",
                        "paper_elapsed_s": "5.0",
                        "upper_bound_delta": "-1",
                        "elapsed_time_ratio": "0.500000",
                        "baseline_provenance": "bb-summary.xlsx#BB summary:2",
                        "baseline_source_file": "bb-summary.xlsx",
                        "baseline_source_sheet": "BB summary",
                        "baseline_source_row": "2",
                        "comparison_status": "paper_matched",
                    },
                    {
                        "case_id": "optional_unmatched_case",
                        "code_id": "steane",
                        "distance_side": "any",
                        "baseline_key": "unmapped:steane",
                        "baseline_required": "false",
                        "local_best_upper_bound": "3",
                        "local_median_elapsed_s": "1.0",
                        "paper_method": "NA",
                        "paper_upper_bound": "NA",
                        "paper_elapsed_s": "NA",
                        "upper_bound_delta": "NA",
                        "elapsed_time_ratio": "NA",
                        "baseline_provenance": "NA",
                        "baseline_source_file": "NA",
                        "baseline_source_sheet": "NA",
                        "baseline_source_row": "NA",
                        "comparison_status": "no_paper_baseline",
                    },
                    {
                        "case_id": "required_missing_case",
                        "code_id": "bb144",
                        "distance_side": "any",
                        "baseline_key": "codeDistancePYPI:bivariate_bicycle:bb144",
                        "baseline_required": "true",
                        "local_best_upper_bound": "11",
                        "local_median_elapsed_s": "9.0",
                        "paper_method": "QDistEvol",
                        "paper_upper_bound": "12",
                        "paper_elapsed_s": "0",
                        "upper_bound_delta": "-1",
                        "elapsed_time_ratio": "NA",
                        "baseline_provenance": "bb-summary.xlsx#BB summary:3",
                        "baseline_source_file": "bb-summary.xlsx",
                        "baseline_source_sheet": "BB summary",
                        "baseline_source_row": "3",
                        "comparison_status": "paper_matched",
                    },
                ],
            )

    def test_markdown_includes_numeric_and_provenance_columns(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            result = self.run_compare(out_dir)

            self.assertEqual(result.returncode, 0, result.stderr)
            markdown = (out_dir / "comparison.md").read_text(encoding="utf-8")
            self.assertIn("## Provenance", markdown)
            self.assertIn("Paper baselines:", markdown)
            self.assertIn(
                "| case_id | local_best_upper_bound | paper_method | paper_upper_bound | upper_bound_delta | elapsed_time_ratio | baseline_provenance | source_file | source_sheet | source_row | status |",
                markdown,
            )
            self.assertIn(
                "| matched_case | 5 | QDistRndMW | 6 | -1 | 0.500000 | bb-summary.xlsx#BB summary:2 | bb-summary.xlsx | BB summary | 2 | paper_matched |",
                markdown,
            )
            self.assertIn(
                "| optional_unmatched_case | 3 | NA | NA | NA | NA | NA | NA | NA | NA | no_paper_baseline |",
                markdown,
            )

    def test_strict_baselines_exits_nonzero_when_required_case_has_no_match(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = self.run_compare(
                Path(tmp),
                paper_baselines=FIXTURES / "compare_paper_baselines_missing_required.csv",
                strict=True,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing required paper baseline rows", result.stderr)
            self.assertIn("required_missing_case", result.stderr)

    def test_non_strict_allows_missing_required_baseline_and_writes_na_row(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            result = self.run_compare(
                out_dir,
                paper_baselines=FIXTURES / "compare_paper_baselines_missing_required.csv",
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            rows = read_csv_rows(out_dir / "comparison.csv")
            missing = next(row for row in rows if row["case_id"] == "required_missing_case")
            self.assertEqual(missing["paper_method"], "NA")
            self.assertEqual(missing["upper_bound_delta"], "NA")
            self.assertEqual(missing["comparison_status"], "no_paper_baseline")

    def test_module_exports_requested_api_names(self) -> None:
        self.assertTrue(issubclass(compare_paper.CompareError, Exception))
        self.assertTrue(callable(compare_paper.load_local_summaries))
        self.assertTrue(callable(compare_paper.load_paper_baselines))
        self.assertTrue(callable(compare_paper.compare_cases))
        self.assertTrue(callable(compare_paper.write_comparison_csv))
        self.assertTrue(callable(compare_paper.write_comparison_md))
        self.assertTrue(callable(compare_paper.run))
        self.assertEqual(
            list(inspect.signature(compare_paper.run).parameters),
            ["args", "argv"],
        )

    def test_run_supports_direct_args_and_custom_argv(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            args = argparse.Namespace(
                cases=FIXTURES / "compare_cases.toml",
                local_summary=FIXTURES / "compare_summary.csv",
                paper_baselines=FIXTURES / "compare_paper_baselines.csv",
                out_dir=Path(tmp),
                strict_baselines=False,
            )
            argv = ["--cases", "fixture path", "--strict-baselines"]

            exit_code = compare_paper.run(args, argv)

            self.assertEqual(exit_code, 0)
            markdown = (Path(tmp) / "comparison.md").read_text(encoding="utf-8")
            self.assertIn('Comparison argv: `["--cases", "fixture path", "--strict-baselines"]`', markdown)

    def test_help_exits_zero(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                "-m",
                "benchmarks.qec_code_random_window.compare_paper",
                "--help",
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(result.returncode, 0)
        self.assertIn("--cases", result.stdout)
        self.assertIn("--local-summary", result.stdout)
        self.assertIn("--paper-baselines", result.stdout)
        self.assertIn("--strict-baselines", result.stdout)
