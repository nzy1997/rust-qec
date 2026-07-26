# Issue 563 Random Two-Block Design

Date: 2026-07-27
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #563, Roadmap ID M2-07

## Summary

Add a deterministic random two-block CSS constructor to `qec-code`. The
constructor samples two finite-group algebra supports with the repository-owned
version-1 random stream from issue #555, lifts those supports through the
validated left and right regular actions from issue #557, and routes the result
through the common CSS family contract used by the Rust API and CLI.

The S3 fixture in issue #563 fixes the v1 sampling stream, support selection
procedure, matrix layout, stats, and distance expectation.

## Goals

- Add `RandomTwoBlockSpec` with a finite group, `support_a_weight`,
  `support_b_weight`, `seed`, and `algorithm_version`.
- Reuse `SplitMix64V1` and `bounded_index_v1` from `regular_classical`.
- Define algorithm version 1 as partial Fisher-Yates sampling over the group
  element list `0..order`, using one stream across support A then support B.
- Reject unknown algorithm versions, missing JSON seed, zero weights, weights
  larger than the group order, invalid group tables, and group orders above
  `MAX_FINITE_GROUP_ORDER`.
- Check the finite-group order limit before validating or sampling the table by
  constructing the existing `FiniteGroupSpec` before any sampler step.
- Build `H_X = [L(A) | R(B)]` and `H_Z = [R(B)^T | L(A)^T]`, then explicitly
  verify CSS orthogonality.
- Record metadata with `group_digest`, `seed`, support weights, and algorithm
  version.
- Expose the constructor through both Rust APIs and the JSON-backed CLI common
  CSS construction contract.

## Non-Goals

- Do not add an external algebra or RNG dependency.
- Do not change existing quantum Tanner, surface, or legacy built-in contracts.
- Do not add a separate CLI-only constructor path outside the common
  `code css construct --spec ...` contract.
- Do not implement other random families such as random HGP or lifted product.

## Approaches Considered

### 1. Implement only a CLI fixture

This would satisfy a narrow golden-output check, but it would not create the
requested Rust API and would duplicate validation outside the family contract.
It is not selected.

### 2. Fold two-block logic into `family_contract.rs`

This keeps all construction code in one file, but `family_contract.rs` already
coordinates several families and should not own group-algebra sampling details.
It is not selected.

### 3. Add a focused `codes::random_two_block` module

Create `qec-code/src/codes/random_two_block.rs` for the spec type, JSON parser,
sampling algorithm, lifted matrix construction, orthogonality check, and
metadata. `family_contract.rs` becomes a thin adapter that parses and lowers
`CssFamilySpec::RandomTwoBlock`. This reuses #555/#557 directly and keeps the
new behavior isolated. This is the selected approach.

## Public API

Expose `pub mod random_two_block` from `qec-code/src/codes/mod.rs`.

The module defines:

```rust
pub const RANDOM_TWO_BLOCK_ALGORITHM_V1: u32 = 1;

pub struct RandomTwoBlockSpec {
    pub group: FiniteGroupSpec,
    pub support_a_weight: usize,
    pub support_b_weight: usize,
    pub seed: u64,
    pub algorithm_version: u32,
}

pub struct RandomTwoBlockCssChecks {
    pub num_cols: usize,
    pub h_x: Vec<Vec<usize>>,
    pub h_z: Vec<Vec<usize>>,
    pub support_a: Vec<usize>,
    pub support_b: Vec<usize>,
    pub metadata: RandomTwoBlockMetadata,
}

pub struct RandomTwoBlockMetadata {
    pub group_digest: String,
    pub seed: u64,
    pub support_a_weight: usize,
    pub support_b_weight: usize,
    pub algorithm_version: u32,
}

pub fn random_two_block_spec_from_json_str(input: &str) -> Result<RandomTwoBlockSpec>;
pub fn random_two_block_css_checks(spec: &RandomTwoBlockSpec) -> Result<RandomTwoBlockCssChecks>;
```

