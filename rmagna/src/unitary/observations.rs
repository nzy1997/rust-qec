//! Fixed-input affine relations between frozen commuting observations.
use super::*;
use crate::gf2::{LinearFactorization, transpose_columns};

pub(super) struct ObservationReduction {
    physical: usize,
    shared: usize,
    selected: Vec<usize>,
    relations: Vec<BitVector>,
    phase_columns: Vec<BitVector>,
    keep: Vec<usize>,
    columns: Vec<BitVector>,
}
impl ObservationReduction {
    pub(super) fn new(
        reference: &AffineKet,
        physical: usize,
        shared: usize,
    ) -> Result<Option<Self>, UnitaryError> {
        let outside = reference
            .width()
            .checked_sub(physical)
            .and_then(|n| n.checked_sub(shared))
            .ok_or(UnitaryError::DimensionMismatch)?;
        let rows = transpose_columns(reference.width(), reference.columns())?;
        let shared_rows = &rows[physical..physical + shared];
        let factor = LinearFactorization::new(
            shared,
            &transpose_columns(reference.columns().len(), shared_rows)?,
        )?;
        let selected = factor.pivot_columns().to_vec();
        let rank = selected.len();
        if rank == shared {
            return Ok(None);
        }
        let basis = selected
            .iter()
            .map(|&j| shared_rows[j].clone())
            .collect::<Vec<_>>();
        let solve =
            LinearFactorization::new(rank, &transpose_columns(reference.columns().len(), &basis)?)?;
        let relations = shared_rows
            .iter()
            .map(|row| {
                solve
                    .solve(row)
                    .map(|s| s.expect("independent observation row basis"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut phase_columns = (0..rank)
            .map(|j| {
                BitVector::from_indices(
                    shared + outside,
                    relations.iter().enumerate().filter_map(|(i, row)| {
                        row.get(j).expect("validated coordinate").then_some(i)
                    }),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        phase_columns.extend(
            (0..outside)
                .map(|j| BitVector::from_indices(shared + outside, [shared + j]))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let keep = (0..physical)
            .chain(selected.iter().map(|&j| physical + j))
            .chain(physical + shared..reference.width())
            .collect::<Vec<_>>();
        let columns = reference
            .columns()
            .iter()
            .map(|column| project(column, &keep))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Self {
            physical,
            shared,
            selected,
            relations,
            phase_columns,
            keep,
            columns,
        }))
    }
    pub(super) fn rank(&self) -> usize {
        self.selected.len()
    }
    pub(super) fn apply(
        &self,
        ket: &AffineKet,
        phase: &ParityPhase,
    ) -> Result<(Arc<AffineKet>, Arc<ParityPhase>), UnitaryError> {
        if ket.width().checked_sub(self.physical) != Some(phase.num_variables())
            || ket.columns().len() != self.columns.len()
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        let selected_offset = BitVector::from_indices(
            self.rank(),
            self.selected.iter().enumerate().filter_map(|(j, &old)| {
                ket.offset()
                    .get(self.physical + old)
                    .expect("validated observation")
                    .then_some(j)
            }),
        )?;
        let mut shift = BitVector::zeros(phase.num_variables())?;
        for (j, relation) in self.relations.iter().enumerate() {
            shift.set(
                j,
                ket.offset().get(self.physical + j)? ^ relation.dot(&selected_offset)?,
            )?;
        }
        debug_assert_eq!(self.relations.len(), self.shared);
        let phase = Arc::new(phase.restrict(&shift, &self.phase_columns)?);
        // Dropped labels are functions of retained labels. This projection is
        // injective on the original support, so no hidden paths are introduced.
        let ket = Arc::new(AffineKet::new(
            project(ket.offset(), &self.keep)?,
            self.columns.clone(),
            ket.phase().clone(),
            ket.coefficient().clone(),
        )?);
        Ok((ket, phase))
    }
}
fn project(value: &BitVector, keep: &[usize]) -> Result<BitVector, Gf2Error> {
    BitVector::from_indices(
        keep.len(),
        keep.iter().enumerate().filter_map(|(j, &old)| {
            value
                .get(old)
                .expect("validated retained label")
                .then_some(j)
        }),
    )
}
