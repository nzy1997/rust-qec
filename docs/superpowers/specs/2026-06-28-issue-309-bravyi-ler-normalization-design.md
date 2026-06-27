# Issue 309 Bravyi LER Normalization Design

## Objective

Add a deterministic verifier and regression tests that prove BB compare CSV
artifacts use Bravyi-style trial-level logical error-rate normalization:
`logical_error_rate = logical_errors / shots_used`.

## Context

Issue #305 pins the Bravyi source contract: paper-style rows are
`physical_error_rate, num_syndrome_cycles, num_monte_carlo_trials,
num_failed_trials`, and the failure unit is one Monte Carlo trial. The
existing BB batched runner already accumulates failed trials and writes
`logical_error_rate` from `logical_errors / shots_used`; this issue adds a
reviewer-friendly gate so future CSV or plot-input changes cannot silently
divide by syndrome cycles or logical observables.

## Chosen Approach

Create `benchmarks.bb_circuit_bposd_compare.verify_bravyi_ler` as a pure CSV
verifier with no plotting dependency. It reads BB compare `results.csv`, accepts
completed or partial batched rows, and checks:

- required batched columns are present,
- accepted rows have positive `shots_used`,
- `logical_error_rate` equals `logical_errors / shots_used` within a tight
  tolerance,
- Bravyi tuples are reported as `(p, num_cycles, shots_used, logical_errors)`,
- per-cycle-looking mismatches are named explicitly in the failure output.

The verifier prints one compact table row per accepted CSV row:
`case_id`, `decoder_impl`, `shots_used`, `logical_errors`,
`logical_error_rate`, and `bravyi_tuple`, each prefixed with `PASS`.

## Alternatives Considered

1. Extend `verify_smoke` or `verify_diagnostic`.
   This would mix tier-specific pairing checks with a normalization contract
   that applies to full and partial batched CSV artifacts.

2. Regenerate full comparison artifacts.
   This is out of scope and unnecessary because the checked-in full CSV already
   contains the data needed for a deterministic verification gate.

3. Check only Rust plotting tests.
   Rust tests can prove adapter behavior, but they do not give reviewers a
   direct PASS/FAIL command for checked-in CSV artifacts.

## Rust Plot Adapter

The Rust BB CSV adapter must keep the CSV `logical_error_rate` metric unchanged
and must expose `num_cycles` as `params.rounds`. The benchmark plot layer's
default `logical_rate_unit` is `per_shot`, so logical-rate plotting uses the
same trial-level count basis unless a spec explicitly opts into another unit.
Add a focused Rust test that reads a batched BB CSV row where per-shot and
per-cycle values differ, then asserts the adapter metric and per-shot plot fit
use `logical_errors / shots_used`.

## Tests

Add Python tests under
`benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_ler_normalization.py`
covering:

- synthetic valid rows, including `ok` and `partial`,
- the checked-in full CSV, including at least one BB144 row with tuple
  `(0.003, 12, 40000, 200)` when present,
- negative control for per-cycle normalization,
- CLI exit and reviewer table output.

Add a Rust adapter regression in `rsinter/tests/bench_cli.rs` because that file
already exercises `plot-bb-compare-csv` fixtures.

## Out Of Scope

- Decoder prediction changes.
- Full comparison regeneration.
- PNG or matplotlib verification.
- Scientific interpretation of any remaining BB72/BB144 accuracy gap.
