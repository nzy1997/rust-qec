# Issue 333 Random-Window Real Paper Baseline Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the qec-code random-window paper-baseline importer discover real codeDistancePYPI workbook tables by contents instead of only by filename and sheet-name tokens.

**Architecture:** Keep the canonical importer CLI and CSV schema unchanged. Broaden workbook scanning to every `.xlsx`, inspect sheets for supported header/case/method evidence, keep legacy selected-sheet strict validation, and preserve required-case failures when `bb72_full` or `bb144_full` cannot be imported.

**Tech Stack:** Python standard library, optional `openpyxl` for real `.xlsx`, existing unittest fixture helpers, existing Make benchmark targets, Cargo workspace tests.

## Global Constraints

- Canonical CSV columns must stay exactly `case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row`.
- Required full-manifest rows are `bb72_full` and `bb144_full`.
- Do not change the `qec-code` random-window algorithm.
- Do not check in upstream codeDistancePYPI spreadsheets or hand-written replacement baseline CSVs.
- Do not weaken `--strict-baselines` or mark required BB cases optional.
- Preserve nonzero failure when required paper baseline rows are absent.
- Use TDD: write and run failing importer tests before production changes.

---

### Task 1: Add Real-Shape Importer Regression Tests

**Files:**
- Modify: `benchmarks/qec_code_random_window/tests/test_import_paper_baselines.py`

**Interfaces:**
- Consumes: existing `write_manifest`, `write_xlsx`, `write_multi_sheet_xlsx`, and `run_importer` test helpers.
- Produces: failing tests that require content-based workbook/sheet detection and method-sheet support.

- [x] **Step 1: Replace obsolete name-token rejection tests with content-discovery expectations**

Change `test_selected_workbook_with_unrelated_sheet_errors` so a selected workbook with sheet `Other Data` and valid headers imports successfully. Change `test_unrelated_workbook_is_ignored_and_missing_selected_sheet_errors` so an unselected workbook named `notes.xlsx` with sheet `BB summary` imports successfully. These expectations document that the importer trusts table evidence more than workbook/sheet names.

- [x] **Step 2: Add an `analysis` sheet regression test**

Add this test method to `ImportPaperBaselinesTest`:

```python
    def test_analysis_sheet_with_real_shape_headers_imports_required_rows(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true

            [[cases]]
            case_id = "bb144_fixture"
            code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 12
            target_upper_bound = 12
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
            baseline_required = true
            """,
            )
            write_xlsx(
                paper_dir / "results.xlsx",
                "analysis",
                [
                    ["dataset", "algorithm", "distance", "time (s)"],
                    ["BB72", "QDistRndMW", 6, 12.5],
                    ["bb 144", "QDistEvol", 12, 30],
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                out.read_text(encoding="utf-8"),
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n"
                "bb72_fixture,bb72,QDistRndMW,6,12.5,results.xlsx,analysis,2\n"
                "bb144_fixture,bb144,QDistEvol,12,30,results.xlsx,analysis,3\n",
            )
```

- [x] **Step 3: Add a method-sheet regression test**

Add this test method:

```python
    def test_qdist_method_sheets_imply_missing_method_column(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            out = tmp_path / "baselines.csv"
            paper_dir = tmp_path / "paper-results"
            paper_dir.mkdir()
            write_manifest(
                manifest,
                """
            [[cases]]
            case_id = "bb72_fixture"
            code_id = "bb72"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 6
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true

            [[cases]]
            case_id = "bb144_fixture"
            code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
            distance_side = "any"
            iterations = 5000
            restarts = 8
            seed = 7
            target_weight = 12
            target_upper_bound = 12
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
            baseline_required = true
            """,
            )
            write_multi_sheet_xlsx(
                paper_dir / "paper-results.xlsx",
                [
                    ("QDistRndMW", [["code", "d", "runtime"], ["bb72", 6, 12.5]]),
                    ("QDistEvol", [["code", "d", "runtime"], ["bb144", 12, 30]]),
                ],
            )

            result = run_importer(
                [
                    "--cases",
                    str(manifest),
                    "--paper-results-dir",
                    str(paper_dir),
                    "--out",
                    str(out),
                ]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                out.read_text(encoding="utf-8"),
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n"
                "bb72_fixture,bb72,QDistRndMW,6,12.5,paper-results.xlsx,QDistRndMW,2\n"
                "bb144_fixture,bb144,QDistEvol,12,30,paper-results.xlsx,QDistEvol,2\n",
            )
```

- [x] **Step 4: Run importer tests and verify RED**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_import_paper_baselines -q
```

Expected: FAIL before production changes. The new content-discovery tests should fail with missing required sheet or missing required method column errors.

### Task 2: Implement Content-Based Sheet Discovery

**Files:**
- Modify: `benchmarks/qec_code_random_window/import_paper_baselines.py`

**Interfaces:**
- Consumes: `BASELINE_KEY_TO_PAPER_CASE`, `PAPER_CASE_LOOKUP`, `REQUIRED_COLUMN_ALIASES`, and `SheetRow`.
- Produces: importer behavior that scans all `.xlsx` files and recognizes table headers and method sheet names.

- [x] **Step 1: Normalize headers robustly**

Import `re` and update `_normalize_name` so punctuation such as `time (s)` normalizes to `time_s`:

```python
def _normalize_name(value: str) -> str:
    normalized = re.sub(r"[^0-9a-zA-Z]+", "_", value.strip().lower()).strip("_")
    while "__" in normalized:
        normalized = normalized.replace("__", "_")
    return normalized
