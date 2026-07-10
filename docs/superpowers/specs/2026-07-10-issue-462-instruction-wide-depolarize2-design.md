# Issue 462 Instruction-Wide DEPOLARIZE2 Design

## Context

Issue #460 added a constant-space rare-event iterator over a flattened
opportunity domain. `FrameSimulator` still applies sparse `DEPOLARIZE2` by
building one event mask per target pair. The selected d11/r100 fixture expands
to 44,000 target pairs, so per-pair sparse setup dominates the low-probability
path even though the logical opportunity domain is just all pair/shot
combinations for one instruction.

Interpreted and compiled frame execution already share
`FrameSimulator::exec_depolarize2_pairs`, so the instruction-wide behavior can
be implemented once in that helper and used by both paths.

## Approaches Considered

1. Add a sparse instruction-wide helper under `exec_depolarize2_pairs` and keep
   the current dense per-pair mask path for `p > 0.02`. This is the selected
   approach because it uses the existing shared interpreted/compiled entry
   point, preserves the dense fallback and probability model, and keeps the
   change localized to frame simulation.
2. Change `random_bits_with_prob_into` so it can fill a virtual multi-pair
   domain. This would reuse the existing sparse-mask helper, but the helper
   writes bit masks and does not naturally select one two-qubit Pauli branch per
   flattened event.
3. Precompute all sparse event indices for the instruction and then replay
   branches. This avoids per-pair iterator construction, but it introduces an
   event vector whose size grows with the number of sampled events and is not
   needed for streaming execution.

The design uses option 1.

## Design

Refactor the rare-event module slightly so the same geometric-skipping state can
be advanced without holding a long-lived mutable RNG borrow. The existing
`rare_error_indices(probability, attempt_count, rng)` iterator remains intact
for issue #460 tests and callers. Internally it wraps a crate-visible
`RareErrorIndexSampler` with:

```rust
pub(crate) fn new(probability: f64, attempt_count: usize) -> Self;
pub(crate) fn next_index(&mut self, rng: &mut (impl RngCore + ?Sized)) -> Option<usize>;
```

`FrameSimulator` uses that sampler for sparse `DEPOLARIZE2`, allowing one RNG
stream to interleave rare-event draws and branch draws without collecting event
indices.

For `0 < p <= SPARSE_BERNOULLI_MAX_PROBABILITY`, `exec_depolarize2_pairs`
computes:

```text
attempt_count = pairs.len() * batch_size
event_index = pair_index * shots + shot_index
pair_index = event_index / shots
shot_index = event_index % shots
```

One `RareErrorIndexSampler` walks `[0, attempt_count)`. For each yielded event,
the helper decodes the pair and shot, samples exactly one branch index from
`0..15`, maps it through the ordered non-identity Pauli list, and XORs the
selected X/Z bit into the two target rows for that shot.

For `p > SPARSE_BERNOULLI_MAX_PROBABILITY`, the existing dense path remains:
prepare one event mask per pair, enumerate set event bits, sample one branch per
set bit, and XOR scratch buffers into the two target rows. This keeps the
medium/high-probability behavior and allocation profile from the current code.

## Branch Mapping

Use an explicit private branch table for the 15 non-identity two-qubit Paulis:

```text
IX IY IZ XI XX XY XZ YI YX YY YZ ZI ZX ZY ZZ
```

`II` is not present in the table and branch sampling uses `0..15`, not `% 16`
or `0..16`. Both sparse and dense `DEPOLARIZE2` branch draws call the same
private `sample_depolarize2_branch_index` helper and then map the branch through
the same table.

## Telemetry

Debug builds expose hidden test helpers from `rstim::sim::frame`:

- `reset_depolarize2_sampling_telemetry()`
- `depolarize2_sampling_telemetry()`
- `depolarize2_decode_event_for_test(event_index, shots)`
- `depolarize2_branch_label_for_test(branch_index)`
- `sample_depolarize2_branch_index_for_test(rng)`

The telemetry struct reports `sampling_path`, `iterator_builds`, and
`attempt_count`. Sparse instruction-wide execution reports
`sampling_path = "sparse"`, `iterator_builds = 1`, and the flattened attempt
count. Dense fallback reports `sampling_path = "dense"`, `iterator_builds = 0`,
and the same flattened attempt count. Release builds do not need these hidden
helpers.

## Testing

Add `rstim/tests/frame_instruction_wide_depolarize2.rs` with tests for:

- pair-major decode boundaries using the shared decode helper;
- exact branch labels for indices `0..14`;
- 1,500,000 seeded branch draws with every branch count in 98,000-102,000 and
  no `II`;
- a scripted first sparse event and branch-zero draw proving branch zero is
  `IX`, not `II`;
- interpreted sparse execution with 110 pairs and 1024 shots reporting one
  iterator build and `attempt_count = 112640`;
- compiled sparse execution reporting the same telemetry;
- `p = 0.3` reporting `sampling_path = "dense"`.

Keep `rstim/tests/frame_depolarize_alloc.rs` unchanged except for any necessary
compatibility with the new sparse helper; it should continue to reject
pair-proportional scratch allocation.

Run the issue's full verification commands, including the release binary and
distribution verifier. The pinned
`stim_depolarize2_two_measured_qubits` distribution remains:

```text
00=.92
01=.02666666666666667
10=.02666666666666667
11=.02666666666666667
```

## Scope

This change is limited to the rare-event sampler shape, private frame
simulation helpers, debug-only test telemetry, and focused integration tests. It
does not change the `DEPOLARIZE2` probability model, CLI formats, benchmark
case definitions, or add SIMD/parallel execution.

## Self-Review

- No placeholders remain.
- The selected approach satisfies the issue's shared interpreted/compiled helper
  requirement through the existing `exec_depolarize2_pairs` route.
- Sparse execution uses exactly one flattened iterator per instruction and the
  dense fallback remains for `p > 0.02`.
- Tests map directly to the negative controls: pair/shot transposition,
  branch-zero-as-`II`, missing/duplicated branch labels, per-pair iterator
  construction, dense/sparse threshold inversion, and allocation regression.
