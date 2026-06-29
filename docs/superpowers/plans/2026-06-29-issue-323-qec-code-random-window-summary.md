# Issue 323 QEC-Code Random-Window Summary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a summarizer that converts local qec-code random-window JSONL runs into stable `summary.csv` and `summary.md` files.

**Architecture:** Add one focused Python module under `benchmarks/qec_code_random_window/` that reuses the existing manifest loader and validator, validates JSONL rows enough to summarize them safely, computes manifest-ordered per-case aggregates, and writes CSV plus Markdown outputs. Tests use committed TOML and JSONL fixtures plus subprocess CLI invocations so the public module interface is covered.

**Tech Stack:** Python 3.11+ standard library (`argparse`, `csv`, `json`, `math`, `pathlib`, `statistics`, `sys`, `tempfile`, `unittest`), existing #321 TOML manifest validator, Cargo workspace verification.

## Global Constraints

- Create `benchmarks/qec_code_random_window/summarize.py`.
- Read one or more JSONL run files emitted by #322.
- Read the #321 manifest with `validate_cases.load_manifest` and `validate_cases.validate_manifest`.
- Emit `summary.csv` and `summary.md` under `--out-dir`.
- Emit exactly one summary row per manifest case in manifest order.
- Attempted seed rows are all JSONL rows for a manifest case.
- Successful seed rows are rows with `status = "ok"`.
- Best upper bound is the minimum `upper_bound` across successful rows for the case.
- Elapsed-time statistics are over successful rows only and median uses Python `statistics.median`.
- If `target_upper_bound` exists in the manifest, target hits are successful rows with `upper_bound <= target_upper_bound`.
- Preserve manifest `baseline_key` and `baseline_required` in `summary.csv`.
- Preserve provenance for the manifest path, run paths, manifest suite/version, exact summarizer argv, manifest command settings, and observed run command settings.
- A `status = "ok"` row missing positive integer `upper_bound` must exit nonzero with a useful file/line error.
- Cases with zero successful rows must appear in `summary.md` and be clearly marked `NO SUCCESSFUL ROWS`.
- Required verification includes `python3 -m benchmarks.qec_code_random_window.summarize --help` and `cargo test`.

---

### Task 1: Add Summary Fixtures, CLI, and Tests

**Files:**
- Create: `benchmarks/qec_code_random_window/summarize.py`
- Create: `benchmarks/qec_code_random_window/tests/test_summarize.py`
- Create: `benchmarks/qec_code_random_window/tests/fixtures/summary_cases.toml`
- Create: `benchmarks/qec_code_random_window/tests/fixtures/summary_runs.jsonl`
- Create: `benchmarks/qec_code_random_window/tests/fixtures/missing_upper_bound_success.jsonl`

**Interfaces:**
- Consumes: `validate_cases.load_manifest(path: Path) -> dict[str, Any]`.
- Consumes: `validate_cases.validate_manifest(manifest: dict[str, Any]) -> list[str]`.
- Produces: `main(argv: list[str] | None = None) -> int`.
- Produces: CLI `python3 -m benchmarks.qec_code_random_window.summarize --cases <manifest> --runs <jsonl> [<jsonl> ...] --out-dir <dir>`.
- Produces: `summary.csv` with the exact field order listed in Step 1.
- Produces: `summary.md` with a provenance section and one table row per manifest case.

- [ ] **Step 1: Write the failing fixtures and tests**

Create `benchmarks/qec_code_random_window/tests/fixtures/summary_cases.toml`:

```toml
manifest_version = 1
suite = "qec_code_random_window"
description = "Fixture cases for qec-code random-window summarizer tests."

[[cases]]
case_id = "target_case"
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
case_id = "no_success_case"
code_id = "steane"
distance_side = "any"
iterations = 20
restarts = 1
seed = 21
target_weight = 3
baseline_key = "unmapped:steane"
baseline_required = false

[[cases]]
case_id = "unattempted_case"
code_id = "toric:d=3"
distance_side = "any"
iterations = 30
restarts = 3
seed = 31
target_weight = 3
target_upper_bound = 3
baseline_key = "unmapped:toric_d3"
baseline_required = false
```

Create `benchmarks/qec_code_random_window/tests/fixtures/summary_runs.jsonl` with exactly these four JSON objects, one per line:

```jsonl
{"case_id":"target_case","status":"ok","seed":11,"iterations":10,"restarts":2,"target_weight":5,"upper_bound":7,"elapsed_s":3.0,"command":["qec-code","code","css-distance","random-window-upper-bound","--code-id","bb72","--seed","11"]}
{"case_id":"target_case","status":"ok","seed":12,"iterations":10,"restarts":2,"target_weight":5,"upper_bound":5,"elapsed_s":1.0,"command":["qec-code","code","css-distance","random-window-upper-bound","--code-id","bb72","--seed","12"]}
{"case_id":"target_case","status":"cli_error","seed":13,"iterations":10,"restarts":2,"target_weight":5,"upper_bound":null,"elapsed_s":0.5,"stderr_context":"fixture failure"}
{"case_id":"no_success_case","status":"cli_error","seed":21,"iterations":20,"restarts":1,"target_weight":3,"upper_bound":null,"elapsed_s":0.2,"stderr_context":"fixture failure"}
```

