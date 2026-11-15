//! Classical suffix proposals after an exactly certified zero physical prefix.
use super::*;
use crate::gf2::AffineSupport;
use crate::records::{AffineProposalDraw, sample_affine_family};

impl PathState<'_> {
    /// A zero-mass branch still executes callbacks and guarded operations.
    /// Preserve every already observed affine relation, including duplicate and
    /// inverted inputs, while assigning a positive proposal to its completion.
    /// This never supplies a physical probability or rescues a positive branch.
    pub(super) fn complete_zero_prefix(
        &self,
        inputs: &[BitExpr],
        random: &mut impl RandomSource,
        options: &SamplingOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<AffineProposalDraw, RuntimeError> {
        observe(self.meter, OperationKind::Support, || {
            let labels = self.conditions.len().checked_add(inputs.len()).ok_or(
                RuntimeError::ResourceLimit {
                    option: "max_history_inputs",
                    limit: self.program.options.max_history_inputs,
                },
            )?;
            let occurrences = self
                .conditions
                .iter()
                .map(|(p, _)| p)
                .chain(inputs)
                .fold(0_u128, |sum, p| sum.saturating_add(p.terms.len() as u128));
            // Joint-label columns avoid a nullspace in the full physical record
            // universe. Charge copies, elimination and packed scratch before
            // allocating any label vectors. Scalar limbs have separate bounds.
            let n = labels as u128;
            let words = 16_u128
                .saturating_mul(occurrences.saturating_add(8 * n + 128))
                .saturating_mul(n.div_ceil(64).saturating_add(8));
            let limit = self.program.options.max_zero_prefix_words;
            if words > limit as u128 {
                return Err(RuntimeError::ResourceLimit {
                    option: "max_zero_prefix_words",
                    limit,
                });
            }
            let mut offset = BitVector::zeros(labels).map_err(InstrumentError::from)?;
            let mut columns = BTreeMap::<usize, BitVector>::new();
            for (j, parity) in self
                .conditions
                .iter()
                .map(|(p, _)| p)
                .chain(inputs)
                .enumerate()
            {
                offset
                    .set(j, parity.constant)
                    .map_err(InstrumentError::from)?;
                for atom in &parity.terms {
                    let BitAtom::Record(record) = atom else {
                        return Err(InstrumentError::NonAffineQuery.into());
                    };
                    if !columns.contains_key(record) {
                        columns.insert(
                            *record,
                            BitVector::zeros(labels).map_err(InstrumentError::from)?,
                        );
                    }
                    columns
                        .get_mut(record)
                        .expect("inserted record column")
                        .set(j, true)
                        .map_err(InstrumentError::from)?;
                }
            }
            let family = AffineSupport::new(offset, columns.into_values().collect())
                .map_err(InstrumentError::from)?;
            let equations = (0..self.conditions.len())
                .map(|j| BitVector::from_indices(labels, [j]))
                .collect::<Result<Vec<_>, _>>()
                .map_err(InstrumentError::from)?;
            let rhs = BitVector::from_indices(
                self.conditions.len(),
                self.conditions
                    .iter()
                    .enumerate()
                    .filter_map(|(j, (_, v))| v.then_some(j)),
            )
            .map_err(InstrumentError::from)?;
            let conditioned = family
                .condition(&equations, &rhs)
                .map_err(InstrumentError::from)?
                .ok_or(SamplingError::ImpossibleSupport)?;
            let keep = (self.conditions.len()..labels)
                .map(|j| BitVector::from_indices(labels, [j]))
                .collect::<Result<Vec<_>, _>>()
                .map_err(InstrumentError::from)?;
            let family = conditioned
                .linear_image(&keep)
                .map_err(InstrumentError::from)?;
            Ok(sample_affine_family(&family, random, options, arithmetic)?)
        })
    }
}
