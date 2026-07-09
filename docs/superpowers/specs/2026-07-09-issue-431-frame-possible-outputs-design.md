# Issue 431 Frame Possible Outputs Design

## Context

Issue #431 asks for a focused regression suite proving that sampled frame-sampler measurement rows are possible under an independent reference path. Existing `rstim` tests already cover frame sampling distributions and Stim-style frame simulator behavior, but they do not directly reject impossible sampled rows with a tableau-style possible-output check.

## Approaches Considered

1. Add a new integration test helper that samples small circuits through `rstim::sampler` and replays each sampled measurement row through `rstim::sim::tableau::StabilizerState`.
   This is the chosen approach because it uses existing public APIs, keeps the helper local to tests, and gives the required negative fixture a real oracle.

2. Compare sampled output distributions to fixed expected counts.
   This is rejected because the issue explicitly wants possible-output checks independent of statistical verification.

3. Add a public possible-output API to `rstim`.
   This is rejected because the issue recommends avoiding new public surface and the needed behavior is currently test-only.

## Design

Create `rstim/tests/frame_possible_outputs.rs` with a local helper that:

- Parses inline `.stim` snippets with `parse_lines`.
- Samples a small number of shots with `sample_batch_with_options`, forcing `SamplingBackend::Interpreted` so the test is about the frame sampler path.
- Replays each sampled measurement row through a separate `StabilizerState` runner that supports the small gate/measurement subset used by these tests.
- Treats deterministic measurement mismatches as impossible and random measurement outcomes as possible by injecting the candidate bit into a tiny test RNG before the measurement call.

The reference helper remains intentionally narrow: it handles `H`, `CX`/`CNOT`, `CZ`, `X`, `Z`, `R`/`RX`, `M`/`MX`, `MR`/`MRX`, `TICK`, coordinate annotations, detectors, observables, and simple `REPEAT` blocks. Unsupported operations return an error so future test expansion cannot silently pass through unknown behavior.

## Test Coverage

The integration test file will include:

- `sampled_outputs_are_possible_for_entangling_circuits`, covering Bell-style entangling circuits, measurement/reset cases, and a small surface-code-shaped smoke circuit with reset, entangling rounds, detectors, and final data measurements.
- `impossible_output_is_rejected`, using the required `H 0; CNOT 0 1; M 0; M 1` fixture where `00` and `11` are possible while `01` and `10` are impossible.

The negative fixture proves the helper is not a vacuous sampler smoke test: changing the helper to accept every row makes `impossible_output_is_rejected` fail.

## Verification

Run:

```sh
cargo test -p rstim --test frame_possible_outputs -- --nocapture
cargo test
```

The focused command must show passing tests named `sampled_outputs_are_possible_for_entangling_circuits` and `impossible_output_is_rejected`.

## Out Of Scope

Do not compare runtime against Stim. Do not port Stim's full frame simulator suite. Do not add new public API.
