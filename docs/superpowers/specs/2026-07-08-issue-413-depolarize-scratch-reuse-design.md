# Issue 413 Depolarizing Scratch Reuse Design

## Context

`FrameSimulator` already uses the integer-threshold noise-mask helper added for
#412. The remaining allocation hotspot is the depolarizing execution path:
`DEPOLARIZE1` allocates event, X, and Z scratch vectors inside each target loop,
and `DEPOLARIZE2` allocates event plus four Pauli scratch vectors inside each
target-pair loop. The selected surface-code fixture expands to many
`DEPOLARIZE2` targets, so this repeats allocation work for every pair.

No public CLI output, file formats, or sampled distributions should change.

## Approaches Considered

1. Add reusable scratch storage to `FrameSimulator`. This is the recommended
   approach because the simulator already owns other operation-local scratch
   state such as `last_correlated_error_occurred`, and reuse naturally spans all
   targets within an operation and across operations for the same simulator.
2. Allocate scratch once inside each depolarizing match arm. This removes the
   worst per-target allocation pattern, but still rebuilds scratch every
   operation and duplicates shape-management code.
3. Introduce a broader frame-operation workspace abstraction. This could unify
   other temporary buffers later, but it is broader than this issue and would
   touch unrelated operations.

The design uses option 1 and keeps the helper private to `rstim/src/sim/frame.rs`.

## Design

Add a private `DepolarizeScratch` type with five reusable `Vec<u64>` buffers:
`events`, `x_a`, `z_a`, `x_b`, and `z_b`. `FrameSimulator` owns one instance.
The scratch type exposes methods that resize buffers to `words_per_row` and
clear only the slices needed by the current operation.

Route `DEPOLARIZE1` and `DEPOLARIZE2` through helper methods on
`FrameSimulator`:

- `exec_depolarize1(targets, p, wpr, rng)`
- `exec_depolarize2(targets, p, wpr, rng)`

For each target or target pair, fill the reusable `events` buffer using a
scratch-filling variant of the existing integer-threshold helper. Then build the
per-basis Pauli masks in the reusable Pauli buffers and XOR them into the target
rows exactly as today.

The helper must preserve the #412 probability semantics:

- `p <= 0` produces no events.
- `p >= 1` sets all valid shot bits and masks unused trailing bits.
- intermediate probabilities compare one RNG `u64` per valid bit against the
  same integer threshold.

## Testing

Add `rstim/tests/frame_depolarize_alloc.rs` with two integration tests:

- `depolarize2_reuses_scratch_across_many_target_pairs` installs a test-local
  global allocator counter and runs a repeated `DEPOLARIZE2(0.001)` fixture with
  many target pairs. It asserts the number of allocation calls stays well below
  the old per-pair allocation shape.
- `depolarize1_and_depolarize2_preserve_distribution_smoke` runs seeded,
  batched samples for `DEPOLARIZE1` and `DEPOLARIZE2` at nonzero probability and
  checks the observed measurement flips fall inside broad statistical bounds.

The allocation test is not a timing test. Its threshold should allow parser,
simulator, and measurement setup allocations while rejecting fresh scratch
vectors inside every `DEPOLARIZE2` target-pair loop.

## Scope

This change is limited to frame simulator scratch reuse and focused tests. It
does not change public output, benchmark pass/fail thresholds, QP101 rendering,
or depolarizing Pauli selection probabilities.

## Self-Review

- No placeholder requirements remain.
- The implementation path covers both `DEPOLARIZE1` and `DEPOLARIZE2`.
- The test plan includes both an allocation negative control and a distribution
  negative control, matching the issue.
