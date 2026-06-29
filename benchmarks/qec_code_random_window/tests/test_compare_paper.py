from __future__ import annotations

import argparse
import csv
import inspect
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


def compare_paper_module():
    from benchmarks.qec_code_random_window import compare_paper

    return compare_paper


def compare_cases_manifest() -> dict[str, dict[str, object]]:
    return {
        "matched_case": {
            "case_id": "matched_case",
            "code_id": "bb72",
            "distance_side": "any",
            "baseline_key": "codeDistancePYPI:bivariate_bicycle:bb72",
            "baseline_required": True,
        },
        "optional_unmatched_case": {
            "case_id": "optional_unmatched_case",
            "code_id": "steane",
            "distance_side": "any",
            "baseline_key": "unmapped:steane",
            "baseline_required": False,
        },
        "required_missing_case": {
            "case_id": "required_missing_case",
            "code_id": "bb144",
            "distance_side": "any",
            "baseline_key": "codeDistancePYPI:bivariate_bicycle:bb144",
            "baseline_required": True,
        },
    }


class ComparePaperTest(unittest.TestCase):
    def run_compare(
        self,
        out_dir: Path,
        *,
        local_summary: Path | None = None,
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
            str(local_summary or FIXTURES / "compare_summary.csv"),
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
        compare_paper = compare_paper_module()

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
        compare_paper = compare_paper_module()

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
        compare_paper = compare_paper_module()

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

    def test_load_local_summaries_rejects_duplicate_case_ids(self) -> None:
        compare_paper = compare_paper_module()
        manifest_cases = compare_cases_manifest()
        case_ids = set(manifest_cases)

        with tempfile.TemporaryDirectory() as tmp:
            local_summary = Path(tmp) / "summary.csv"
            local_summary.write_text(
                "case_id,code_id,distance_side,baseline_key,baseline_required,best_upper_bound,median_elapsed_s\n"
                "matched_case,bb72,any,codeDistancePYPI:bivariate_bicycle:bb72,true,5,2.5\n"
                "matched_case,bb72,any,codeDistancePYPI:bivariate_bicycle:bb72,true,5,2.5\n",
                encoding="utf-8",
            )

            with self.assertRaises(compare_paper.CompareError) as cm:
                compare_paper.load_local_summaries(local_summary, case_ids, manifest_cases)
            self.assertIn("duplicate case_id", str(cm.exception))
            self.assertIn("matched_case", str(cm.exception))

    def test_load_local_summaries_rejects_metadata_mismatch(self) -> None:
        compare_paper = compare_paper_module()
        manifest_cases = compare_cases_manifest()
        case_ids = set(manifest_cases)

        with tempfile.TemporaryDirectory() as tmp:
            local_summary = Path(tmp) / "summary.csv"
            local_summary.write_text(
                "case_id,code_id,distance_side,baseline_key,baseline_required,best_upper_bound,median_elapsed_s\n"
                "matched_case,bb73,any,codeDistancePYPI:bivariate_bicycle:bb72,true,5,2.5\n",
                encoding="utf-8",
            )
            with self.assertRaises(compare_paper.CompareError) as cm:
                compare_paper.load_local_summaries(local_summary, case_ids, manifest_cases)
            self.assertIn("metadata mismatch", str(cm.exception))
            self.assertIn("code_id expected", str(cm.exception))

    def test_run_rejects_unknown_case_id_in_local_summary(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            local_summary = Path(tmp) / "summary.csv"
            local_summary.write_text(
                "case_id,code_id,distance_side,baseline_key,baseline_required,best_upper_bound,median_elapsed_s\n"
                "not_a_case,bb72,any,codeDistancePYPI:bivariate_bicycle:bb72,true,5,2.5\n",
                encoding="utf-8",
            )

            result = self.run_compare(Path(tmp), local_summary=local_summary)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('unknown case_id "not_a_case"', result.stderr)

    def test_run_rejects_unknown_case_id_in_paper_baselines(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            paper_baselines = Path(tmp) / "paper_baselines.csv"
            paper_baselines.write_text(
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n"
                "unknown_case,bb72,QDistRndMW,6,5.0,bb-summary.xlsx,BB summary,2\n",
                encoding="utf-8",
            )

            result = self.run_compare(
                Path(tmp),
                paper_baselines=paper_baselines,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('unknown case_id "unknown_case"', result.stderr)

    def test_load_local_summaries_rejects_missing_required_columns(self) -> None:
        compare_paper = compare_paper_module()
        case_ids = set(compare_cases_manifest())

        with tempfile.TemporaryDirectory() as tmp:
            local_summary = Path(tmp) / "summary.csv"
            local_summary.write_text(
                "case_id,code_id,distance_side,baseline_key,best_upper_bound,median_elapsed_s\n"
                "matched_case,bb72,any,codeDistancePYPI:bivariate_bicycle:bb72,5,2.5\n",
                encoding="utf-8",
            )
            with self.assertRaises(compare_paper.CompareError) as cm:
                compare_paper.load_local_summaries(
                    local_summary,
                    case_ids,
                )
            self.assertIn("missing required column(s)", str(cm.exception))
            self.assertIn("baseline_required", str(cm.exception))

    def test_compare_cases_rejects_malformed_numeric_local_summary(self) -> None:
        compare_paper = compare_paper_module()
        cases = [
            {
                "case_id": "matched_case",
                "code_id": "bb72",
                "distance_side": "any",
                "baseline_key": "codeDistancePYPI:bivariate_bicycle:bb72",
                "baseline_required": True,
            }
        ]
        local_summaries = {
            "matched_case": {
                "_row_location": "summary.csv:2",
                "best_upper_bound": "abc",
                "median_elapsed_s": "2.5",
            }
        }
        paper_baselines: dict[str, dict[str, str]] = {}

        with self.assertRaises(compare_paper.CompareError) as cm:
            compare_paper.compare_cases(cases, local_summaries, paper_baselines)
        self.assertIn("summary.csv:2", str(cm.exception))
        self.assertIn('field "best_upper_bound"', str(cm.exception))
