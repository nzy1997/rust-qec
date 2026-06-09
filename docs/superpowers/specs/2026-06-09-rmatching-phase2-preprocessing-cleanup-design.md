# rmatching Phase 2 Preprocessing Cleanup Design

Date: 2026-06-09
Status: Proposed
Scope: `rmatching` negative-weight detection-event preprocessing in `rmatching/src/driver/decoding.rs`

## Summary

This design defines the second performance step that follows the merged
bit-packed fast path work. The scope is intentionally narrow: replace the
temporary `HashSet`-based negative-weight detection-event toggle path in
`rmatching/src/driver/decoding.rs` with an index-buffer-oriented symmetric
difference pass.

The current implementation constructs a temporary `HashSet<usize>` from the
fired detection-event indices, toggles every negative-weight detector into that
set, filters boundary nodes, then sorts the result back into ascending order.
That preserves semantics but adds allocation, hashing, and extra data movement
to every decode that encounters negative-weight detection events.

The chosen direction is:

- keep the current `Matching` public API and decode entry points unchanged
- keep phase 1 bit-packed decode behavior unchanged
- replace the temporary `HashSet` toggle path with a sorted linear merge over
  detector-index buffers
- keep output semantics identical: symmetric difference with the configured
  negative-weight detector set, boundary filtering, and ascending detector
  order

## Goals

- Remove the per-decode temporary `HashSet` allocation from negative-weight
  preprocessing.
- Preserve current decode semantics for byte-oriented and packed-oriented paths.
- Keep the implementation local to `rmatching/src/driver/decoding.rs`.
- Make the benchmark effect easy to attribute by avoiding unrelated refactors.

## Non-Goals

- Do not redesign the public `Matching` API.
- Do not change the MWPM, flooder, shatter, or extraction logic.
- Do not unify all preprocessing helpers across byte and packed inputs in this
  phase.
- Do not change benchmark schemas, benchmark drivers, or `rsinter` decoder
  integration.
- Do not introduce persistent new scratch state unless it is required to finish
  the narrow optimization safely.

## Current State

`apply_negative_weight_events_into(...)` currently has two modes:

1. if no negative-weight detectors exist, it filters boundary nodes from the
   fired detector indices into the output buffer
2. otherwise, it:
   - allocates a temporary `HashSet<usize>` from `detection_events`
   - toggles every detector in `negative_weight_detection_events_set`
   - filters boundary nodes
   - sorts the remaining indices before writing them to the output buffer

The surrounding decode helpers already produce fired detector indices in
ascending order:

- `syndrome_to_detection_events_into(...)` scans the syndrome left-to-right
- `packed_dets_to_detection_events_into(...)` enumerates set bits in ascending
  detector order

That means the current `HashSet` path discards useful ordering information and
then pays to reconstruct it.

## Alternatives Considered

### 1. Sorted symmetric-difference merge

Precompute an ascending view of the negative-weight detector indices and merge
it with the already-sorted `detection_events` buffer using a standard symmetric
difference walk.

Benefits:

- no per-decode `HashSet` allocation
- preserves output order naturally
- small, local change
- easy to test against the current semantics

Costs:

- needs an ordered view of the negative-weight detector indices
- adds a little branching logic to the helper

This is the recommended option.

### 2. Toggle bitmap or mark array

Maintain a reusable detector-sized marker buffer, set fired detectors, toggle
negative-weight detectors, then collect active indices linearly.

Benefits:

- simple semantics
- no hashing

Costs:

- needs extra persistent scratch storage sized to detector count
- may do more work than necessary for sparse syndromes
- broadens this phase beyond the agreed narrow scope

This is not the recommended option for phase 2.

### 3. Full preprocessing-layer convergence

Refactor byte-oriented and packed-oriented preprocessing into one shared
pipeline in the same change.

Benefits:

- cleaner long-term structure

Costs:

- larger review surface
- harder benchmark attribution
- exceeds the agreed narrow scope

This is explicitly deferred.

## Decision

Phase 2 should replace the `HashSet` toggle path with a sorted symmetric
difference helper while leaving the broader preprocessing structure intact.