```

- [x] **Step 2: Broaden aliases and method sheet names**

Add `dataset`, `code_name`, and `label` to `paper_case`; add `runtime`, `elapsed`, `time`, `wall_time`, and `walltime` to elapsed aliases. Do not use bare `t` as an elapsed-time alias because QDist workbooks use `T` as a count column. Add:

```python
METHOD_SHEET_ALIASES = {
    "qdistrndmw": "QDistRndMW",
    "qdist_rnd_mw": "QDistRndMW",
    "qdistevol": "QDistEvol",
    "qdist_evol": "QDistEvol",
}
```

Add `_method_from_sheet_name(sheet_name: str) -> str | None` that returns the canonical method when an alias appears in the normalized sheet name.

- [x] **Step 3: Resolve header rows instead of only row 1**

Add helper functions:

```python
def _header_indexes(row: list[str]) -> dict[str, int]:
    return {_normalize_name(value): index for index, value in enumerate(row) if value.strip()}


def _resolve_column_indexes(header_indexes: dict[str, int], *, method_from_sheet: str | None) -> dict[str, int]:
    indexes: dict[str, int] = {}
    for column in REQUIRED_COLUMNS:
        if column == "baseline_method" and method_from_sheet is not None:
            continue
        for alias in REQUIRED_COLUMN_ALIASES[column]:
            index = header_indexes.get(_normalize_name(alias))
            if index is not None:
                indexes[column] = index
                break
    return indexes
```

Use these helpers to scan each sheet row as a possible header. A resolved table is valid when it has all required columns, except `baseline_method` may be supplied by `method_from_sheet`.

- [x] **Step 4: Extract candidate rows from any sheet**

Replace the old sheet-name selection in `_extract_sheet_rows` with all-sheet scanning. For each valid header row, extract following data rows. Use `method_from_sheet` when the method column is absent. Append only non-empty rows, and let `_match_rows` discard rows whose paper case is not recognized.

- [x] **Step 5: Preserve selected legacy sheet validation**

If a sheet name still matches `_is_selected_name(sheet_name)` and no valid table was extracted, inspect the first non-empty row. If that row contains at least one required-column alias but not all required logical columns, raise:

```python
ValueError(f'{path.name}: sheet "{sheet_name}" missing required column "{column}"')
```

This preserves the existing malformed selected-sheet failure tests.

- [x] **Step 6: Scan every workbook**

Change `_paper_result_files` to return every `.xlsx` in the directory sorted by path. Leave the global no-rows failure in `import_rows` so an empty directory or unrelated spreadsheets still exit nonzero.

- [x] **Step 7: Run importer tests and verify GREEN**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_import_paper_baselines -q
```

Expected: PASS.

### Task 3: Verification And Branch Review

**Files:**
- Modify if needed: `benchmarks/qec_code_random_window/import_paper_baselines.py`
- Modify if needed: `benchmarks/qec_code_random_window/tests/test_import_paper_baselines.py`

**Interfaces:**
- Consumes: Tasks 1-2.
- Produces: verified branch ready for PR creation.

- [x] **Step 1: Run all qec random-window Python tests**

Run:

```bash
python3 -m unittest discover benchmarks/qec_code_random_window/tests -q
```

Expected: PASS.

- [x] **Step 2: Run the negative control**

Run:

```bash
rm -rf /tmp/qec-rw-empty-paper-results
mkdir -p /tmp/qec-rw-empty-paper-results
python3 -m benchmarks.qec_code_random_window.import_paper_baselines \
  --cases benchmarks/qec_code_random_window/cases.full.toml \
  --paper-results-dir /tmp/qec-rw-empty-paper-results \
  --out /tmp/qec-rw-empty-baselines.csv
```

Expected: nonzero exit with a message about missing required sheets or rows.

- [x] **Step 3: Run real upstream verification when possible**

Run the issue's clone/import/full-pipeline commands if shell network access is available or `/tmp/codeDistancePYPI-rw-baselines/paper results` already exists. If the sandbox blocks GitHub network access, record the exact network error and do not substitute committed baselines.

- [x] **Step 4: Run Rust verification**

Run:

```bash
cargo test -p qec-code
cargo test
```

Expected: PASS.

- [x] **Step 5: Run diff hygiene and review**

Run:

```bash
git diff --check
git diff -- benchmarks/qec_code_random_window/import_paper_baselines.py benchmarks/qec_code_random_window/tests/test_import_paper_baselines.py
```

Expected: no whitespace errors; diff only touches the importer, tests, and Superpowers docs for this issue.
