//! Stable, dependency-facing packed GF(2) primitives.
//!
//! This module deliberately wraps the crate's internal elimination machinery
//! instead of exposing its storage layout. Downstream crates can use packed
//! rows, reusable kernel workspaces, and reduced row-space membership without
//! depending on private implementation details.
//!
//! # Example
//!
//! ```
//! use qec_code::packed_gf2::{KernelWorkspace, PackedRow, ReducedRowSpace};
//!
//! let rows = vec![vec![1, 1, 0], vec![0, 1, 1]];
//! let span = ReducedRowSpace::from_dense_rows(&rows, 3)?;
//! let target = PackedRow::from_dense(&[1, 0, 1])?;
//! assert!(span.contains(&target)?);
//!
//! let mut workspace = KernelWorkspace::new();
//! let basis = workspace.kernel_basis(&rows, 3, &[0, 1, 2])?;
//! assert_eq!(basis, &[vec![1, 1, 1]]);
//! # Ok::<(), qec_code::QecError>(())
//! ```

use crate::error::Result;
use crate::gf2;

/// A binary row represented internally by packed machine words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedRow {
    inner: gf2::BitPackedRow,
}

impl PackedRow {
    /// Pack one dense binary row.
    pub fn from_dense(row: &[u8]) -> Result<Self> {
        Ok(Self {
            inner: gf2::BitPackedRow::try_from_dense(row, row.len())?,
        })
    }

    /// Construct an all-zero row with an explicit width.
    pub fn zeros(width: usize) -> Self {
        Self {
            inner: gf2::BitPackedRow::zeros(width),
        }
    }

    /// Return the logical width, excluding storage padding.
    pub fn width(&self) -> usize {
        self.inner.width()
    }

    /// Read one logical bit, rejecting an out-of-range index.
    pub fn bit(&self, index: usize) -> Result<u8> {
        self.inner.try_bit(index)
    }

    /// Convert the packed row back to a dense binary vector.
    pub fn to_dense(&self) -> Vec<u8> {
        self.inner.to_dense()
    }

    /// XOR another row into this row.
    pub fn xor_assign(&mut self, rhs: &Self) -> Result<()> {
        self.inner.xor_assign(&rhs.inner)
    }

    /// Return the GF(2) dot product.
    pub fn dot_parity(&self, rhs: &Self) -> Result<u8> {
        self.inner.dot_parity(&rhs.inner)
    }

    /// Return the Hamming weight.
    pub fn weight(&self) -> usize {
        self.inner.weight()
    }

    /// Return whether every logical bit is zero.
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }
}

/// A row space reduced once for repeated packed membership queries.
#[derive(Debug, Clone)]
pub struct ReducedRowSpace {
    inner: gf2::PackedReducedRows,
    rank: usize,
}

impl ReducedRowSpace {
    /// Reduce dense binary rows with an explicit width.
    ///
    /// An explicit width preserves the meaning of an empty matrix.
    pub fn from_dense_rows(rows: &[Vec<u8>], width: usize) -> Result<Self> {
        let reduced = gf2::try_rref_with_width(rows, width)?;
        let rank = reduced.pivot_cols.len();
        Ok(Self {
            inner: gf2::PackedReducedRows::try_from_reduced_rows(&reduced)?,
            rank,
        })
    }

    /// Return the ambient vector width.
    pub fn width(&self) -> usize {
        self.inner.width()
    }

    /// Return the dimension of the row space.
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// Test whether a packed row belongs to this row space.
    pub fn contains(&self, target: &PackedRow) -> Result<bool> {
        gf2::try_in_packed_reduced_row_span(&self.inner, &target.inner)
    }

    /// Pack a dense target and test row-space membership.
    pub fn contains_dense(&self, target: &[u8]) -> Result<bool> {
        self.contains(&PackedRow::from_dense(target)?)
    }
}

/// Reusable allocation workspace for permuted GF(2) kernel bases.
#[derive(Debug, Default)]
pub struct KernelWorkspace {
    inner: gf2::RandomWindowKernelWorkspace,
}

impl KernelWorkspace {
    /// Construct an empty workspace.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute a kernel basis after applying a column permutation.
    ///
    /// `column_permutation[permuted_column]` names the corresponding input
    /// column. The returned rows use the original column order and borrow the
    /// workspace until the next mutable call.
    pub fn kernel_basis(
        &mut self,
        matrix: &[Vec<u8>],
        width: usize,
        column_permutation: &[usize],
    ) -> Result<&[Vec<u8>]> {
        self.inner
            .try_kernel_basis_with_width(matrix, width, column_permutation)
    }
}