`RandomTwoBlockSpec::new` accepts a validated `FiniteGroupSpec` plus weights,
seed, and algorithm version. Invalid raw tables are rejected by
`FiniteGroupSpec::new`; JSON input constructs that type before sampling.

## Version 1 Sampling

For each support, create `pool = [0, 1, ..., order - 1]`. For
`i = 0..weight`, draw `offset = bounded_index_v1(&mut stream, (order - i) as
u64)`, set `j = i + offset as usize`, and swap `pool[i]` with `pool[j]`.
The selected support is the first `weight` entries, canonicalized in ascending
order for group-algebra use and metadata.

The same `SplitMix64V1` stream is used first for support A and then for support
B. With seed 7 and S3 order 6, the first four bounded draws use bounds
`6, 5, 6, 5`, producing support A `[3, 5]` and support B `[0, 4]`.

## Matrix Layout

Let `A` be the left-regular lift of support A and `B` be the right-regular lift
of support B over the validated group. The two-block CSS checks are:

```text
H_X = [A | B]
H_Z = [B^T | A^T]
```

The transpose placement makes
`H_X * H_Z^T = A * B + B * A = 0` over GF(2) because left and right regular
actions commute. Construction still calls an explicit orthogonality verifier so
any implementation or future contract regression returns `InvalidCssOrthogonality`.

For the issue S3 fixture, all sampled support elements are self-inverse, so the
transposed `H_Z` rows match the fixture rows exactly.

## JSON And Common Contract

The common CSS construction JSON accepts:

```json
{
  "schema_version": 1,
  "construction": "random_two_block",
  "group": {
    "name": "S3",
    "element_order": "0=e,1=r,2=r^2,3=s,4=rs,5=r^2s",
    "order": 6,
    "identity": 0,
    "multiplication_table": [
      [0,1,2,3,4,5],
      [1,2,0,4,5,3],
      [2,0,1,5,3,4],
      [3,5,4,0,2,1],
      [4,3,5,1,0,2],
      [5,4,3,2,1,0]
    ]
  },
  "support_a_weight": 2,
  "support_b_weight": 2,
  "seed": 7,
  "algorithm_version": 1
}
```

`name` and `element_order` are accepted for fixture readability but are not part
of the validated finite-group digest. The digest is computed from deterministic
compact JSON over `order`, `identity`, and `multiplication_table`.

`construct_css` sets `construction_id = "random_two_block"`,
`requested_family_id = Some(RandomTwoBlock)`, `adapter = "random_two_block"`,
and `source = "CssFamilySpec::RandomTwoBlock"`. `normalized_parameters`
contains the validated group data, sampled supports, `group_digest`, `seed`,
both weights, and `algorithm_version`.

## Errors

Add typed `QecError` variants:

```rust
InvalidRandomTwoBlockSpec { option: &'static str, reason: String }
UnsupportedRandomTwoBlockAlgorithm { algorithm_version: u32 }
```

Group-table errors continue to use the `finite_group` typed errors from #557,
including `GroupOrderLimitExceeded`.

## Testing

Add `qec-code/tests/random_two_block.rs` with:

- `random_two_block_s3_seed7_matches_fixture`
- `random_two_block_rejects_invalid_sampling_specs`

The fixture test asserts exact sampled supports, exact `H_X`/`H_Z` sparse rows,
orthogonality, `n = 12`, `rank_x = 5`, `rank_z = 5`, `k = 2`, exact distance 2,
metadata fields, and equality between direct Rust construction, parsed common
contract construction, and CLI `code css construct` output.

The negative test covers weights above order, missing seed in JSON, unknown
algorithm versions, invalid group tables, empty supports through zero weights,
and group-order limit precedence before table validation.

Verification:

```text
cargo test -p qec-code --test random_two_block random_two_block_s3_seed7_matches_fixture -- --exact
cargo test -p qec-code --test random_two_block random_two_block_rejects_invalid_sampling_specs -- --exact
cargo test -p qec-code
cargo test
```
