# Issue 458 Packed Reference Routing Design

Issue: #458
Date: 2026-07-10

## Context

Issue #452 added `CompiledMeasurementSampler` but kept its cached reference
sample on the legacy reference builder. Issue #457 added packed inverse-tableau
measurement and reset operations. Issue #458 connects these layers: supported
noiseless reference construction should use the packed inverse backend, direct
compiled-sampler construction should reject circuits outside the fast path with
typed reasons, and top-level `Auto` sampling should recover through interpreted
legacy sampling while exposing that recovery decision.

The current code already has a compiled sampler gate, but its fallback reason is
a string. It also silently falls back in `SamplingBackend::Auto`, and the
noiseless reference builder has no test-visible routing decision.

## Automatic Scope Decisions

This Agent Desk run is non-interactive, so the Standing Answer Policy resolves
the Superpowers gates:

- Visual companion: not used because this is backend Rust routing and
  simulator behavior.
- Clarifying questions: answered from issue #458, #452, #457, and the existing
  `rstim` APIs.
- Recommended design: add typed routing enums, route supported reference
  construction through `PackedInverseTableau`, keep the existing public
  `CompiledMeasurementSampler::compile(...)->Result<Self, String>` wrapper for
  compatibility, and expose typed decisions through test-visible hidden helpers.
- Design approval: accepted automatically because the issue provides exact
  behavior, circuits, expected bits, and verification commands.
- Spec review: this document is approved for planning under the non-interactive
  run policy after the placeholder, consistency, and scope checks pass.

## Alternatives Considered

1. Add typed routing decisions beside the existing public wrappers and use them
   internally. This is the chosen approach because it satisfies the tests and
   telemetry need without breaking the #452 string-returning compile API.
2. Replace every existing string fallback API with typed errors. This is
   cleaner long term, but it is a broader public API change than the issue
   requires.
3. Route only the benchmark fixture through packed reference construction. This
   would satisfy one positive case but would leave nested repeats, typed fallback
   reasons, and `Auto` recovery ambiguous.

## Chosen Design

Add a typed fallback reason enum in the compiled routing layer:

- `Loss`
- `MeasurementRecordFeedback`
- `SweepDependent`
- `UnsupportedOperation(String)`

`CompiledPathDecision::Fallback` will carry this enum instead of a static
message. The enum will provide the existing human-readable message strings so
current public wrappers can still return `String`.

Add a reference-routing result in `rstim/src/data_path.rs`:

- `ReferenceSampleDecision::PackedInverse`
- `ReferenceSampleDecision::LegacyFallback(reason)`
- `ReferenceSampleResult { bits, decision }`

`build_reference_sample_with_decision` will return the bits and decision. The
existing `build_reference_sample` remains as a compatibility wrapper returning
only bits.

The packed reference builder will recursively scan and execute supported
instructions with `PackedInverseTableau`:

- supported Clifford/setup: `I`, `H`, `S`, `S_DAG`, `X`, `Y`, `Z`, `CX`, and
  common aliases already implemented by the packed tableau where they map to
  existing methods;
- supported measurement/reset: `M`, `MX`, `MY`, `MR`, `MRX`, `MRY`, `R`, `RX`,
  and `RY` variants already implemented in #457;
- metadata/noiseless skips: `TICK`, `QUBIT_COORDS`, `SHIFT_COORDS`,
  `DETECTOR`, `OBSERVABLE_INCLUDE`, `X_ERROR`, `Y_ERROR`, `Z_ERROR`,
  `DEPOLARIZE1`, `DEPOLARIZE2`, `I_ERROR`, and `II_ERROR`;
- nested `REPEAT` by recursive execution.

The router will return typed legacy fallback before attempting packed execution
for:

- loss and loss-visible measurement/reset instructions;
- measurement-record feedback such as `CX rec[-1] 1`;
- sweep-dependent control such as `CX sweep[0] 1`;
- unsupported operations such as `CZ`.

