# Issue 562 Coprime Bivariate-Bicycle CSS Constructor Design

Date: 2026-07-27
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #562, Roadmap ID M2-06

## Summary

Issue #562 adds a parameterized coprime bivariate-bicycle constructor to
`qec-code`. The selected design introduces a typed
`CoprimeBivariateBicycleSpec { l, m, a_exponents, b_exponents }`, validates
that the periods are nonzero and coprime, validates canonical supports for the
single generator `pi = xy`, and lowers the construction to the existing
#558 cyclic generalized-bicycle constructor of order `l * m`.

The constructor reuses #558's exact block convention:

```text
H_X = [A | B]
H_Z = [B^T | A^T]
```

where `A` and `B` are cyclic circulants over the reduced `pi` exponents. This
keeps the block and transpose convention shared across generalized-bicycle and
coprime bivariate-bicycle families.

## Existing Context

- `.AGENTS/AGENTS.md` requires Rust 2024 style, focused integration tests, and
  exact verification commands before claiming completion.
- Issue #558 is merged on `master` and provides
  `qec-code/src/codes/generalized_bicycle.rs`,
  `GeneralizedBicycleSpec`, cyclic circulant row construction, known fixture
  distances, and `construct_css` integration for `generalized_bicycle`.
- `RequestedFamilyId` already reserves `coprime_bb`, but
  `CssFamilySpec::callable_requested_family_ids()` does not yet include it.
- `code css construct --spec <path> hx|hz|metadata` is the shared CLI entry
  point for structured family requests.

## Approaches Considered

### 1. Thin coprime wrapper over generalized bicycle - selected

Add `qec-code/src/codes/coprime_bb.rs` with typed validation and provenance,
then convert the normalized coprime spec into
`GeneralizedBicycleSpec { order: l * m, a_exponents, b_exponents }`.

Benefits:

- directly implements the issue requirement to reduce to order `l * m`
- reuses #558's tested sparse matrix construction and transpose convention
- keeps coprime-period validation and `pi = xy` documentation localized
- exposes a distinct requested-family identity and provenance

Costs:

- adds a small adapter layer and one more JSON parser branch

### 2. Rebuild the cyclic blocks directly in a coprime module

The coprime module could duplicate #558's circulant construction.

Benefits:

- fewer cross-module calls

Costs:

- risks drifting from #558's exact block and transpose convention
- duplicates normalization and sparse GF(2) assembly behavior

This is not selected.

### 3. Route through the legacy rectangular `bb` parser

The legacy bivariate-bicycle code path can build some two-dimensional examples.

Benefits:

- uses an existing CLI parser

Costs:

- does not expose the requested typed `l`, `m`, and `pi` exponent contract
- does not record `requested_family_id = coprime_bb`
- does not reduce through #558's generalized-bicycle constructor

This is not selected.

## Public Contract

Add:

```rust
pub struct CoprimeBivariateBicycleSpec {
    pub l: usize,
    pub m: usize,
    pub a_exponents: Vec<usize>,
    pub b_exponents: Vec<usize>,
}
```

The JSON contract is:

```json
{
  "schema_version": 1,
  "construction": "coprime_bb",
  "l": 3,
  "m": 5,
  "a_exponents": [0, 1, 2],
  "b_exponents": [0, 2, 7]
}
```

Successful construction returns:

- `schema_version = 1`
- `construction_id = "coprime_bb"`
- `requested_family_id = Some(RequestedFamilyId::CoprimeBb)`
- normalized parameters `l`, `m`, `cyclic_order`, `pi`, `a_exponents`, and
  `b_exponents`
- canonical sparse `H_X` and `H_Z`
- shared `n`, `m_x`, `m_z`, `rank_x`, `rank_z`, and `k` stats
- fixture distances `d_x = 6` and `d_z = 6` for the issue fixture
- provenance adapter `coprime_bb` and source `CssFamilySpec::CoprimeBb`

`CssFamilySpec::CoprimeBb(CoprimeBivariateBicycleSpec)` becomes a callable
requested-family variant, and `CssFamilySpec::callable_requested_family_ids()`
includes `RequestedFamilyId::CoprimeBb`.

## `pi = xy` Index Map

The coprime periods define quotient relations `x^l = 1` and `y^m = 1`. Because
`gcd(l, m) = 1`, powers of `pi = xy` enumerate the full product torus:

```text
pi^t = x^(t mod l) y^(t mod m)
t in 0..(l * m)
```

The map `t -> (t mod l, t mod m)` is a bijection on
`Z/(l*m) -> Z/l x Z/m`, so the constructor can use reduced `pi` exponents
directly as cyclic generalized-bicycle exponents of order `l * m`.

For `l = 3`, `m = 5`, `t = 7`, the mapped monomial is:

```text
pi^7 = x^(7 mod 3) y^(7 mod 5) = x y^2
```

## Input Validation

The spec normalizes exponent lists by sorting them after validation. It rejects:

- `l = 0`
- `m = 0`
- `gcd(l, m) != 1`
- multiplication overflow while computing `l * m`
- empty `a_exponents`
- empty `b_exponents`
- any exponent `>= l * m`
- duplicate exponents after normalization

Unsorted but otherwise valid exponent lists are accepted so Rust and JSON
callers can submit natural support lists while metadata still records canonical
normalized supports.

## Fixture

Store the full reviewed fixture in:

```text
qec-code/tests/fixtures/coprime_bb/l3_m5_pi_fixture.json
```

The fixture request is `l = 3`, `m = 5`,
`a_exponents = [0, 1, 2]`, and `b_exponents = [0, 2, 7]`.

It must include the complete `H_X` and `H_Z` arrays, and the test must verify:

- `H_X[0] = [0,1,2,15,17,22]`
- `H_Z[0] = [0,8,13,15,28,29]`
- `[[30,4,6]]`
- `m_x = m_z = 15`
- `rank_x = rank_z = 13`
- every check row has weight 6
- the full checks are orthogonal

## Testing

Add `qec-code/tests/coprime_bb.rs` with the issue-required exact tests:

- `coprime_bb_3_5_matches_30_4_6_fixture`
- `coprime_bb_rejects_non_coprime_periods`

The positive test verifies exact fixture rows, canonical supports,
orthogonality, stats, known fixture distances, Rust API construction, JSON
lowering, deterministic metadata, and CLI export through
`code css construct --spec`.

The negative test verifies non-coprime periods, zero periods, out-of-range
reduced exponents, duplicate canonical exponents, and empty `a`/`b` supports.

Required verification:

```text
cargo test -p qec-code --test coprime_bb coprime_bb_3_5_matches_30_4_6_fixture -- --exact
cargo test -p qec-code --test coprime_bb coprime_bb_rejects_non_coprime_periods -- --exact
cargo test
```

## Self-Review

- Placeholder scan: no placeholder or unresolved marker text remains.
- Internal consistency: Rust API, JSON parsing, CLI export, metadata, fixture,
  and matrix construction all use `CssFamilySpec::CoprimeBb`.
- Scope check: the design adds one requested-family constructor, one reviewed
  fixture, and tests; it does not change finite-group lifts or legacy BB
  parsing.
- Ambiguity check: `pi` exponent mapping, acceptance of unsorted supports,
  known fixture distances, and lack of compact inline syntax are specified
  explicitly.
