# Rsinter Memory-Z Parity Design

Date: 2026-06-16
Status: Proposed
Scope: Issue #65, explicit rsinter rotated-memory-Z benchmark support and parity evidence

## Summary

Issue #65 reports that a rotated surface-code memory-Z sweep run through
`rsinter`/`rstim` shows logical error rates increasing with distance across
the whole near-threshold sweep. The issue-shaped task is the Stim/sinter
example:

- `stim.Circuit.generated("surface_code:rotated_memory_z", ...)`
- `distance in {3, 5, 7}`
- `rounds = 3 * distance`
- `p in {0.008, 0.009, 0.010, 0.011, 0.012}`
- the same `p` applied to after-Clifford depolarization, after-reset flips,
  before-measure flips, and before-round data depolarization

Current `rsinter` surface benchmark input support only accepts the legacy
`surface_rotated_memory_x` input type, with that value also used as the
default when `input_type` is omitted. The first fix is to make
`surface_rotated_memory_z` an explicit benchmark input and preserve the
existing memory-X default. The second part is a small, reproducible parity
regression that can distinguish circuit/noise mismatch, DEM mismatch, and
decoder-output mismatch.

## Goals

- Let `rsinter` TOML specs explicitly request rotated surface-code memory-Z
  workloads.
- Preserve compatibility for existing specs that omit `input_type`; they still
  mean `surface_rotated_memory_x`.
- Ensure memory-Z uses the same four noise-channel placements as the Stim
  issue task.
- Add focused parity evidence so a future inverted-scaling result can be
  attributed to the correct layer.
- Keep regular tests small enough for local and CI iteration.

## Non-Goals

- Do not run the full 15-case, 100k-shot issue sweep in ordinary tests.
- Do not make `rsinter` a general surface-code task registry beyond the two
  rotated memory tasks needed here.
- Do not change the existing memory-X benchmark semantics.
- Do not rewrite the decoder abstraction or benchmark result schema unless the
  parity evidence exposes a decoder-specific defect during implementation.

## Architecture

Add `surface_rotated_memory_z` as a peer to `surface_rotated_memory_x` in the
`rsinter` benchmark input path.

`rsinter/src/bench/registry.rs` should accept:

```toml
[runner.params]
input_type = "surface_rotated_memory_z"
distance = [3]
rounds = [9]
p = [0.008, 0.009, 0.010, 0.011, 0.012]
max_shots = 100000
max_errors = 1000
batch_size = 256
```

This design does not add paired sweep semantics. Existing `rsinter` surface
params expand `distance`, `rounds`, and `p` as a Cartesian product. To recreate
the issue sweep exactly, use singleton `distance` and `rounds` arrays per
distance, for example `(3, 9)`, `(5, 15)`, and `(7, 21)`.

Specs that omit `input_type` continue to expand as
`surface_rotated_memory_x`. Surface input validation remains shared:

- `distance` must be nonempty and every entry must be `>= 2`
- `rounds` must be nonempty and every entry must be `>= 1`
- `p` must be nonempty and numeric
- `batch_size` must be positive
- shot, error, and wall-clock budgets keep their existing validation

`rsinter/src/bench/circuit_source.rs` should dispatch memory-X to
`rstim::codegen::surface_code::rotated_memory_x` and memory-Z to
`rstim::codegen::surface_code::rotated_memory_z`. Result params should record
the actual `input_type`, not a hard-coded memory-X value.

The existing sampling, DEM analysis, decoder compilation, decoding, logical
error counting, artifact writing, and plotting paths should be reused
unchanged unless a parity check demonstrates a separate defect.

## Data Flow

The memory-Z flow is:

1. TOML runner params set `input_type = "surface_rotated_memory_z"`.
2. `expand_runner_points_for_runner` creates `BenchCasePoint` values with
   `input_type` preserved.
3. `build_circuit_for_point` calls
   `rotated_memory_z(distance, rounds, p)`.
4. The runner derives a decomposed DEM with
   `ErrorAnalyzer::circuit_to_dem_decomposed`.
5. The selected decoder compiles once from that DEM.
6. `rstim::sampler::sample_batch` produces bit-packed detection events and
   observable flips.
7. The decoder prediction bytes are compared with observable-flip bytes, with
   one logical error counted for each shot where any observable byte differs.

Result rows and artifacts should include enough metadata to prove the workload
identity:

- `input_type`
- `distance`
- `rounds`
- `p`
- `max_shots`
- `max_errors`
- `batch_size`
- `num_dets`
- `num_obs`

## Noise Placement

