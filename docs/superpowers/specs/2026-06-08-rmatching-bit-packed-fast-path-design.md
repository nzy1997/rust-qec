# rmatching Bit-Packed Fast Path Design

Date: 2026-06-08
Status: Proposed
Scope: `rmatching` decode-path performance work for `rsinter` integration and benchmark-facing bit-packed execution

## Summary

This design targets the largest immediately actionable source of overhead in
the current `rmatching` benchmark path: the conversion layer between the
bit-packed detector/observable buffers used by `rsinter` and the byte-per-bit
`Vec<u8>` interface used by `rmatching::Matching`.

The current checked-in surface decoder comparison results show `rmatching`
substantially slower than `pymatching` on the shared surface-code workloads.
For the tracked `full` tier, `rmatching` decode time per shot ranges from
roughly `4.0 us` to `93.6 us`, while `pymatching` ranges from roughly
`0.054 us` to `1.65 us`.

The chosen direction is:

- add a dedicated bit-packed decode fast path inside `rmatching`
- route `rsinter::decode::CompiledDecoder::decode_shots_bit_packed(...)`
  through that fast path
- avoid per-shot unpacking into `Vec<u8>` syndromes and repacking from
  `Vec<u8>` predictions
- reuse scratch buffers across shots
- keep the public `Matching` byte-oriented API stable in the first phase
- reserve a second phase for deeper index-based pre-processing cleanup inside
  `rmatching::driver::decoding`

## Goals

- Reduce `rmatching` decode time in
  `benchmarks/surface_decoder_compare` without changing benchmark rules or
  measurement boundaries.
- Remove avoidable per-shot allocation and per-bit unpack/pack work from the
  `rsinter` integration path.
- Preserve decode correctness and current external behavior for
  `Matching::decode`, `Matching::decode_into`, `Matching::decode_batch`, and
  `Matching::decode_batch_into`.
- Keep the first implementation phase narrow enough that performance gains and
  regressions are easy to attribute.
- Prepare a clean entry point for a second phase that optimizes detection-event
  extraction and negative-weight pre-processing.

## Non-Goals

- Do not rewrite the MWPM, blossom, flooder, or shatter core in this work.
- Do not change the `surface_decoder_compare` bridge protocol or CSV schema.
- Do not redesign the public `Matching` API around bit-packed buffers in the
  first phase.
- Do not fold the second-phase pre-processing rewrite into the first-phase
  delivery.
- Do not trade correctness for speed.

## Current State

The current `rsinter` integration path for `rmatching` is expensive before the
actual MWPM work begins.

### Current integration hot path

`rmatching/src/decoder.rs` currently implements
`CompiledDecoder::decode_shots_bit_packed(...)` by:

- allocating a new `Vec<u8>` syndrome for every shot
- unpacking every detector bit into that syndrome
- calling `Matching::decode_into(&syndrome, ...)`
- allocating a new packed output buffer for every shot
- repacking one-byte-per-observable predictions into output bytes

This means the benchmark-facing decode path pays substantial work in:

- per-shot allocation
- per-detector bit unpacking
- per-observable bit repacking
- extra temporary buffer churn under a mutex-protected decoder state

### Current `Matching` pre-processing costs

Inside `rmatching/src/driver/decoding.rs`, the general decode path currently:

- scans the full byte-per-detector syndrome to collect fired detector indices
- applies negative-weight detector toggles through a temporary `HashSet` when
  negative-weight events are present
- converts the final observable mask into one-byte-per-observable predictions

These are reasonable for a simple public API, but they are not well aligned
with the bit-packed batch data already available at the `rsinter` boundary.

### Current correctness baseline

The repository already has useful protection around `rmatching` behavior:

- `rmatching` unit tests for decode helpers and buffer reuse
- `surface_decoder_compare` Rust bridge tests
- benchmark-level correctness comparison against other decoders

That makes a focused, correctness-preserving performance refactor feasible.

## Alternatives Considered

### 1. Keep the current API path and only trim obvious allocations

This option would keep the current unpack-to-`Vec<u8>` and repack-from-`Vec<u8>`
flow, but try to reuse some temporary buffers in `rmatching/src/decoder.rs`.

Benefits:

- smallest code change
- low surface-area risk

Costs:

- still pays full per-detector unpack and per-observable repack costs
- leaves the main format-conversion bottleneck intact
- unlikely to close a large benchmark gap

This is not the recommended option.

### 2. Add a dedicated bit-packed fast path while keeping the public API stable

This option adds internal bit-packed decode helpers and routes the `rsinter`
integration through them, while keeping the existing public `Matching` methods
unchanged.

Benefits:

- directly targets the hottest avoidable work on the benchmark path
- keeps the change narrowly scoped
- preserves current public API behavior
- makes benchmark gains easy to validate

Costs:

- introduces a second internal decode entry point that must stay consistent
  with the public byte-oriented path
- requires careful testing around bit ordering and scratch reuse

This is the recommended first phase.

### 3. Rewrite the whole decode pre-processing layer in one pass

This option would combine the bit-packed fast path with a deeper refactor of
`driver::decoding`, including index-based event extraction and removal of the
temporary `HashSet` path for negative-weight events.

Benefits:

- highest potential upside in one branch
- avoids revisiting the same files twice

Costs:

- makes it harder to attribute performance gains
- increases regression risk
- mixes interface adaptation work with deeper decode helper changes

This is not the recommended first delivery. It remains the planned second
phase after the fast path lands and is measured.

## Decision Summary

The work should be split into two phases under one design:

1. **Phase 1: bit-packed fast path**
2. **Phase 2: index-based pre-processing cleanup**

Only Phase 1 is intended for the first implementation pass.

