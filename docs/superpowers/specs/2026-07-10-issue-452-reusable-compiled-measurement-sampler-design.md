# Issue 452 Reusable Compiled Measurement Sampler Design

## Context

`sample_batch_with_options` currently compiles the typed sampler IR and builds the
reference sample each time the compiled sampler path is used. Issue #452 needs a
compile-once/sample-many API so repeated sampling can measure steady-state
sampling cost without repeatedly paying setup cost.

The existing compiled sampler already has the correct routing gate:
`compile_circuit` lowers source `StimInstr` values into `CompiledCircuit`, and
`choose_sampler_path` rejects loss, feedback, and unsupported sampler
instructions with the required interpreted-path reasons. The existing
`sample_compiled_batch` also already produces the correct `BatchOutput`, but it
builds the reference sample internally on every call.

## Approaches Considered

1. Add a reusable `CompiledMeasurementSampler` that owns a `CompiledCircuit`, a
   cached reference bit vector, and lightweight diagnostics, then delegates each
   sample call to a compiled-batch helper that accepts the cached reference. This
   is the selected approach because it reuses the existing typed execution path
   while changing only setup lifetime.
2. Change `sample_compiled_batch` to require callers to pass a reference sample.
   This would force every current compiled sampling caller to know about
   reference lifetime and would be wider than the requested public API.
3. Replace `sample_batch_with_options` with the reusable sampler internally.
   This could reduce duplication, but it risks changing automatic fallback and
   legacy one-shot semantics that the issue names as out of scope.

The design uses option 1.

## Design

Add `CompiledMeasurementSampler` and
`CompiledMeasurementSamplerDiagnostics` in `rstim/src/sampler.rs`.
`CompiledMeasurementSampler::compile(instrs, reference_mode)` will:

- call `compile_circuit(instrs)` once;
- run `choose_sampler_path(&compiled)` before returning;
- return the existing fallback reason as a `String` when the compiled sampler
  path is unsupported;
- build the exact reference sample once with
  `build_reference_sample(instrs, reference_mode)`;
- store diagnostics initialized as
  `compiled_ir_builds = 1`, `reference_builds = 1`, `sample_calls = 0`.

`CompiledMeasurementSampler::sample(shots, rng, output_mode)` will increment
`sample_calls`, build a `SampleOptions` value from the cached
`reference_sample_mode` and requested `output_mode`, and call a new
crate-private compiled sampler helper that accepts `&[bool]` for the reference
sample. It must not rebuild the typed IR or reference sample.

`CompiledMeasurementSamplerDiagnostics` is `#[doc(hidden)]`, `Clone`, `Copy`,
`Debug`, `Default`, `PartialEq`, and `Eq`, with the required public fields.
The `diagnostics()` accessor is also `#[doc(hidden)]` and returns the current
snapshot by value.

Re-export the two public types from `rstim/src/lib.rs` as:

- `rstim::CompiledMeasurementSampler`
- `rstim::CompiledMeasurementSamplerDiagnostics`

Keep `sample_batch_with_options`, `SamplingBackend::Auto`, the interpreted
fallback, loss behavior, feedback behavior, and top-level automatic fallback
unchanged.

## Testing

Add `rstim/tests/reusable_compiled_measurement_sampler.rs` with the issue
required tests:

- `compile_once_samples_many_batches`
- `compiled_sampler_caches_nonzero_reference_bits`
- `compiled_sampler_preserves_both_output_modes`
- `unsupported_circuit_is_rejected_at_compile_time`

The lifecycle test calls `sample` nine times with shot counts
`0, 1, 2, 3, 7, 16, 31, 64, 1024`, alternates both output modes, and asserts
diagnostics finish as `compiled_ir_builds=1 reference_builds=1 sample_calls=9`.
The nonzero-reference test compiles `X 0\nM 0\n` with
`ReferenceSampleMode::SimulateNoiseless` and observes measurement bit 1. The
unsupported test compiles `S 0\nM 0\n` and expects
`unsupported sampler instructions require the interpreted path`. The acceptance
test prints `PASS reusable compiled measurement sampler`.

Run:

- `cargo test -p rstim --test reusable_compiled_measurement_sampler -- --nocapture`
- `cargo test`

## Scope

This change is limited to a reusable compiled measurement sampler API, a
reference-aware compiled sampling helper, root re-exports, and focused tests. It
does not replace the legacy reference tableau, extend unsupported instruction
coverage, or change loss, feedback, or automatic fallback semantics.

## Self-Review

- No placeholders remain.
- The selected approach caches exactly the two setup objects named in the issue.
- Unsupported circuits are rejected at compile time through the existing sampler
  path gate.
- Diagnostics are hidden but public enough for integration tests.
- The test plan includes the required positive, negative, output-mode, and
  acceptance-print checks.
