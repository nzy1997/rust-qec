# Issue 343 Random-Window Search Timing Design

## Context

Issue #343 builds on the random-window benchmark surface from issues #337,
#338, and #339. The Rust random-window result already serializes
`search_stats` counters, and `benchmarks/qec_code_random_window/run_local.py`
already preserves the raw CLI JSON inside each JSONL row. The current counters
describe search volume, but they do not show whether elapsed time is dominated
by permutation generation, GF(2) kernel-basis generation, span filtering,
witness construction/validation, best-witness update work, or overhead outside
those buckets.

The implementation must keep `randomized-upper-bound` JSON stable, avoid
optimizing the algorithm, avoid hard performance thresholds, and keep old
JSONL fixtures without timing fields valid.

This run is non-interactive. The design is approved under the standing answer
policy because the issue gives the required schema shape, verification
commands, and out-of-scope constraints.

## Design Options

1. Extend `RandomWindowSearchStats` with nanosecond timing fields and update the
   existing random-window search loop to accumulate coarse stage timings.
   This keeps timings beside the counters they explain, reuses the established
   JSON surface, and leaves `randomized-upper-bound` unchanged.

2. Add a separate top-level timing object to `DistanceBoundResult`.
   This would separate counters from timings, but it would create a second
   random-window diagnostic location and risk accidental exposure on other
   distance-bound methods.

3. Measure timing only in the Python benchmark runner.
   This would preserve Rust code, but it cannot split elapsed time across
   kernel generation, span filtering, witness validation, and best-update work.

Chosen approach: option 1. It is the narrowest contract change and matches the
issue recommendation.

## Result Contract

Add these `u64` nanosecond fields to `RandomWindowSearchStats`:

- `permutation_time_ns`
- `kernel_basis_time_ns`
- `span_filter_time_ns`
- `witness_validation_time_ns`
- `best_update_time_ns`
- `total_search_time_ns`

`u64` is used for stable JSON/Python integer handling. Durations are converted
from `Duration::as_nanos()` with saturation at `u64::MAX`, which is sufficient
for benchmark-scale runs and prevents overflow from becoming a panic.

`total_search_time_ns` measures the random-window search body from just before
the restart/iteration loops until a completed result is constructed. It
therefore includes the named measured stages and loop overhead, but not CLI
printing or Python summary/reporting time. Completed non-empty random-window
runs must report a positive total. Error paths do not produce a timing-bearing
result.

`randomized-upper-bound` results continue to serialize without `search_stats`,
so they do not gain timing fields.

## Timing Semantics

The coarse stage buckets are:

- `permutation_time_ns`: time spent creating shuffled column permutations.
- `kernel_basis_time_ns`: time spent in
  `gf2::try_random_window_kernel_basis_with_width`.
- `span_filter_time_ns`: time spent in zero-candidate and stabilizer component
  span rejection checks.
- `witness_validation_time_ns`: time spent converting component candidates to
  Paulis and validating witnesses against the code.
- `best_update_time_ns`: time spent comparing valid witnesses against the
  current best witness and storing replacements.
- `total_search_time_ns`: enclosing search-loop time.

The instrumentation uses `std::time::Instant` around these existing coarse
steps only. It does not introduce external profilers or alter random-window
sampling semantics.

## Benchmark Summary

`run_local.py` needs no schema change because it already preserves raw CLI
JSON. `summarize.py` should treat timing fields as optional inside
`raw_cli_json.search_stats`: rows with old counter-only stats remain valid.

If any timing field is present, the summarizer validates that every timing
field listed in this spec is present, is an integer, and is non-negative. It
also rejects rows where `total_search_time_ns` is smaller than the sum of the
five named stage timings, naming the offending `search_stats` timing field in
the error.

Per-case summaries add timing totals to `summary.csv`, using fields named
`search_timing_total_<field>`, plus `search_timing_rows`. `summary.md` keeps
the existing `search_stats` column and appends a compact timing note for cases
with timing rows, formatted in milliseconds for human scanning.

## Testing

Rust adds `random_window_upper_bound_reports_search_timing`. It runs a small
completed random-window case, asserts all timing fields serialize under
`search_stats` as non-negative integers, asserts `total_search_time_ns` is
positive, and asserts `randomized-upper-bound` serialization still lacks
`search_stats`.

Python adds
`benchmarks.qec_code_random_window.tests.test_summarize_search_timing`. The
positive test writes fake JSONL rows with timing fields and asserts CSV totals
and Markdown timing notes. The negative-control test writes rows with either a
negative timing field or `total_search_time_ns` smaller than the sum of named
stage timings and asserts the summarizer rejects them with an error naming the
offending `search_stats` field.

The issue verification commands remain the acceptance gate:

- `cargo test -p qec-code random_window_upper_bound_reports_search_timing -q`
- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_timing -q`
- `make qec-code-random-window-bench-no-target-ladder-smoke`
- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_timing.SearchTimingSummaryTest.test_rejects_negative_or_inconsistent_timing -q`

## Out Of Scope

Do not optimize the random-window algorithm. Do not add hard runtime
thresholds. Do not require external profilers or reference implementations. Do
not change `upper_bound`, `bound_type`, random-window sampling semantics, or
the `randomized-upper-bound` JSON contract.