Create `benchmarks/qec_code_random_window/tests/fixtures/missing_upper_bound_success.jsonl`:

```jsonl
{"case_id":"target_case","status":"ok","seed":11,"iterations":10,"restarts":2,"target_weight":5,"elapsed_s":1.0}
```

Create `benchmarks/qec_code_random_window/tests/test_summarize.py`:

```python
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
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize -q
```

Expected: FAIL because `benchmarks.qec_code_random_window.summarize` does not exist.

- [ ] **Step 3: Implement the summarizer**

Create `benchmarks/qec_code_random_window/summarize.py` with these concrete components:

```python
CSV_FIELDS = [
    "case_id",
    "code_id",
    "distance_side",
    "baseline_key",
    "baseline_required",
    "manifest_seed",
    "manifest_iterations",
    "manifest_restarts",
    "manifest_target_weight",
    "target_upper_bound",
    "attempted_seed_rows",
    "successful_seed_rows",
    "best_upper_bound",
    "median_elapsed_s",
    "min_elapsed_s",
    "max_elapsed_s",
    "target_hit_count",
    "target_hit_rate",
    "run_seed_values",
    "run_iterations_values",
    "run_restarts_values",
    "run_target_weight_values",
    "run_status_values",
    "summary_status",
]
```

Implement:

- `SummaryError(Exception)` for user-facing validation errors.
- `_positive_int(value: object) -> bool` returning true only for non-bool positive integers.
- `_numeric(value: object) -> bool` returning true only for non-bool finite `int` or `float` values.
- `_format_bool(value: object) -> str` returning lowercase `true` or `false`.
- `_format_optional_int(value: object) -> str` returning `""` for `None`, otherwise decimal integer text.
- `_format_float(value: float) -> str` returning `str(float(value))`.
- `_join_sorted_ints(rows: list[dict[str, Any]], field: str) -> str` collecting valid integer values from rows and joining sorted unique values with `;`.
- `_join_sorted_strings(rows: list[dict[str, Any]], field: str) -> str` collecting non-empty string values from rows and joining sorted unique values with `;`.
- `load_run_rows(paths: list[Path], known_case_ids: set[str]) -> dict[str, list[dict[str, Any]]]` that reads JSONL, reports `<path>:<line>: <message>` on malformed lines, rejects unknown cases, and rejects `status = "ok"` rows missing positive integer `upper_bound` or numeric `elapsed_s`.
- `summarize_cases(manifest: dict[str, Any], grouped_rows: dict[str, list[dict[str, Any]]]) -> list[dict[str, str]]` that computes the CSV rows exactly as tested.
- `write_summary_csv(path: Path, rows: list[dict[str, str]]) -> None`.
- `write_summary_md(path: Path, manifest: dict[str, Any], cases_path: Path, run_paths: list[Path], rows: list[dict[str, str]], argv: list[str]) -> None`.
- `build_parser() -> argparse.ArgumentParser` with required `--cases`, `--runs` (`nargs="+"`), and `--out-dir`.
- `run(args: argparse.Namespace, argv: list[str]) -> int` that creates `--out-dir`, writes both summary files, and returns 0 or 2.
- `main(argv: list[str] | None = None) -> int` that preserves exact argv for Markdown provenance.

The Markdown table must include one row per summary row and use `NO SUCCESSFUL ROWS` when `summary_status == "no_success"`.

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize -q
```

Expected: PASS, 4 tests.

- [ ] **Step 5: Run combined benchmark Python tests**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases benchmarks.qec_code_random_window.tests.test_run_local benchmarks.qec_code_random_window.tests.test_summarize -q
```

Expected: PASS, 14 tests.

- [ ] **Step 6: Run CLI help verification**

Run:

```bash
python3 -m benchmarks.qec_code_random_window.summarize --help
```

Expected: exit 0 and help text includes `--cases`, `--runs`, and `--out-dir`.

- [ ] **Step 7: Commit**

Run:

```bash
git add benchmarks/qec_code_random_window/summarize.py \
  benchmarks/qec_code_random_window/tests/test_summarize.py \
  benchmarks/qec_code_random_window/tests/fixtures/summary_cases.toml \
  benchmarks/qec_code_random_window/tests/fixtures/summary_runs.jsonl \
  benchmarks/qec_code_random_window/tests/fixtures/missing_upper_bound_success.jsonl
git commit -m "benchmarks: summarize qec random-window runs"
```

Expected: one implementation commit containing only the summarizer, tests, and fixtures.

## Plan Self-Review

- Spec coverage: Task 1 covers the requested CLI, manifest preservation, CSV and Markdown outputs, per-case metrics, target-hit metrics, malformed JSONL handling, baseline preservation, help behavior, and verification commands.
- Placeholder scan: no unresolved placeholder or deferred implementation steps remain.
- Type consistency: function names and CSV field names are defined once in this plan and reused consistently.