This sequencing keeps the first change focused on the most obvious source of
overhead and gives the benchmark a clean before/after measurement point.

## Recommended Architecture

### Phase 1: bit-packed fast path

Phase 1 introduces a narrow internal path that consumes bit-packed detector
data directly and emits bit-packed observables directly.

#### Component 1: reusable compiled decoder state

`CompiledMwpmDecoder` should continue to own one reusable `Matching` instance,
but the protected state should expand from only `Matching` to:

- `Matching`
- fired-detector scratch buffer
- effective-detector scratch buffer
- output scratch buffer as needed for packed writes

The protected state remains serialized behind the existing mutex model. This
work does not attempt to make one compiled decoder decode multiple shots in
parallel.

#### Component 2: internal bit-packed `Matching` entry point

`Matching` should gain an internal helper dedicated to the bit-packed path.

Its responsibilities are:

- read one shot from bit-packed detector bytes
- extract fired detector indices directly from set bits
- apply the existing negative-weight correction logic
- run the existing MWPM/shatter/extraction logic unchanged
- write the final observable result directly into a caller-provided packed
  output slice

This helper is internal in the first phase. The public `Matching` API remains
byte-oriented.

#### Component 3: low-level packed helper functions

`rmatching::driver::decoding` should grow small helper functions for:

- enumerating fired detector indices from a packed detector slice
- writing an `ObsMask` directly into packed observable bytes

These helpers isolate the data-format work from the MWPM core and keep the new
path testable without changing the algorithm implementation.

### Phase 1 data flow

The first-phase data flow becomes:

1. `rsinter` passes bit-packed detector bytes for a batch
2. `CompiledMwpmDecoder` reuses internal scratch buffers
3. a packed helper enumerates fired detector indices directly from set bits
4. existing negative-weight correction is applied to those indices
5. existing MWPM / flooder / shatter logic runs unchanged
6. the final `ObsMask` is written directly into packed observable bytes

The key architectural choice is that only the representation entering and
leaving the core changes; the core matching algorithm does not.

### Phase 2: index-based pre-processing cleanup

Phase 2 is intentionally deferred until after Phase 1 is implemented and
measured.

Its design direction is:

- converge the byte-oriented and packed-oriented helper layers around
  index-based event extraction
- reduce or remove temporary `HashSet` allocation in negative-weight event
  handling
- make detection-event extraction and effective-event production operate on
  reusable index buffers first, instead of on byte-per-detector intermediates

Phase 2 should not begin until the Phase 1 benchmark effect is measured.

## API and Compatibility

### Public API compatibility

The following public methods keep their current signatures and semantics:

- `Matching::decode`
- `Matching::decode_into`
- `Matching::decode_batch`
- `Matching::decode_batch_into`
- `Matching::decode_to_edges`

This design does not require any caller migration in the first phase.

### Integration compatibility

The following interfaces also remain unchanged:

- `rsinter::decode::CompiledDecoder` trait
- `surface_decoder_compare` bridge request/response payloads
- benchmark CSV schema and plotting inputs

This is intended to be a pure internal performance refactor from the caller's
point of view.

## Error Handling and Correctness Constraints

The new bit-packed path must preserve the current packing conventions exactly.

### Required correctness properties

- detector bits remain interpreted in the same LSB-first order as the current
  implementation
- observable bits remain written in the same LSB-first order as the current
  implementation
- scratch output slices are fully cleared before writing each shot
- directly enumerated fired detectors are exactly equivalent to the current
  `syndrome_to_detection_events_into(...)` behavior
- negative-weight correction semantics remain identical to the current path

### Failure model

This work is not intended to introduce new recoverable errors. The current
decode path assumes the caller passes buffers sized consistently with the
compiled DEM, and that contract remains unchanged.

## Testing Strategy

### Unit tests

Add focused tests in `rmatching/src/decoder.rs` for
`decode_shots_bit_packed(...)` behavior:

- one shot and multiple shots
- detector counts not divisible by 8
- observable counts not divisible by 8
- all-zero inputs
- cross-byte fired detector bits
- representative non-zero observable outputs

### Path-equivalence tests

Add tests that compare:

- the existing byte-oriented `Matching::decode_into(...)` path
- the new internal bit-packed fast path

These tests should use the same logical syndromes encoded both ways and assert
identical observable outputs.

### Existing integration tests

Keep the existing `surface_decoder_compare` bridge tests and `rmatching`
correctness tests in the verification set. The bridge protocol is unchanged, so
those tests should continue to pass without adaptation.

### Benchmark validation

After Phase 1 lands, validate the benchmark effect by rerunning the
`surface_decoder_compare` path for `rmatching` and comparing
`decode_us_per_shot` against the current checked-in baseline.

The success signal for Phase 1 is reduced decode time. Changes to compile time
are secondary.

## Success Criteria

This design is successful if all of the following are true:

- Phase 1 removes per-shot syndrome unpacking into `Vec<u8>` and per-shot
  prediction repacking from `Vec<u8>` on the `rsinter` decode path.
- public `Matching` API behavior remains unchanged.
- existing correctness tests continue to pass.
- new fast-path tests prove packed-path equivalence with the current decode
  semantics.
- `rmatching` benchmark decode time in `surface_decoder_compare` improves
  measurably enough that the remaining gap can be evaluated separately from
  format-conversion overhead.

## Implementation Sequencing

The implementation should proceed in this order:

1. add tests that pin down current bit-packed decode behavior
2. introduce reusable compiled decoder scratch state
3. add the internal packed decode helper and packed observable writer
4. route `decode_shots_bit_packed(...)` through the new path
5. rerun tests and benchmark checks
6. decide, based on measured results, whether to begin Phase 2

Phase 2 is a follow-on optimization step, not part of the first delivery.
