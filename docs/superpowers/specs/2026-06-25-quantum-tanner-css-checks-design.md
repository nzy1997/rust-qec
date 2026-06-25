# Quantum Tanner CSS Checks Design

## Context

Issue #183 adds the next narrow quantum Tanner step in `qec-code`: a validated explicit group, Cayley face incidence, and local tensor-code helper output must become ordinary CSS sparse row supports. Existing work already provides:

- `QuantumTannerSpec` parsing and local-code validation in `qec-code/src/codes/quantum_tanner.rs`.
- `ValidatedFiniteGroup` and deterministic Cayley face/incidence enumeration.
- Local tensor rows `x_sector_rows` and `z_sector_rows`.
- `SparseRowsMatrix` and `CssCode::from_hx_hz` for sparse row validation and CSS orthogonality checks.

The qLDPC reference semantics place physical qubits on Cayley complex faces and build local X/Z checks from each source vertex's ordered `(a, b)` incident faces. The existing Rust incidence records already encode those ordered local coordinates for X and Z neighborhoods.

## Chosen Approach

Add a narrow builder in `qec-code/src/codes/quantum_tanner.rs` that exposes both:

- `quantum_tanner_css_checks(&QuantumTannerSpec)`, a convenience path that validates the group, enumerates faces, computes local tensor rows, then builds CSS checks.
- `quantum_tanner_css_checks_from_validated_parts(...)`, the issue's middle-shape constructor for already validated spec/group/complex/local helper data.

This follows the repository's current quantum Tanner module style and avoids CLI, importer, or serialization changes.

## Matrix Semantics

The builder sets `num_cols = complex.faces.len()`. A physical qubit column is the deterministic `face_id` already assigned by the Cayley enumerator.

For each group vertex and each local X tensor row, the builder gathers that vertex's `x_incidence` records in `(a_index, b_index)` order. The tensor coordinate is `a_index * |B| + b_index`, matching `tensor_product_rows()` flattening. A bit value of `1` includes the incident `face_id` in the sparse row.

The Z sector uses the same flow with `z_incidence` and `z_sector_rows`. This preserves the existing X-side `(a, b)` and Z-side `(a^-1, b)` relationship encoded by the enumerator.

## Validation

The validated-parts constructor still performs consistency checks because it is public:

- The local code widths must match the group's A/B generator counts.
- Each local tensor row must have length `|A| * |B|`.
- For every group vertex, each sector must provide exactly one incidence record per local `(a, b)` coordinate.
- Incidence records must refer to valid face ids and expected generator values.
- Generated sparse rows are validated with `SparseRowsMatrix::new`.
- CSS orthogonality is validated with `CssCode::from_hx_hz`; non-orthogonal data returns `QecError::InvalidCssOrthogonality`.

For other mismatched validated parts, add `QecError::InvalidQuantumTannerCssConstruction { reason }` so callers get a typed quantum Tanner construction error instead of panics or malformed sparse rows.

## Acceptance Tests

Add `quantum_tanner_toric_d4_generates_css_checks` in `qec-code/tests/code.rs`. It builds from the `toric_d4` fixture and asserts:

- `num_cols == 16`.
- Every non-empty `Hx` and `Hz` row has weight `4`.
- `Hx * Hz^T == 0 mod 2`.
- `CssCode::from_hx_hz` has `k == 2`.
- `compute_distance` returns exact distance `4`.
- The catalog `invalid_non_symmetric_a` fixture is rejected before sparse rows are produced.

The existing exhaustive distance path must be made practical for this small `n = 16` fixture by searching Pauli candidates in increasing weight order while preserving the current unsupported behavior for configurations whose symplectic mask width does not fit in `usize`.

## Out Of Scope

No CLI flags, file import commands, `rsinter` integration, random search, qTanner/qLDPC importers, or new CSS matrix serialization formats are added.
