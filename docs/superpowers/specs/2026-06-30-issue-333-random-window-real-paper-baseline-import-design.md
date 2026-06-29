# Issue 333 Random-Window Real Paper Baseline Import Design

## Context

Issue #333 fixes the paper-baseline importer added for issue #324 after the
full benchmark path from issue #326 failed against the real
`codeDistancePYPI/paper results` workbooks. The current importer filters both
workbook filenames and sheet names by tokens such as `bb`, `bivariate`, `qc`,
and `summary` before it inspects table contents. The reported upstream
workbooks contain relevant sheets named `analysis`, `best`, `QDistRndMW`, and
`QDistEvol`, so the importer can reject real evidence before looking at headers
or rows.

The required full-manifest paper rows are still only `bb72_full` and
`bb144_full`. The fix must keep the canonical CSV interface unchanged and must
not weaken strict-baseline comparison, commit external spreadsheets, or change
the `qec-code` random-window algorithm.

## Approaches Considered

1. Add `analysis`, `best`, `QDistRndMW`, and `QDistEvol` to the sheet-name
   allowlist.
   - Pros: smallest patch.
   - Cons: keeps the brittle filename/sheet-name design and will fail on the
     next equivalent upstream table name.

2. Select candidate sheets by evidence in their contents.
   - Pros: matches the real failure, handles renamed sheets, preserves
     provenance, and keeps required-row strictness tied to manifest mappings.
   - Cons: needs careful tests so unrelated spreadsheets are not silently
     accepted.

3. Hard-code known upstream workbook and sheet names for the current
   `codeDistancePYPI` checkout.
   - Pros: could match today's data if the exact files are known.
   - Cons: couples the importer to external repository layout and is less
     defensible than recognizing the table contract.

Chosen approach: content-based sheet discovery with explicit manifest-key and
case aliases.

## Design

Keep `benchmarks/qec_code_random_window/import_paper_baselines.py` as the only
production module changed. The importer will scan all `.xlsx` files in the
provided paper-results directory. For each workbook it will inspect every sheet
and treat a sheet as a candidate when it contains a row whose normalized cells
can satisfy the required logical columns:

- `paper_case`
- `baseline_method`
- `baseline_upper_bound`
- `baseline_elapsed_s`

Header aliases from issue #324 remain supported. The importer will also support
real-codeDistance-style method sheets by allowing the sheet name to provide the
method when the table has case, distance, and elapsed-time columns but no
method column. This covers sheets named `QDistRndMW` and `QDistEvol` without
requiring a duplicated method column in every row.

Candidate detection is evidence based:

- At least one extracted row must match a known required paper case alias such
  as `bb72` or `bb144`.
- The extracted method must be non-empty, either from the method column or the
  recognized method sheet name.
- The extracted upper bound and elapsed time cells must be non-empty.

Sheets with selected legacy names remain subject to strict column validation so
the existing bad-selected-sheet tests keep catching malformed explicitly
selected fixture tables. Unselected sheets that do not look like a supported
table are ignored; if no supported rows are found, the importer exits nonzero
with a missing required paper baseline rows or missing candidate sheet message.

## Error Handling

The importer must preserve strict failure behavior:

- If the paper-results directory has no usable `.xlsx` candidate rows, exit
  nonzero.
- If a selected legacy sheet is malformed, exit nonzero and name the missing
  required logical column.
- If a required manifest case such as `bb72_full` or `bb144_full` is absent
  after scanning, exit nonzero and name the missing `case_id`.
- Never fabricate `NA` rows or placeholder provenance in the importer output.

## Tests

Add focused tests to
`benchmarks/qec_code_random_window/tests/test_import_paper_baselines.py` using
the existing synthetic `.xlsx` fixture writer:

- A workbook named without legacy tokens and a sheet named `analysis` imports
  `bb72` and `bb144` when the sheet has recognizable headers.
- A workbook with `QDistRndMW` and `QDistEvol` sheets imports rows when the
  method is implied by the sheet name.
- An unrelated workbook with no matching required paper cases still exits
  nonzero instead of passing with an empty CSV.

Run the existing random-window Python unittest suite and the issue's negative
control. When external network or a pre-existing upstream checkout is available,
also run the real `codeDistancePYPI` import command and the full Make target.

## Acceptance

The canonical CSV columns stay exactly:

```text
case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row
```

For real upstream data, imported rows for `bb72_full` and `bb144_full` must
have non-empty method, upper-bound, source file, source sheet, and source row
provenance. The full strict comparison must then show non-`NA` paper baselines
for both BB full cases.
