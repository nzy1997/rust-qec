# Packed Inverse Z Collapse Design

## Context

Issue #486 asks for a direct packed-inverse Z measurement collapse subsystem. The existing
`PackedInverseTableau::measure_z_raw_biased` path materializes canonical rows, mutates them,
and may reconstruct the inverse tableau. The new subsystem should operate directly on the
packed inverse representation, preserve deterministic signs, bias random outcomes to zero, and
create at most one transposed working view for a batch that actually needs collapse.

Issue #485 is complete on `master`, so `ReferenceBuildPhaseCounters` already exposes
`direct_inverse_batches`, `transposed_collapse_batches`, and `collapse_pivots`.

## Chosen Approach

Add a direct `PackedInverseTableau::collapse_z_many_biased` entrypoint for internal/test use.
It will:

1. Validate and scan all targets in order.
2. Return deterministic Z signs directly from the inverse Z row sign bit when the row has no X
   support.
3. Collect only targets whose inverse Z row has X support.
4. Build one transposed working view only when at least one collected target requires collapse.
5. For each random target, find a pivot X bit in the transposed Z/X quadrant, eliminate later X
   bits with `append_ZCX`, rotate the pivot into a Z measurement row with `append_H_XZ` or
   `append_H_YZ`, and apply `append_X` if needed so the biased result is zero.
6. Copy the transposed working view back to the packed inverse tableau once for the batch.

The transposed view stores qubit columns as packed row-bit words for both X and Z planes, plus
the packed signs. This mirrors the Stim technique while keeping the existing row-major packed
layout as the canonical storage.

## Boundaries

The existing production reference sampling path stays on the canonical-row implementation for
this issue. The new subsystem is callable directly for tests and future integration work, but
`measure_z_many_biased_with_counters`, reset paths, and X/Y adapters do not route through it yet.

This keeps the change scoped to the requested subsystem and avoids shifting existing phase
profile expectations in `packed_reference_routing`.

## Counters

`collapse_z_many_biased` increments:

- `direct_inverse_batches` once per call.
- `transposed_collapse_batches` only when a batch has at least one random Z collapse target.
- `collapse_pivots` once for each random pivot actually collapsed.

It does not increment canonical materialization or writeback counters.

## Tests

Add `rstim/tests/packed_inverse_direct_collapse.rs` with tests for:

- Deterministic batches returning signs without transposition.
- Random biased collapse matching the legacy tableau snapshot.
- Mixed deterministic/random batches reusing one transposed view.
- Deterministic one results after `X 0; M 0`.
- 64- and 128-qubit word-boundary crossings.

The focused verification command is:

```sh
cargo test -p rstim --test packed_inverse_direct_collapse -- --nocapture
```

The broader required verification command is:

```sh
cargo test
```

## Self-Review

The design is intentionally limited to Z collapse. It does not add production routing, X/Y
adapters, repeat folding, or timing claims. The chosen transposed view avoids canonical rows and
has explicit counter behavior, so tests can distinguish deterministic scans from random collapse
work and catch per-target transposition regressions.
