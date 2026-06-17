# Issue 88 BpLsdDecoder Public API Design

Date: 2026-06-17
Status: Proposed
Scope: `rbposd` public API, minimal LSD configuration boundary, and API-level tests for GitHub issue #88

## Summary

Issue #88 adds a first-class `BpLsdDecoder` public API to `rbposd`.

The work should make LSD support discoverable from crate exports and constructible through the same matrix/syndrome workflow as `BpOsdDecoder`, while keeping the existing OSD public API source-compatible. The first landing is deliberately narrow: it establishes the public decoder family boundary, validates inputs, reuses existing BP setup, and provides an order-0 validity fallback that can return a correction satisfying the requested syndrome.

This issue does not implement the full LSD algorithm. Follow-on issues own that deeper behavior:

- #89 implements the first real deterministic LSD solve path.
- #90 expands shared BP workspace reuse and borrowed LSD fixture coverage.
- #91-#93 wire LSD into `rsinter` runner params, DEM adapters, and result rows.
- #98 and later fixture/benchmark issues grow the comparison catalog and benchmark coverage.

## Goals

- Add a top-level `BpLsdDecoder` export from `rbposd`.
- Add explicit LSD configuration types with one supported method field and an `lsd_order` field.
- Keep `BpOsdDecoder`, `DecoderConfig`, and existing call sites source-compatible.
- Avoid folding LSD into `BpOsdDecoder` or making `DecoderConfig` carry LSD-family semantics.
- Avoid duplicating the whole BP hot path in the new decoder.
- Add focused API tests that construct `BpLsdDecoder`, decode a small syndrome, and validate `H * correction == syndrome`.
- Document the public LSD surface and its intentionally limited first behavior.

## Non-Goals

- Do not implement full LSD post-BP search in #88.
- Do not add `rsinter` integration or benchmark runner parameters.
- Do not add LSD fixture catalogs, borrowed upstream differential cases, or parity harness support.
- Do not expand BP method or schedule options beyond the current default path.
- Do not change the meaning or shape of `DecodeResult` in this issue.
- Do not make `rbposd` depend on `rstim` or `rsinter`.

## Current Context

`rbposd` currently exposes `BpOsdDecoder` as the public matrix decoder:

```rust
let decoder = BpOsdDecoder::new(pcm, channel, DecoderConfig::default())?;
let result = decoder.decode(&syndrome)?;
```

Internally, `BpOsdDecoder` owns:

- `ParityCheckMatrix`
- `CompiledGraph`
- `DecoderConfig`
- prior LLRs derived from `ChannelModel`
- reusable `BpWorkspace`
- reusable `OsdWorkspace`

The current BP execution machinery already lives in `rbposd/src/bp.rs` and was recently optimized around reusable compiled graph and workspace state. `compute_prior_llrs` and `prior_hard_decision` are private helper functions in `rbposd/src/decoder.rs`; a separate LSD decoder should not copy those helpers or fork the whole BP orchestration.

The existing `DecodeResult` fields are:

- `correction`
- `converged`
- `bp_iterations`
- `used_osd`
- `residual_syndrome_weight`

For #88, this result type remains unchanged.

## Alternatives Considered

### 1. Small Shared Decoder Support Module

Add `BpLsdDecoder` as a separate public decoder type and move only the small shared support needed by both families into an internal helper module.

Benefits:

- Keeps OSD and LSD public families distinct.
- Avoids copying channel validation and prior LLR setup.
- Aligns with #90's shared-BP direction without doing the whole #90 refactor.
- Keeps the implementation honest about #88's narrow public-API scope.

Cost:

- Slightly larger #88 diff than a wrapper-only approach.

This is the chosen approach.

### 2. Wrapper Around `BpOsdDecoder`

Implement `BpLsdDecoder` as a thin wrapper over `BpOsdDecoder`.

Benefits:

- Smallest implementation.
- Easy to pass the positive API contract test.

Costs:

- Makes the LSD family boundary misleading.
- Forces #89/#90 to undo the wrapper shape when real LSD state appears.
- Makes documentation awkward because the public LSD decoder would secretly be an OSD decoder.

This is rejected.

### 3. Independent Copy Of `BpOsdDecoder` Orchestration

Create `lsd_decoder.rs` with a copy of BP orchestration and channel validation.

Benefits:

- Straightforward module split.
- Requires little refactoring of existing OSD code.

Costs:

- Conflicts with #90's explicit goal of not duplicating the BP hot path.
- Makes later BP fixes land in two decoder files.
- Creates preventable maintenance debt.

