# Issue 415 Typed Compiled Sampler IR Design

## Context

The compiled sampler currently stores each compiled operation as
`CompiledOp { name: String, args: Vec<f64>, targets: Vec<StimTarget> }`.
`FrameSimulator::run_compiled_blocks` then calls `exec_op` with the string name
and original targets, so the compiled sampler still pays per-operation string
dispatch and target decoding.

Issue #415 asks for a real typed fast-path representation for the supported
sampler subset used by the checked d11/r100 surface-code fixture and existing
gating tests. Unsupported, loss, and feedback circuits must keep using the
interpreted fallback instead of entering the compiled sampler through a generic
operation escape hatch.

## Approaches Considered

1. Replace `CompiledOp` with a typed enum and predecode all supported fast-path
   payloads. This is the selected approach because it removes string dispatch
   from the compiled sampler executor while preserving the current block and
   repeat structure.
2. Add a typed wrapper beside the existing generic `CompiledOp`. This would be
   lower risk mechanically, but it would leave a generic operation path close to
   the fast path and make it easier to accidentally execute unsupported
   instructions through string dispatch.
3. Build a broad Stim operation IR now. This would reduce future follow-up work,
   but it is wider than the issue and would risk changing uncommon instruction
   behavior without fixture coverage.

The design uses option 1.

## Design

Change `rstim/src/compiled/circuit.rs` so `CompiledOp` is an enum with explicit
variants for the supported sampler subset:

- `Tick`
- `QubitCoords`
- `ShiftCoords`
- `H { qubits }`
- `Reset { basis, qubits }`
- `XError { probability, qubits }`
- `Depolarize1 { probability, qubits }`
- `Cx { pairs }`
- `Depolarize2 { probability, pairs }`
- `Measure { basis, qubits }`
- `MeasureReset { basis, qubits }`
- `Detector { rec_offsets }`
- `ObservableInclude { observable_index, rec_offsets }`
- `UnsupportedSamplerOp { name }`

`UnsupportedSamplerOp` is not an execution fallback. It is a lowering marker
that makes unsupported operations explicit and lets `choose_sampler_path`
produce a clear interpreted-path decision. The compiled sampler executor returns
an error if this marker is ever executed directly.

Add small lowering helpers that predecode:

- qubit lists, skipping sweep targets the same way the existing executor treats
  them as no-ops;
- qubit pairs, skipping pairs containing sweep targets;
- `rec[-k]` lookbacks into positive `usize` offsets;
- probabilities and observable indices from numeric arguments.

Only the selected fixture and current smoke coverage need to stay on the fast
path. This includes `QUBIT_COORDS`, `R`, `X_ERROR`, `TICK`, `H`, `CX`,
`DEPOLARIZE1`, `DEPOLARIZE2`, `MR`, `M`, `SHIFT_COORDS`, `DETECTOR`, and
`OBSERVABLE_INCLUDE`. Uncommon operations such as `S` remain unsupported for
this issue and force fallback.

Update `rstim/src/compiled/path.rs` so `choose_sampler_path` still rejects loss
and feedback first, then recursively rejects any compiled block containing
`UnsupportedSamplerOp`.

Update `rstim/src/sim/frame.rs` so `run_compiled_blocks` dispatches on typed
`CompiledOp` variants directly. The typed executor should reuse the same private
gate, noise, measurement, detector, and observable helper logic as the string
executor, preserving random-number consumption and sampled bits.

## Testing

Add `rstim/tests/compiled_sampler_ir.rs` with the issue-required tests:

- `selected_surface_fixture_lowers_to_typed_sampler_ops` parses the checked
  d11/r100 fixture, asserts the sampler path is fast, asserts typed variants for
  `Depolarize2`, `Cx`, measurement/reset, detector, and observable operations,
  and asserts no unsupported marker appears in the lowered IR.
- `compiled_sampler_ir_preserves_sample_bits_on_smoke_fixture` compares compiled
  and interpreted sampling on a small supported circuit with deterministic RNGs.
- `loss_and_feedback_circuits_still_choose_fallback` asserts loss and feedback
  circuits still choose fallback.
- `unsupported_sampler_ops_do_not_enter_typed_fast_path` uses an operation
  outside the initial typed scope and asserts the sampler path falls back.

Run the focused issue command and the broader worker command:

- `cargo test -p rstim --test compiled_sampler_ir`
- `cargo test`

## Scope

This change is limited to typed compiled sampler lowering, sampler path gating,
typed compiled sampler execution, and focused tests. It does not remove the
interpreted fallback, does not implement broad Stim instruction coverage, and
does not claim broad performance parity with Stim.

## Self-Review

- No placeholder requirements remain.
- The selected approach removes string dispatch from the compiled sampler fast
  path.
- Unsupported operations are represented only as a fallback marker and are
  rejected by sampler path selection.
- The test plan covers typed lowering, behavior preservation, and negative
  fallback controls.
