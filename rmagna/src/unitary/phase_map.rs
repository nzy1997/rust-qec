//! Fixed coordinate maps for composing phase legs on retained boundaries.
use super::*;
use std::collections::HashMap;

pub(super) struct PhaseRestriction {
    widths: Vec<usize>,
    width: usize,
    output_width: usize,
    rows: Vec<BitVector>,
    axes: Vec<HashMap<BitVector, BitVector>>,
}
impl PhaseRestriction {
    pub(super) fn new(
        widths: &[usize],
        phases: &[(&ParityPhase, bool)],
        columns: &[BitVector],
    ) -> Result<Self, UnitaryError> {
        let width = widths
            .iter()
            .try_fold(0_usize, |a, &b| a.checked_add(b))
            .ok_or(UnitaryError::DimensionOverflow)?;
        if widths.len() != phases.len()
            || widths
                .iter()
                .zip(phases)
                .any(|(&n, (p, _))| n != p.num_variables())
            || columns.iter().any(|v| v.width() != width)
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        let mut plan = Self {
            widths: widths.to_vec(),
            width,
            output_width: columns.len(),
            rows: crate::gf2::transpose_columns(width, columns)?,
            axes: Vec::with_capacity(phases.len()),
        };
        let mut offset = 0;
        for ((phase, _), &n) in phases.iter().zip(widths) {
            let axes = phase
                .terms()
                .iter()
                .map(|(axis, _)| Ok((axis.clone(), plan.map_axis(axis, offset)?)))
                .collect::<Result<_, UnitaryError>>()?;
            plan.axes.push(axes);
            offset += n;
        }
        Ok(plan)
    }
    fn map_axis(&self, axis: &BitVector, offset: usize) -> Result<BitVector, UnitaryError> {
        let mut mapped = BitVector::zeros(self.output_width)?;
        for j in axis.ones() {
            mapped.xor_assign(&self.rows[offset + j])?;
        }
        Ok(mapped)
    }
    pub(super) fn apply(
        &self,
        phases: &[(&ParityPhase, bool)],
        shift: &BitVector,
    ) -> Result<ParityPhase, UnitaryError> {
        if phases.len() != self.widths.len()
            || shift.width() != self.width
            || phases
                .iter()
                .zip(&self.widths)
                .any(|((p, _), &n)| p.num_variables() != n)
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        let mut constant = 0;
        let mut terms = Vec::new();
        let mut offset = 0;
        for (leg, ((phase, conjugate), &n)) in phases.iter().zip(&self.widths).enumerate() {
            let sign = |c: u8| if *conjugate { (8 - c) & 7 } else { c };
            constant = (constant + sign(phase.constant())) & 7;
            for (axis, coefficient) in phase.terms() {
                let mut coefficient = sign(*coefficient);
                let flipped = axis
                    .ones()
                    .try_fold(false, |bit, j| shift.get(offset + j).map(|v| bit ^ v))?;
                if flipped {
                    constant = (constant + coefficient) & 7;
                    coefficient = (8 - coefficient) & 7;
                }
                // Reference cancellation can hide an axis that reappears after
                // Pauli sign updates. Map such axes exactly without mutating
                // this immutable reference table or growing a warm cache.
                let mapped = match self.axes[leg].get(axis) {
                    Some(mapped) => mapped.clone(),
                    None => self.map_axis(axis, offset)?,
                };
                terms.push((mapped, coefficient));
            }
            offset += n;
        }
        Ok(ParityPhase::new(self.output_width, constant, terms)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn next(s: &mut u64) -> u64 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    }
    fn vector(width: usize, seed: &mut u64) -> BitVector {
        BitVector::from_indices(width, (0..width).filter(|_| next(seed) & 3 == 0)).unwrap()
    }
    #[test]
    fn mapped_phase_legs_match_independent_truth_tables_across_word_boundaries() {
        let widths = [65, 0, 64, 1];
        let mut seed = 20261021;
        let reference = widths
            .iter()
            .map(|&width| ParityPhase::new(width, 3, [(vector(width, &mut seed), 1)]).unwrap())
            .collect::<Vec<_>>();
        let original = reference
            .iter()
            .enumerate()
            .map(|(j, p)| (p, j % 2 == 0))
            .collect::<Vec<_>>();
        let mut columns = (0..130).map(|_| vector(130, &mut seed)).collect::<Vec<_>>();
        columns[129] = columns[64].clone(); // Dependence and zero images are valid.
        let plan = PhaseRestriction::new(&widths, &original, &columns).unwrap();
        for _ in 0..32 {
            let shift = vector(130, &mut seed);
            let updates = widths
                .iter()
                .zip(&reference)
                .map(|(&width, reference)| {
                    let mut terms = reference.terms().to_vec();
                    terms.extend(
                        (0..6).map(|_| (vector(width, &mut seed), (next(&mut seed) & 7) as u8)),
                    );
                    if let Some((axis, _)) = terms.first().cloned() {
                        terms.push((axis, 7)); // Reference-axis cancellation may remove it.
                    }
                    ParityPhase::new(width, (next(&mut seed) & 7) as u8, terms).unwrap()
                })
                .collect::<Vec<_>>();
            let phases = updates
                .iter()
                .enumerate()
                .map(|(j, p)| (p, j % 2 == 0))
                .collect::<Vec<_>>();
            let mapped = plan.apply(&phases, &shift).unwrap();
            for bits in 0..128 {
                let selected = [0, 1, 63, 64, 65, 128, 129];
                let value = BitVector::from_indices(
                    130,
                    selected
                        .into_iter()
                        .enumerate()
                        .filter_map(|(j, k)| (bits >> j & 1 != 0).then_some(k)),
                )
                .unwrap();
                let mut input = shift.clone();
                for j in value.ones() {
                    input.xor_assign(&columns[j]).unwrap();
                }
                let mut expected = 0;
                let mut offset = 0;
                for ((phase, conjugate), &width) in phases.iter().zip(&widths) {
                    let local = BitVector::from_indices(
                        width,
                        (0..width).filter(|&j| input.get(offset + j).unwrap()),
                    )
                    .unwrap();
                    let v = phase.evaluate(&local).unwrap();
                    expected = (expected + if *conjugate { (8 - v) & 7 } else { v }) & 7;
                    offset += width;
                }
                assert_eq!(mapped.evaluate(&value).unwrap(), expected);
            }
        }
    }
    #[test]
    fn scalar_empty_and_invalid_phase_maps_are_explicit() {
        let scalar = ParityPhase::new(0, 5, []).unwrap();
        let zero = BitVector::zeros(0).unwrap();
        let plan =
            PhaseRestriction::new(&[0, 0], &[(&scalar, false), (&scalar, true)], &[]).unwrap();
        assert_eq!(
            plan.apply(&[(&scalar, false), (&scalar, true)], &zero)
                .unwrap()
                .evaluate(&zero)
                .unwrap(),
            0
        );
        assert!(matches!(
            plan.apply(&[], &zero),
            Err(UnitaryError::DimensionMismatch)
        ));
        assert!(matches!(
            plan.apply(
                &[(&scalar, false), (&scalar, true)],
                &BitVector::zeros(1).unwrap()
            ),
            Err(UnitaryError::DimensionMismatch)
        ));
        assert!(matches!(
            PhaseRestriction::new(&[usize::MAX, 1], &[], &[]),
            Err(UnitaryError::DimensionOverflow)
        ));
        assert!(matches!(
            PhaseRestriction::new(&[1], &[(&scalar, false)], &[]),
            Err(UnitaryError::DimensionMismatch)
        ));
        assert!(matches!(
            PhaseRestriction::new(&[0], &[(&scalar, false)], &[BitVector::zeros(1).unwrap()]),
            Err(UnitaryError::DimensionMismatch)
        ));
    }
}
