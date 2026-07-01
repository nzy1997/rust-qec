# Issue 345 Random-Window Weight Pruning Design

## Context

Issue #345 asks `random-window-upper-bound` to skip component candidates that
cannot improve the current best witness in release/no-target fixed-budget runs.
Issues #337 and #338 are already merged locally: the no-target ladder smoke
target exists, and random-window results expose `search_stats` counters in CLI
JSON. Issue #343 is also merged, so `RandomWindowSearchStats` already includes
per-stage timing fields.

Live GitHub issue/comment lookup was blocked by the sandbox proxy, so this spec
uses the complete Agent Desk issue body plus merged local history for issues
#337, #338, and #343.

## Design Options

1. Add a current-best weight guard inside the existing component-candidate loop
   after zero rejection and before stabilizer span membership. This follows the
   issue placement, avoids Pauli construction for pruned rows, and leaves random
   permutation and kernel-basis generation unchanged.

2. Add pruning during kernel-basis generation. This would avoid returning some
   rows to the search loop, but it would couple distance-bound semantics into
   the generic GF(2) helper and would make the diagnostic counters harder to
   preserve.

3. Prune only after witness construction. This is correct but misses the stated
   optimization target because span filtering and Pauli construction still run
   for candidates that cannot improve the bound.

Chosen approach: option 1. It is the narrowest change, matches the requested
guard placement, and preserves deterministic sampling for a fixed seed.

## Result Contract

Extend `RandomWindowSearchStats` with:

- `weight_pruned_candidates: usize`

The counter is serialized under `search_stats.weight_pruned_candidates`. It
counts nonzero component candidates whose Hamming weight is greater than or
equal to the current best witness weight at the time they are considered.

The existing upper-bound contract is unchanged. The search still returns the
smallest valid witness found under the sampled budget. Equal-weight candidates
are pruned because the result promises an upper-bound value and one valid
witness, not exhaustive equal-weight witness enumeration.

## Candidate Flow

For each component candidate row:

1. Count the candidate Hamming weight.
2. Reject zero candidates exactly as before.
3. If a current best witness exists and `candidate_weight >= best.weight()`,
   increment `weight_pruned_candidates` and skip the stabilizer-span check,
   Pauli construction, and validation.
4. Otherwise run the existing stabilizer component span check.
5. Convert the survivor into an X-like or Z-like Pauli and validate it against
   the full stabilizer code.
6. Update the best witness only when the candidate is strictly lighter than the
   current best.

The guard uses the component row Hamming weight because these candidates become
pure X-like or pure Z-like witnesses, so the row weight equals the Pauli weight.

## Benchmark Summary

Update the benchmark summarizer's search-stat field list so
`weight_pruned_candidates` is validated, summed into `summary.csv`, and visible
in the Markdown `search_stats` note. Existing fake search-stat fixtures in the
Python tests should include the new field.

## Tests

Add Rust unit tests named exactly:

- `random_window_prunes_candidates_that_cannot_improve_best`
- `random_window_pruning_does_not_skip_strictly_better_candidate`

The positive test uses a hand-built current best and candidate sequence to
prove equal and heavier rows are counted as pruned before span/validation work,
while a strictly lighter candidate still reaches validation and updates the
best witness. The negative control starts from a weight-5 witness followed by a
valid weight-3 candidate and asserts the best witness becomes weight 3.

Update existing Rust serialization coverage so `search_stats` exposes the new
field. Update Python summarizer tests so the aggregate CSV and Markdown output
cover `weight_pruned_candidates`.

Run the issue verification commands:

- `cargo test -p qec-code random_window_prunes_candidates_that_cannot_improve_best -q`
- `cargo test -p qec-code random_window_pruning_does_not_skip_strictly_better_candidate -q`
- `cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture`
- `make qec-code-random-window-bench-no-target-ladder-smoke`
- `cargo test`

Use offline Cargo mode where the network-restricted sandbox blocks registry
access, but also record the requested command outcomes.

## Risks And Limits

This issue does not add target-weight early stopping to no-target benchmarks,
change random seed semantics, change the issue-225 ladder fixture, or claim
exact distance certification. Timing fields may change in magnitude because the
guard deliberately moves some rows out of span filtering and witness validation.
