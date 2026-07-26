# Issue 556 Hypergraph Product CSS Design

Date: 2026-07-27
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #556, Roadmap ID M1-05

## Summary

Issue #556 upgrades the generic hypergraph-product route introduced by the CSS
family contract into a complete constructor from two explicit classical binary
parity-check matrices. The constructor must accept independent rectangular
matrices, build the standard CSS checks, validate dimensions and supports before
construction, and return normalized construction metadata through the existing
`CssConstructionResult` surface.

The selected design keeps the public entry point as
`CssConstructionSpec::HypergraphProduct(HypergraphProductSpec)`, represents each
classical input with `CssClassicalCheckSpec`, lowers those inputs to
`sparse_gf2::SparseGf2Matrix`, and constructs:

```text
H_X = [H_1 tensor I_n2 | I_m1 tensor H_2^T]
H_Z = [I_n1 tensor H_2 | H_1^T tensor I_m2]
```

using the shared sparse GF(2) identity, transpose, Kronecker product, and
horizontal concatenation primitives.

## Existing Context

- `qec-code/src/family_contract.rs` owns `CssConstructionSpec`,
  `HypergraphProductSpec`, normalized metadata, canonical sparse checks, CSS
  orthogonality verification, rank calculation, and JSON construction parsing.
- `qec-code/src/sparse_gf2.rs` owns checked sparse GF(2) composition primitives
  from dependency issue #554.
- `qec-code/src/css.rs` owns the existing sparse-row JSON contract and dense CSS
  conversion used by distance tests.
- `qec-code/src/cli.rs` already supports `code css construct --spec <path> hx`
  and `hz`; it does not yet expose construction metadata directly.
- The issue's referenced `docs/design/2026-07-26-qec-code-family-support.md` is
  absent in this worktree, so the implemented dependency specs for #553 and
  #554 are the local design source of truth.

## Approaches Considered

### 1. Upgrade the existing contract route - selected

Keep `CssConstructionSpec::HypergraphProduct` as the Rust API and structured
JSON route, but replace the hand-built row arithmetic with sparse GF(2)
composition primitives. Add fixture tests for the exact 2 by 3 matrices and add
a `metadata` CLI output selector that serializes the same normalized
`CssConstructionResult`.

Benefits:

- preserves the existing contract and JSON shape from #553
- uses the checked sparse GF(2) primitives required by #554
- avoids a duplicate constructor API
- keeps CLI matrix exports backward compatible
- exposes normalized metadata without changing legacy `hx` and `hz` output

Cost:

- `code css construct --spec <path> metadata` is a new selector on an existing
  command, so tests must cover it explicitly.

### 2. Add a separate `codes::hypergraph_product` module

Move the constructor out of `family_contract.rs` into a dedicated module and
have the contract call it.

Benefits:

- clearer long-term module boundary if many HGP variants are added later

Costs:

- more churn for one constructor
- duplicates nearby normalization helpers unless additional refactoring is done

This is not selected for the issue-sized change.

### 3. Add only CLI matrix file flags

Add a standalone CLI command that accepts two classical matrix files and emits
checks, without changing the typed contract route.

Benefits:

- direct CLI affordance for users

Costs:

- bypasses the existing normalized construction contract
- does not improve the Rust API path
- risks a second input format before the shared construction JSON has been fully
  exercised

This is not selected.

## Public Contract

`CssClassicalCheckSpec` remains:

```rust
pub struct CssClassicalCheckSpec {
    pub num_cols: usize,
    pub rows: Vec<Vec<usize>>,
}
```

The row count is `rows.len()`, so two independent rectangular matrices are
accepted without additional fields. Each input is lowered to
`SparseGf2Matrix::new(rows.len(), num_cols, rows)` before any product blocks are
constructed. A support equal to `num_cols`, such as support `3` in a matrix with
`num_cols = 3`, returns `QecError::SparseGf2SupportOutOfRange` before CSS
construction.

For left shape `m1 x n1` and right shape `m2 x n2`, the constructor returns:

- data qubits: `n1 * n2 + m1 * m2`
- X checks: `m1 * n2`
- Z checks: `n1 * m2`
- `construction_id = "hypergraph_product"`
- `requested_family_id = None`
- normalized parameters containing canonical `left` and `right` classical
  matrices
- canonical sparse `h_x` and `h_z` rows
- shared stats from `construction_result`
- `d_x` and `d_z` left as `None` because generic HGP distance is not known from
  dimensions alone

The exact issue fixture computes distance 3 in tests by converting the returned
checks to `CssCode` and using the existing exact distance routine.

## CLI Contract

Existing matrix exports remain unchanged:

```text
qec-code code css construct --spec <spec.json> hx
qec-code code css construct --spec <spec.json> hz
```

The command gains a third output selector:

```text
qec-code code css construct --spec <spec.json> metadata
```

`metadata` serializes the full `CssConstructionResult` as compact JSON. This
gives the CLI the same normalized construction metadata as the Rust API while
leaving existing sparse-row matrix JSON outputs byte stable.

## Error Handling

The constructor must rely on typed sparse GF(2) errors for shape and arithmetic
failures:

- `SparseGf2SupportOutOfRange` for out-of-range classical supports
- `SparseGf2DimensionOverflow` for checked multiplication, addition, identity,
  transpose, Kronecker, or horizontal concatenation arithmetic failures
- `SparseGf2HorizontalRowMismatch` if a product block shape bug creates
  incompatible row counts
- `InvalidCssConstruction` only for an impossible internal mismatch between the
  final `H_X` and `H_Z` column counts

The shared contract still verifies canonical sparse rows and CSS
orthogonality before returning a result.

## Testing

Add `qec-code/tests/hypergraph_product.rs` with the issue-required exact tests:

- `hypergraph_product_matches_2x3_fixture`
- `hypergraph_product_rejects_out_of_range_input`

The fixture test must assert:

- exact `H_X` and `H_Z` rows after canonicalization
- `n=13`, `m_x=6`, `m_z=6`, `rank_x=6`, `rank_z=6`, and `k=1`
- construction identity and normalized input metadata
- orthogonality through `verify_css_orthogonality`
- exact distance 3 through `CssCode` and `compute_distance`
- CLI `hx`, `hz`, and `metadata` output for the same explicit JSON input

The negative control must assert that support `3` in a `num_cols = 3` matrix
returns `QecError::SparseGf2SupportOutOfRange` before construction.
