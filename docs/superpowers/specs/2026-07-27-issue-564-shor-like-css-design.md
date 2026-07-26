# Issue 564 Shor-Like CSS Family Design

## Context

Issue #564 adds the requested-family constructor for rectangular generalized Shor-like CSS codes. Issue #553 is already merged, so `qec-code/src/family_contract.rs` is the public boundary for typed CSS construction, deterministic metadata, sparse-row canonicalization, rank/stat calculation, orthogonality verification, and versioned JSON parsing.

No repository `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, or `CONVENTIONS.md` file is present in this worktree. The implementation follows the existing `family_contract.rs`, `surface_family.rs`, and `family_contract.rs` test patterns.

## Requirements

- Add a typed `ShorLikeSpec` with `outer_blocks >= 2` and `inner_block >= 2`.
- Construct sparse checks directly:
  - `n = outer_blocks * inner_block`
  - X rows compare adjacent outer repetition blocks, so row `b` contains all qubits in blocks `b` and `b + 1`.
  - Z rows are adjacent pairs inside each inner repetition block.
- Expose the constructor through `CssFamilySpec::ShorLike`, `construct_css`, and versioned JSON `{"schema_version":1,"construction":"shor_like",...}`.
- Preserve the common family contract: canonical sparse rows, orthogonality verification, deterministic normalized metadata, `requested_family_id = Some(ShorLike)`, `construction_id = "shor_like"`, `k = 1`, and distances `d_x = inner_block`, `d_z = outer_blocks`.
- Add CLI coverage through the existing `code css construct --spec <path> <matrix>` route.
- Reject missing dimensions, dimensions below 2, zero dimensions, and multiplication overflow with typed `InvalidCssConstruction` errors.

## Approaches Considered

1. Extend the common family contract directly.
   This matches the issue and #553 architecture. It keeps Rust API and CLI behavior on one route and reuses existing metadata/stat validation.

2. Add Shor-like as a legacy built-in CSS family.
   This would make inline export simple, but it would not satisfy the requested typed `CssFamilySpec` API without extra adapters and would blur legacy-vs-requested-family identity.

3. Build it as a generic hypergraph-product alias.
   This would reuse generic construction machinery but would not expose the requested Shor-like parameters or exact sparse-row ordering directly.

Chosen approach: extend the common family contract directly. The design is issue-scoped and avoids adding an inline compact syntax unless a future issue asks for one.

## API Design

Add:

```rust
pub struct ShorLikeSpec {
    pub outer_blocks: usize,
    pub inner_block: usize,
}
```

Extend:

```rust
pub enum CssFamilySpec {
    Surface(SurfaceFamilySpec),
    QuantumTanner(QuantumTannerSpec),
    ShorLike(ShorLikeSpec),
}
```

`CssFamilySpec::callable_requested_family_ids()` will include `Surface`, `QuantumTanner`, and `ShorLike`.

`construct_css(CssFamilySpec::ShorLike(spec).into())` returns the common `CssConstructionResult` with deterministic normalized parameters:

```json
{"inner_block": <usize>, "outer_blocks": <usize>}
```

## Check Construction

Qubit index `block * inner_block + offset` is row-major by outer block.

X checks:

```text
for block in 0..outer_blocks - 1:
  support = block qubits followed by next block qubits
```

Z checks:

```text
for block in 0..outer_blocks:
  for offset in 0..inner_block - 1:
    support = [block * inner_block + offset, block * inner_block + offset + 1]
```

For `outer_blocks = 3` and `inner_block = 3`, this gives exactly:

```text
H_X = [[0,1,2,3,4,5], [3,4,5,6,7,8]]
H_Z = [[0,1], [1,2], [3,4], [4,5], [6,7], [7,8]]
```

The constructor sets known distances as `(d_x, d_z) = (inner_block, outer_blocks)`. The common `CssCodeStats` then reports the code distance as `min(d_x, d_z)` in tests by checking both values.

## JSON And CLI

`parse_css_construction_json` accepts:

```json
{
  "schema_version": 1,
  "construction": "shor_like",
  "outer_blocks": 3,
  "inner_block": 3
}
```

The existing `code css construct --spec <path> hx|hz` CLI path already lowers JSON through `parse_css_construction_json` and `construct_css`, so no new CLI subcommand is needed.

## Testing

Add `qec-code/tests/shor_like.rs` with the issue-named tests:

- `shor_like_3x3_matches_fixture`
- `shor_like_rectangular_3x4_has_expected_parameters`
- `shor_like_rejects_invalid_dimensions`

The tests cover exact rows, rank/stat values, `k = 1`, distance `min(outer_blocks, inner_block)`, orthogonality, deterministic metadata, Rust API construction, JSON parsing, CLI export, missing fields, dimensions below 2, zero dimensions, and multiplication overflow.

## Self-Review

- Completion-marker scan: no unfinished markers remain.
- Consistency: API, JSON, CLI, metadata, and tests all use `outer_blocks` and `inner_block`.
- Scope: the work is limited to the common family contract and focused tests.
- Ambiguity: no inline syntax is added because the issue only requires common-contract API and CLI JSON routing.
