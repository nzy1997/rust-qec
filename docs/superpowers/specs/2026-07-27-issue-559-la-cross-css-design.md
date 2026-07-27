# Issue 559 La-Cross CSS Design

Date: 2026-07-27
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #559, Roadmap ID M2-03

## Summary

Issue #559 adds a parameterized La-cross CSS family generated from the
classical polynomial support `h(x)=1+x+x^z`. The family has explicit open and
periodic boundary modes, validates the seed length and reach, generates a
deterministic classical sparse-row check matrix, and passes that matrix through
the existing hypergraph-product constructor from issue #556.

The public construction route is `CssFamilySpec::LaCross(LaCrossSpec)`, with
JSON construction `{"schema_version":1,"construction":"la_cross",...}` for the
CLI's existing `code css construct --spec <path> hx|hz|metadata` command.

## Context

- `.AGENTS/AGENTS.md` asks for Rust 2024 style, behavior tests in the owning
  crate, focused verification first, and scoped commits.
- Issue #556 is closed by merged PR #590. Its HGP constructor accepts two
  `CssClassicalCheckSpec` inputs and emits canonical checks, stats, metadata,
  and CLI `metadata` output through the common family contract.
- PR #589 for issue #558 added the closest sibling pattern: a dedicated code
  family module, typed spec, JSON construction parsing, normalized parameters,
  deterministic provenance, and focused integration tests.
- `RequestedFamilyId::LaCross` already exists as a roadmap ID, but it is not yet
  callable through `CssFamilySpec`.

## Approaches Considered

### 1. Dedicated La-cross family adapter over HGP - selected

Add `qec-code/src/codes/la_cross.rs` with `LaCrossSpec`,
`LaCrossBoundary`, validation, classical-row generation, and known fixture
distances. Extend `CssFamilySpec`, `construct_css`, and
`parse_css_construction_json` so Rust API and CLI JSON specs lower through the
same route. Internally, construct the identical left/right HGP input from the
generated classical matrix and reuse the general HGP constructor before
returning a La-cross result.

Benefits:

- satisfies the explicit boundary enum requirement
- keeps La-cross provenance instead of exposing only a generic HGP result
- reuses issue #556's checked HGP arithmetic and orthogonality path
- matches sibling family modules such as generalized bicycle

Cost:

- recomputes the shared construction boundary once when wrapping the HGP result,
  which is acceptable for this issue-sized implementation and keeps the family
  contract simple.

### 2. Require users to submit generated HGP matrices directly

Document La-cross as a recipe for `construction="hypergraph_product"` and make
tests generate the classical matrix in test code only.

Benefits:

- minimal production code

Costs:

- no typed La-cross spec
- no explicit boundary enum
- no La-cross normalized parameters or provenance
- misses the Rust family API acceptance criterion

This is rejected.

### 3. Fold La-cross into legacy built-in CSS code IDs

Add a legacy code string such as `la_cross:length=5,z=2,boundary=open` to
`built_in_css_checks`.

Benefits:

- existing `code css <CODE_ID> hx|hz` export would work

Costs:

- duplicates parsing outside the common construction JSON route
- does not naturally expose normalized construction metadata
- expands the older built-in registry instead of the current family contract

This is not selected.

## Public Contract

Add:

```rust
pub enum LaCrossBoundary {
    Open,
    Periodic,
}

pub struct LaCrossSpec {
    pub seed_length: usize,
    pub reach: usize,
    pub boundary: LaCrossBoundary,
}
```

Both types derive the existing family-facing traits used by sibling specs.
`LaCrossBoundary` serializes and parses as `open` or `periodic`.

Validation rules:

- `seed_length` must be at least 2.
- `reach` must be nonzero.
- `reach` must be strictly less than `seed_length`.
- The HGP result dimensions `seed_length * seed_length + rows * rows` and
  `rows * seed_length` must fit in `usize` before rows are generated.
- Invalid JSON boundary strings return `QecError::InvalidCssConstruction` for
  construction `la_cross` with a message naming the unknown boundary.

Classical row generation:

- Open boundary uses `seed_length - reach` rows:
  `row i = [i, i + 1, i + reach]` for `0 <= i < seed_length - reach`.
- Periodic boundary uses `seed_length` rows:
  `row i = [i, (i + 1) mod seed_length, (i + reach) mod seed_length]`.
- Rows are canonicalized by the sparse GF(2) matrix layer, so duplicates cancel
  deterministically if a valid parameter set produces repeated supports.

For `seed_length = 5`, `reach = 2`, and `boundary = open`, generated classical
rows are exactly:

```text
[[0,1,2], [1,2,3], [2,3,4]]
```

The generated classical matrix is used as both HGP inputs.

## Result Metadata

The returned construction result uses:

- `construction_id = "la_cross"`
- `requested_family_id = Some(RequestedFamilyId::LaCross)`
- `provenance.adapter = "la_cross"`
- `provenance.source = "CssFamilySpec::LaCross"`

Normalized parameters are a deterministic `BTreeMap` containing:

- `seed_length`
- `reach`
- `boundary`
- `classical_check = {"num_cols": seed_length, "rows": ...}`

For open `seed_length = 5`, `reach = 2`, set known distances to `(3, 3)` so
`stats.d_x = Some(3)` and `stats.d_z = Some(3)`. Tests also compute the exact
CSS distance and assert scalar distance 3.

## CLI Contract

The shared construction CLI accepts La-cross JSON specs:

```json
{"schema_version":1,"construction":"la_cross","seed_length":5,"reach":2,"boundary":"open"}
```

and:

```json
{"schema_version":1,"construction":"la_cross","seed_length":5,"reach":2,"boundary":"periodic"}
```

Existing outputs continue to work:

```text
qec-code code css construct --spec <spec.json> hx
qec-code code css construct --spec <spec.json> hz
qec-code code css construct --spec <spec.json> metadata
```

No legacy `code css <CODE_ID>` syntax is added in this issue.

## Testing

Add `qec-code/tests/la_cross.rs` with the issue-required exact test names:

- `la_cross_open_5_2_matches_fixture`
- `la_cross_periodic_5_2_is_orthogonal`
- `la_cross_rejects_invalid_reach`

Coverage includes:

- exact open classical rows in normalized parameters
- open fixture stats `n=34`, `m_x=15`, `m_z=15`, `rank_x=15`,
  `rank_z=15`, `k=4`, `d_x=Some(3)`, and `d_z=Some(3)`
- exact open CSS distance 3 through `compute_distance`
- periodic fixture stats `n=50`, `m_x=25`, `m_z=25`
- orthogonality for periodic checks
- deterministic repeated serialization and provenance digest
- Rust API, JSON parser, and CLI `hx`, `hz`, and `metadata` routes
- negative controls for reach zero, reach outside the seed length, invalid
  boundary strings, and dimension overflow before HGP construction
