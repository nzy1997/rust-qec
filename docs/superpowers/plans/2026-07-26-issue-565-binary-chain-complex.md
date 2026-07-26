# Issue 565 Binary Chain Complex Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add validated binary cellular boundary maps and sparse homological CSS checks to `qec-code`.

**Architecture:** Add `qec-code/src/binary_chain_complex.rs` as a narrow public module over `SparseGf2Matrix`. Boundary maps carry cellular domain/codomain dimensions and matrix shape; the chain complex stores maps in deterministic dimension order and verifies consecutive GF(2) compositions before exposing a CSS view.

**Tech Stack:** Rust 2024, `qec-code`, `thiserror`, existing `SparseGf2Matrix`, Cargo integration tests.

## Global Constraints

- Implementation must be pure Rust.
- Implementation must use `qec_code::sparse_gf2::SparseGf2Matrix` for sparse boundary maps.
- Boundary matrices use rows as codomain cells and columns as domain cells.
- `boundary_d` has shape `#(d-1)-cells x #(d)-cells`.
- For qubits on `k`-cells, `H_X = boundary_k` and `H_Z = boundary_(k+1)^T`.
- The square fixture uses vertex-edge boundary rows exactly `[[0,3], [0,1], [1,2], [2,3]]`.
- The square fixture uses one face boundary support exactly `[0,1,2,3]`.
- The square CSS view must have `n=4`, `m_x=4`, and `m_z=1`.
- Replacing the face boundary by `[0,1,2]` must return `QecError::NonzeroBoundaryComposition`.
- `NonzeroBoundaryComposition` must identify the two cellular dimensions and one nonzero composed row.
- Canonical ordering must be deterministic.
- Do not add external dependencies.
- Do not add family constructors, CLI commands, dense homology APIs, or JSON format changes.

---

## File Structure

- Create `qec-code/src/binary_chain_complex.rs`: public boundary-map, chain-complex, CSS-view types, deterministic ordering, and sparse GF(2) composition validation.
- Modify `qec-code/src/lib.rs`: export `binary_chain_complex`.
- Modify `qec-code/src/error.rs`: add typed boundary-map and composition errors.
- Create `qec-code/tests/binary_chain_complex.rs`: exact square fixture test and corrupt-face negative control.

### Task 1: Validated Binary Chain Complex

**Files:**
- Create: `qec-code/src/binary_chain_complex.rs`
- Modify: `qec-code/src/lib.rs`
- Modify: `qec-code/src/error.rs`
- Create: `qec-code/tests/binary_chain_complex.rs`
- Modify: `docs/superpowers/plans/2026-07-26-issue-565-binary-chain-complex.md`

**Interfaces:**
- Consumes: `qec_code::sparse_gf2::SparseGf2Matrix`.
- Produces: `qec_code::binary_chain_complex::BinaryBoundaryMap`.
- Produces: `qec_code::binary_chain_complex::BinaryChainComplex`.
- Produces: `qec_code::binary_chain_complex::HomologicalCssView`.
- Produces: `BinaryBoundaryMap::new(domain_dimension: usize, codomain_dimension: usize, matrix: SparseGf2Matrix) -> qec_code::error::Result<Self>`.
- Produces: `BinaryChainComplex::new(boundaries: Vec<BinaryBoundaryMap>) -> qec_code::error::Result<Self>`.
- Produces: `BinaryChainComplex::boundary(domain_dimension: usize) -> Option<&SparseGf2Matrix>`.
- Produces: `BinaryChainComplex::boundary_map(domain_dimension: usize) -> Option<&BinaryBoundaryMap>`.
- Produces: `BinaryChainComplex::boundaries() -> &[BinaryBoundaryMap]`.
- Produces: `BinaryChainComplex::css_view(qubit_dimension: usize) -> qec_code::error::Result<HomologicalCssView>`.

- [x] **Step 1: Write the failing integration test**

Create `qec-code/tests/binary_chain_complex.rs` before production code:

```rust
use qec_code::binary_chain_complex::{BinaryBoundaryMap, BinaryChainComplex};
use qec_code::sparse_gf2::SparseGf2Matrix;
use qec_code::QecError;

fn square_complex(face_boundary: Vec<usize>) -> Result<BinaryChainComplex, QecError> {
    let boundary_1 = BinaryBoundaryMap::new(
        1,
        0,
        SparseGf2Matrix::new(
            4,
            4,
            vec![vec![0, 3], vec![0, 1], vec![1, 2], vec![2, 3]],
        )?,
    )?;
    let boundary_2 =
        BinaryBoundaryMap::new(2, 1, SparseGf2Matrix::new(4, 1, face_rows(4, face_boundary))?)?;

    BinaryChainComplex::new(vec![boundary_2, boundary_1])
}

fn face_rows(num_edges: usize, face_boundary: Vec<usize>) -> Vec<Vec<usize>> {
    let mut rows = vec![Vec::new(); num_edges];
    for edge in face_boundary {
        rows[edge].push(0);
    }
    rows
}

fn rows_are_orthogonal(hx: &SparseGf2Matrix, hz: &SparseGf2Matrix) -> bool {
    hx.rows().iter().all(|x_row| {
        hz.rows()
            .iter()
            .all(|z_row| sparse_dot_mod_2(x_row, z_row) == 0)
    })
}

fn sparse_dot_mod_2(left: &[usize], right: &[usize]) -> usize {
    let mut parity = 0;
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => {
                parity ^= 1;
                left_index += 1;
                right_index += 1;
            }
        }
    }
    parity
}

#[test]
fn square_cell_boundary_maps_match_fixture() {
    let complex = square_complex(vec![0, 1, 2, 3]).unwrap();

    let ordered_dimensions = complex
        .boundaries()
        .iter()
        .map(BinaryBoundaryMap::domain_dimension)
        .collect::<Vec<_>>();
    assert_eq!(ordered_dimensions, vec![1, 2]);

    let boundary_1 = complex.boundary(1).unwrap();
    assert_eq!(boundary_1.num_rows(), 4);
    assert_eq!(boundary_1.num_cols(), 4);
    assert_eq!(
        boundary_1.rows(),
        &[vec![0, 3], vec![0, 1], vec![1, 2], vec![2, 3]]
    );

    let boundary_2 = complex.boundary(2).unwrap();
    assert_eq!(boundary_2.num_rows(), 4);
    assert_eq!(boundary_2.num_cols(), 1);
    assert_eq!(boundary_2.rows(), &[vec![0], vec![0], vec![0], vec![0]]);

    let css = complex.css_view(1).unwrap();
    assert_eq!(css.qubit_dimension(), 1);
    assert_eq!(css.num_qubits(), 4);
    assert_eq!(css.num_x_checks(), 4);
    assert_eq!(css.num_z_checks(), 1);
    assert_eq!(
        css.hx().rows(),
        &[vec![0, 3], vec![0, 1], vec![1, 2], vec![2, 3]]
    );
    assert_eq!(css.hz().rows(), &[vec![0, 1, 2, 3]]);
    assert!(rows_are_orthogonal(css.hx(), css.hz()));
}

#[test]
fn corrupt_face_boundary_is_rejected() {
    assert_eq!(
        square_complex(vec![0, 1, 2]),
        Err(QecError::NonzeroBoundaryComposition {
            lower_dimension: 1,
            upper_dimension: 2,
            row: 0,
            support: vec![0],
        })
    );
}
```

- [x] **Step 2: Run focused test to verify RED**

Run:

```bash
cargo test -p qec-code --test binary_chain_complex square_cell_boundary_maps_match_fixture -- --exact
```

Expected: FAIL because `qec_code::binary_chain_complex` is not yet exported.

- [x] **Step 3: Add typed errors**

Modify `qec-code/src/error.rs` by adding these `QecError` variants after the
existing sparse GF(2) variants:

