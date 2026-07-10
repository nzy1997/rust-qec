# Issue 461 Instruction-Wide One-Qubit Noise Design

## Context

Issue #460 added the internal `RareErrorIterator` primitive over a flattened
attempt domain. `X_ERROR` and `DEPOLARIZE1` still build low-probability masks
one target at a time in the frame simulator, so an instruction with 100 targets
and 1024 shots constructs 100 sparse walks instead of one walk over 102400
target-shot opportunities.

This issue wires only `X_ERROR` and `DEPOLARIZE1` to the instruction-wide
iterator in the frame simulator paths used by interpreted sampling and typed
compiled sampling. Other one-qubit channels and wall-clock thresholds remain
out of scope.

## Approaches Considered

1. Add shared frame helpers for `X_ERROR` and `DEPOLARIZE1` that branch once per
   instruction. The sparse branch builds one `RareErrorIterator` over
   `target_count * shots`; the dense branch keeps the existing per-target dense
   mask fallback. This is selected because interpreted and compiled execution
   already meet at `FrameSimulator`, so one helper gives both paths the same
   flattening and telemetry contract.
2. Keep the current per-target mask helpers and add only telemetry tests. This
   would be a small edit, but it preserves the exact inefficiency the issue
   exists to remove and fails the iterator-build negative control.
3. Replace all one-qubit noise channels with an instruction-wide abstraction.
   This may become useful later, but it broadens scope beyond `X_ERROR` and
   `DEPOLARIZE1` and risks changing unrelated seeded behavior.

The design uses option 1.

## Design

Add frame-level helpers that accept a qubit slice, probability, words per row,
and RNG:

- `exec_x_error_qubits(&mut self, qubits, p, wpr, rng) -> Result<(), String>`
- `exec_depolarize1_qubits(&mut self, qubits, p, wpr, rng) -> Result<(), String>`

For `p <= SPARSE_BERNOULLI_MAX_PROBABILITY` the helper computes
`attempt_count = qubits.len() * self.batch_size`, builds exactly one
`rare_error_indices(p, attempt_count, rng)` iterator, and decodes each yielded
event as:

```text
target_index = event_index / shots
shot_index = event_index % shots
```

where `shots` is `self.batch_size`. The decoded target index selects the qubit
from the instruction's target list, and the decoded shot index selects the bit
within that qubit's frame row. `X_ERROR` toggles the X frame bit. `DEPOLARIZE1`
draws one uniform branch per yielded event and toggles X, Y, or Z frame bits.

For `p > SPARSE_BERNOULLI_MAX_PROBABILITY` the helpers record the dense path and
reuse the existing dense mask behavior. This preserves the p=0.3 dense path and
does not construct `RareErrorIterator`.

The typed compiled sampler already lowers `X_ERROR` and `DEPOLARIZE1` into
`CompiledOp::XError` and `CompiledOp::Depolarize1` and executes them through
`FrameSimulator`; those arms will call the same helpers as interpreted
execution.

## Telemetry

Debug builds expose hidden frame-level telemetry for the last one-qubit noise
instruction:

```rust
pub enum OneQubitNoiseSamplingPath { None, Sparse, Dense }

pub struct OneQubitNoiseInstructionTelemetry {
    pub sampling_path: OneQubitNoiseSamplingPath,
    pub iterator_builds: usize,
    pub attempt_count: usize,
}
```

The sparse branch snapshots `rare_error_telemetry().iterator_builds` before and
after execution and stores the difference, so rebuilding inside the target loop
would report 100 builds for the 100-target acceptance case. The dense branch
reports zero iterator builds. Tests reset this telemetry before each interpreted
or compiled run.

## Testing

Add `rstim/tests/frame_instruction_wide_one_qubit_noise.rs` with:

- Known-answer decoding for event indices `0`, `1023`, `1024`, and `102399` at
  `shots = 1024`.
- `X_ERROR` and `DEPOLARIZE1` sparse-path checks for both interpreted and
  compiled backends using 100 targets, 1024 shots, and `p = 0.001`; each must
  report sparse path, one iterator build, and `attempt_count = 102400`.
- Medium-probability checks at `p = 0.3` for both instructions and backends;
  each must report dense path and zero iterator builds.

Update `rstim/tests/frame_noise_masks.rs` so its integer-threshold dense-mask
source check follows the new dense helper name instead of the old match arm.
The existing distribution catalog already contains the pinned
`stim_x_error_two_measured_qubits` and `stim_depolarize1_two_measured_qubits`
joint distributions required for the external verifier.

Focused verification:

```sh
cargo test -p rstim --test frame_instruction_wide_one_qubit_noise -- --nocapture
cargo test -p rstim --test frame_noise_masks -- --nocapture
```

Final verification also runs the release CLI build, the distribution verifier,
and `cargo test`.

## Scope

This change is limited to frame execution of `X_ERROR` and `DEPOLARIZE1`,
debug-only frame telemetry, focused integration tests, and the existing dense
mask test update. It does not change `Y_ERROR`, `Z_ERROR`, `PAULI_CHANNEL_1`,
`DEPOLARIZE2`, CLI output formats, or benchmark case definitions.

## Self-Review

- No placeholders remain.
- The selected design wires both interpreted and typed compiled paths through
  one helper contract.
- Sparse flattening, dense fallback, iterator lifecycle telemetry, and
  known-answer index decoding all have direct tests.
- The named negative controls map to concrete checks: target-loop rebuilding
  changes `iterator_builds`, transposed flattening fails the decode oracle,
  target broadcasting fails pinned joint distributions, p=0.3 sparse routing
  fails path telemetry, and no-op noise fails distribution verification.
