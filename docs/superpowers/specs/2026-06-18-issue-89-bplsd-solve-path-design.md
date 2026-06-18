# Issue 89 BpLsd Solve Path Design

Date: 2026-06-18
Status: Proposed
Scope: GitHub issue #89, `rbposd` internal LSD solve path, minimal Rust-side LSD fixtures, and focused tests

## Summary

Issue #89 turns `BpLsdDecoder` from an order-0 validity bridge into a decoder
with one real deterministic LSD post-BP solve path.

The implementation should stay inside `rbposd`. It should keep
`BpLsdDecoder` as the public entrypoint, add an internal `rbposd/src/lsd.rs`
module for LSD-specific state and candidate selection, enable `lsd_order = 1`,
and add a clear LSD failure error when no valid localized solution is found.

This issue also checks in a minimal Rust-side LSD fixture set with the exact
case ids named by the issue:

- `lsd_small_sparse_code.json`
- `lsd_order_one_improves_over_baseline.json`
- `lsd_unsatisfiable_case.json`

The fixture manifest and Python differential harness remain follow-up work for
#90 and #98.

## Goals

- Implement the first supported nonzero-order LSD path behind `BpLsdDecoder`.
- Keep OSD and LSD post-BP logic separate.
- Preserve existing `BpLsdDecoder` construction and decode shape.
- Keep `LsdMethod::LocalizedStatistics` as the only public LSD method variant.
- Support `lsd_order = 0` and `lsd_order = 1`.
- Reject `lsd_order > 1` deterministically.
- Return an LSD-specific decoder error when localized solving cannot satisfy the target syndrome.
- Add minimal checked-in LSD JSON fixtures and focused Rust tests.

## Non-Goals

- Do not modify `rsinter`, DEM adapters, benchmark runner params, result rows, or benchmark specs.
- Do not add a fixture manifest or fixture catalog validation.
- Do not extend the Python parity harness or add upstream `ldpc` differential plumbing.
- Do not implement additional BP methods, schedules, `bits_per_step`, or `always_run_lsd`.
- Do not add new public LSD method variants.
- Do not change the public shape of `DecodeResult`.

## Current Context

#88 is complete. `rbposd` now exports:

- `BpLsdDecoder`
- `LsdConfig`
- `LsdMethod`

The current `BpLsdDecoder` runs existing `BpCore` and, when BP leaves a
nonzero residual, uses `LsdFallbackWorkspace` in `rbposd/src/lsd_decoder.rs`.
That workspace is a global reliability-ordered GF(2) residual solve. It is a
valid order-0 bridge, but it is not a localized LSD algorithm.

The current constructor rejects every nonzero `lsd_order` with
`DecodeError::UnsupportedLsdOrder { order }`. #89 should make order 1 usable
and keep higher orders unsupported.

The installed upstream `ldpc==2.4.1` package exposes `BpLsdDecoder` and a
standalone `LsdDecoder`. Its public wrapper contract is useful as a reference:
LSD consumes a syndrome plus bit weights, grows localized clusters guided by
soft information, and applies a per-cluster order parameter. This design uses
that boundary, but does not copy the upstream API surface wholesale.

## Alternatives Considered

### 1. Minimal Rust-Side Fixtures With Internal LSD Module

Add `rbposd/src/lsd.rs`, move LSD-specific post-BP work there, enable order 1,
and check in the exact fixture ids that #89 names. Keep the fixtures small and
test-local.

Benefits:

- Satisfies #89's fixture-id acceptance language.
- Keeps #89 focused on the decoder behavior.
- Avoids pulling #90/#98 fixture infrastructure into this milestone.
- Makes LSD logic reviewable without mixing it into OSD.

Cost:

- The initial fixture schema is intentionally narrow and will be superseded by
  the later manifest work.

This is the chosen approach.

### 2. Hard-Code The Cases In Rust Tests

Implement the algorithm and encode the two positive cases directly in
`rbposd/tests/lsd.rs`.

Benefits:

- Smallest diff.
- No temporary JSON schema.

Costs:

- Does not really satisfy the issue's "fixture set includes exact case ids"
  wording.
- Gives #90/#98 less concrete input to catalog later.

This is rejected.

### 3. Build The Full Fixture Manifest And Differential Harness Now

Implement the algorithm plus manifest validation and Python `ldpc` comparison.

Benefits:

- Strongest cross-runtime evidence.
- Reduces later fixture-harness work.

Costs:

- Duplicates the scope of #90 and #98.
- Makes a decoder-core milestone depend on test-infrastructure work.

This is rejected.

## Public API Contract

No new public type is required.

`LsdConfig` keeps the existing fields:

```rust
pub struct LsdConfig {
    pub method: LsdMethod,
    pub lsd_order: usize,
}
```

Supported #89 behavior:

