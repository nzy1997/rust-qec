# Lifted-Product CSS Design

## Context

Issue #561 asks `qec-code` to construct lifted-product CSS codes from finite
group-algebra protographs. The repository already has:

- validated finite group tables and left/right regular lifts in
  `qec-code/src/finite_group.rs`
- sparse GF(2) identity, transpose, Kronecker, and horizontal concat helpers in
  `qec-code/src/sparse_gf2.rs`
- the common CSS construction contract and hypergraph-product constructor in
  `qec-code/src/family_contract.rs`
- CLI export through `code css construct --spec ... hx|hz|metadata`

The issue is scoped to deterministic Rust API and CLI construction through that
common contract, with focused fixtures for `C3` and a trivial-group regression
against the ordinary HGP constructor from #556.

## Approaches Considered

1. Extend the common CSS construction contract with a `lifted_product`
   construction. Recommended: this reuses the existing JSON adapter, metadata
   shape, CLI path, orthogonality checks, ranks, and canonical sparse rows.
2. Add a standalone lifted-product module and separate CLI subcommand. This
   would isolate the code but duplicate construction output, metadata, and CLI
   behavior already standardized in `family_contract`.
3. Build only binary lifted matrices from protographs without retaining the
   ring-level chain complex. This is smaller but does not satisfy the
   ring-level `6 x 13` acceptance criterion and makes transpose/inversion less
   visible.

The chosen design is approach 1.

## Design

Add serializable finite-group and group-algebra protograph specs to the public
contract:

- `FiniteGroupTableSpec { order, identity, multiplication_table }`
- `GroupAlgebraElementSpec { support }`
- `GroupAlgebraProtographSpec { rows }`
- `LiftedProductSpec { group, left, right }`

The adapter validates the group with `FiniteGroupSpec::new`, converts every
entry into `GroupAlgebraElement`, and rejects ragged protographs, malformed
group supports, missing inverses, and overflow through the existing typed
errors. The JSON form is deterministic and compact:

```json
{
  "schema_version": 1,
  "construction": "lifted_product",
  "group": {
    "order": 3,
    "identity": 0,
    "multiplication_table": [[0,1,2],[1,2,0],[2,0,1]]
  },
  "left": {
    "rows": [
      [{"support":[1,2]},{"support":[0]},{"support":[]}],
      [{"support":[]},{"support":[0,1]},{"support":[1]}]
    ]
  },
  "right": {
    "rows": [
      [{"support":[1,2]},{"support":[0]},{"support":[]}],
      [{"support":[]},{"support":[0,1]},{"support":[1]}]
    ]
  }
}
```

Implement ring-level lifted-product checks over the group algebra using the
ordinary HGP block formula:

- `H_X = [A kron I_{n_B} | I_{m_A} kron B^T]`
- `H_Z = [I_{n_A} kron B | A^T kron I_{m_B}]`

Here transpose means both matrix transpose and group inversion for each support
element. Identity entries are the group identity singleton and zero entries are
empty supports. The C3 fixture has ring-level shape `6 x 13` on both `H_X` and
`H_Z`.

Binary lifting uses the existing left regular lift. This matches the finite
group lift fixture from #557 and preserves the expected leading rows:

- `H_X[0] = [1, 2, 9, 28, 29]`
- `H_Z[0] = [1, 2, 3, 28, 29]`

The common `construction_result` remains responsible for canonical sparse rows,
orthogonality verification, ranks, `k`, and deterministic provenance digest.
The `C3` fixture supplies known distances `(3, 3)` so metadata returns
`d_x = 3` and `d_z = 3`; other lifted-product inputs leave distances unset.

## API And CLI

Rust callers can use `CssConstructionSpec::LiftedProduct(LiftedProductSpec)`
and `construct_css`. JSON callers can use `code css construct --spec <path>
hx|hz|metadata`, the same CLI path used by HGP. No separate subcommand is
needed.

Normalized parameters store canonical group data and canonical protograph
supports. This gives deterministic JSON metadata and stable provenance hashes.

## Error Handling

The constructor rejects:

- invalid finite group tables, including missing inverses, via
  `FiniteGroupSpec::new`
- out-of-range group-algebra supports via `GroupAlgebraElement::new`
- ragged protograph rows via `GroupAlgebraMatrixRowWidthMismatch`
- zero-width protographs and incompatible shapes via
  `InvalidCssConstruction`
- lift-dimension overflow via existing group-algebra and sparse GF(2) overflow
  errors

## Testing

Add `qec-code/tests/lifted_product.rs` with three required tests:

- `lifted_product_c3_matches_fixture`
- `lifted_product_trivial_group_matches_hgp`
- `lifted_product_rejects_malformed_protographs`

The C3 test stores complete reviewed expected `H_X` and `H_Z` rows, checks the
ring-level shape, verifies the leading rows, checks stats
`n=39, m_x=18, m_z=18, rank_x=18, rank_z=18, k=3, d_x=3, d_z=3`, exercises the
JSON CLI path, and verifies orthogonality after the binary lift. The trivial
group test compares byte-identical canonical rows and metadata-equivalent
dimensions with the ordinary HGP constructor.

## Self-Review

- Placeholder scan: no TBD, TODO, or unresolved fields.
- Internal consistency: the ring formula, binary lift, fixture rows, and CLI
  route all use the existing common contract.
- Scope check: this is one focused implementation in `qec-code`.
- Ambiguity check: distances are known only for the C3 fixture; other
  lifted-product constructions leave distances unset.