The memory-Z path must use the same four-channel noise semantics as the
issue's Stim/sinter task. The public `rotated_memory_z(distance, rounds, p)`
wrapper already maps `p` through `NoiseParams::uniform(p)`, which means:

- `before_round_data_depolarization`: apply once at the start of each
  stabilizer round on data qubits, not again in the final data-measurement
  tail.
- `after_clifford_depolarization`: apply after Clifford operations, including
  X-ancilla `H` layers and CNOT layers.
- `after_reset_flip_probability`: apply after data initialization and after
  ancilla `MR` reset behavior.
- `before_measure_flip_probability`: apply before ancilla measurements and
  before the final data measurement.

The implementation should not introduce a benchmark-local noise shortcut. All
surface memory-Z benchmark generation should go through the normal `rstim`
surface-code generator so noise placement stays in one code path.

## Parity Evidence

Add a small, issue-shaped parity regression instead of relying on visual
distance-scaling judgment.

The parity checks should separate failures by layer:

- Circuit/noise placement mismatch: compare key generated-circuit properties
  against Python Stim for `surface_code:rotated_memory_z`.
- DEM mismatch: compare decomposed DEM semantics against Stim, or use the
  existing sample-statistics style check when factorization differs.
- Decoder-output mismatch: feed the same DEM and same deterministic sampled
  shots to the Rust decoder path. When PyMatching is available in the test
  environment, compare predictions against PyMatching on the same inputs; when
  it is absent, keep the Rust-side decoder smoke required and leave the
  PyMatching comparison as a skipped external-dependency check.

The regular suite only needs representative cases. A good minimum is
`d=3, rounds=9, p=0.008` plus isolated one-channel noise-placement tests.
Longer sweeps can remain manual or benchmark-only.

## Error Handling

Unknown input types should fail before benchmark artifacts are written, with a
direct message such as:

```text
unknown input_type: surface_rotated_memory_y
```

Malformed memory-Z surface params should reuse the same error messages as
memory-X for missing or invalid `distance`, `rounds`, `p`, `max_shots`,
`max_errors`, `max_wall_seconds`, and `batch_size`.

The parity harness should make mismatches actionable. Failure messages should
name the layer and include the case tuple `(input_type, distance, rounds, p)`.

## Testing

### Rsinter Registry And Circuit Source

Add tests that:

- `expand_runner_points` accepts
  `input_type = "surface_rotated_memory_z"`.
- the expanded point preserves `input_type`, `distance`, `rounds`, and `p`.
- specs without `input_type` still default to `surface_rotated_memory_x`.
- unknown input types still produce `unknown input_type: ...`.
- `build_circuit_for_point` records `surface_rotated_memory_z` in result
  params and dispatches to the memory-Z generator.

### Rstim And Stim Parity

Add or extend tests that:

- detector and observable counts match Stim for the representative issue case
  `d=3, rounds=9, p=0.008`.
- isolated one-channel configurations verify before-round-data,
  after-Clifford, after-reset, and before-measure noise placement.
- the decomposed memory-Z DEM is graphlike and semantically matches Stim for a
  representative case, or passes a bounded sample-statistics parity check when
  exact factorization differs.

### End-To-End Smoke

Add a tiny `rsinter` memory-Z spec that runs through `rmatching` and verifies:

- the run completes without solver or sampler failure,
- result params contain `input_type = "surface_rotated_memory_z"`,
- case summary includes detector and observable counts,
- if a PyMatching comparison is available, `rmatching` and PyMatching agree on
  logical-error counts for the same deterministic DEM and shot set.

The smoke should be small enough for CI and should not make statistical claims
from a tiny shot budget. The full issue sweep can be left as a manual benchmark
command or documented reproduction command.

## Acceptance Criteria

1. `rsinter` accepts `input_type = "surface_rotated_memory_z"` in runner params.
2. Existing specs without `input_type` continue to run as memory-X.
3. Memory-Z result rows identify themselves as memory-Z.
4. Memory-Z benchmark circuit generation uses the normal `rstim`
   `rotated_memory_z` path and therefore the four Stim/sinter noise channels.
5. Representative circuit/noise, DEM, and decoder parity checks pass.
6. A tiny memory-Z `rmatching` benchmark smoke test passes.
7. The issue has enough evidence to tell whether any remaining inverted
   scaling comes from decoder behavior rather than benchmark task selection or
   circuit/noise generation.

## Open Follow-Up

If the parity smoke still shows memory-Z logical error rates that clearly
increase with distance after this change, the next design should focus on the
decoder boundary: DEM lowering into `rmatching`, observable-index propagation,
boundary-edge handling, and bit-packed prediction comparison.