```rust
#[error(
    "invalid boundary map dimensions: domain dimension {domain_dimension}, codomain dimension {codomain_dimension}"
)]
InvalidBoundaryMapDimensions {
    domain_dimension: usize,
    codomain_dimension: usize,
},
#[error("duplicate boundary map for domain dimension {domain_dimension}")]
DuplicateBoundaryMapDimension { domain_dimension: usize },
#[error("missing boundary map for domain dimension {domain_dimension}")]
MissingBoundaryMap { domain_dimension: usize },
#[error(
    "boundary composition dimension mismatch between dimensions {lower_dimension} and {upper_dimension}: lower domain has {lower_domain_cells} cells, upper codomain has {upper_codomain_cells} cells"
)]
BoundaryCompositionDimensionMismatch {
    lower_dimension: usize,
    upper_dimension: usize,
    lower_domain_cells: usize,
    upper_codomain_cells: usize,
},
#[error(
    "nonzero boundary composition between dimensions {lower_dimension} and {upper_dimension}: row {row} has support {support:?}"
)]
NonzeroBoundaryComposition {
    lower_dimension: usize,
    upper_dimension: usize,
    row: usize,
    support: Vec<usize>,
},
```

- [x] **Step 4: Add the chain-complex module**

Create `qec-code/src/binary_chain_complex.rs`:

```rust
use crate::error::{QecError, Result};
use crate::sparse_gf2::SparseGf2Matrix;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryBoundaryMap {
    domain_dimension: usize,
    codomain_dimension: usize,
    matrix: SparseGf2Matrix,
}

impl BinaryBoundaryMap {
    pub fn new(
        domain_dimension: usize,
        codomain_dimension: usize,
        matrix: SparseGf2Matrix,
    ) -> Result<Self> {
        if codomain_dimension.checked_add(1) != Some(domain_dimension) {
            return Err(QecError::InvalidBoundaryMapDimensions {
                domain_dimension,
                codomain_dimension,
            });
        }

        Ok(Self {
            domain_dimension,
            codomain_dimension,
            matrix,
        })
    }

    pub fn domain_dimension(&self) -> usize {
        self.domain_dimension
    }

    pub fn codomain_dimension(&self) -> usize {
        self.codomain_dimension
    }

    pub fn matrix(&self) -> &SparseGf2Matrix {
        &self.matrix
    }

    pub fn num_domain_cells(&self) -> usize {
        self.matrix.num_cols()
    }

    pub fn num_codomain_cells(&self) -> usize {
        self.matrix.num_rows()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryChainComplex {
    boundaries: Vec<BinaryBoundaryMap>,
}

impl BinaryChainComplex {
    pub fn new(mut boundaries: Vec<BinaryBoundaryMap>) -> Result<Self> {
        boundaries.sort_by_key(BinaryBoundaryMap::domain_dimension);

        for pair in boundaries.windows(2) {
            if pair[0].domain_dimension == pair[1].domain_dimension {
                return Err(QecError::DuplicateBoundaryMapDimension {
                    domain_dimension: pair[0].domain_dimension,
                });
            }
        }

        for pair in boundaries.windows(2) {
            let lower = &pair[0];
            let upper = &pair[1];
            if lower.domain_dimension == upper.codomain_dimension {
                verify_zero_composition(lower, upper)?;
            }
        }

        Ok(Self { boundaries })
    }

    pub fn boundaries(&self) -> &[BinaryBoundaryMap] {
        &self.boundaries
    }

    pub fn boundary_map(&self, domain_dimension: usize) -> Option<&BinaryBoundaryMap> {
        self.boundaries
            .binary_search_by_key(&domain_dimension, BinaryBoundaryMap::domain_dimension)
            .ok()
            .map(|index| &self.boundaries[index])
    }

    pub fn boundary(&self, domain_dimension: usize) -> Option<&SparseGf2Matrix> {
        self.boundary_map(domain_dimension)
            .map(BinaryBoundaryMap::matrix)
    }

    pub fn css_view(&self, qubit_dimension: usize) -> Result<HomologicalCssView> {
        let hx = self
            .boundary(qubit_dimension)
            .ok_or(QecError::MissingBoundaryMap {
                domain_dimension: qubit_dimension,
            })?
            .clone();
        let upper_dimension =
            qubit_dimension
                .checked_add(1)
                .ok_or(QecError::MissingBoundaryMap {
                    domain_dimension: qubit_dimension,
                })?;
        let hz = self
            .boundary(upper_dimension)
            .ok_or(QecError::MissingBoundaryMap {
                domain_dimension: upper_dimension,
            })?
            .transpose()?;

        Ok(HomologicalCssView {
            qubit_dimension,
            hx,
            hz,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomologicalCssView {
    qubit_dimension: usize,
    hx: SparseGf2Matrix,
    hz: SparseGf2Matrix,
}

impl HomologicalCssView {
    pub fn qubit_dimension(&self) -> usize {
        self.qubit_dimension
    }

    pub fn hx(&self) -> &SparseGf2Matrix {
        &self.hx
    }

    pub fn hz(&self) -> &SparseGf2Matrix {
        &self.hz
    }

    pub fn num_qubits(&self) -> usize {
        self.hx.num_cols()
    }

    pub fn num_x_checks(&self) -> usize {
        self.hx.num_rows()
    }

    pub fn num_z_checks(&self) -> usize {
        self.hz.num_rows()
    }
}

fn verify_zero_composition(lower: &BinaryBoundaryMap, upper: &BinaryBoundaryMap) -> Result<()> {
    if lower.matrix.num_cols() != upper.matrix.num_rows() {
        return Err(QecError::BoundaryCompositionDimensionMismatch {
            lower_dimension: lower.domain_dimension,
            upper_dimension: upper.domain_dimension,
            lower_domain_cells: lower.matrix.num_cols(),
            upper_codomain_cells: upper.matrix.num_rows(),
        });
    }

    for (row_index, lower_row) in lower.matrix.rows().iter().enumerate() {
        let support = compose_row_support(lower_row, upper.matrix.rows());
        if !support.is_empty() {
            return Err(QecError::NonzeroBoundaryComposition {
                lower_dimension: lower.domain_dimension,
                upper_dimension: upper.domain_dimension,
                row: row_index,
                support,
            });
        }
    }

    Ok(())
}

fn compose_row_support(lower_row: &[usize], upper_rows: &[Vec<usize>]) -> Vec<usize> {
    let mut support = BTreeSet::new();
    for &intermediate_cell in lower_row {
        for &upper_support in &upper_rows[intermediate_cell] {
            if !support.insert(upper_support) {
                support.remove(&upper_support);
            }
        }
    }
    support.into_iter().collect()
}
```