Legacy fallback calls the existing noiseless reference implementation, extended
where needed to handle measurement-record feedback and sweep-controlled pairs.

## Compiled Sampler And Auto Routing

`CompiledMeasurementSampler::compile_with_decision` will be added as a
test-visible typed variant. It will:

1. compile typed IR once;
2. reject `choose_sampler_path` fallback with the typed reason;
3. build the cached reference through `build_reference_sample_with_decision`;
4. reject if reference construction falls back, rather than embedding legacy
   reference construction inside the direct compiled sampler.

The existing `CompiledMeasurementSampler::compile` remains and maps typed
errors to the existing string messages.

Add `sample_batch_with_options_and_decision` as a test-visible top-level helper.
It returns `(BatchOutput, SampleBatchDecision)`, where
`SampleBatchDecision::InterpretedLegacy(reason)` records `Auto` recovery. The
existing `sample_batch_with_options` remains and returns only `BatchOutput`.

`SamplingBackend::Auto` will try the typed compiled sampler first. On typed
rejection, it will run interpreted legacy sampling and return
`InterpretedLegacy(reason)` through the decision helper. Feedback and sweep
fallback cases will use the legacy executor path so their samples match the
legacy semantics instead of being rejected by the frame simulator.

## Testing

Add `rstim/tests/packed_reference_routing.rs`.

The tests cover all three layers for the issue-required fallback reasons:

- reference construction returns the expected legacy bits and
  `LegacyFallback(reason)`;
- `CompiledMeasurementSampler::compile_with_decision` rejects with the same
  typed reason and never embeds a legacy reference;
- top-level `Auto` returns the same samples through interpreted legacy recovery
  and exposes `InterpretedLegacy(reason)`.

The required fallback cases are:

- `LOSS(0) 0; M 0` -> `[false]`, reason `Loss`;
- `X 0; M 0; CX rec[-1] 1; M 1` -> `[true, true]`, reason
  `MeasurementRecordFeedback`;
- `X 1; CX sweep[0] 1; M 1` with `sweep[0]=true` -> `[false]`, reason
  `SweepDependent`;
- `H 1; X 0; CZ 0 1; H 1; M 0 1` -> `[true, true]`, reason
  `UnsupportedOperation("CZ")`.

Positive packed-reference tests assert that the canonical d11/r100 fixture
selects `PackedInverse` and returns exactly 12,121 false bits, and that a nested
`REPEAT` containing `H`, `M`, `X_ERROR`, `TICK`, `DETECTOR`, and
`OBSERVABLE_INCLUDE` remains packed with the exact measurement count.

Negative controls are encoded by checking the decision as well as the bits:
forcing the fixture through legacy would fail the decision assertion even though
the bits match; forcing `CZ` into packed or silently accepting it in direct
compile would fail the unsupported assertions; removing `Auto` recovery would
fail the end-to-end fallback assertions.

Run:

```sh
cargo test -p rstim --test packed_reference_routing -- --nocapture
cargo build --release -p rstim --bin rstim
test "$(shasum -a 256 benchmarks/rstim_vs_stim_simulator/distribution_cases.toml | awk '{print $1}')" = \
  6f28ad3cd13f4464c59548eef5cc135ad68c439ba01292c7132562f748970432
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --rstim target/release/rstim --shots 10000 --seeds 7 \
  --out /tmp/rstim-packed-reference-distributions.json
cargo test
```

## Out Of Scope

This design does not remove legacy support, add packed implementations for
unsupported gates such as `CZ`, add timing thresholds, or replace unrelated
sampler/analyzer routing.

## Self-Review

- No placeholders remain.
- The typed decisions match the issue's required observable states.
- Direct compile rejects unsupported cases before building a legacy reference.
- `Auto` recovery remains explicit and test-visible.
- Nested repeat and metadata/noise skip behavior are covered by focused tests.
