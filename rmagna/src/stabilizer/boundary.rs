//! Immutable computational-parity boundary maps for fixed support columns.
use super::*;

/// A fixed computational parity map. Offsets, coefficients and the complete
/// quadratic phase may change, while source columns and width must agree.
/// This relabels boundary indices; it does not sum internal paths.
pub struct PreparedLinearBoundary {
    width: usize,
    source_columns: Vec<BitVector>,
    rows: Vec<BitVector>,
    mapped: AffineKet,
}
impl PreparedLinearBoundary {
    pub fn new(reference: &AffineKet, rows: &[BitVector]) -> Result<Self, StabilizerError> {
        Ok(Self {
            width: reference.width(),
            source_columns: reference.columns().to_vec(),
            rows: rows.to_vec(),
            mapped: reference.linear_boundary(rows)?,
        })
    }
    pub(crate) fn reference(&self) -> &AffineKet {
        &self.mapped
    }
    /// Reject structural incompatibility even when the input coefficient is zero.
    pub fn apply(&self, input: &AffineKet) -> Result<AffineKet, StabilizerError> {
        if input.width() != self.width || input.columns() != self.source_columns {
            return Err(StabilizerError::IncompatibleBoundaryMap);
        }
        let mut offset = BitVector::zeros(self.rows.len())?;
        for (j, row) in self.rows.iter().enumerate() {
            offset.set(j, row.dot(input.offset())?)?;
        }
        AffineKet::new(
            offset,
            self.mapped.columns().to_vec(),
            input.phase().clone(),
            input.coefficient().clone(),
        )
    }
}