- [x] **Step 5: Export the module**

Modify `qec-code/src/lib.rs` by adding:

```rust
pub mod binary_chain_complex;
```

Place it near the existing `pub mod binary;` line.

- [x] **Step 6: Run focused tests and fix only implementation defects**

Run:

```bash
cargo test -p qec-code --test binary_chain_complex square_cell_boundary_maps_match_fixture -- --exact
cargo test -p qec-code --test binary_chain_complex corrupt_face_boundary_is_rejected -- --exact
```

Expected: both PASS.

- [x] **Step 7: Format and run crate tests**

Run:

```bash
cargo fmt --all --check
cargo test -p qec-code
```

Expected: both PASS. If `cargo fmt --all --check` fails, run `cargo fmt --all`,
then re-run `cargo fmt --all --check` and `cargo test -p qec-code`.

- [x] **Step 8: Commit the implementation**

Run:

```bash
git add qec-code/src/binary_chain_complex.rs qec-code/src/lib.rs qec-code/src/error.rs qec-code/tests/binary_chain_complex.rs docs/superpowers/plans/2026-07-26-issue-565-binary-chain-complex.md
git commit -m "feat: add binary chain complex"
```

Expected: one implementation commit containing the production module, errors,
export, tests, and this plan.

## Self-Review

- Spec coverage: Task 1 covers checked boundary dimensions, composition-zero
  validation, square fixture exact rows, orthogonal CSS checks, deterministic
  ordering, nonzero-composition errors with dimensions and row support, pure
  Rust implementation, and reuse of `SparseGf2Matrix`.
- Placeholder scan: no unresolved markers or incomplete sections remain.
- Type consistency: public method names in the test match the production API
  signatures in the implementation step.
