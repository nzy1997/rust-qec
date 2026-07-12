# Issue 487 Direct Pauli Reference Routing Design

Issue: #487
Date: 2026-07-12

## Context

Issue #486 added `PackedInverseTableau::collapse_z_many_biased`, a direct
packed-inverse Z-collapse primitive that avoids `canonical_rows` and
`replace_from_canonical_rows`. Production packed reference sampling still routes
measurement and reset operations through the older canonical materialization
helpers, so the surface-code reference-build profile reports canonical work.

Issue #487 routes supported packed reference operations through the direct
collapse primitive:

- measurement: `M`, `MX`, `MY`
- measure-reset: `MR`, `MRX`, `MRY`
- reset-only: `R`, `RX`, `RY`

The required surface fixture profile is:

```text
PASS reference phase profile batches=103 canonical=0 writebacks=0 transposed=2 pivots=120 repeats=99 bits=12121
```

## Automatic Scope Decisions

This Agent Desk run is non-interactive, so the Standing Answer Policy resolves
the Superpowers gates:

- Visual companion: not used because this is backend Rust routing and counter
  behavior.
- Clarifying questions: answered from issue #487, the merged #486 direct
  collapse implementation, and existing packed reference tests.
- Recommended design: route production packed reference measurement/reset
  methods through direct Z collapse, add batch X/Y basis wrappers, and keep the
  canonical helpers internal.
- Design approval: accepted automatically because the issue gives exact
  operations, duplicate semantics, negative controls, and verification commands.
- Spec review: this document is approved for planning after placeholder,
  consistency, and scope checks pass.

## Alternatives Considered

1. Route only `M`, `MR`, and `R` to direct Z collapse and leave X/Y operations
   single-target canonical. This is rejected because the issue requires X/Y
   operations to apply batch basis transforms, collapse once, and undo them.
2. Replace the canonical helpers entirely. This is rejected because the issue
   allows the old strategy to remain as an internal future baseline.
3. Route production reference sampling through direct collapse while preserving
   canonical helpers for non-production public methods. This is the chosen
   approach because it satisfies the production no-canonical requirement while
   keeping legacy code available for later benchmarks.

## Chosen Design

`PackedInverseTableau` keeps the old canonical row helpers private. Production
counter-bearing paths stop calling them.

Z-basis batch operations call `collapse_z_many_biased` directly:

- `measure_z_many_biased_with_counters` returns direct measurement bits.
- `measure_reset_z_many_biased_with_counters` collapses once for unique targets,
  applies `X` corrections from raw results, and reports inversion-flipped bits.
- `reset_z_many_biased_with_counters` collapses once for unique targets and
  applies corrections without appending measurement bits.

X/Y batch operations are implemented as basis wrappers around the same direct
Z batch:

- X basis: apply `H` to each unique target qubit, run the Z batch, undo `H`.
- Y basis: apply `S_DAG` then `H` to each unique target qubit, run the Z batch,
  undo with `H` then `S`.

Measurement-only duplicates can use a single batch because repeated
measurements are non-resetting and direct collapse biases random results to
zero. Duplicate measure-reset and reset-only targets remain sequential because
the reset after an earlier target changes the state observed by a later target.

`rstim/src/data_path.rs` routes `MX`, `MY`, `MRX`, `MRY`, `RX`, and `RY` as
operation-wide batches instead of per-target loops, so unique targets share one
direct-collapse call and one transposed view when collapse is needed.

`benchmarks/rstim_vs_stim_simulator/profile_reference_build.py` includes
`transposed` and `pivots` in its pass line so the issue's profile command can
prove direct routing avoided canonical materialization while preserving the
expected pivot work.

## Tests

Add or update tests to cover:

- production `M`, `MX`, and `MY` report direct counters and zero canonical
  counters;
- unique `MR`, `MRX`, `MRY`, `R`, `RX`, and `RY` share direct batches and
  preserve deterministic reset behavior;
- duplicate `MR 0 0` stays sequential and returns `[true, false]` after
  `X 0`;
- the surface fixture profile counters become canonical-free:
  `batches=103`, `canonical=0`, `writebacks=0`, `transposed=2`,
  `pivots=120`, `repeats=99`, `bits=12121`;
- profile script output includes `transposed=...` and `pivots=...`.

Focused verification:

```sh
cargo test -p rstim \
  --test packed_inverse_direct_collapse \
  --test packed_inverse_tableau_measurement \
  --test packed_reference_routing
```

Final verification also runs the issue's release worker profile and
distribution commands, plus `cargo test`.

## Out Of Scope

This design does not implement repeat-cycle detection, publish checked
performance evidence, broaden gate support, or remove the legacy canonical
fallback code.

## Self-Review

The spec has no placeholders or contradictory requirements. It is scoped to one
backend routing change plus the profile output needed to verify it. Public API
compatibility is preserved because the existing packed tableau methods remain
available; only production counter-bearing routing changes behavior and
counters.
