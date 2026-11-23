//! Value-aware incremental affine knowledge with readonly, ordered requests.
//!
//! Masks use the existing explicit-width [`BitVector`]. The native sparse-word
//! and sorted-index representations are replaced by fixed-width vectors and a
//! hash index, while pending labels retain their first-seen order.

use crate::gf2::{BitVector, Gf2Error};
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CausalError {
    Vector(Gf2Error),
    ContradictoryObservation { pending: usize },
}

impl From<Gf2Error> for CausalError {
    fn from(error: Gf2Error) -> Self {
        Self::Vector(error)
    }
}

impl fmt::Display for CausalError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vector(error) => error.fmt(output),
            Self::ContradictoryObservation { pending } => write!(
                output,
                "sampled parity at pending label {pending} contradicts an existing relation"
            ),
        }
    }
}

impl std::error::Error for CausalError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CausalRequest {
    pending: Vec<BitVector>,
    outputs: Vec<(Option<usize>, bool)>,
}

impl CausalRequest {
    pub fn pending(&self) -> &[BitVector] {
        &self.pending
    }

    /// Each output XORs its constant with the optional pending-label value.
    pub fn outputs(&self) -> &[(Option<usize>, bool)] {
        &self.outputs
    }
}

#[derive(Clone, Debug)]
pub struct CausalKnowledge {
    width: usize,
    basis: Vec<Option<(BitVector, bool)>>,
    rows: Vec<BitVector>,
    values: Vec<bool>,
}

fn highest(mask: &BitVector) -> Option<usize> {
    mask.words()
        .iter()
        .enumerate()
        .rev()
        .find(|(_, word)| **word != 0)
        .map(|(index, word)| index * 64 + 63 - word.leading_zeros() as usize)
}

impl CausalKnowledge {
    pub fn new(width: usize) -> Self {
        Self {
            width,
            basis: Vec::new(),
            rows: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    /// Newly independent observations, in pending-label order. Direct inserts
    /// update the basis without adding to this observation history.
    pub fn rows(&self) -> &[BitVector] {
        &self.rows
    }

    pub fn values(&self) -> &[bool] {
        &self.values
    }

    fn check_width(&self, mask: &BitVector) -> Result<(), CausalError> {
        if mask.width() != self.width {
            return Err(Gf2Error::WidthMismatch {
                expected: self.width,
                actual: mask.width(),
            }
            .into());
        }
        Ok(())
    }

    /// Reduction stops at the first unknown highest pivot. Lower known pivots
    /// are deliberately retained until that high pivot can be eliminated.
    pub fn reduce(
        &self,
        mask: &BitVector,
        mut bit: bool,
    ) -> Result<(BitVector, bool), CausalError> {
        self.check_width(mask)?;
        let mut residual = mask.clone();
        while let Some(pivot) = highest(&residual) {
            let Some(Some((row, value))) = self.basis.get(pivot) else {
                break;
            };
            residual.xor_assign(row)?;
            bit ^= value;
        }
        Ok((residual, bit))
    }

    /// Return whether a basis row was added and the reduced affine bit.
    /// A dependent row with returned bit `true` exposes a contradiction.
    pub fn insert(&mut self, mask: &BitVector, bit: bool) -> Result<(bool, bool), CausalError> {
        let (residual, value) = self.reduce(mask, bit)?;
        let Some(pivot) = highest(&residual) else {
            return Ok((false, value));
        };
        if pivot >= self.basis.len() {
            self.basis
                .try_reserve(pivot + 1 - self.basis.len())
                .map_err(|_| Gf2Error::AllocationFailed)?;
            self.basis.resize_with(pivot + 1, || None);
        }
        self.basis[pivot] = Some((residual, value));
        Ok((true, value))
    }

    pub fn request(&self, expressions: &[(BitVector, bool)]) -> Result<CausalRequest, CausalError> {
        let mut pending = Vec::new();
        let mut outputs = Vec::with_capacity(expressions.len());
        let mut indices = HashMap::new();
        for (mask, offset) in expressions {
            let (residual, known) = self.reduce(mask, false)?;
            let index = if residual.is_zero() {
                None
            } else {
                let next = pending.len();
                let index = *indices.entry(residual.clone()).or_insert_with(|| {
                    pending.push(residual);
                    next
                });
                Some(index)
            };
            outputs.push((index, *offset ^ known));
        }
        Ok(CausalRequest { pending, outputs })
    }

    /// Validate the word before inserting observations. A later contradictory
    /// dependent label returns an error while retaining preceding independent
    /// observations, matching the source's ordered partial-update behavior.
    /// Extra declared word width is accepted when its extra bits are zero.
    pub fn observe(
        &mut self,
        request: &CausalRequest,
        word: &BitVector,
    ) -> Result<Vec<bool>, CausalError> {
        for mask in &request.pending {
            self.check_width(mask)?;
        }
        if let Some(index) = word.ones().find(|index| *index >= request.pending.len()) {
            return Err(Gf2Error::IndexOutOfBounds {
                index,
                width: request.pending.len(),
            }
            .into());
        }
        let bit = |index| index < word.width() && word.get(index).expect("checked word coordinate");
        for (index, mask) in request.pending.iter().enumerate() {
            let value = bit(index);
            let (added, contradiction) = self.insert(mask, value)?;
            if added {
                self.rows.push(mask.clone());
                self.values.push(value);
            } else if contradiction {
                return Err(CausalError::ContradictoryObservation { pending: index });
            }
        }
        Ok(request
            .outputs
            .iter()
            .map(|(index, constant)| *constant ^ index.is_some_and(bit))
            .collect())
    }
}
