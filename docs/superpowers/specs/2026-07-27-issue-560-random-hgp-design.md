# Issue 560 Random HGP Design

Date: 2026-07-27
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #560, Roadmap ID M2-04

## Summary

Add a deterministic random hypergraph-product CSS constructor to `qec-code`.
The constructor owns a `random_hgp` family specification containing two
independent deterministic regular-classical specifications. Each side is
sampled by the repository-owned version-1 regular classical sampler from issue
#555, then lowered into the general hypergraph-product constructor from issue
#556. The result is exposed through the existing common CSS family contract used
by both the Rust API and `code css construct --spec`.

The issue fixture fixes both classical sides to:

```text
column_count = 6
row_count = 4
column_weight = 2
row_weight = 3
seed = 7
algorithm_version = 1
```

Both sides must sample the canonical rows:

```text
[[0, 1, 2], [0, 3, 4], [1, 3, 5], [2, 4, 5]]
```

The HGP output has `n=52`, `m_x=24`, `m_z=24`, `rank_x=21`, `rank_z=21`, and
`k=10`. Every CSS check row has weight 5, all checks are orthogonal, repeated
construction serializes identically, and no distance is claimed.

## Goals

- Add `RandomHgpSpec` with `left` and `right` deterministic regular classical
  specs.
- Require `seed` and `algorithm_version` for each classical side in JSON input.
- Use only `deterministic_regular_matrix`; do not call an external RNG, algebra
  tool, or distance routine.
- Lower sampled classical rows into the existing HGP construction path.
- Expose `random_hgp` through `CssFamilySpec`, `construct_css`,
  `parse_css_construction_json`, `CssFamilySpec::callable_requested_family_ids`,
  and CLI `code css construct --spec`.
- Include normalized metadata for both requested classical specs, sampled rows,
  and sampler version.
- Reject missing seed, impossible degree sequences, unknown sampler versions,
  and retry-exhausted classical inputs.

## Non-Goals

- Do not compute, brute-force, estimate, or claim general distance for random
  HGP codes. `d_x` and `d_z` remain `None`.
- Do not change the deterministic regular-classical version-1 algorithm.
- Do not add a separate CLI command outside the common construction JSON route.
- Do not promote unrelated planned families or change the family manifest
  availability policy.

## Approaches Considered

### 1. Add a focused `codes::random_hgp` module

Create `qec-code/src/codes/random_hgp.rs` for the spec types, JSON parser,
sampler orchestration, sampled-matrix metadata, and conversion into
`HypergraphProductSpec`. `family_contract.rs` remains the common adapter that
sets construction identity, requested family identity, normalized metadata, and
stats.

This is the selected approach because it matches the neighboring
`random_two_block` pattern, keeps sampling details outside the already broad
family contract file, and still routes all public construction through the
common Rust and CLI contract.

### 2. Parse `random_hgp` directly as generic HGP JSON

Accept `construction = "random_hgp"`, sample the matrices inside
`parse_css_construction_json`, then return
`CssConstructionSpec::HypergraphProduct`.

This would reuse the HGP constructor, but it loses the requested family identity
and makes metadata look like an explicit HGP input rather than a random-family
construction. It is not selected.

### 3. Add a CLI-only fixture command

Add a narrow command that emits the seed-7 random-HGP fixture matrices.

This would satisfy only a golden-output smoke test. It would miss the Rust API
contract, duplicate validation outside the common path, and leave the family
unavailable to downstream consumers. It is not selected.

## Public API

Expose `pub mod random_hgp` from `qec-code/src/codes/mod.rs`.

The module defines:

```rust
pub struct RegularClassicalCodeSpec {
    pub column_count: usize,
    pub row_count: usize,
    pub column_weight: usize,
    pub row_weight: usize,
    pub seed: u64,
    pub algorithm_version: u32,
    pub retry_limit: usize,
}

pub struct RandomHgpSpec {
    pub left: RegularClassicalCodeSpec,
    pub right: RegularClassicalCodeSpec,
}

pub struct RandomHgpClassicalSample {
    pub spec: RegularClassicalCodeSpec,
    pub rows: Vec<Vec<usize>>,
}

pub struct RandomHgpClassicalSamples {
    pub left: RandomHgpClassicalSample,
    pub right: RandomHgpClassicalSample,
}

pub fn random_hgp_spec_from_json_str(input: &str) -> Result<RandomHgpSpec>;
pub fn sample_random_hgp_classical_matrices(
    spec: &RandomHgpSpec,
) -> Result<RandomHgpClassicalSamples>;
pub fn sampled_random_hgp_to_hgp_spec(
    samples: &RandomHgpClassicalSamples,
) -> HypergraphProductSpec;
```

