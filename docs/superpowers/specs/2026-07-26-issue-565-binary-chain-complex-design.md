# Issue 565 Binary Chain Complex Design

Date: 2026-07-26
Status: Accepted by non-interactive Agent Desk standing policy
Scope: GitHub issue #565, validated binary cellular boundary maps in `qec-code`

## Summary

Add a small pure-Rust `BinaryChainComplex` API to `qec-code` for homological CSS
families. The implementation reuses `SparseGf2Matrix` from issue #554 for all
boundary matrices, validates that each boundary map has adjacent cellular
dimensions, canonicalizes sparse supports through the shared sparse primitive,
and verifies that every consecutive boundary composition is zero over GF(2).

Boundary matrices use the repository's sparse-row convention: rows are codomain
cells and columns are domain cells. Thus `boundary_d` has shape
`#(d-1)-cells x #(d)-cells`. For qubits on `k`-cells, the CSS view exposes
`H_X = boundary_k` and `H_Z = boundary_(k+1)^T`.

## Goals

- Add a public `BinaryBoundaryMap` value carrying checked domain and codomain
  cell dimensions plus a validated sparse GF(2) matrix.
- Add a public `BinaryChainComplex` value that stores boundary maps in
  deterministic dimension order and rejects duplicate dimensions.
- Verify every consecutive composition `boundary_d * boundary_(d+1)` is zero
  over GF(2), reporting the two cell dimensions involved and one nonzero
  composed row when validation fails.
- Expose a CSS view for qubits on a requested cell dimension with sparse
  `H_X`, sparse `H_Z`, and check/qubit counts.
- Cover the square fixture exactly, including the corrupt face negative
  control from the issue.

## Non-Goals

- Do not add toric, surface, color-code, or other family constructors.
- Do not change the existing CSS JSON sparse-row format.
- Do not materialize dense matrices for validation.
- Do not add external dependencies.
- Do not define homology, cohomology, distance, orientation, or geometry APIs.

## Alternatives Considered

### 1. Put cellular validation directly in family constructors

This would be the smallest immediate change for a single family, but it would
duplicate boundary validation across toric and later homological families. It
also would not create the explicit reusable representation requested by the
issue. This is rejected.

### 2. Add only free functions over sparse row vectors

Free functions could validate the square fixture, but raw `Vec<Vec<usize>>`
values do not carry shape, cellular dimension, or canonical-state guarantees.
Callers would have to pass loose dimensions through every composition check.
This is rejected.

### 3. Add a dedicated chain-complex module over `SparseGf2Matrix`

Create `qec-code/src/binary_chain_complex.rs` with typed boundary maps, a
validated chain complex, and a sparse CSS view. This keeps the feature narrow,
reuses the shared sparse GF(2) primitive, and gives future constructors a
single validation boundary. This is the selected approach.

## Public API

The module exposes these values:

```rust
pub struct BinaryBoundaryMap {
    domain_dimension: usize,
    codomain_dimension: usize,
    matrix: SparseGf2Matrix,
}

pub struct BinaryChainComplex {
    boundaries: Vec<BinaryBoundaryMap>,
}

pub struct HomologicalCssView {
    qubit_dimension: usize,
    hx: SparseGf2Matrix,
    hz: SparseGf2Matrix,
}
```

`BinaryBoundaryMap::new(domain_dimension, codomain_dimension, matrix)` checks
that `domain_dimension == codomain_dimension + 1`. The matrix shape itself
comes from `SparseGf2Matrix`: rows are codomain cell count, columns are domain
cell count.

`BinaryChainComplex::new(boundaries)` sorts maps by `domain_dimension`, rejects
duplicate domain dimensions, and checks each adjacent pair where
`lower.domain_dimension == upper.codomain_dimension`.

`BinaryChainComplex::css_view(qubit_dimension)` requires both `boundary_k` and
`boundary_(k+1)`. It returns `H_X = boundary_k` and
`H_Z = boundary_(k+1).transpose()`, and validates that both sparse checks have
the same qubit column count.

## Composition Validation

For two consecutive maps:

- `lower = boundary_d` has shape `C_{d-1} x C_d`.
- `upper = boundary_(d+1)` has shape `C_d x C_{d+1}`.
- The shared cell count requires `lower.num_cols() == upper.num_rows()`.

The implementation computes each composed row sparsely. For a lower row, it XORs
the corresponding rows of the upper matrix into a `BTreeSet`, toggling supports
over GF(2). The first nonempty composed row rejects the complex with
`QecError::NonzeroBoundaryComposition`, including `lower_dimension`,
`upper_dimension`, `row`, and the canonical nonzero support.

## Errors

Add these typed `QecError` variants:

```rust
InvalidBoundaryMapDimensions { domain_dimension: usize, codomain_dimension: usize }
DuplicateBoundaryMapDimension { domain_dimension: usize }
MissingBoundaryMap { domain_dimension: usize }
BoundaryCompositionDimensionMismatch {
    lower_dimension: usize,
    upper_dimension: usize,
    lower_domain_cells: usize,
    upper_codomain_cells: usize,
}
NonzeroBoundaryComposition {
    lower_dimension: usize,
    upper_dimension: usize,
    row: usize,
    support: Vec<usize>,
}
```

The composition errors identify the two cellular dimensions whose composition
failed. The nonzero-composition error also identifies one composed row and its
canonical support.

## Square Fixture

Use cell counts `[4, 4, 1]`: four vertices, four edges, and one face.

`boundary_1` is the vertex-edge matrix:

```text
[[0,3], [0,1], [1,2], [2,3]]
```

`boundary_2` is represented as edge-face rows:

```text
[[0], [0], [0], [0]]
```

The convenience fixture helper may derive those edge-face rows from the face
boundary support `[0,1,2,3]`. For qubits on edges (`k=1`), the CSS view has
`n=4`, `m_x=4`, `m_z=1`, `H_X` equal to the vertex-edge rows above, and `H_Z`
equal to `[[0,1,2,3]]`.

The corrupt face support `[0,1,2]` leaves a nonzero composed row, so
construction must return `NonzeroBoundaryComposition` with a nonempty support.

## Testing

Add `qec-code/tests/binary_chain_complex.rs` with the issue's exact tests:

- `square_cell_boundary_maps_match_fixture`
- `corrupt_face_boundary_is_rejected`
- `boundary_maps_reject_invalid_cell_dimensions`
- `chain_complex_rejects_duplicate_boundary_dimensions`
- `chain_complex_reports_composition_shape_mismatch`, including both cellular
  dimensions and both mismatched cell counts in the error.
- `css_view_reports_missing_boundary_maps`

The fixture test checks deterministic canonical row ordering, exact boundary
rows, exact CSS rows and counts, and sparse orthogonality by attempting to build
the view through the validated complex.

The negative test replaces the face boundary by `[0,1,2]` and asserts
`NonzeroBoundaryComposition`, including the two dimensions and a nonzero
composed row.

Verification:

```text
cargo test -p qec-code --test binary_chain_complex square_cell_boundary_maps_match_fixture -- --exact
cargo test -p qec-code --test binary_chain_complex corrupt_face_boundary_is_rejected -- --exact
cargo test -p qec-code
cargo test
```

## Self-Review

- Placeholder scan: no unresolved markers or incomplete sections remain.
- Internal consistency: boundary orientation, CSS view equations, and square
  fixture shapes agree.
- Scope check: the design is limited to one reusable chain-complex primitive and
  its tests.
- Ambiguity check: the issue's face-boundary notation is resolved explicitly as
  a fixture support that produces edge-face rows under the matrix convention.
