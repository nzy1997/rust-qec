//! Bounded exact Walsh characters of a complete mod-eight phase table.
use super::*;

impl PhaseCatalogue {
    /// Construct a separate certified catalogue for an arbitrary complete table.
    /// Coordinate zero is the least significant assignment bit. Each surviving
    /// character has coefficient `2^-dimension * sum_x (-1)^(mask.x) omega^table[x]`
    /// and the unconstrained Clifford phase `4 * parity(mask)`.
    ///
    /// `max_transform_work` bounds scalar additions/subtractions in the butterfly:
    /// exactly `dimension * 2^dimension`. Root construction and normalization
    /// also perform linear table work, bounded by `max_table_values`; arithmetic
    /// limits apply throughout. Certificate reconstruction has the independent
    /// `CatalogueLimits::max_verification_work` bound. Retained term/parity limits
    /// count only nonzero coefficients. No constraint rows are constructed.
    ///
    /// All corresponding shape, work and vector-capacity checks precede their
    /// allocations. Limits describe logical work/storage, not process memory.
    /// The returned catalogue owns its data. This constructor does not change
    /// runtime selection or any source catalogue, and has no fallback route.
    pub fn from_walsh_table(
        dimension: usize,
        phase_table: &[u8],
        max_transform_work: u128,
        limits: &CatalogueLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, CatalogueError> {
        let count = assignments(dimension, limits)?;
        if phase_table.len() != count {
            return Err(error(
                "catalogue.walsh.core_phase_mod8",
                format!(
                    "dimension {dimension} needs {count} phase values, found {}",
                    phase_table.len()
                ),
            ));
        }
        if let Some((index, _)) = phase_table.iter().enumerate().find(|(_, v)| **v > 7) {
            return Err(error(
                format!("catalogue.walsh.core_phase_mod8.{index}"),
                "phase must be an integer from zero through seven",
            ));
        }
        let work = (count as u128)
            .checked_mul(dimension as u128)
            .ok_or_else(|| error("catalogue.walsh.max_transform_work", "work exceeds u128"))?;
        limit(
            "catalogue.walsh.max_transform_work",
            work,
            max_transform_work,
        )?;
        limit("catalogue.max_entries", 1, limits.max_entries as u128)?;
        vector_capacity::<u8>("catalogue.walsh.phase_table_capacity", count as u128)?;
        vector_capacity::<ExactScalar>("catalogue.walsh.transform_capacity", count as u128)?;
        let mut coefficients = phase_table
            .iter()
            .map(|&phase| {
                arithmetic
                    .omega(i64::from(phase), 0)
                    .map_err(|e| error("catalogue.walsh.coefficient", e))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut half = 1;
        while half < count {
            for block in coefficients.chunks_exact_mut(2 * half) {
                for index in 0..half {
                    let left = block[index].clone();
                    let right = block[index + half].clone();
                    block[index] = arithmetic
                        .add(&left, &right)
                        .map_err(|e| error("catalogue.walsh.transform", e))?;
                    block[index + half] = arithmetic
                        .subtract(&left, &right)
                        .map_err(|e| error("catalogue.walsh.transform", e))?;
                }
            }
            half *= 2;
        }
        let terms = coefficients.iter().filter(|v| !v.is_zero()).count();
        let parities = coefficients
            .iter()
            .enumerate()
            .filter(|(mask, value)| *mask != 0 && !value.is_zero())
            .count();
        limit(
            "catalogue.max_terms",
            terms as u128,
            limits.max_terms as u128,
        )?;
        limit(
            "catalogue.max_parity_rows",
            parities as u128,
            limits.max_parity_rows as u128,
        )?;
        // Use the same planner as insertion, which independently reconstructs
        // from the returned indicators rather than this forward scratch buffer.
        let proof = verification::plan(dimension, count, terms as u128, 0, parities as u128, true)?;
        limit(
            "catalogue.max_verification_work",
            proof.work,
            limits.max_verification_work,
        )?;
        vector_capacity::<CliffordIndicator>("catalogue.walsh.terms_capacity", terms as u128)?;
        let exponent = i64::try_from(dimension)
            .map_err(|_| error("catalogue.walsh.dimension", "scalar exponent exceeds i64"))?;
        let normalization = arithmetic
            .scalar([1.into(), 0.into(), 0.into(), 0.into()], -exponent)
            .map_err(|e| error("catalogue.walsh.normalization", e))?;
        let mut indicators = Vec::with_capacity(terms);
        for (mask, coefficient) in coefficients.into_iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }
            let parities = if mask == 0 {
                Vec::new()
            } else {
                vec![(
                    BitVector::from_indices(
                        dimension,
                        (0..dimension).filter(|j| mask >> j & 1 != 0),
                    )
                    .map_err(|e| error("catalogue.walsh.parity", e))?,
                    4,
                )]
            };
            indicators.push(CliffordIndicator {
                coefficient: arithmetic
                    .multiply(&coefficient, &normalization)
                    .map_err(|e| error("catalogue.walsh.coefficient", e))?,
                constraints: Vec::new(),
                phase: ParityPhase::new(dimension, 0, parities)
                    .map_err(|e| error("catalogue.walsh.parity", e))?,
            });
        }
        let mut result = Self::new();
        result.insert(
            PhaseDecompositionInput {
                dimension,
                phase_table: phase_table.to_vec(),
                terms: indicators,
            },
            limits,
            arithmetic,
        )?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_capacity_matches_platform_layout_without_allocating() {
        let capacity = isize::MAX as usize / std::mem::size_of::<ExactScalar>();
        assert!(std::alloc::Layout::array::<ExactScalar>(capacity).is_ok());
        assert!(vector_capacity::<ExactScalar>("capacity", capacity as u128).is_ok());
        assert!(std::alloc::Layout::array::<ExactScalar>(capacity + 1).is_err());
        assert!(vector_capacity::<ExactScalar>("capacity", capacity as u128 + 1).is_err());
        assert!(vector_capacity::<ExactScalar>("capacity", u128::MAX).is_err());
    }
}
