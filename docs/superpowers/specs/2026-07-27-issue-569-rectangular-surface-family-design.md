# Issue 569 Rectangular Surface Family Design

Date: 2026-07-27
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #569, Roadmap ID M3-05

## Summary

Issue #569 extends the #553 CSS construction contract so the requested
`surface` family can describe rotated and ordinary planar rectangular patches.
The legacy compact API `surface_rotated:d=<distance>` remains a compatibility
adapter and must keep producing byte-for-byte identical sparse-row matrix JSON
for every distance that was valid before, including even distances.

The selected design keeps surface construction inside the existing
`CssConstructionSpec` and `CssFamilySpec` route. It retains the square-only
`SurfaceFamilySpec { distance }` compatibility adapter and adds an explicit
`SurfaceSpec` for generalized construction:

- `layout`: `rotated` or `unrotated`
- `row_distance`: row distance, minimum `2`
- `column_distance`: column distance, minimum `2`

The Rust helper for generalized square rotated requests builds a normalized
`SurfaceSpec` with `layout = rotated` and both dimensions equal to `d`.

## Existing Context

- `qec-code/src/codes/built_in_css.rs` owns legacy compact parsing and the
  square rotated sparse-row generator.
- `qec-code/src/family_contract.rs` owns the #553 typed construction route,
  deterministic metadata, rank and `k` statistics, and shared orthogonality
  verification.
- `qec-code/src/cli.rs` routes compact and structured CSS constructor requests
  through `CssConstructionSpec` before matrix export.
- `qec-code/tests/family_contract.rs` pins the #553 compatibility behavior for
  rotated distance `3`.
- Issue #553 is closed and PR #578 is merged into this branch's base commit, so
  the common typed contract is available and should be extended in place.

There is no repository `AGENTS.md` in this worktree.

## Approaches Considered

### 1. Extend the #553 contract in place - selected

Add `SurfaceLayout` and `SurfaceSpec` to `qec-code/src/family_contract.rs`,
generate rotated and ordinary planar supports from that typed spec, and retain
`surface_rotated:d=<distance>` through the legacy `SurfaceFamilySpec` adapter.

Benefits:

- uses the established common contract instead of adding a parallel API
- keeps CLI compact and JSON routes in the common construction contract
- preserves legacy matrix serialization by routing square rotated requests to
  the existing built-in generator
- lets the contract report surface layout, dimensions, and known directional
  distances alongside existing stats

Costs:

- moves new surface-family generation into the contract layer while the legacy
  square generator remains in `built_in_css.rs`

### 2. Generalize `built_in_css.rs` directly

Teach the legacy built-in parser about rectangular and ordinary surface IDs,
then adapt those through `CssConstructionSpec::LegacyBuiltIn`.

Benefits:

- all built-in sparse support functions stay together

Costs:

- requested-family layout and dimensions would be represented as legacy strings
- generic utilities and requested families would be less clearly separated
- structured JSON would not be the primary source of the public contract

This is not selected.

### 3. Add a new standalone surface module first

Create a dedicated `surface.rs` module under `qec-code/src/codes/` and expose
it independently from the family contract.

Benefits:

- clearer long-term home if surface construction grows substantially

Costs:

- larger refactor than needed for this issue
- more public surface area before the minimal contract is stable

This is not selected.

## Public Contract

`SurfaceLayout` serializes as lowercase snake-case layout names:

```text
rotated
unrotated
```

`SurfaceSpec` is the public Rust API for the requested `surface` family:

```rust
pub struct SurfaceSpec {
    pub layout: SurfaceLayout,
    pub row_distance: usize,
    pub column_distance: usize,
}
```

`SurfaceFamilySpec { distance }` remains the #553 compatibility adapter and
uses the existing built-in square rotated constructor. `SurfaceSpec::rotated_square(distance)`
constructs the generalized square rotated specification, which enters through
`CssConstructionSpec::Surface`.

The JSON construction contract accepts:

```json
{
  "schema_version": 1,
  "construction": "surface",
  "layout": "rotated",
  "row_distance": 3,
  "column_distance": 5
}
```

The legacy JSON shape remains accepted:

```json
{
  "schema_version": 1,
  "construction": "surface",
  "distance": 3
}
```

It lowers to `CssFamilySpec::Surface(SurfaceFamilySpec { distance: 3 })`, the
same compatibility route as `surface_rotated:d=3`.

Conflicting legacy and new parameters are rejected. A request that contains
`distance` together with `layout`, `row_distance`, or `column_distance` returns a
typed `InvalidCssConstruction` error. Unknown layouts, missing dimensions,
dimension values below `2`, and `usize` overflows return typed errors without
panicking.

Generalized surface results include normalized parameters:

- `layout`
- `row_distance`
- `column_distance`

`CssCodeStats` gains `d_x` and `d_z` as optional known directional distances.
For surface layouts, `d_x = column_distance` and `d_z = row_distance`. Other
construction families report `None` for these fields.

## Matrix Construction

### Rotated Layout

Rotated surface data qubits use row-major indexing over
`row_distance * column_distance` data positions:

```text
index(row, column) = row * column_distance + column
```

The generator is the existing square algorithm with independent row and column
bounds. It scans ancilla coordinates
`ax in 0..=row_distance`, `ay in 0..=column_distance`, applies the same parity
and boundary omissions as the current constructor, and collects diagonal data
neighbors inside the rectangular data grid. This preserves every square rotated
output exactly because the square case executes the same coordinate scan and
sort order.

For rotated `3 x 5`, the exact supports are:

```text
H_X = [[0,5], [1,2,6,7], [3,4,8,9], [5,6,10,11], [7,8,12,13], [9,14]]
H_Z = [[1,2], [3,4], [0,1,5,6], [2,3,7,8], [6,7,11,12], [8,9,13,14], [10,11], [12,13]]
```

### Ordinary Planar Layout

The ordinary planar layout uses the standard parity grid on a
`(2 * row_distance - 1) x (2 * column_distance - 1)` coordinate lattice.
Data qubits occupy even-parity positions and are indexed in row-major scan order.
X checks occupy odd-row, even-column positions and Z checks occupy even-row,
odd-column positions. Each check supports the in-range north, west, east, and
south neighboring data positions.

For unrotated distance `3`, the exact supports are:

```text
H_X = [[0,3,5], [1,3,4,6], [2,4,7], [5,8,10], [6,8,9,11], [7,9,12]]
H_Z = [[0,1,3], [1,2,4], [3,5,6,8], [4,6,7,9], [8,10,11], [9,11,12]]
```

The resulting code reports `[[13, 1, 3]]` for square distance `3`.

## CLI Contract

Existing compact commands remain valid:

```text
qec-code code css surface_rotated:d=3 hx
qec-code code css export surface_rotated:d=3 hz
```

Structured JSON exposes the new layout and dimensions through the common
`code css construct --spec <path> <matrix>` route. The compact legacy route and
the structured rotated-square route must lower to equivalent typed specs.

The built-in catalog keeps documenting the legacy compact
`surface_rotated:d=<distance>` syntax. The new rectangular and ordinary
parameters are documented through the versioned construction contract, not as
additional legacy built-in IDs.

## Testing

Add `qec-code/tests/surface_family.rs` with issue-named tests:

- `rectangular_rotated_surface_3x5_matches_fixture`
- `ordinary_surface_d3_matches_fixture`
- `legacy_rotated_surface_outputs_are_unchanged`
- `surface_family_rejects_invalid_dimensions`

The tests cover exact matrix fixtures, canonical sparse rows, orthogonality,
rank and `k` stats, known directional distances, JSON and Rust API lowering, CLI
matrix export for structured JSON, legacy byte-for-byte fixture preservation,
and invalid dimensions/layout/conflicting parameters/overflow errors.

Final verification includes the four exact issue commands plus `cargo test`.
