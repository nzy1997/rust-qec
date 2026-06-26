# Issue 284 Small-LDPC Catalog Design

## Context

Issue #284 extends the BB circuit BP-OSD compare package from the smoke-only
surface introduced by #217/#272 into a complete manifest for the #209
`small_ldpc.png` target set. The existing compare runner should not execute the
full 50,000-trial campaign by default, but reviewers need a checked-in catalog
that proves every target point is named, stable, and configured with the same
decoder settings.

The Rust BB circuit constructor currently supports `bb72`, `bb90`, and `bb144`.
It does not support `bb108` or `bb288`, so the catalog must keep those target
points visible with an explicit unsupported status.

## Considered Approaches

1. Expand `SMOKE_CASES` to include all 31 small-LDPC points. This would reuse
   the current runner path, but it would make the default smoke tier unsuitable
   for local runs and risk launching 50,000-trial points by accident.

2. Add a separate `SMALL_LDPC_CASES` manifest with validation helpers and a
   dry-run manifest tier. This keeps the executable smoke tier small, preserves
   all #209 target metadata, and gives tests a direct catalog contract.

3. Store the catalog only in README prose. This is easy to review, but it is not
   machine-validating and would not give negative-control tests a real API.

Chosen approach: option 2.

## Architecture

`benchmarks/bb_circuit_bposd_compare/cases.py` will remain the source of case
truth. It will keep `SMOKE_CASES` unchanged for runnable diagnostics and add:

- `SMALL_LDPC_CASES`: the 31 #209 target points.
- `CATALOG_HEADER` and `small_ldpc_manifest_rows()` for a dry-run CSV manifest.
- `validate_small_ldpc_catalog()` for exact target, settings, status, and
  `case_id` validation.
- a stable `case_id` formatter using code id, p label, cycles, trial budget,
  and seed.

Each small-LDPC catalog case uses:

- `num_trials = 50000`
- `seed = 12345`
- `bp_method = "ms"`
- `max_iter = 10000`
- `osd_method = "osd_cs"` as the documented upstream equivalent of the
  requested `ldpc_cs` setting in this branch
- `osd_order = 7`
- `scaling = 0`

`bb72`, `bb90`, and `bb144` cases have `catalog_status = "supported"`.
`bb108` and `bb288` cases have
`catalog_status = "unsupported_rust_constructor"` and a note naming the missing
Rust constructor support.

## Data Flow

The normal `smoke` tier continues to execute only `SMOKE_CASES`.

A new manifest-only tier in `run_compare.py` will validate `SMALL_LDPC_CASES`
and write a dry-run `manifest.csv` without invoking Rust or Python decoders.
This makes the complete target set observable without running any trials.

## Error Handling

The validator reports missing and unexpected targets using code id, p value, and
cycle count so negative controls identify the exact catalog defect. It also
reports duplicate `case_id` values, mismatched generated `case_id` values,
wrong trial budgets, wrong decoder settings, and unsupported-status mistakes.

## Testing

Add `benchmarks/bb_circuit_bposd_compare/tests/test_cases.py` with pytest tests
filtered by `-k small_ldpc_catalog` that verify:

- exactly 31 catalog cases,
- the #209 p sweeps and cycles match BB72, BB90, BB108, BB144, and BB288,
- every `case_id` includes code, p, cycles, trials, and seed,
- decoder settings match the issue contract,
- BB108 and BB288 are explicitly marked unsupported,
- negative controls reject a missing BB108 catalog and a wrong BB288 p value
  with messages naming the affected target.

Update the README with the dry-run command and target coverage table.
