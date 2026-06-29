# Issue 325 QEC-Code Random-Window Paper Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a comparison CLI that joins local random-window summaries with canonical codeDistancePYPI paper baselines and writes auditable CSV and Markdown evidence tables.

**Architecture:** Add one standard-library Python module under `benchmarks/qec_code_random_window/` that validates the existing manifest, loads the #323 summary CSV and #324 baseline CSV, joins rows by `case_id`, computes deltas/ratios, writes manifest-ordered outputs, and enforces optional strict baseline checks. Tests use checked CSV/TOML fixtures and subprocess invocations to verify the public CLI behavior and exact artifacts.

**Tech Stack:** Python 3.11+ standard library (`argparse`, `csv`, `json`, `math`, `pathlib`, `subprocess`, `sys`, `tempfile`, `unittest`), existing benchmark manifest validator, Cargo workspace verification.

## Global Constraints

- Create `benchmarks/qec_code_random_window/compare_paper.py`.
- Read the #321 manifest, #323 `summary.csv`, and #324 canonical baseline CSV.
- Join baseline rows by canonical `case_id` because #324 already resolves explicit manifest `baseline_key` values.
- Emit `comparison.csv` and `comparison.md`.
- Include columns for local best upper bound, local median elapsed seconds, paper method, paper upper bound, paper elapsed seconds, upper-bound delta, elapsed-time ratio, and baseline provenance.
- Define `upper_bound_delta = local_best_upper_bound - paper_upper_bound`; negative values mean the local run found a smaller upper bound.
- Define `elapsed_time_ratio = local_median_elapsed_s / paper_elapsed_s`; emit `NA` when either timing is missing, nonpositive, nonfinite, or not comparable.
- Clearly display `NA` for cases with no defensible paper baseline match.
- Add `--strict-baselines` so missing rows for `baseline_required = true` cases fail.
- Keep the implementation standard-library only and local to `benchmarks/qec_code_random_window/`.

---

### Task 1: Add Failing Comparison Tests and Fixtures

**Files:**
- Create: `benchmarks/qec_code_random_window/tests/test_compare_paper.py`
- Create: `benchmarks/qec_code_random_window/tests/fixtures/compare_cases.toml`
- Create: `benchmarks/qec_code_random_window/tests/fixtures/compare_summary.csv`
- Create: `benchmarks/qec_code_random_window/tests/fixtures/compare_paper_baselines.csv`
- Create: `benchmarks/qec_code_random_window/tests/fixtures/compare_paper_baselines_missing_required.csv`

**Interfaces:**
- Consumes: future CLI `python3 -m benchmarks.qec_code_random_window.compare_paper --cases <path> --local-summary <path> --paper-baselines <path> --out-dir <path> [--strict-baselines]`.
- Produces: failing tests that assert exact CSV, Markdown provenance columns, strict-baseline behavior, help behavior, and module API exports.

- [ ] **Step 1: Create comparison manifest fixture**

Create `benchmarks/qec_code_random_window/tests/fixtures/compare_cases.toml`:

```toml
manifest_version = 1
suite = "qec_code_random_window"
description = "Fixture cases for qec-code random-window paper comparison tests."

[[cases]]
case_id = "matched_case"
code_id = "bb72"
distance_side = "any"
iterations = 10
restarts = 2
seed = 11
target_weight = 5
target_upper_bound = 5
baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
baseline_required = true

[[cases]]
case_id = "optional_unmatched_case"
code_id = "steane"
distance_side = "any"
iterations = 20
restarts = 1
seed = 21
target_weight = 3
target_upper_bound = 3
baseline_key = "unmapped:steane"
baseline_required = false

[[cases]]
case_id = "required_missing_case"
code_id = "bb144"
distance_side = "any"
iterations = 30
restarts = 3
seed = 31
target_weight = 12
target_upper_bound = 12
baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
baseline_required = true
```

- [ ] **Step 2: Create local summary fixture**

Create `benchmarks/qec_code_random_window/tests/fixtures/compare_summary.csv`:

