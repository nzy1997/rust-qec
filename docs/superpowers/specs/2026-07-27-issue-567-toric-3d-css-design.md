# Issue 567 Toric 3D CSS Design

Date: 2026-07-27
Status: Accepted by non-interactive Agent Desk standing policy
Scope: GitHub issue #567, parameterized periodic 3D toric CSS construction

## Summary

Add a pure-Rust `Toric3dSpec { lx, ly, lz }` constructor in `qec-code` for the
periodic cubic 3D toric code. The constructor validates all periods as at least
3, builds the cellular boundary maps through `BinaryBoundaryMap` and
`BinaryChainComplex`, then exposes qubits on edges with vertex X checks and
plaquette Z checks through the common CSS family contract.

The indexing is fixed so the `3 x 3 x 3` fixture has the exact leading rows
from the issue. Vertices use `((x * ly) + y) * lz + z`. Edge blocks are ordered
as all x-edges, all y-edges, then all z-edges, each with the same coordinate
index. Plaquette blocks are ordered as all xy plaquettes, all xz plaquettes,
then all yz plaquettes, each with the same coordinate index.

## Goals

- Add public `qec_code::codes::toric_3d::Toric3dSpec` with fields `lx`, `ly`,
  and `lz`.
- Reject any period below 3 before constructing matrices.
- Reject dimension and coordinate arithmetic overflow with typed `QecError`
  values instead of panicking or wrapping.
- Build `boundary_1` and `boundary_2` as sparse GF(2) matrices and validate
  `boundary_1 * boundary_2 = 0` through `BinaryChainComplex`.
- Expose `H_X = boundary_1` and `H_Z = boundary_2^T` through
  `CssFamilySpec::Toric3d`, JSON construction specs, built-in CSS code IDs, and
  the existing CLI.
- Fixture-test the full `3 x 3 x 3` sparse rows, exact leading rows, row
  weights, orthogonality, stats, and analytic distances.

## Non-Goals

- Do not add decoder support, syndrome simulation, or visualization.
- Do not change the sparse-row JSON schema.
- Do not add a new CLI subcommand; reuse `code css`, `code css construct`, and
  existing `--code-id` distance inputs.
- Do not compute exact distances for large 3D toric instances by exhaustive or
  ILP search in this issue; expose analytic distances from the constructor.

## Alternatives Considered

### 1. Hand-build `H_X` and `H_Z` directly

This is compact, but it bypasses the boundary-map layer that issue #567
requires and would leave the corrupt-boundary negative control uncovered. This
is rejected.

### 2. Add a generic cubical-complex abstraction first

A generic cubical cellulation API could support future surface and color-code
work, but it is broader than this issue and would create additional public API
compatibility questions. This is rejected for now.

### 3. Add a focused 3D toric module over `BinaryChainComplex`

Create `qec-code/src/codes/toric_3d.rs` with period validation, deterministic
cell indexing, boundary construction, CSS checks, and analytic distances. This
uses the shared boundary-map layer, keeps the new public surface focused, and
matches the existing family contract pattern. This is the selected approach.

## Public API

The module exposes:

```rust
pub struct Toric3dSpec {
    pub lx: usize,
    pub ly: usize,
    pub lz: usize,
}

pub struct Toric3dCssChecks {
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
    pub distances: Toric3dDistances,
}

pub struct Toric3dDistances {
    pub d_x: usize,
    pub d_z: usize,
    pub distance: usize,
}

pub fn toric_3d_css_checks(spec: Toric3dSpec) -> Result<Toric3dCssChecks>;
pub fn toric_3d_chain_complex(spec: Toric3dSpec) -> Result<BinaryChainComplex>;
```

`Toric3dSpec` remains a plain field struct like sibling family specs; all
runtime constructors validate it before use. `Toric3dDistances` reports
`d_z = min(lx, ly, lz)`, `d_x = min(lx * ly, lx * lz, ly * lz)`, and
`distance = min(d_x, d_z)`.

## Cell Indexing

Let `V = lx * ly * lz` and
`cell(x, y, z) = ((x * ly) + y) * lz + z`.

Qubit edge columns:

- `x_edge(x, y, z) = cell(x, y, z)`
- `y_edge(x, y, z) = V + cell(x, y, z)`
- `z_edge(x, y, z) = 2V + cell(x, y, z)`

Z-check plaquette rows:

- `xy_plaquette(x, y, z) = cell(x, y, z)`
- `xz_plaquette(x, y, z) = V + cell(x, y, z)`
- `yz_plaquette(x, y, z) = 2V + cell(x, y, z)`

The first vertex row is incident on outgoing and incoming edges in each axis,
so for `lx=ly=lz=3` it canonicalizes to:

```text
H_X[0] = [0,18,27,33,54,56]
```

The first plaquette rows canonicalize to:

```text
H_Z_xy[0] = [0,3,27,36]
H_Z_xz[0] = [0,1,54,63]
H_Z_yz[0] = [27,28,54,57]
```

## Contract Integration

Add `Toric3d(Toric3dSpec)` to `CssFamilySpec` and include
`RequestedFamilyId::Toric3d` in `callable_requested_family_ids()`.
`construct_css` lowers the family to checks with construction ID `toric_3d`,
requested family ID `toric_3d`, normalized parameters `{lx, ly, lz}`, adapter
`toric_3d_chain_complex`, and source `CssFamilySpec::Toric3d`.

`parse_css_construction_json` accepts:

```json
{"schema_version":1,"construction":"toric_3d","lx":3,"ly":3,"lz":3}
```

`CssConstructionSpec::from_inline` and `built_in_css_checks` accept:

```text
toric_3d:lx=3,ly=3,lz=3
```

The built-in CSS catalog adds
`toric_3d:lx=<period-x>,ly=<period-y>,lz=<period-z>` with a description that
states each period must be at least 3.

## Testing

Add `qec-code/tests/toric_3d.rs` for the constructor and common contract:

- `toric_3d_3x3x3_matches_fixture` checks exact full fixture rows, leading
  rows, `n=81`, `m_x=27`, `m_z=81`, `rank_x=26`, `rank_z=52`, `k=3`, all row
  weights, analytic `d_z=3`, `d_x=9`, overall distance 3, and orthogonality.
- `toric_3d_rejects_degenerate_periods` checks every period position rejects
  values below 3 through the Rust API and CLI path.
- `toric_3d_accepts_rectangular_periods` checks a non-cubic valid spec through
  Rust API, JSON construction parsing, and the CLI code ID parser.
- `toric_3d_rejects_overflowing_dimensions` covers period products and cell
  coordinate arithmetic that would exceed `usize`.

Add a focused unit test in `codes::toric_3d` that deliberately corrupts one
plaquette boundary before `BinaryChainComplex::new` and asserts
`NonzeroBoundaryComposition`.

Update `qec-code/tests/cli.rs` and `qec-code/tests/family_contract.rs` for
catalog, inline, JSON, and callable-family coverage.

Verification:

```text
cargo test -p qec-code --test toric_3d toric_3d_3x3x3_matches_fixture -- --exact
cargo test -p qec-code --test toric_3d toric_3d_rejects_degenerate_periods -- --exact
cargo test -p qec-code
cargo test
```

## Self-Review

- Placeholder scan: no unresolved markers or incomplete sections remain.
- Internal consistency: boundary orientation, row indexing, and family-contract
  routes agree with the required leading rows.
- Scope check: the design is limited to the 3D toric CSS constructor, its API,
  CLI exposure, fixtures, and negative controls.
