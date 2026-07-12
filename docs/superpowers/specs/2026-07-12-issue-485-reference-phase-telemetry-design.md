# Issue 485 Reference Phase Telemetry Design

Issue: #485 Add deterministic phase telemetry to packed reference construction
Date: 2026-07-12

## Context

Issue #458 routes supported noiseless reference construction through
`PackedInverseTableau`, and issue #459 added a long-lived
`rstim_reference_build_worker` plus checked reference-build evidence. The
canonical d11/r100 fixture now builds its reference sample through the packed
path, but the current measurement/reset implementation still materializes
canonical rows for every measurement/reset batch. Issue #485 needs deterministic
phase counters before the algorithm changes.

This Agent Desk run is non-interactive, so the Standing Answer Policy resolves
the Superpowers gates:

- Visual companion: not used because this is backend Rust/Python telemetry.
- Clarifying questions: answered from issue #485, the merged #484 dependency,
  and the existing #457, #458, and #459 packed-reference designs.
- Recommended design: instrument the existing packed-reference construction
  boundaries without changing the algorithm, expose the counters only when the
  reference worker request opts in, and add a standalone profile command outside
  the checked bundle schema.
- Design approval: accepted automatically because the issue gives exact
  counters, interface, expected profile line, negative control, and verification
  commands.
- Spec review: this document is approved for planning after checking for
  placeholders, contradictions, ambiguity, and unrelated scope.

## Alternatives Considered

1. Add counters directly to the checked M1 reference-build bundle. This is
   rejected because the issue says profiling output is separate from the M1
   checked bundle schema.
2. Count only at the Python profile layer by scanning Stim text. This is
   rejected because the negative control requires collapse/writeback differences
   that cannot be inferred safely from instruction counts.
3. Instrument the existing Rust packed-reference execution path and surface the
   data through an opt-in worker field. This is the chosen approach because it
   reports the actual work performed and keeps existing benchmark artifacts
   unchanged.

## Chosen Design

Add `ReferenceBuildPhaseCounters` in `rstim/src/data_path.rs` with public
integer fields:

- `measurement_reset_batches`
- `canonical_materializations`
- `canonical_writebacks`
- `direct_inverse_batches`
- `transposed_collapse_batches`
- `collapse_pivots`
- `expanded_repeat_iterations`
- `measurement_bits`

`ReferenceSampleResult` will carry these counters. Existing callers that only
read `bits` and `decision` keep working. `build_reference_sample` remains a
bits-only compatibility wrapper.

The packed-reference interpreter will update counters while it recursively
walks instructions:

- Each expanded measurement/reset instruction (`M`, `MX`, `MY`, `MR`, `MRX`,
  `MRY`, `R`, `RX`, `RY`, and Z aliases) increments
  `measurement_reset_batches` once per executed Stim operation.
- `REPEAT count { ... }` increments `expanded_repeat_iterations` by `count`
  before recursively executing the body `count` times. For the canonical
  d11/r100 fixture this is `99`.
- `measurement_bits` is set from `rstim::stats::num_measurements`.

`PackedInverseTableau` will increment the full-tableau counters at the existing
private phase boundaries:

- `canonical_materializations` increments whenever the current algorithm calls
  `canonical_rows()`.
- `collapse_pivots` increments when the Z measurement path finds an
  anti-commuting stabilizer pivot.
- `canonical_writebacks` increments whenever changed canonical rows are written
  back through `replace_from_canonical_rows`.
- `direct_inverse_batches` and `transposed_collapse_batches` remain zero in
  this issue; later direct-collapse issues will increment them.

## Worker Protocol

The existing `reference-build-v1` worker request gains an optional
`include_phase_counters: true` field on `build_reference`. The normal checked
benchmark runner does not send it, so existing raw bundle rows remain unchanged.

When requested, a `reference_built` response includes `phase_counters` with the
`ReferenceBuildPhaseCounters` field names. The worker still rejects legacy
fallback decisions for this reference-build benchmark path.

## Profile Command

Add:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.profile_reference_build \
  --fixture <path> --worker <path> --out <json>
```

The command starts the worker with `--protocol reference-build-v1`, sends one
`load`, sends one `build_reference` with `include_phase_counters: true`,
validates the counter payload, writes a standalone JSON profile to `--out`, and
prints:

```text
PASS reference phase profile batches=103 canonical=103 writebacks=2 repeats=99 bits=12121
```

for the canonical fixture. The JSON profile is intentionally separate from the
M1 checked evidence bundle schema and does not publish timing claims.

## Testing

Rust tests cover the actual telemetry source:

- The canonical d11/r100 fixture returns `PackedInverse`, 12,121 bits,
  `measurement_reset_batches=103`, `canonical_materializations=103`,
  `canonical_writebacks=2`, `expanded_repeat_iterations=99`,
  `measurement_bits=12121`, and zero direct/transposed counters.
- The negative control compares `X 0; M 0` with `H 0; M 0`: both have one
  measurement/reset batch and one canonical materialization, but only
  `H 0; M 0` has a collapse pivot and canonical writeback.
- The reference worker omits `phase_counters` by default and includes them only
  when `include_phase_counters` is true.

Python tests cover `profile_reference_build` with a fake worker, asserting the
opt-in request field, JSON output, exact PASS line, and rejection of missing or
malformed counter payloads.

## Verification

Required issue verification:

```sh
cargo build --release -p rstim --bin rstim_reference_build_worker
python3 -m benchmarks.rstim_vs_stim_simulator.profile_reference_build \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --worker target/release/rstim_reference_build_worker \
  --out /tmp/rstim-reference-phase-profile.json
```

Additional focused checks:

```sh
cargo test -p rstim --test packed_reference_routing -- --nocapture
cargo test -p rstim --test rstim_reference_build_worker
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_profile_reference_build -q
```

Final Agent Desk verification:

```sh
cargo test
```

## Out Of Scope

This design does not optimize tableau operations, fold repeats, alter the M1
checked evidence bundle schema, publish timing claims, add direct inverse
collapse, or route production reference sampling away from the current
canonical-row strategy.

## Self-Review

- No placeholders remain.
- The design preserves the checked bundle schema by making worker counters
  opt-in.
- The negative control depends on actual collapse/writeback behavior, not just
  instruction counts.
- Direct inverse and transposed counters are present but remain zero until the
  follow-up optimization issues implement those phases.