This is rejected.

## Public API Design

### `LsdMethod`

Add a public enum in `rbposd/src/config.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LsdMethod {
    LocalizedStatistics,
}
```

`LocalizedStatistics` is the only #88 method variant. It gives downstream code a typed method field without exposing speculative future methods or stringly typed names.

### `LsdConfig`

Add a public config struct:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LsdConfig {
    pub method: LsdMethod,
    pub lsd_order: usize,
}
```

Default:

```rust
impl Default for LsdConfig {
    fn default() -> Self {
        Self {
            method: LsdMethod::LocalizedStatistics,
            lsd_order: 0,
        }
    }
}
```

`LsdConfig` is intentionally separate from `DecoderConfig`. `DecoderConfig` remains the OSD-family public config used by `BpOsdDecoder`, `CssDecoders`, and existing `rsinter` code. This avoids silently making OSD config carry LSD-specific fields.

### `BpLsdDecoder`

Add a public decoder type in `rbposd/src/lsd_decoder.rs`:

```rust
pub struct BpLsdDecoder { ... }
```

Constructor:

```rust
impl BpLsdDecoder {
    pub fn new(
        pcm: ParityCheckMatrix,
        channel: ChannelModel,
        config: LsdConfig,
    ) -> Result<Self, DecodeError>;

