# APM Delta/Gamma Active Row Sets Design

Scope: GitHub issue #137, crate-private Delta/Gamma active-row set construction
for the APM-CSS Table A1 parameters in `qec-code`.

## Context

Issue #132 added the checked Table A1 manifest for `apm_kasai:p=96` and
`apm_kasai:p=192`. Issue #134 added `qec-code/doc/apm_css.md` as the local
construction contract. Issue #136 added the crate-private affine commutation
primitive in `qec-code/src/codes/apm.rs`.

This issue builds the next small layer: deterministic construction of the
active row sets

```text
Delta = { (k - i) mod L2 | i,k in [0,J-1] }
Gamma = { (i,j) | (i + j) mod L2 in Delta }
```

where `L2 = L / 2`. It must not build `Hx` or `Hz`.

The local `drafts/construct_apm_css_code` reference clone named by the issue is
not present in this worktree. The implementation therefore uses the issue text,
the merged manifest, and the existing crate docs as its source of truth.

## Chosen Approach

Add a small crate-private helper beside the existing APM affine helpers in
`qec-code/src/codes/apm.rs`:

- `ApmActiveRowSets { delta: Vec<usize>, gamma: Vec<(usize, usize)> }`
- `ApmActiveRowSetError`
- `build_apm_active_row_sets(j: usize, l: usize) -> Result<ApmActiveRowSets, ApmActiveRowSetError>`

The helper validates inputs before constructing any sets:

- `L` must be even.
- `L2 = L / 2` must be greater than zero.
- `J` must be greater than zero.
- `J` must be `<= L / 2`.

The helper returns sorted `delta` values and lexicographically sorted `gamma`
pairs. For `J=3,L=12,L2=6`, it returns:

```text
Delta = [0, 1, 2, 4, 5]
Gamma = [
  (0,0), (0,1), (0,2), (0,4), (0,5),
  (1,0), (1,1), (1,3), (1,4), (1,5),
  (2,0), (2,2), (2,3), (2,4), (2,5),
  (3,1), (3,2), (3,3), (3,4), (3,5),
  (4,0), (4,1), (4,2), (4,3), (4,4),
  (5,0), (5,1), (5,2), (5,3), (5,5)
]
```

Also update `qec-code/doc/apm_css.md` so its Delta definition uses `mod L2`
instead of `mod L`, matching #137.

## Alternatives Considered

1. Keep Delta/Gamma construction only inside a unit test.

   This would satisfy one regression test but would not provide the sorted
   vectors that the later commutation validator and matrix generator need.

2. Expose a public APM active-set API.

   This would create public compatibility surface before the generator exists.
   Existing APM helpers are crate-private, so this is premature.

3. Add a crate-private helper in `qec-code/src/codes/apm.rs`.

   This is the selected approach. It follows the existing APM helper boundary,
   keeps ordering deterministic, and gives later crate code one reusable source
   of truth.

## Data Flow

`build_apm_active_row_sets` derives `l2 = l / 2` after validating that `l` is
even. It fills a `BTreeSet<usize>` with `(k + l2 - i) % l2` for
`i,k in 0..j`, then copies the sorted values into `delta`. It then scans
`left,right in 0..l2` and appends `(left, right)` whenever
`(left + right) % l2` is in the Delta set. This scan order is the returned
Gamma order.

The `apm_delta_gamma_matches_kasai_reference` unit test loads the P=96 manifest
entry, constructs the active sets from manifest `J` and `L`, asserts the exact
vectors above, and creates one `AffineCommutationCheck` for every generated
Gamma pair. It validates those checks with the existing #136
`validate_affine_commutation_checks` helper over the full P=96 modulus.

## Error Handling

`ApmActiveRowSetError` variants name the invalid parameter and format concise
messages. The negative control required by #137 expects the `J=4,L=6` error to
state that `J` must be `<= L/2`.

## Testing

Add module tests in `qec-code/src/codes/apm.rs`:

- `apm_delta_gamma_matches_kasai_reference` asserts the exact Delta/Gamma
  vectors for `J=3,L=12`, sweeps every generated Gamma pair through the #136
  commutation validator for `apm_kasai:p=96`, and checks the required negative
  control `J=4,L=6`.
- `apm_active_row_sets_reject_invalid_parameters` covers the other early input
  validation branches.

Focused verification:

```sh
cargo test -p qec-code apm_delta_gamma_matches_kasai_reference -q
```

Broad verification:

```sh
cargo test
```
