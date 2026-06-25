# Issue #214 rbposd OSD Forced-Column Optimization Design

Date: 2026-06-25
Status: Non-interactive Agent Desk design, auto-approved by standing policy
Scope: GitHub issue #214, optimize `rbposd` OSD order-7 forced-column solving

## Context

Issue #214 builds on the merged #210 BB90 hard-syndrome fixture and #212
decode-profile counters. The current `rbposd` OSD path computes an OSD-0 base
solution, then enumerates forced free-column combinations. Each candidate calls
`PreparedLinearSystem::solve_with_column_order_detailed_counting`, which clones
dense scratch rows and repeats GF(2) elimination. For order 7 and a 16-column
frontier, that makes full elimination scale with candidate count.

The existing implementation already exposes the counters this issue needs:
`osd_candidate_count`, `gf2_solve_count`, and
`gf2_full_elimination_count`. It also preserves deterministic solution
selection through `is_better_solution`, which compares residual cost and then
correction bit order.

## Goals

- Compute the ordered GF(2) reduced system once per OSD decode target.
- Reuse that reduced system for OSD-0 and forced free-column candidates.
- Count candidate back-substitution as GF(2) solve work while counting only the
  base reduction as full elimination.
- Preserve existing corrections, logical predictions, and deterministic
  tie-breaking for checked fixtures.
- Add a small deterministic order-7 regression fixture in `rbposd` tests and
  keep the BB90 hard fixture as the integration smoke.

## Non-Goals

- Do not change BP scheduling, BP update rules, public BP-OSD semantics, or
  residual-cost comparison behavior.
- Do not add the Python `ldpc` comparison runner.
- Do not replace the whole matrix representation with a bitset in this PR.
- Do not claim broad benchmark readiness from this single optimization.

## Approaches Considered

### 1. Reuse one reduced linear system and back-substitute candidates

Add a reduced-system object in `rbposd/src/gf2.rs` that stores the pivot rows,
right-hand side, pivot columns, and ordered free columns from one elimination.
Candidate evaluation validates that forced columns are in the free set, assigns
those free variables, and back-substitutes pivot variables. This is the chosen
approach because it removes repeated full elimination without changing the OSD
search order or comparison semantics.

### 2. Bit-pack all dense rows first

Rewrite `PreparedLinearSystem` rows into a compact bitset before optimizing the
candidate loop. This could improve constant factors, but it is broader than the
issue's required gate. Repeated elimination is the observed algorithmic
bottleneck, so this PR keeps row storage stable.

### 3. Cache complete candidate corrections

Memoize candidate outputs by forced-column set. The OSD search never revisits
the same forced combination inside one decode, so this adds memory and
complexity without addressing the repeated elimination source.

## Design

`PreparedLinearSystem` gains a factorization path that performs the existing
ordered row reduction once and returns a `ReducedLinearSystem`. The legacy
`solve_with_column_order*` helpers remain available and are implemented through
this path so existing callers keep their behavior.

`ReducedLinearSystem::solve_with_forced_columns` validates each forced column:
the column must be in range, in the ordered free set, and not a pivot. It then
sets those free variables to true and computes pivot variables from the stored
reduced rows and RHS. The returned `DetailedSolution` carries the same
correction, pivot-column list, and free-column list as the current full solve
would have returned for that forced assignment.

`decode_osd_with_workspace` and the bounded profile helper factor the target
syndrome once. They evaluate the base solution and every forced candidate by
calling `solve_with_forced_columns` on the same reduced system. The OSD
candidate traversal, frontier limit, residual-cost comparison, and
lexicographic tie-break remain unchanged.

Counter semantics after the optimization are:

- `gf2_full_elimination_count`: one for a nontrivial OSD decode that reaches the
  reduced-system path, zero for BP-only or zero-syndrome fast paths.
- `gf2_solve_count`: one for the base OSD-0 back-substitution plus one for each
  forced candidate evaluated.
- `osd_candidate_count`: one per forced free-column combination visited, as
  before.

## Testing

The core regression test is
`osd_order7_reuses_factorization_without_changing_correction`. It uses a small
matrix with eight free columns and `osd_order = 7`, records the expected
correction, and asserts candidate count is positive while
`gf2_full_elimination_count <= 1`.

The negative-control test
`osd_forced_pivot_columns_are_rejected_after_optimization` exercises the reduced
solver directly with a pivot column, an out-of-range column, and a column that
is free in the full matrix but outside the provided ordered column set.

Existing profile tests are updated so bounded candidate profiling expects
candidate solve count to scale with candidates, but full elimination count to
remain one. The BB90 hard fixture smoke keeps its expected logical prediction
and candidate-limit behavior while asserting the new full-elimination counter
behavior.

## Spec Self-Review

Placeholder scan: passed. Scope check: this is one bounded algorithm change in
`rbposd` plus tests that consume existing #212 counters. Ambiguity check: solve
and full-elimination counter meanings are explicit above.
