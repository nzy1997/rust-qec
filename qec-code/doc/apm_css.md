# APM-CSS Construction Contract

This note is the implementation contract for the APM-CSS Table A1 fixtures from
arXiv:2604.16209. It translates the paper and the Kasai reference-code
vocabulary into the data model used by `qec-code`.

## Fixture Scope

- Source manifest: `qec-code/tests/fixtures/apm/table_a1_manifest.json` from
  <https://github.com/nzy1997/rust-qec/issues/132>.
- Known-answer sparse fixture target:
  <https://github.com/nzy1997/rust-qec/issues/133>.
- Paper source: <https://arxiv.org/abs/2604.16209>, Appendix A and Table A1.
- Construction background: <https://arxiv.org/abs/2601.08824>, active
  orthogonality and affine permutation construction.
- Reference-code paths when available locally:
  `drafts/construct_apm_css_code/README.md` and
  `drafts/construct_apm_css_code/apm_g8_mod.cpp`.

## Searcher Integration

Future APM searcher work is tracked separately in
[`apm_searcher_integration.md`](qec-code/doc/apm_searcher_integration.md). The searcher
roadmap preserves the fixed Table A1 built-ins as the production path and starts
future integration with manifest import validation before any wrapper or native
searcher work.

## Data Model

Use `AffineMap { a, b, modulus }` for every affine permutation:

```rust
struct AffineMap {
    a: u64,
    b: u64,
    modulus: u64,
}
```

It represents `x -> a*x + b (mod modulus)`. A map is valid only when
`gcd(a, modulus) == 1`; a non-unit slope such as `{ a: 2, b: 0, modulus: 96 }`
must be rejected.

Manifest `f_i=(a_i,b_i)` entries map directly to `AffineMap`. Manifest
`g_i=(c_i,d_i)` entries map to the same struct with `a=c_i` and `b=d_i`.

## Shape

For the checked Table A1 instances, `J=3`, `L=12`, and `L2=L/2=6`.
The active matrices use the top `J` block rows from the parent block-circulant
template, so each side has `J*P` active check rows and `L*P` data columns:

```text
n  = L * P
mx = J * P
mz = J * P
```

For `P=96,J=3,L=12`, this gives `n=1152` and `mx=mz=288`. For `P=192`, this
gives `n=2304` and `mx=mz=576`.

## Delta And Gamma

Let the active block-row set be `A = {0, 1, ..., J-1}` for the standard top-row
choice. The active difference set is:

```text
Delta = { (r - s) mod L2 | r in A, s in A }
```

`Gamma` is the sorted set of affine block pairs whose indices sum to an active
difference:

```text
Gamma = { (i, j) | (i + j) mod L2 in Delta }
```

Every generated Gamma pair must commute for the APM-CSS construction. The
manifest `required_commuting_pairs` field is a smaller pinned subset of
column-component constraints from earlier fixture issues, not the full
generated Gamma set.

The latent/noncommuting controls are the manifest
`required_noncommuting_pairs`; for Table A1 they are interpreted as
`f[left_index]` against `g[right_index]` over the full modulus `P`.

## Commutation Residual

For two affine maps `u(x)=a*x+b` and `v(x)=c*x+d` over the same modulus `M`,
the maps commute exactly when `u(v(x)) == v(u(x))`. The linear terms match
automatically, so the implementation residual is:

```text
residual(u, v) = (a*d + b - c*b - d) mod M
```

The maps commute iff `residual == 0`.

For the P=96 manifest entry, `required_commuting_pairs[0]` is a Gamma pair
checked modulo `32` and has residual zero. A documented noncommuting control is
`f0` against `g3` over modulus `96`; its residual is nonzero.

## See Also

- [QEC-Code CSS Construction showcase](docs/showcases/qec-code-css-construction.md)
- [Showcase index](docs/showcases/README.md)

## Sparse-Row Output Contract

The future generator should emit `SparseRowsMatrix` JSON compatible with
`qec-code/src/css.rs`:

- `Hx.num_cols == Hz.num_cols == n`
- `Hx.rows.len() == mx`
- `Hz.rows.len() == mz`
- every row support is sorted, unique, and in range
- for the Table A1 fixtures, every row has weight `L=12`
- every data column has X degree `J=3` and Z degree `J=3`
- `Hx * Hz^T == 0 mod 2`

The P=96 known-answer matrices belong to #133; this note does not generate or
pin them.

## Validation Checklist

- Parse every `f` and `g` entry as an affine map and reject non-unit slopes.
- Check `L2 == L/2`.
- Check `n`, `mx`, and `mz` from `P`, `J`, and `L`.
- Check every manifest Gamma pair has zero residual under its documented
  modulus.
- Check at least one manifest noncommuting pair has nonzero residual under the
  full `P`.
- Keep distance values as upper-bound metadata from Table A1, not exact
  minimum-distance claims.