    pub fn decode(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError>;
}
```

The top-level crate exports should include:

```rust
pub use config::{LsdConfig, LsdMethod};
pub use lsd_decoder::BpLsdDecoder;
```

This makes a cold reader able to discover and construct the LSD decoder from the same crate root as the OSD decoder.

## Internal Architecture

### `decoder_core.rs`

Add an internal module such as `rbposd/src/decoder_core.rs`.

Responsibilities:

- compute prior LLRs from `ChannelModel`
- validate channel probability shape and values
- build prior hard decisions
- provide a small helper or state struct for shared BP setup and BP result handoff

This module must remain internal. It is not a new public abstraction.

The initial extraction should be conservative. It should not redesign the full BP engine or workspace ownership model in #88. It should simply avoid duplicating private helper logic when `BpLsdDecoder` lands.

### `BpOsdDecoder`

`BpOsdDecoder` remains in `rbposd/src/decoder.rs`. It should call the shared core helpers after extraction but keep the same constructor and decode behavior:

```rust
BpOsdDecoder::new(pcm, channel, DecoderConfig::default())?
decoder.decode(&syndrome)?
```

Its `OsdWorkspace` ownership and OSD fallback behavior remain unchanged.

### `BpLsdDecoder`

`BpLsdDecoder` owns its own:

- `ParityCheckMatrix`
- `CompiledGraph`
- `LsdConfig`
- prior LLRs
- `Mutex<BpWorkspace>`
- minimal post-BP fallback workspace required for #88

It should not own `OsdWorkspace` as its semantic post-BP state. If the order-0 fallback needs access to existing GF(2) solve machinery, that solve helper should be framed as a temporary validity fallback, not as the LSD algorithm implementation. #89 can then replace or extend the post-BP path with real LSD state.

## Decode Data Flow

### Construction

`BpLsdDecoder::new(...)` performs:

1. validate and convert `ChannelModel` into prior LLRs through shared helper code
2. build `CompiledGraph::from_pcm(&pcm)`
3. create a reusable `BpWorkspace`
4. store `LsdConfig`
5. prepare only the minimal order-0 fallback state required for the public API contract

Channel length mismatch should return:

```rust
DecodeError::DimensionMismatch {
    what: "channel probabilities",
    expected: pcm.num_bits(),
    actual: probabilities.len(),
}
```

Invalid probabilities should return `DecodeError::InvalidProbability`, matching `BpOsdDecoder`.

### Decode

`BpLsdDecoder::decode(&syndrome)` performs:

1. validate syndrome length
2. handle zero-syndrome prior hard-decision fast path, matching current OSD semantics
3. run existing minimum-sum BP over `CompiledGraph` and `BpWorkspace`
4. if BP residual is zero, return the BP hard decision
5. if BP residual is nonzero, run the #88 order-0 validity fallback to produce a correction satisfying `H * correction == syndrome`
6. return `DecodeResult`

For #88, `converged` and `bp_iterations` report the BP run. `residual_syndrome_weight` should be `0` after a successful fallback. `used_osd` should remain `false` for `BpLsdDecoder`, because it is an OSD-family diagnostic field and this issue does not reshape `DecodeResult`.

### `lsd_order`

The first public config includes `lsd_order`, but #88 should not claim nonzero-order LSD behavior.

Required #88 behavior:

- `lsd_order = 0` is the only supported order.
- `lsd_order > 0` must be rejected during `BpLsdDecoder::new(...)`.
- issue #89 is responsible for enabling and testing nonzero-order LSD behavior.

## Error Handling

Reuse existing errors where possible:

- `DecodeError::DimensionMismatch` for syndrome length and channel length mismatches
- `DecodeError::InvalidProbability` for non-finite, zero, negative, or `>= 1.0` probability values
- existing matrix construction errors from `ParityCheckMatrix`

Avoid adding a broad LSD failure error in #88. Issue #89 owns deterministic failure behavior for real LSD solve paths.

Add a targeted error variant for unsupported order values:

```rust
DecodeError::UnsupportedLsdOrder { order: usize }
```

Update `Display`, `rbposd/tests/smoke.rs`, and parity dev error-code mapping if touched by tests. Do not use `NoOsdSolution` or a broad LSD failure error for unsupported order.

## Testing Strategy

### Public API Contract Tests

Add `rbposd/tests/lsd.rs`.

Positive test:

```rust
bplsddecoder_public_api_matches_reference_contract
```

This test should:

- import `BpLsdDecoder`, `ChannelModel`, `LsdConfig`, `ParityCheckMatrix`, and `Syndrome` from the crate root
- construct a small matrix and syndrome
- construct `BpLsdDecoder::new(...)`
- call `decode(&syndrome)`
- assert `pcm.multiply(&result.correction) == syndrome`

Negative test:

```rust
bplsddecoder_rejects_channel_length_mismatch
```

This test should:

- construct a matrix with `num_bits > probabilities.len()`
- use `ChannelModel::BitFlipProbabilities`
- assert construction returns `DecodeError::DimensionMismatch` with `what: "channel probabilities"`

### Config And Export Tests

Update `rbposd/tests/smoke.rs` to cover:

- `LsdConfig::default().method`
- `LsdConfig::default().lsd_order == 0`
- public use of `LsdMethod`
- `DecodeError::UnsupportedLsdOrder` display text

Update `rbposd/tests/reference.rs` so the crate-level API surface documentation check includes `BpLsdDecoder` and the LSD config types.

### Compatibility Tests

Keep existing `BpOsdDecoder` tests passing unchanged. Add no migration requirement for OSD users.

### Documentation Tests

Update `rbposd/doc/ldpc_mvp_reference.md` or add a follow-on section in it covering:

- `BpLsdDecoder`
- `LsdConfig`
- `LsdMethod`
- supported #88 behavior
- explicit handoff to #89/#90 for full LSD solving and fixtures

## Verification Commands

Implementation should verify:

```bash
cargo test -p rbposd bplsddecoder_public_api_matches_reference_contract
cargo test -p rbposd bplsddecoder_rejects_channel_length_mismatch
cargo test -p rbposd
```

The full `cargo test -p rbposd` run must include coverage for `LsdConfig::default()` and `DecodeError::UnsupportedLsdOrder` display text.

## Risks And Mitigations

### Risk: Public LSD API Looks More Complete Than It Is

Mitigation:

- document #88 as API contract plus order-0 fallback only
- avoid adding fixture catalog or differential claims in #88
- cite #89/#90 as the real LSD algorithm milestones

### Risk: BP Logic Is Duplicated

Mitigation:

- extract only small shared support into `decoder_core.rs`
- have both OSD and LSD decoders call that support
- leave the larger shared-workspace refactor to #90

### Risk: `DecodeResult::used_osd` Is Confusing For LSD

Mitigation:

- keep `DecodeResult` unchanged in #88
- set `used_osd = false` for `BpLsdDecoder`
- document that richer decoder-family diagnostics are a future extension

### Risk: Scope Leaks Into `rsinter`

Mitigation:

- do not modify `rsinter` in #88
- keep LSD runner params, DEM adapter selection, and result-row recording for #91-#93

## Acceptance Criteria

- `rbposd` exports `BpLsdDecoder`, `LsdConfig`, and `LsdMethod`.
- Existing `BpOsdDecoder` public API remains source-compatible.
- `BpLsdDecoder::new(...)` validates channel dimensions and probabilities using the same error semantics as `BpOsdDecoder`.
- `BpLsdDecoder::decode(&Syndrome)` can decode a small known syndrome to a correction satisfying `H * correction == syndrome`.
- Documentation explains that #88 is not the full LSD algorithm milestone.
- Focused tests and full `cargo test -p rbposd` pass.