- `method = LsdMethod::LocalizedStatistics` remains the only accepted method.
- `lsd_order = 0` keeps the order-0 residual solve behavior established by #88.
- `lsd_order = 1` runs the first localized LSD solve path.
- `lsd_order > 1` returns `DecodeError::UnsupportedLsdOrder { order }`.

Update the unsupported-order display text to say that only orders 0 and 1 are
supported.

## Internal Architecture

### `rbposd/src/lsd.rs`

Add an internal module for LSD-specific work.

Responsibilities:

- own reusable LSD scratch state
- compute localized residual candidates from a target syndrome
- keep cluster state separate from OSD workspaces
- apply deterministic candidate ordering and tie-breaking
- return `NoLsdSolution` when all localized candidates fail

Primary internal API:

```rust
pub(crate) struct LsdWorkspace { ... }

pub(crate) fn decode_lsd_with_workspace(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
    lsd_order: usize,
    workspace: &mut LsdWorkspace,
) -> Result<Correction, DecodeError>;
```

`LsdWorkspace` should own:

- a reusable `PreparedLinearSystem`
- column-order scratch
- cluster scratch
- candidate correction buffers

The module may reuse `PreparedLinearSystem` from `gf2.rs`, but it should not use
`OsdWorkspace` or `decode_osd_with_workspace`.

### `BpLsdDecoder`

`BpLsdDecoder` remains in `rbposd/src/lsd_decoder.rs` and continues to own the
public decode flow.

Changes:

- replace `LsdFallbackWorkspace` with `Mutex<LsdWorkspace>`
- accept `lsd_order = 1` in `new(...)`
- reject only `lsd_order > 1`
- call `decode_lsd_with_workspace(...)` after a nonzero BP residual
- keep `used_osd = false`

### `DecodeError`

Add:

```rust
DecodeError::NoLsdSolution
```

This error is used when LSD cannot produce a correction whose parity matches
the target syndrome.

## Decode Data Flow

`BpLsdDecoder::decode(&syndrome)` should:

1. Validate syndrome length.
2. Preserve the zero-syndrome prior fast path.
3. Run existing BP through `BpCore`.
4. Return the BP hard decision immediately if BP leaves residual weight zero.
5. Compute the residual target:

   ```text
   target_syndrome = H * bp_hard_decision XOR requested_syndrome
   ```

6. Run LSD on `target_syndrome`, BP reliability, and configured `lsd_order`.
7. XOR the returned residual correction with the BP hard decision.
8. Verify the final correction satisfies the requested syndrome.
9. Return `DecodeResult` with:
   - `used_osd = false`
   - BP convergence and iteration diagnostics from the BP run
   - `residual_syndrome_weight = 0` after successful LSD

If final verification fails, return `DecodeError::NoLsdSolution`.

## LSD Order Semantics

### Order 0

Order 0 preserves the #88 behavior: solve the full residual syndrome using a
reliability-ordered GF(2) column order. This remains the baseline and protects
existing tests.

### Order 1

Order 1 is the first localized solve path.

Algorithm sketch:

1. Initialize one active cluster for each unsatisfied check in the target syndrome.
2. Each cluster starts with:
   - one active check
   - the bits neighboring that check
3. Process clusters in deterministic order by their lowest check index.
4. For each active cluster:
   - build a local submatrix induced by the cluster's checks and bits
   - try a local order-0 solve for the cluster syndrome
   - if `lsd_order >= 1`, try candidates that force one free local column true
   - score candidates by weighted correction cost using BP reliability
   - break equal-cost ties by lexicographic correction bits
5. If a cluster cannot satisfy its local syndrome, grow it by adding the next
   frontier bit with the lowest reliability. Ties use the bit index.
6. Growing a cluster adds neighboring checks touched by the new bit.
7. If two clusters now share checks or bits, merge them deterministically.
8. Repeat until all active clusters are solved or no growth/candidate path remains.
9. Combine local residual corrections with XOR.
10. Verify the combined residual correction satisfies the full target syndrome.

This gives #89 one concrete localized path without exposing a broad tuning
surface. The internal growth step is effectively `bits_per_step = 1`; public
configuration for `bits_per_step` remains out of scope.

## Determinism

The solver must be deterministic for equal-cost choices.

Required tie-break rules:

- checks are considered by ascending index
- bit frontiers are sorted by `(reliability, bit_index)`
- candidates are ranked by `(weighted_cost, correction_bits)`
- merged clusters preserve sorted check and bit sets

No hash-map iteration order should affect the result.

## Error Handling

Reuse existing errors for existing validation:

- `DecodeError::DimensionMismatch` for syndrome or channel length mismatch
- `DecodeError::InvalidProbability` for invalid channel probabilities
- `DecodeError::UnsupportedLsdOrder { order }` for `lsd_order > 1`

Add `DecodeError::NoLsdSolution` for:

- local clusters that cannot be solved after exhausting growth
- full residual verification failure
- an unsatisfiable target syndrome