The implementation should treat the existing `detection_events` buffer as an
ascending slice and combine it with an ascending view of the configured
negative-weight detector set. The resulting indices should be written directly
into the existing `effective_events_buf`.

## Architecture

### Component boundaries

Only the negative-weight preprocessing helper in
`rmatching/src/driver/decoding.rs` changes behavior internally.

The following stay unchanged:

- `Matching::decode`
- `Matching::decode_into`
- `Matching::decode_batch`
- `Matching::decode_batch_into`
- `Matching::decode_bit_packed_into`
- output buffer ownership and reuse patterns
- MWPM execution after preprocessing

### Data dependencies

The new path depends on two ordered detector-index sequences:

- `detection_events: &[usize]`, already produced in ascending order
- `negative_weight_detection_events_sorted: &[usize]`, stored on
  `MatchingGraph` in ascending order

Boundary filtering remains part of the same helper so that the output buffer
contains the exact detector list consumed by the MWPM layer.

## Algorithm

For the negative-weight case, the helper should:

1. clear the caller-provided output buffer
2. walk `detection_events` and the ascending negative-weight detector sequence
   with two indices
3. emit the smaller detector when it appears in only one input sequence
4. skip a detector when it appears in both input sequences, because symmetric
   difference removes duplicates
5. apply boundary filtering before pushing a detector into the output
6. finish any remaining tail from either input sequence

This preserves the existing semantics:

- detectors fired in the syndrome are active unless canceled by a
  negative-weight toggle
- detectors absent from the syndrome become active if they appear in the
  negative-weight set
- boundary detectors are removed from the effective detector list
- final detector order is ascending

## Representation Choice

This phase should avoid new persistent scratch fields on `Matching`.

The ordered negative-weight detector sequence should be stored directly on
`MatchingGraph` as `negative_weight_detection_events_sorted: Vec<usize>`.

Construction rule:

1. continue using `negative_weight_detection_events_set` while building the
   graph, because edge insertion currently expresses negative-weight toggles as
   set XOR operations
2. after `UserGraph::to_matching_graph(...)` has finished adding edges and
   boundary edges, materialize `negative_weight_detection_events_sorted` once
   from the set and sort it ascending
3. decode-time preprocessing reads the sorted slice directly and does not
   rebuild or resort it per decode

This keeps graph construction simple and moves all ordered-view cost out of the
hot decode path.

## Compatibility

This change must preserve:

- decode outputs for all existing tests
- byte-oriented and bit-packed path agreement on the same syndrome
- current output ordering assumptions used by the MWPM preprocessing path

No public signatures change.

## Testing

Add focused tests around the new helper behavior:

- symmetric difference against a non-empty negative-weight detector set
- no-negative-weight fast path unchanged
- boundary nodes still filtered when introduced by the negative-weight set
- outputs remain ascending without a trailing sort
- byte and packed decode helpers still agree with the public decode path on a
  graph with negative-weight detectors

Existing `cargo test -p rmatching` remains the required regression suite.

## Benchmark Plan

After implementation:

1. rerun `cargo test -p rmatching`
2. rerun the `rmatching` slice of `surface_decoder_compare` on the `full` tier
3. compare the updated `rmatching` timings against the post-phase-1 merged
   baseline

Benchmark interpretation should focus on whether negative-weight preprocessing
cost drops measurably without any correctness regression.

## Risks

- If the negative-weight detector sequence is not provided in stable ascending
  order, the merge logic could silently produce unsorted outputs.
- If symmetric difference logic handles equal indices incorrectly, detector
  toggles could be inverted.
- If boundary filtering is moved to the wrong stage, outputs could differ from
  current semantics in subtle negative-weight cases.

These risks are addressed by keeping the change local, asserting ordering where
appropriate, and covering edge cases with focused tests.

## Rollout

1. implement the sorted symmetric-difference helper
2. add focused unit coverage in `rmatching/src/driver/decoding.rs`
3. run `cargo test -p rmatching`
4. rerun the `rmatching` benchmark slice and refresh artifacts if results are
   worth keeping
5. review whether a broader preprocessing unification is still justified after
   this narrower optimization lands