`RandomHgpSpec::new` validates both sides by converting each
`RegularClassicalCodeSpec` into `RegularClassicalMatrixConfig` and invoking the
regular-classical validation through `deterministic_regular_matrix` during
sampling. JSON parsing treats `seed` as `Option<u64>` so a missing seed returns a
typed reproducibility error before sampling.

## JSON Contract

The common CSS construction JSON accepts:

```json
{
  "schema_version": 1,
  "construction": "random_hgp",
  "left": {
    "column_count": 6,
    "row_count": 4,
    "column_weight": 2,
    "row_weight": 3,
    "seed": 7,
    "algorithm_version": 1,
    "retry_limit": 16
  },
  "right": {
    "column_count": 6,
    "row_count": 4,
    "column_weight": 2,
    "row_weight": 3,
    "seed": 7,
    "algorithm_version": 1,
    "retry_limit": 16
  }
}
```

`retry_limit` is explicit in the family spec because the underlying
regular-classical sampler has a bounded retry contract. The issue fixture uses
the existing M1 fixture values and `retry_limit = 16`, matching the current
regular-classical tests.

## Common Contract Metadata

`construct_css(CssFamilySpec::RandomHgp(spec).into())` returns:

- `construction_id = "random_hgp"`
- `requested_family_id = Some(RequestedFamilyId::RandomHgp)`
- `adapter = "random_hgp"`
- `source = "CssFamilySpec::RandomHgp"`
- normalized parameters with `left` and `right`, each containing:
  - `classical_spec`: the normalized deterministic regular-classical spec
  - `rows`: the sampled canonical classical parity-check rows
  - `sampler_version`: the regular-classical `algorithm_version`
- CSS checks and stats from the same canonical sparse-row and rank machinery as
  every other common construction result
- `d_x = None` and `d_z = None`

The normalized parameter map uses `BTreeMap` and serializable structs so repeated
construction is byte-for-byte deterministic under `serde_json::to_string`.

## Matrix Construction

For each side, sample rows with:

```rust
deterministic_regular_matrix(RegularClassicalMatrixConfig {
    column_count,
    row_count,
    column_weight,
    row_weight,
    seed,
    algorithm_version,
    retry_limit,
})
```

Then lower to:

```rust
HypergraphProductSpec {
    left: CssClassicalCheckSpec {
        num_cols: left.spec.column_count,
        rows: left.rows.clone(),
    },
    right: CssClassicalCheckSpec {
        num_cols: right.spec.column_count,
        rows: right.rows.clone(),
    },
}
```

The HGP block construction itself remains the general constructor from issue
#556:

```text
H_X = [H_1 tensor I_n2 | I_m1 tensor H_2^T]
H_Z = [I_n1 tensor H_2 | H_1^T tensor I_m2]
```

For the seed-7 fixture, each sampled classical row has weight 3 and each sampled
classical column has weight 2. Therefore every generated CSS check row has
weight `3 + 2 = 5`.

## Errors

Add typed `QecError` variants:

```rust
InvalidRandomHgpSpec { option: &'static str, reason: String }
```

Unknown sampler versions continue to use
`UnsupportedRegularClassicalMatrixAlgorithm`. Impossible degree sequences and
retry exhaustion continue to use the typed regular-classical errors from issue
#555. Out-of-range or duplicate sampled rows are not expected from the sampler,
but HGP lowering still relies on `SparseGf2Matrix` validation.

JSON missing seed returns:

```rust
QecError::InvalidRandomHgpSpec {
    option: "seed",
    reason: "must be provided".to_owned(),
}
```

## Testing

Add `qec-code/tests/random_hgp.rs` with:

- `random_hgp_seed7_matches_fixture`
- `random_hgp_rejects_unreproducible_specs`

The fixture test asserts:

- direct sampler output for both sides matches the M1 seed-7 classical rows
- common construction returns `n=52`, `m_x=24`, `m_z=24`, `rank_x=21`,
  `rank_z=21`, and `k=10`
- `d_x` and `d_z` are `None`
- every X and Z check row has weight 5
- all checks are orthogonal
- repeated construction serializes byte-for-byte identically
- metadata contains both normalized classical specs, rows, and sampler version
- parsed JSON, direct Rust API, and CLI `code css construct --spec` agree

The negative test covers:

- missing seed in either side
- incompatible stub counts such as `column_count=5`, `column_weight=2`,
  `row_count=4`, `row_weight=3`
- unknown sampler version `2`
- retry exhaustion using the existing deterministic configuration
  `column_count=3`, `row_count=3`, `column_weight=2`, `row_weight=2`,
  `seed=1`, `algorithm_version=1`, `retry_limit=1`

Verification:

```text
cargo test -p qec-code --test random_hgp random_hgp_seed7_matches_fixture -- --exact
cargo test -p qec-code --test random_hgp random_hgp_rejects_unreproducible_specs -- --exact
cargo test -p qec-code
cargo test
```
