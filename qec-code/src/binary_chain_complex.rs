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
