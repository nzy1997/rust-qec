# Issue 568 Triangular 6.6.6 Color CSS Constructor Design

Date: 2026-07-26
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #568, Roadmap ID M3-04

## Summary

Issue #568 adds the triangular 6.6.6 color-code requested family to the
`qec-code` CSS construction contract from issue #553. The selected design is a
pure-Rust constructor exposed through `CssFamilySpec::Color666`, versioned JSON
construction `color_666`, and compact inline CLI syntax
`color_666:d=<distance>`.

The constructor supports only the triangular layout. It accepts odd distances
`d >= 3`, uses `n = (3d^2 + 1) / 4`, emits identical `H_X` and `H_Z` sparse
rows from face supports, and returns the common `CssConstructionResult` with
requested-family ID `color_666`.

## Existing Context

- `qec-code/src/family_contract.rs` owns the typed requested-family contract,
  construction metadata, schema-versioned JSON parser, shared sparse-row
  canonicalization, rank computation, and orthogonality verification.
- `qec-code/src/codes/built_in_css.rs` owns compact inline CSS constructors and
  the built-in catalog used by `code css list` and legacy `--code-id` flows.
- `qec-code/src/cli.rs` already routes compact `code css <CODE_ID> <MATRIX>` and
  structured `code css construct --spec <path> <matrix>` requests through the
  common construction contract.
- `qec-code/tests/family_contract.rs` is the natural home for contract-level
  requested-family behavior, while `qec-code/tests/cli.rs` and
  `qec-code/tests/code.rs` cover CLI and built-in catalog behavior.
- The Error Correction Zoo identifies color codes as qubit CSS codes on
  colorable lattices, and qecsim documents a triangular 6.6.6 code with site and
  plaquette coordinates that give the issue's required distance-3 rows.

## Approaches Considered

### 1. qecsim-compatible coordinate algorithm - selected

Implement the triangular lattice directly in Rust using qecsim's public
coordinate rules:

- `bound = 3(d - 1) / 2`
- lattice coordinates are `(row, column)` with `0 <= column <= row <= bound`
- plaquettes satisfy `column mod 3 == 2 - (row mod 3)`
- sites are all in-bounds non-plaquette coordinates
- qubits are ordered by increasing row, then increasing column
- faces are ordered by increasing row, then increasing column
- a face at `(r, c)` supports the six neighboring sites
  `(r-1,c-1)`, `(r-1,c)`, `(r,c-1)`, `(r,c+1)`, `(r+1,c)`, `(r+1,c+1)`,
  with out-of-bounds positions omitted

Benefits:

- reproduces the exact distance-3 fixture rows from the issue
- gives a stable documented coordinate and ordering contract
- scales directly to every odd distance without hard-coded matrices
- keeps implementation pure Rust and deterministic
- gives distance-5 six weight-4 and three weight-6 faces

Costs:

- adds a small coordinate helper surface that must stay documented

### 2. Hard-code reviewed fixtures for each supported distance

Store distance-3 and distance-5 matrices and add larger distances later.

Benefits:

- smallest initial implementation

Costs:

- violates the parameterized constructor requirement
- creates no stable lattice-ordering contract for future odd distances
- risks treating distance 5 as a special case

This is not selected.

### 3. Generate color checks through external Python/qecsim

Call qecsim at build or runtime to generate sparse rows.

Benefits:

- delegates lattice details to an existing reference implementation

Costs:

- violates the pure-Rust deliverable
- introduces runtime dependency and availability risk
- makes deterministic CLI behavior depend on external tooling

This is not selected.

## Public Contract

Add:

```rust
pub enum Color666Layout {
    Triangular,
}

pub struct Color666FamilySpec {
    pub distance: usize,
    pub layout: Color666Layout,
}
```

`CssFamilySpec::Color666(Color666FamilySpec)` is a callable requested-family
variant and `CssFamilySpec::callable_requested_family_ids()` includes
`RequestedFamilyId::Color666`.

`construct_css(CssFamilySpec::Color666(...).into())` returns:

- `schema_version = 1`
- `construction_id = "color_666"`
- `requested_family_id = Some(RequestedFamilyId::Color666)`
- normalized parameters containing `distance` and `layout = "triangular"`
- canonical sparse `H_X` and `H_Z` with identical face-support rows
- shared stats computed by the existing contract boundary
- provenance adapter `color_666`

## CLI And JSON Routing

Compact inline syntax is:

```text
color_666:d=<distance>
```

The built-in catalog lists `color_666:d=<distance>` as a triangular 6.6.6
color CSS code for odd distance `>= 3`. `code css color_666:d=5 hx` and
`code css export color_666:d=5 hz` route through `CssConstructionSpec::from_inline`
and the same `construct_css` path as Rust API construction.

Structured JSON accepts:

```json
{"schema_version":1,"construction":"color_666","distance":5}
```

and the explicit layout form:

```json
{"schema_version":1,"construction":"color_666","distance":5,"layout":"triangular"}
```

Any other layout value is rejected with a typed `QecError::InvalidCssConstruction`
for construction `color_666`.

## Stable Steane Permutation

The distance-3 color rows are:

```text
[[0,1,2,3], [1,2,4,5], [2,3,5,6]]
```

The existing Steane constructor uses rows:

```text
[[0,3,5,6], [1,3,4,6], [2,4,5,6]]
```

The stable column permutation from color-666 qubit order to existing Steane
qubit order is:

```text
[6, 3, 5, 0, 1, 4, 2]
```

Applying that permutation to every distance-3 color row and then sorting each
row maps the color fixture exactly to the existing Steane row set.

## Error Handling

The constructor rejects:

- distance below 3
- even distance
- unsupported layout selectors
- `3 * d * d + 1` arithmetic overflow before computing `n`
- sparse-row index overflow during coordinate-to-qubit indexing

All validation failures return typed `QecError` values. Construction must not
panic on user input.

## Testing

Add `qec-code/tests/color_666.rs` with the issue-required exact tests:

- `color_666_d3_matches_steane_under_stable_permutation`
- `color_666_d5_matches_fixture`
- `color_666_rejects_even_distance`

The tests also cover odd-distance parameter parsing, structured JSON routing,
unsupported layout rejection, distance below three, checked size arithmetic, row
weights, canonical sparse rows, ranks, `k = 1`, and exact distance 3/5 through
the existing exact CSS distance backend.

Required verification:

```text
cargo test -p qec-code --test color_666 color_666_d3_matches_steane_under_stable_permutation -- --exact
cargo test -p qec-code --test color_666 color_666_d5_matches_fixture -- --exact
cargo test -p qec-code --test color_666 color_666_rejects_even_distance -- --exact
cargo test
```

## References

- https://errorcorrectionzoo.org/c/color
- https://qecsim.github.io/api/models/color.html
- https://github.com/qecsim/qecsim/blob/master/src/qecsim/models/color/_color666code.py
- https://github.com/qecsim/qecsim/blob/master/src/qecsim/models/color/_color666pauli.py

## Self-Review

- Placeholder scan: no incomplete marker text remains.
- Internal consistency: inline, JSON, and Rust API all lower to
  `CssFamilySpec::Color666` and return the same construction metadata.
- Scope check: this is one requested-family constructor plus tests and docs; no
  unrelated QP101, decoder, or visualization work is included.
- Ambiguity check: layout handling, qubit ordering, face ordering, and the
  Steane permutation are specified explicitly.
