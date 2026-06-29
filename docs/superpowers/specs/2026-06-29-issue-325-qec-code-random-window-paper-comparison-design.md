# Issue 325 QEC-Code Random-Window Paper Comparison Design

## Context

Issue #325 adds the final reporting layer for the QEC-code random-window
benchmark flow. Issue #321 defines benchmark manifests with `case_id`,
`baseline_key`, and `baseline_required`. Issue #323 writes manifest-ordered
local `summary.csv` files with `best_upper_bound` and `median_elapsed_s`.
Issue #324 imports defensible codeDistancePYPI paper rows into a canonical CSV
with `case_id`, method, upper bound, elapsed seconds, and source provenance.

The GitHub connector returned no comments for issues #321, #323, #324, or
#325. The design uses the provided issue body plus the merged local specs and
current benchmark modules.

## Approaches Considered

1. Add a standalone standard-library comparison CLI.
   - Pros: matches the requested module interface, keeps collection,
     summarization, paper import, and comparison as separate auditable steps,
     and follows the existing benchmark package pattern.
   - Cons: duplicates a small amount of CSV/Markdown formatting logic.

2. Add comparison output to `summarize.py`.
   - Pros: fewer commands for users who already have local JSONL output.
   - Cons: couples local-only summaries to external paper baseline data and
     makes it harder to compare previously generated summary files.

3. Add comparison output to `import_paper_baselines.py`.
   - Pros: baseline provenance is already in memory during import.
   - Cons: paper import should not depend on local benchmark runs, and #324
     deliberately emits only canonical paper rows.

Chosen approach: a standalone Python standard-library module
`benchmarks/qec_code_random_window/compare_paper.py`.

## Design

The comparison CLI is:

```bash
python3 -m benchmarks.qec_code_random_window.compare_paper \
  --cases benchmarks/qec_code_random_window/cases.smoke.toml \
  --local-summary /tmp/qec-rw-summary/summary.csv \
  --paper-baselines /tmp/codeDistancePYPI-baselines.csv \
  --out-dir /tmp/qec-rw-compare
```

It also supports `--strict-baselines`. The module loads and validates the
manifest with the existing #321 validator, reads the #323 local summary CSV,
and reads the #324 canonical baseline CSV. Output contains exactly one row per
manifest case in manifest order.

The join key is `case_id`. The #324 importer already resolves explicit
manifest `baseline_key` values to canonical baseline rows, so `case_id` is the
least ambiguous comparison contract. Manifest cases whose baseline key is
`unmapped:*` or whose canonical CSV has no row are still emitted, but all paper
baseline fields, delta fields, and ratio fields are `NA`.

If multiple paper rows exist for the same case, the comparison emits the first
baseline row in CSV order. That preserves source order from the importer and
keeps the initial comparison to one human-readable row per benchmark case. A
later issue can add method selection if the canonical paper CSV begins carrying
multiple defensible methods per case.

## Output Contract

The command creates the output directory and writes:

- `comparison.csv`
- `comparison.md`

CSV columns are:

- `case_id`
- `code_id`
- `distance_side`
- `baseline_key`
- `baseline_required`
- `local_best_upper_bound`
- `local_median_elapsed_s`
- `paper_method`
- `paper_upper_bound`
- `paper_elapsed_s`
- `upper_bound_delta`
- `elapsed_time_ratio`
- `baseline_provenance`
- `baseline_source_file`
- `baseline_source_sheet`
- `baseline_source_row`
- `comparison_status`

`upper_bound_delta = local_best_upper_bound - paper_upper_bound`. Negative
values mean the local run found a smaller upper bound than the paper row.

`elapsed_time_ratio = local_median_elapsed_s / paper_elapsed_s`. It is `NA`
when either timing field is missing, nonnumeric, nonfinite, nonpositive, or not
comparable. The ratio is formatted with six digits after the decimal point for
stable fixture output.

`comparison_status` is `paper_matched` when a canonical baseline row joins to
the case and `no_paper_baseline` otherwise. Rows with no paper match must not
look like paper-backed comparisons.

Markdown includes a provenance section for all three inputs plus the exact CLI
argv. Its table includes both numeric comparison fields and baseline
provenance columns (`baseline_provenance`, `source_file`, `source_sheet`, and
`source_row`) so the evidence can be audited without opening the CSV.

## Strict Baselines

`--strict-baselines` exits nonzero when any manifest case has
`baseline_required = true` and no canonical paper row joins by `case_id`.
Non-strict mode still writes comparison artifacts and marks those rows
`no_paper_baseline`.

Cases with `baseline_required = false` never cause strict-baseline failure when
unmatched. They still appear in output with `NA` paper fields.

## Validation and Error Handling

The command exits nonzero with file context when:

- the manifest fails the existing #321 validation
- the local summary CSV is missing required #323 columns
- the paper baseline CSV is missing required #324 columns
- a local summary references a case outside the manifest
- a paper baseline references a case outside the manifest
- a required numeric comparison field is malformed in a matched row
- `--strict-baselines` finds missing required baseline rows
- output files cannot be written

Blank local result fields are allowed and render as `NA`; they produce `NA`
deltas and ratios. This lets no-success local cases remain visible without
inventing comparison numbers.

## Testing

Add `benchmarks/qec_code_random_window/tests/test_compare_paper.py` and
fixture files under `benchmarks/qec_code_random_window/tests/fixtures/`.

Required coverage:

- fixture local summary plus fixture paper baseline produces exact expected
  `comparison.csv` and `comparison.md`
- `upper_bound_delta` follows `local_best_upper_bound - paper_upper_bound`
- `elapsed_time_ratio` follows `local_median_elapsed_s / paper_elapsed_s` and
  uses stable six-decimal formatting
- unmatched `baseline_required = false` cases appear with `NA` paper fields
  and do not fail non-strict comparison
- `--strict-baselines` exits nonzero for an unmatched
  `baseline_required = true` case
- Markdown includes baseline provenance columns, not only numeric values
- `python3 -m benchmarks.qec_code_random_window.compare_paper --help` exits 0

Required verification:

- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_compare_paper -q`
- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases benchmarks.qec_code_random_window.tests.test_summarize benchmarks.qec_code_random_window.tests.test_import_paper_baselines benchmarks.qec_code_random_window.tests.test_compare_paper -q`
- `python3 -m benchmarks.qec_code_random_window.compare_paper --help`
- `cargo test`
