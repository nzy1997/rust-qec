# Issue 338 Random-Window Search Stats Design

## Context

Issue #338 asks for diagnostic counters that explain where
`random-window-upper-bound` spends search effort. The current Rust result JSON
contains the witness and options, and the benchmark runner already preserves
the raw CLI JSON under `raw_cli_json`. The summarizer currently validates and
aggregates top-level run fields only.

The sandbox blocked live GitHub issue lookup, so this spec uses the complete
Agent Desk issue body plus local history for #335. Commit `77ca4ea` added the
no-target benchmark surface by preserving raw CLI JSON and extending summary
handling for `target_weight = null`.

## Design Options

1. Add a random-window-only `search_stats` object to
   `DistanceBoundResult<RandomWindowUpperBoundOptions>` and thread a mutable
   stats struct through the existing search loop.
   This keeps the existing randomized upper-bound result contract stable and
   puts the counters next to the exact code paths they describe.

2. Add an optional generic stats field to `DistanceBoundResult<Options>`.
   This would avoid a dedicated result shape, but it would alter every distance
   bound result type and risk ambiguous empty stats on methods that do not
   collect them.

3. Derive counters only in the benchmark runner from command options and raw
   CLI output. This cannot observe kernel generation, rejection, validation, or
   best-update events, so it does not meet the diagnostic objective.

Chosen approach: option 1. It is the narrowest contract change that can measure
the required events.

## Result Contract

Add `RandomWindowSearchStats` in `qec-code/src/distance_bound.rs`:

- `permutations_sampled: usize`
- `kernel_basis_generations: usize`
- `component_candidates_generated: usize`
- `zero_candidates_rejected: usize`
- `stabilizer_span_candidates_rejected: usize`
- `witness_validation_candidates_rejected: usize`
- `valid_witnesses_found: usize`
- `best_witness_updates: usize`
- `target_reached: bool`

Add a `search_stats` field to
`DistanceBoundResult<RandomWindowUpperBoundOptions>` only. Completed
random-window results serialize this object in CLI JSON. Existing
`DistanceBoundResult<RandomizedUpperBoundOptions>` values remain unchanged.

## Counter Semantics

Increment `permutations_sampled` once per random permutation sampled by the
outer search loop. Increment `kernel_basis_generations` once per call to
`gf2::try_random_window_kernel_basis_with_width`; a full iteration attempts
both X-like and Z-like components unless the X-like side reaches the target and
returns early.

Increment `component_candidates_generated` for every component vector returned
by the kernel-basis call. Classify each candidate through the existing checks:
zero candidate, stabilizer component span, full witness validation, valid
witness, and best-witness update. Set `target_reached` to true only for the
early-return path caused by `target_weight`; full-budget no-target runs leave it
false.

The stats are diagnostic. Validation should ensure serialized counters remain
structurally sane but must not introduce performance thresholds.

## Benchmark Summary

`benchmarks/qec_code_random_window/run_local.py` already writes raw CLI JSON.
No runner schema change is needed beyond preserving that object.

`summarize.py` should validate optional `raw_cli_json.search_stats` objects on
successful rows. It should reject negative or non-integer counters, require
`target_reached` to be a boolean, and reject inconsistent relationships:

- `component_candidates_generated >= valid_witnesses_found`
- `component_candidates_generated >= best_witness_updates`
- `valid_witnesses_found >= best_witness_updates`

When search stats are present, per-case summaries should include simple sums for
integer counters and counts of `target_reached` true rows. The CSV should expose
the aggregate fields, and the Markdown summary should include a compact search
stats column.

## Testing

Rust tests should add a pinned random-window case named
`random_window_upper_bound_reports_search_stats`. It should assert JSON
serialization includes `search_stats`, integer counters are non-negative,
`permutations_sampled` is positive, candidate totals dominate valid witnesses
and best updates, `target_reached` is true when `target_weight` ends the run,
and a no-target full-budget run reports `target_reached = false`.

Python tests should add
`benchmarks.qec_code_random_window.tests.test_summarize_search_stats` with fake
JSONL rows. Positive coverage should assert aggregate CSV and Markdown fields.
Negative coverage should assert that a negative counter or
`best_witness_updates > component_candidates_generated` fails with an error
naming the offending `search_stats` field.

## Out Of Scope

Do not optimize the random-window search algorithm. Do not add external
profiling dependencies. Do not add timing thresholds. Do not change the meaning
of `upper_bound` or `bound_type`.