```csv
case_id,code_id,distance_side,baseline_key,baseline_required,manifest_seed,manifest_iterations,manifest_restarts,manifest_target_weight,target_upper_bound,attempted_seed_rows,successful_seed_rows,best_upper_bound,median_elapsed_s,min_elapsed_s,max_elapsed_s,target_hit_count,target_hit_rate,run_seed_values,run_iterations_values,run_restarts_values,run_target_weight_values,run_status_values,summary_status
matched_case,bb72,any,codeDistancePYPI:bivariate_bicycle:bb72,true,11,10,2,5,5,2,2,5,2.5,2.0,3.0,1,0.500000,11;12,10,2,5,ok,ok
optional_unmatched_case,steane,any,unmapped:steane,false,21,20,1,3,3,1,1,3,1.0,1.0,1.0,1,1.000000,21,20,1,3,ok,ok
required_missing_case,bb144,any,codeDistancePYPI:bivariate_bicycle:bb144,true,31,30,3,12,12,1,1,11,9.0,9.0,9.0,1,1.000000,31,30,3,12,ok,ok
```

- [ ] **Step 3: Create paper baseline fixtures**

Create `benchmarks/qec_code_random_window/tests/fixtures/compare_paper_baselines.csv`:

```csv
case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row
matched_case,bb72,QDistRndMW,6,5.0,bb-summary.xlsx,BB summary,2
required_missing_case,bb144,QDistEvol,12,0,bb-summary.xlsx,BB summary,3
```

Create `benchmarks/qec_code_random_window/tests/fixtures/compare_paper_baselines_missing_required.csv`:

```csv
case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row
matched_case,bb72,QDistRndMW,6,5.0,bb-summary.xlsx,BB summary,2
```

- [ ] **Step 4: Write failing unittest module**

Create `benchmarks/qec_code_random_window/tests/test_compare_paper.py`:

```python
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
```

- [ ] **Step 5: Run test to verify it fails**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_compare_paper -q
```

Expected: FAIL or ERROR because `benchmarks.qec_code_random_window.compare_paper` does not exist yet. This is the required TDD red state.

---

### Task 2: Implement Comparison Module

**Files:**
- Create: `benchmarks/qec_code_random_window/compare_paper.py`

**Interfaces:**
- Consumes: `validate_cases.load_manifest(path: Path) -> dict[str, Any]` and `validate_cases.validate_manifest(manifest: dict[str, Any]) -> list[str]`.
- Produces: `CSV_FIELDS: list[str]`.
- Produces: `CompareError(ValueError)`.
- Produces: `load_local_summaries(path: Path, known_case_ids: set[str]) -> dict[str, dict[str, str]]`.
- Produces: `load_paper_baselines(path: Path, known_case_ids: set[str]) -> dict[str, dict[str, str]]`.
- Produces: `compare_cases(cases: list[dict[str, Any]], local_summaries: dict[str, dict[str, str]], paper_baselines: dict[str, dict[str, str]]) -> list[dict[str, str]]`.
- Produces: `write_comparison_csv(path: Path, rows: list[dict[str, str]]) -> None`.
- Produces: `write_comparison_md(path: Path, *, manifest_path: Path, local_summary_path: Path, paper_baselines_path: Path, argv: list[str], manifest: dict[str, Any], rows: list[dict[str, str]]) -> None`.
- Produces: `run(args: argparse.Namespace, argv: list[str] | None = None) -> int`.
- Produces: `main(argv: list[str] | None = None) -> int`.

- [ ] **Step 1: Write minimal implementation**

Create `benchmarks/qec_code_random_window/compare_paper.py` with these implementation requirements:

```python
from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from pathlib import Path
from typing import Any

from benchmarks.qec_code_random_window.validate_cases import load_manifest, validate_manifest
```

Define the output fields:

```python
NA = "NA"

CSV_FIELDS = [
    "case_id",
    "code_id",
    "distance_side",
    "baseline_key",
    "baseline_required",
    "local_best_upper_bound",
    "local_median_elapsed_s",
    "paper_method",
    "paper_upper_bound",
    "paper_elapsed_s",
    "upper_bound_delta",
    "elapsed_time_ratio",
    "baseline_provenance",
    "baseline_source_file",
    "baseline_source_sheet",
    "baseline_source_row",
    "comparison_status",
]
```

Implement helpers with these exact behaviors:

- `_validated_cases(path)` loads and validates the manifest, raising
  `CompareError` with one line per validation error.
- `_require_columns(path, rows, required)` raises `CompareError` naming missing
  columns.
- `load_local_summaries` requires `case_id`, `best_upper_bound`, and
  `median_elapsed_s`, rejects unknown cases, and returns rows keyed by
  `case_id`.
- `load_paper_baselines` requires the #324 canonical columns, rejects unknown
  cases, keeps the first row for duplicate `case_id`, and returns rows keyed by
  `case_id`.
- `_parse_positive_or_none(value)` returns `None` for blank or `NA`, parses
  finite positive floats, and raises `CompareError` for malformed nonblank
  values.
- `_parse_int_or_none(value)` returns `None` for blank or `NA`, parses integer
  strings, and raises `CompareError` for malformed nonblank values.
- `_format_delta(local, paper)` returns `NA` if either value is absent;
  otherwise returns the integer delta string.
- `_format_ratio(local_elapsed, paper_elapsed)` returns `NA` unless both
  elapsed values are finite and strictly positive; otherwise returns
  `f"{local_elapsed / paper_elapsed:.6f}"`.
- `_baseline_provenance(row)` returns
  `"{source_file}#{source_sheet}:{source_row}"`.

`compare_cases` should emit one row per manifest case in manifest order. It
should use local summary values when present, use `NA` for missing/blank local
result fields, join paper rows by `case_id`, and set paper fields to `NA` when
no baseline row exists.

`run` should write both artifacts before applying strict-baseline failure, so
strict runs still leave evidence files for diagnosis. It should return `2` for
manifest validation errors, `1` for CSV/output/strict errors, and `0` on
success.

- [ ] **Step 2: Run focused comparison tests**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_compare_paper -q
```

