# Issue 324 codeDistancePYPI Paper Baselines Design

## Context

Issue #324 asks for a static importer for codeDistancePYPI paper-result
spreadsheets. The local benchmark manifests from issue #321 already define
`case_id`, `baseline_key`, and `baseline_required`, with defensible
codeDistancePYPI keys only for `bb72` and `bb144`. This issue converts
selected upstream rows into a canonical CSV for later comparison code. It must
not rerun codeDistancePYPI algorithms or commit upstream spreadsheets.

Shell network access is blocked in this Agent Desk sandbox, and `openpyxl` is
not installed locally. The design therefore keeps normal verification on small
synthetic `.xlsx` fixtures and documents `openpyxl` as the package required for
real upstream spreadsheets.

## Approaches Considered

1. Add a narrow, manifest-keyed importer for bivariate-bicycle paper rows.
   - Pros: follows the existing `baseline_key` contract, maps only rows that
     explicitly match local cases, and avoids speculative Steane/surface/toric
     matches.
   - Cons: the first importer covers only the BB subset until more upstream
     rows are inspected and mapped.

2. Add a generic spreadsheet normalizer that accepts arbitrary sheet and column
   names.
   - Pros: flexible for future paper tables.
   - Cons: easier to silently mis-map aggregate or unrelated rows, which the
     issue explicitly forbids.

3. Add committed canonical CSV rows by hand.
   - Pros: fastest consumer path.
   - Cons: loses provenance to source file, sheet, and row, and does not give
     maintainers a repeatable conversion command.

Chosen approach: a narrow importer with explicit mappings for
`codeDistancePYPI:bivariate_bicycle:bb72` and
`codeDistancePYPI:bivariate_bicycle:bb144`.

## Design

Create `benchmarks/qec_code_random_window/import_paper_baselines.py`. The CLI
accepts:

- `--cases <manifest.toml>`
- `--paper-results-dir <dir>`
- `--out <csv>`

If `--paper-results-dir` is omitted, the importer reads
`CODEDISTANCE_PAPER_RESULTS_DIR`. If neither is present, it exits nonzero with
an actionable error.

The importer loads the TOML manifest with `tomllib`, selects cases whose
`baseline_key` is explicitly mapped, scans selected `.xlsx` workbooks under the
paper-results directory, and writes canonical CSV rows with these columns:

- `case_id`
- `paper_case`
- `baseline_method`
- `baseline_upper_bound`
- `baseline_elapsed_s`
- `source_file`
- `source_sheet`
- `source_row`

The importer omits manifest cases whose `baseline_key` starts with `unmapped:`
or has no known mapping. This is the documented canonical CSV policy: the CSV
contains only defensible baseline evidence rows. Unmapped cases are not
silently matched to similar paper rows, and no `NA` row is emitted.

## Spreadsheet Contract

The smallest useful supported paper table is a bivariate-bicycle summary table.
The importer searches workbook files whose names contain `bb`,
`bivariate`, or `summary`, and sheets whose names contain `bb`,
`bivariate`, or `summary`.

Required logical columns are:

- a paper-case column, accepted from headers such as `paper_case`, `case`,
  `name`, or `code`
- a method column, accepted from headers such as `baseline_method`,
  `method`, `algorithm`, or `decoder`
- an upper-bound column, accepted from headers such as
  `baseline_upper_bound`, `upper_bound`, `ub`, `distance`, or `d`
- an elapsed-seconds column, accepted from headers such as
  `baseline_elapsed_s`, `elapsed_s`, `seconds`, `time_s`, or `runtime_s`

Rows match local cases only when the normalized paper-case value is one of the
explicit aliases for the manifest key:

- `codeDistancePYPI:bivariate_bicycle:bb72`: `bb72`
- `codeDistancePYPI:bivariate_bicycle:bb144`: `bb144`

If a selected sheet is missing a required logical column, the importer exits
nonzero and names the missing field. This catches upstream layout drift instead
of producing partial or incorrectly matched CSV output.

## README

Add `benchmarks/qec_code_random_window/README.md` with:

- the upstream repository URL and expected `paper results/` directory
- a local setup path, for example cloning upstream outside this repository and
  passing `--paper-results-dir`
- the `openpyxl` dependency for reading `.xlsx`
- the canonical CSV policy that unmapped manifest cases are omitted
- a note that upstream `.xlsx` files are not committed here

## Testing

Add `benchmarks/qec_code_random_window/tests/test_import_paper_baselines.py`.
Tests create small synthetic `.xlsx` fixtures and verify:

- a workbook with the expected columns converts to an exact canonical CSV
- a missing required sheet exits nonzero and names the sheet
- a missing required column exits nonzero and names the missing field
- a case with an unmapped manifest key is omitted rather than matched by
  similar text
- `CODEDISTANCE_PAPER_RESULTS_DIR` works when `--paper-results-dir` is omitted

Focused verification:

- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_import_paper_baselines -q`
- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases -q`
- `python3 -m benchmarks.qec_code_random_window.import_paper_baselines --cases benchmarks/qec_code_random_window/cases.full.toml --paper-results-dir <dir> --out /tmp/codeDistancePYPI-baselines.csv`

Repository verification:

- `cargo test`

If the real upstream `paper results` directory is available locally, run the
documented command and require at least one mapped row for `cases.full.toml`.
That check is not part of ordinary CI because the large upstream spreadsheets
are external data.
