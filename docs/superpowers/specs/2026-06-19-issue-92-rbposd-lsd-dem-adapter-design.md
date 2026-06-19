# Issue 92 Rbposd LSD DEM Adapter Design

Date: 2026-06-19
Status: Approved by non-interactive Standing Answer Policy
Scope: GitHub issue #92, `rsinter` DEM adapter and `rbposd` runner wiring for the LSD decoder family

## Summary

Issue #92 removes the execution boundary left by #91. `rsinter` already lowers
a `DetectorErrorModel` into a parity-check matrix problem for the OSD-backed
`RbposdDemDecoder`. The LSD path should reuse that same lowering, compile the
same filtered matrix problem into `rbposd::BpLsdDecoder`, and return observable
predictions through the existing `CompiledDecoder::decode_shots_bit_packed`
interface.

The change should keep OSD behavior source-compatible. Existing `rbposd` specs
without LSD params continue to instantiate `RbposdDemDecoder`. Specs with
validated LSD params instantiate a new `RbposdLsdDemDecoder` using the typed
`LsdConfig` produced by the runner parser.

## Goals

- Add an LSD-backed DEM decoder adapter in `rsinter`.
- Reuse the current DEM-to-matrix lowering logic in `rsinter/src/rbposd_adapter.rs`.
- Preserve forced syndrome bits, baseline observables, observable-only terms,
  exact-probability filtering, and bit-packed observable output behavior.
- Keep OSD and LSD selection explicit in the typed `RbposdRunnerParams` family.
- Replace the #91 LSD execution-boundary error with real LSD DEM execution.
- Return clear adapter compile errors for invalid DEM-lowered matrix problems.
- Add the issue-named positive and negative verification tests.

## Non-Goals

- Do not normalize LSD result rows beyond the params already available in #91.
- Do not update smoke or full benchmark specs.
- Do not expand BP method, BP schedule, or LSD method support.
- Do not alter `rbposd` core LSD algorithm behavior.
- Do not change the `CompiledDecoder` trait or bit-packing contract.

## Current Context

#91 and PR #111 added `RbposdRunnerParams` with a private
`RbposdDecoderFamily` enum. Valid LSD params already parse into
`rbposd::LsdConfig`, but `run_point` currently returns:

```text
rbposd LSD DEM decoding is not implemented yet; see issue #92
```

The OSD adapter in `rsinter/src/rbposd_adapter.rs` performs all DEM lowering
and edge-case handling inline:

- lowers `error(...)` targets into detector and observable columns
- handles `repeat` and `shift_detectors` offsets
- filters `p <= 0` terms
- folds `p >= 1` terms into forced syndrome and baseline observables
- folds detector-free terms with `p > 0.5` into baseline observables
- creates a `ParityCheckMatrix` for nonempty detector columns
- maps decoded correction bits back to observable bits

That behavior is the compatibility boundary for both OSD and LSD adapters.

## Alternatives Considered

### 1. Shared Backend Adapter In `rbposd_adapter.rs`

Refactor the current adapter so DEM lowering and compiled decode output are
shared, while backend construction and decode are selected by a small internal
enum. Add `RbposdLsdDemDecoder` as a public sibling to `RbposdDemDecoder`.

Benefits:

- Reuses the exact lowering path required by the issue.
- Keeps OSD source compatibility.
- Avoids a second module with duplicated forced-syndrome and observable logic.
- Keeps the new public surface small and discoverable through `decode.rs`.

Cost:

- Requires a targeted refactor of the existing adapter internals.

This is the chosen approach.

### 2. Parallel LSD Adapter Module With Shared Lowering Helpers

Move the lowering helpers into a shared utility and create a separate
`rbposd_lsd_adapter.rs`.

Benefits:

- Keeps OSD and LSD source files separate.

Costs:

- Adds module overhead for one backend switch.
- Still requires most of the same refactor to avoid duplicating lowering.
- Makes future shared fixes span more files.

This is rejected for this narrow issue.

### 3. Build LSD Through The Existing OSD Adapter Type

Add a constructor flag to `RbposdDemDecoder` so one public adapter type can
create either backend.

Benefits:

- Avoids adding a new exported type.

Costs:

- Blurs the OSD/LSD family boundary that #91 made explicit.
- Makes call sites less self-documenting.
- Risks accidental silent family changes in existing OSD uses.

This is rejected.

## Architecture

### Public Adapter Surface

Keep the existing OSD constructor unchanged:

```rust
RbposdDemDecoder::new(config: DecoderConfig) -> RbposdDemDecoder
```

Add a public LSD sibling:

```rust
RbposdLsdDemDecoder::new(config: LsdConfig) -> RbposdLsdDemDecoder
```

Export `RbposdLsdDemDecoder` through `rsinter/src/decode.rs`.

### Internal Backend Selection

Inside `rsinter/src/rbposd_adapter.rs`, introduce private backend types:

```rust
enum RbposdDemBackendConfig {
    Osd(DecoderConfig),
    Lsd(LsdConfig),
}

enum RbposdDemBackend {
    Osd(BpOsdDecoder),
    Lsd(BpLsdDecoder),
}
```

`RbposdDemBackendConfig::compile` takes the lowered parity matrix and filtered
probabilities, then builds either `BpOsdDecoder` or `BpLsdDecoder` with
`ChannelModel::BitFlipProbabilities`.

Compile errors should stay adapter-specific and clear:

```text
invalid rbposd parity matrix: <source error>
failed to compile rbposd decoder: <source error>
```

These messages apply to both families. They satisfy the negative control by
returning `Err` instead of panicking or silently producing an empty decoder when
matrix construction or backend construction rejects the lowered problem.

### Shared Compiled Decoder

`CompiledRbposdDemDecoder` should store:

- `decoder: Option<RbposdDemBackend>`
- `num_dets`
- `num_obs`
- `observable_columns`
- `forced_syndrome`
- `baseline_observables`

The `None` path remains for DEMs with no filtered detector columns. It should
still preserve baseline observables and avoid constructing an empty parity
matrix.

`decode_shots_bit_packed` remains unchanged at the interface level. It builds
the syndrome, XORs forced syndrome bits, decodes through the selected backend,
maps correction columns to observables, XORs baseline observables, and writes
b8-packed output.

### Runner Wiring

Update `rsinter/src/bench/runners/rbposd.rs`:

- import `RbposdLsdDemDecoder`
- remove the obsolete #91 execution-boundary error
- for `RbposdDecoderFamily::Lsd`, instantiate
  `RbposdLsdDemDecoder::new(*lsd_config)` and call
  `run_decoder_point_with_dem_mode` with the existing normalized params

The OSD branch remains unchanged except for any field destructuring needed to
avoid unused warnings.

## Error Handling

- Invalid parity matrices return `invalid rbposd parity matrix: <source error>`.
- Backend construction failures return `failed to compile rbposd decoder: <source error>`.
- Decode failures return `rbposd decode failed: <source error>`.
- Existing runner parse errors from #91 remain unchanged.

No adapter path should panic on malformed lowered matrix data. Probability
filtering and detector-free terms should preserve the current OSD semantics.

## Testing

### Positive Adapter Test

Add `lsd_dem_decoder_predicts_a_known_single_observable_flip` in
`rsinter/tests/decode_rbposd.rs`.

The test should:

- parse `error(0.125) D0 L0` plus a detector-only term
- compile with `RbposdLsdDemDecoder::new(LsdConfig::default())`
- decode a shot with detector `D0 = 1`
- assert the packed observable prediction is `0b0000_0001`

This mirrors the existing OSD adapter behavior while proving the LSD backend
uses the DEM-lowered matrix.

### Negative Compile Test

Add `lsd_dem_decoder_returns_compile_error_for_invalid_matrix_problem`.

Use a programmatically constructed DEM with a non-finite error probability,
for example `f64::NAN`, on a detector-and-observable term. Lowering should
still produce a nonempty detector matrix, but `BpLsdDecoder::new` should reject
the invalid channel probability and return an error containing:

```text
failed to compile rbposd decoder
```

### Runner Test Update

Replace the #91 boundary test with an execution test showing a valid LSD
benchmark point no longer fails. The existing helper can continue to use
`max_shots = 0`, which proves runner wiring without expanding benchmark result
normalization scope.

Rename or update the test to assert:

- `run_rust_benchmark` returns `Ok`
- the `rbposd_lsd` artifact directory is created
- the result row status is `ok`

### Regression Coverage

Existing OSD tests must keep passing:

- single observable flip
- reused compiled instance
- observable-only terms
- exact-probability terms
- zero-syndrome map cases
- OSD order LER behavior

## Verification

Issue #92 names these commands:

```bash
cargo test -p rsinter lsd_dem_decoder_predicts_a_known_single_observable_flip --offline
cargo test -p rsinter lsd_dem_decoder_returns_compile_error_for_invalid_matrix_problem --offline
```

Additional required and recommended checks:

```bash
cargo test -p rsinter --offline
cargo test --offline
cargo fmt --check --package rsinter
git diff --check
```

The non-offline forms are the intended project commands. This Agent Desk
workspace cannot reach the crates.io index, so verification should use
`--offline` after the first network failure proves the environmental root cause.

## Acceptance Criteria

- `RbposdLsdDemDecoder` is exported from `rsinter::decode`.
- OSD adapter behavior is preserved.
- LSD adapter compiles DEM-lowered matrix problems into `BpLsdDecoder`.
- LSD runner params route benchmark execution through the LSD adapter.
- Forced syndrome bits, baseline observables, filtered probabilities, and
  observable-column mapping are shared by OSD and LSD.
- Issue-named positive and negative tests pass.
- `cargo test -p rsinter --offline` and `cargo test --offline` pass before PR
  creation.