Expected: PASS.

- [ ] **Step 3: Refactor after green**

If the first green implementation has duplicated conversion logic, keep
behavior unchanged and extract helper functions only inside
`compare_paper.py`. Re-run the focused test:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_compare_paper -q
```

Expected: PASS.

---

### Task 3: Run Package and Repository Verification

**Files:**
- No new files.

**Interfaces:**
- Consumes: completed `compare_paper.py` and fixtures.
- Produces: verification evidence for the PR.

- [ ] **Step 1: Run focused comparison tests**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_compare_paper -q
```

Expected: PASS.

- [ ] **Step 2: Run qec random-window benchmark package tests**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases benchmarks.qec_code_random_window.tests.test_summarize benchmarks.qec_code_random_window.tests.test_import_paper_baselines benchmarks.qec_code_random_window.tests.test_compare_paper -q
```

Expected: PASS.

- [ ] **Step 3: Run CLI help smoke test**

Run:

```bash
python3 -m benchmarks.qec_code_random_window.compare_paper --help
```

Expected: exit 0 and stdout includes `--cases`, `--local-summary`,
`--paper-baselines`, `--out-dir`, and `--strict-baselines`.

- [ ] **Step 4: Run required repository verification**

Run:

```bash
cargo test
```

Expected: PASS. If the sandbox cannot fetch dependencies or the command times
out, capture the failure and continue only after confirming the implementation
tests passed.

---

### Task 4: Commit, Review, and Prepare PR

**Files:**
- Modify: `docs/superpowers/plans/2026-06-29-issue-325-qec-code-random-window-paper-comparison.md` by checking off completed steps if desired.

**Interfaces:**
- Produces: implementation commit and PR-ready branch.

- [ ] **Step 1: Inspect changed files**

Run:

```bash
git status --short
git diff -- benchmarks/qec_code_random_window/compare_paper.py benchmarks/qec_code_random_window/tests/test_compare_paper.py benchmarks/qec_code_random_window/tests/fixtures/compare_cases.toml benchmarks/qec_code_random_window/tests/fixtures/compare_summary.csv benchmarks/qec_code_random_window/tests/fixtures/compare_paper_baselines.csv benchmarks/qec_code_random_window/tests/fixtures/compare_paper_baselines_missing_required.csv docs/superpowers/plans/2026-06-29-issue-325-qec-code-random-window-paper-comparison.md
```

Expected: only issue #325 comparison files and the plan are changed.

- [ ] **Step 2: Commit implementation**

Run:

```bash
git add benchmarks/qec_code_random_window/compare_paper.py benchmarks/qec_code_random_window/tests/test_compare_paper.py benchmarks/qec_code_random_window/tests/fixtures/compare_cases.toml benchmarks/qec_code_random_window/tests/fixtures/compare_summary.csv benchmarks/qec_code_random_window/tests/fixtures/compare_paper_baselines.csv benchmarks/qec_code_random_window/tests/fixtures/compare_paper_baselines_missing_required.csv docs/superpowers/plans/2026-06-29-issue-325-qec-code-random-window-paper-comparison.md
git commit -m "benchmarks: compare qec random-window paper baselines"
```

Expected: commit succeeds.

- [ ] **Step 3: Use finishing workflow**

Invoke `superpowers:verification-before-completion` and
`superpowers:finishing-a-development-branch`. When asked what to do with the
branch, choose "Push and create a Pull Request" because the Agent Desk
instruction requires a PR and explicitly says not to merge.