Internal `PreparedLinearSystem` failures are candidate failures inside LSD.
They should not leak as `SingularSystem` unless the failure occurs outside the
LSD candidate-search context.

Update stable parity error-code mapping so `NoLsdSolution` is visible through
the existing parity-dev error taxonomy.

## Fixture Design

Add `rbposd/tests/fixtures/lsd/`.

Use a minimal JSON schema local to `rbposd/tests/lsd.rs`:

```json
{
  "id": "lsd_small_sparse_code",
  "matrix": {
    "num_checks": 2,
    "num_bits": 3,
    "rows": [[0, 1], [1, 2]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.05
  },
  "syndrome": [true, false],
  "lsd_order": 1,
  "expected": {
    "status": "success"
  }
}
```

The fixture loader should validate only what the #89 tests need. It should not
become a manifest system.

Required fixtures:

### `lsd_small_sparse_code.json`

Small satisfiable sparse matrix case proving order 1 returns a correction with
exact target parity.

### `lsd_order_one_improves_over_baseline.json`

Small satisfiable case where order 1 produces a documented correction distinct
from order 0. The test should assert that at least this case is not silently
using the order-0 fallback result.

### `lsd_unsatisfiable_case.json`

Small inconsistent system proving the decoder returns `NoLsdSolution` instead
of emitting a fake correction.

## Testing Strategy

### Positive Fixture Test

Add:

```text
bplsd_order_one_recovers_the_borrowed_small_matrix_cases
```

This test should:

- load `lsd_small_sparse_code.json`
- load `lsd_order_one_improves_over_baseline.json`
- construct `BpLsdDecoder` with `LsdConfig { lsd_order: 1, ..Default::default() }`
- decode each syndrome
- assert `pcm.multiply(&result.correction) == syndrome`
- assert `used_osd == false`
- assert `residual_syndrome_weight == 0`
- assert the improves-over-baseline case differs from the order-0 correction

### Negative Fixture Test

Add:

```text
bplsd_returns_a_decoder_error_for_an_unsatisfiable_case
```

This test should:

- load `lsd_unsatisfiable_case.json`
- construct `BpLsdDecoder` with `lsd_order = 1`
- assert decode returns `DecodeError::NoLsdSolution`

### Config And Error Tests

Update the current nonzero-order rejection test to use `lsd_order = 2`.

Update smoke/parity-dev tests for:

- new unsupported-order display text
- `NoLsdSolution` display text
- stable `NoLsdSolution` parity error code

### Compatibility Tests

Keep existing `BpLsdDecoder` order-0 tests passing. Keep existing `BpOsdDecoder`
tests unchanged.

## Documentation

Update `rbposd/doc/ldpc_mvp_reference.md`:

- note that #89 supports `lsd_order = 0` and `lsd_order = 1`
- describe `NoLsdSolution`
- mention the minimal Rust-side LSD fixture set
- explicitly leave fixture manifests and Python differential harness coverage
  to #90/#98

## Verification Commands

Implementation should run:

```bash
cargo test -p rbposd bplsd_order_one_recovers_the_borrowed_small_matrix_cases
cargo test -p rbposd bplsd_returns_a_decoder_error_for_an_unsatisfiable_case
cargo test -p rbposd --test lsd
cargo test -p rbposd
```

The full `cargo test -p rbposd` run should include existing OSD, BP, parity-dev,
and reference documentation tests.

## Risks And Mitigations

### Risk: Order 1 Is Accidentally A Global OSD-Style Solve

Mitigation:

- keep LSD code in `lsd.rs`
- avoid `OsdWorkspace`
- add a fixture where order 1 differs from the order-0 baseline

### Risk: Fixture Work Expands Into #90/#98

Mitigation:

- use only a test-local JSON schema
- add no manifest
- add no Python harness mapping
- document the follow-up boundary

### Risk: Local Candidate Failures Leak As Generic GF(2) Errors

Mitigation:

- treat local linear-solve failures as candidate failures
- return `NoLsdSolution` only after the LSD path exhausts valid candidates

### Risk: Tie-Break Drift Causes Flaky Fixtures

Mitigation:

- define all ordering rules explicitly
- assert expected correction on the improves-over-baseline fixture
- avoid unordered container iteration in candidate selection

## Acceptance Criteria

- `BpLsdDecoder` accepts `lsd_order = 1`.
- `BpLsdDecoder` still accepts `lsd_order = 0`.
- `BpLsdDecoder` rejects `lsd_order > 1`.
- `rbposd/src/lsd.rs` owns LSD-specific post-BP logic.
- `DecodeError::NoLsdSolution` exists and is used for LSD failure.
- The required LSD fixture files are checked in under `rbposd/tests/fixtures/lsd/`.
- `bplsd_order_one_recovers_the_borrowed_small_matrix_cases` passes.
- `bplsd_returns_a_decoder_error_for_an_unsatisfiable_case` passes.
- Existing `rbposd` tests pass.
