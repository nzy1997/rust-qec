# Task 1 Report: BB144 CLI Smoke Regression

## Summary
Implemented the requested qec-code CLI regression coverage for BB144 in `qec-code/tests/cli.rs` only. No production code, fixtures, or unrelated docs were changed.

## Files Changed
- `qec-code/tests/cli.rs`
- `.agent-sdd/task-1-report.md`

## What Changed
- Added `BB144_PARAMETERIZED_SPEC` immediately after `BB72_PARAMETERIZED_SPEC`.
- Added `code_css_bb144_parameterized_hx_prints_sparse_rows_shape()` to assert the BB144 `code css ... hx` CLI path returns valid `sparse_rows` JSON with:
  - `format == "sparse_rows"`
  - `num_cols == 144`
  - `rows.len() == 72`
  - every row has weight 6
- Added `code_css_bb_parameterized_malformed_shift_term_fails_without_json()` as a negative control for malformed BB family shift syntax.

## TDD Evidence
- The BB144 smoke regression test passed on the first focused run, so this task stayed in the coverage-only lane and did not require production code changes.
- The malformed-shift negative control also passed, confirming the parser failure path stays non-JSON and reports the expected family-`bb` integer-parameter error.

## Test Results
- `cargo test -p qec-code --test cli bb144`
  - Passed
  - Result: `code_css_bb144_parameterized_hx_prints_sparse_rows_shape ... ok`
- `cargo test -p qec-code --test cli malformed_shift_term`
  - Passed
  - Result: `code_css_bb_parameterized_malformed_shift_term_fails_without_json ... ok`

## Notes
- This task intentionally did not touch `rsinter` or any production code.
- The requested BB144 CLI behavior already existed; this change adds regression coverage so it stays covered.
