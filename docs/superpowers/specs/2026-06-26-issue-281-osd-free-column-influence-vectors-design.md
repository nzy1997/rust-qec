# Issue 281 OSD free-column influence vectors design

Issue: #281 Precompute OSD free-column influence vectors for candidate evaluation

Date: 2026-06-26

## Context

Issue #214 is already merged and changed OSD candidate evaluation so a reduced
GF(2) system is built once per OSD decode. Candidate solves now reuse that
reduced system instead of rerunning full elimination. Issues #277 and #278 are
also merged on this branch: `rbposd` has an explicit
`OsdVariant::LdpcCombinationSweep` planner and ranks `ldpc` candidates with
channel-prior objective weights.

The remaining hot-path work in #281 is within each reduced system. The current
candidate loop still calls `solve_with_forced_columns_counting` for every forced
free-column candidate, which scans the reduced rows and back-substitutes each
candidate separately. The new behavior should build candidate corrections by
XORing precomputed free-column effects.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming review
gates use the standing answer policy:

- No visual companion is needed because the work is decoder internals and test
  behavior, not visual design.
- The design is approved from the issue text, the merged #214 reduced-system
  reuse, and the merged #277/#278 `ldpc` planner/scoring work.
- Keep the representation crate-private in `rbposd/src/gf2.rs`. This is the
  safest compatibility choice because no public API change is required.
- Use a sparse toggle-list representation for each influence vector. It is still
  an influence-vector abstraction, but avoids allocating a dense correction
  vector per free column and keeps candidate assembly simple.
- Update counters to reflect the new path: candidate count still records the
  number of evaluated candidate sets, while `gf2_solve_count` no longer scales
  with that count after the base solution has been found.

## Approaches Considered

1. Add `FreeColumnInfluenceVectors` beside `ReducedLinearSystem` and assemble
   candidate corrections by XORing sparse precomputed toggle lists.
   This is recommended because it keeps validation next to the reduced-system
   invariants, avoids public API churn, and removes per-candidate row scans.
2. Store one dense `Correction` per free-column influence and XOR the full bit
   vector for each forced column.
   This is straightforward but allocates `num_bits * free_columns` booleans and
   does more work per candidate than needed.
3. Only optimize `ldpc_osd_cs` singles and leave legacy candidate evaluation on
   `solve_with_forced_columns_counting`.
   This would satisfy the smallest visible hot path but duplicates candidate
   construction paths and leaves existing legacy OSD counters misleading.

## Design

`rbposd/src/gf2.rs` will define a crate-private
`FreeColumnInfluenceVectors` type. It owns:

- the base OSD-0 correction for the reduced system,
- the ordered free-column set covered by the representation,
- a column-to-influence index map used for validation,
- one sparse toggle list per free column.

For a reduced row system in row-reduced form, forcing a free column toggles that
free column and every pivot column whose pivot row contains a `1` in the forced
free column. Candidate assembly copies the base correction, validates each
forced column against the precomputed ordered set, skips duplicate forced
columns so it preserves set semantics, and XORs each toggle list into the copy.

The builder will require a base solution from the same reduced system and an
ordered free-column slice. It will reject out-of-range columns, pivot columns,
duplicates in the ordered free set, and base solutions with non-zero free bits.
The candidate assembly method will reject forced pivot columns, out-of-range
columns, and free columns outside the precomputed ordered set.

`rbposd/src/osd.rs` will build influence vectors after the existing base solve:

- legacy OSD builds them for the legacy candidate frontier,
- `ldpc_osd_cs` builds them for all free columns because singles cover all free
  columns and pairs cover the first `osd_order` of that same ordered list,
- profile traversal uses the same assembly path so the counters represent the
  optimized candidate work.

Candidate scoring and tie-breaking remain unchanged. BP behavior, candidate
semantics, and planner shapes remain unchanged.

## Testing

Add GF(2) unit coverage with the exact issue-required names:

- `osd_candidate_influence_vectors_match_back_substitution` builds a reduced
  system with three free columns, checks all candidate sets up to order two,
  asserts influence-vector assembly equals
  `solve_with_forced_columns_counting`, asserts each correction satisfies the
  syndrome, and prints the candidate-set count.
- `osd_influence_vectors_reject_invalid_forced_columns` checks pivot columns,
  out-of-range columns, and columns outside the ordered free set.

Update existing OSD integration counter assertions so candidate count remains
positive but `gf2_solve_count` no longer grows with the number of candidates.

Regression checks:

```bash
cargo test -p rbposd osd_candidate_influence_vectors_match_back_substitution -- --nocapture
cargo test -p rbposd osd_influence_vectors_reject_invalid_forced_columns -q
cargo test -p rbposd
cargo test
```
