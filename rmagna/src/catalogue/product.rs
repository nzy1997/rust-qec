//! Independent positive-T cores from consecutive certified one-/two-bit factors.
use super::*;

fn popcount_table(dimension: usize) -> Vec<u8> {
    (0_usize..1 << dimension)
        .map(|word| (word.count_ones() & 7) as u8)
        .collect()
}
fn shifted(row: &BitVector, dimension: usize, offset: usize) -> Result<BitVector, CatalogueError> {
    BitVector::from_indices(dimension, row.ones().map(|j| j + offset))
        .map_err(|e| error("catalogue.product", e))
}
fn checked(value: Option<u128>) -> Result<u128, CatalogueError> {
    value.ok_or_else(|| error("catalogue.product", "product size exceeds u128"))
}
fn product_rows(a: u128, b: u128, c: u128, d: u128) -> Result<u128, CatalogueError> {
    checked(checked(a.checked_mul(b))?.checked_add(checked(c.checked_mul(d))?))
}
impl PhaseCatalogue {
    /// Construct a separate, single-entry catalogue for `sum_j x_j (mod 8)`.
    /// Consecutive pairs use the certified two-T core; an odd last coordinate
    /// uses the one-T core. Missing required factors return None, without trying
    /// a different grouping. Dimension zero is the unit, even for an empty source.
    ///
    /// Limits apply to the returned catalogue, independently of source usage.
    /// All product row counts and full reconstruction work are checked before
    /// the Cartesian product is allocated. Vector byte capacities are checked
    /// separately from logical quotas; this does not guarantee available memory.
    /// Arithmetic/validation failures remain errors. The source and its shared
    /// immutable certificates are unchanged.
    /// This does not search arbitrary component decompositions or select a
    /// runtime phase resource.
    pub fn paired_t_product(
        &self,
        dimension: usize,
        limits: &CatalogueLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Option<Self>, CatalogueError> {
        let count = assignments(dimension, limits)?;
        vector_capacity::<u8>("catalogue.product.phase_table_capacity", count as u128)?;
        let mut factors = Vec::with_capacity(dimension.div_ceil(2));
        for offset in (0..dimension).step_by(2) {
            let width = (dimension - offset).min(2);
            let Some(entry) = self.get(width, &popcount_table(width)) else {
                return Ok(None);
            };
            factors.push((offset, entry));
        }
        let mut terms = 1_u128;
        let mut constraints = 0_u128;
        let mut parities = 0_u128;
        let mut max_term_constraints = 0_u128;
        let mut max_term_parities = 0_u128;
        for (_, entry) in &factors {
            let next = entry.terms().len() as u128;
            let rows = entry
                .terms()
                .iter()
                .map(|t| t.constraints.len() as u128)
                .sum::<u128>();
            let phases = entry
                .terms()
                .iter()
                .map(|t| t.phase.terms().len() as u128)
                .sum::<u128>();
            constraints = product_rows(constraints, next, terms, rows)?;
            parities = product_rows(parities, next, terms, phases)?;
            terms = checked(terms.checked_mul(next))?;
            max_term_constraints = checked(
                max_term_constraints.checked_add(
                    entry
                        .terms()
                        .iter()
                        .map(|t| t.constraints.len() as u128)
                        .max()
                        .unwrap_or(0),
                ),
            )?;
            max_term_parities = checked(
                max_term_parities.checked_add(
                    entry
                        .terms()
                        .iter()
                        .map(|t| t.phase.terms().len() as u128)
                        .max()
                        .unwrap_or(0),
                ),
            )?;
            limit("catalogue.max_terms", terms, limits.max_terms as u128)?;
            limit(
                "catalogue.max_constraint_rows",
                constraints,
                limits.max_constraint_rows as u128,
            )?;
            limit(
                "catalogue.max_parity_rows",
                parities,
                limits.max_parity_rows as u128,
            )?;
        }
        limit("catalogue.max_entries", 1, limits.max_entries as u128)?;
        limit("catalogue.max_terms", terms, limits.max_terms as u128)?;
        let rows = checked(
            checked(checked(1_u128.checked_add(terms))?.checked_add(constraints))?
                .checked_add(parities),
        )?;
        let work = checked((count as u128).checked_mul(rows))?;
        limit(
            "catalogue.max_verification_work",
            work,
            limits.max_verification_work,
        )?;
        vector_capacity::<CliffordIndicator>("catalogue.product.terms_capacity", terms)?;
        vector_capacity::<(BitVector, bool)>(
            "catalogue.product.constraint_capacity",
            max_term_constraints,
        )?;
        vector_capacity::<(BitVector, u8)>("catalogue.product.parity_capacity", max_term_parities)?;
        let mut product = vec![CliffordIndicator {
            coefficient: ExactScalar::one(),
            constraints: vec![],
            phase: ParityPhase::new(dimension, 0, []).map_err(|e| error("catalogue.product", e))?,
        }];
        for (offset, entry) in factors {
            let mut next = Vec::with_capacity(product.len() * entry.terms().len());
            for left in &product {
                for right in entry.terms() {
                    let coefficient = arithmetic
                        .multiply(&left.coefficient, &right.coefficient)
                        .map_err(|e| error("catalogue.product.coefficient", e))?;
                    let mut constraints =
                        Vec::with_capacity(left.constraints.len() + right.constraints.len());
                    constraints.extend_from_slice(&left.constraints);
                    for (row, bit) in &right.constraints {
                        constraints.push((shifted(row, dimension, offset)?, *bit));
                    }
                    let mut phases =
                        Vec::with_capacity(left.phase.terms().len() + right.phase.terms().len());
                    phases.extend_from_slice(left.phase.terms());
                    for (row, c) in right.phase.terms() {
                        phases.push((shifted(row, dimension, offset)?, *c));
                    }
                    let phase = ParityPhase::new(
                        dimension,
                        (left.phase.constant() + right.phase.constant()) & 7,
                        phases,
                    )
                    .map_err(|e| error("catalogue.product", e))?;
                    next.push(CliffordIndicator {
                        coefficient,
                        constraints,
                        phase,
                    });
                }
            }
            product = next;
        }
        let mut result = Self::new();
        result.insert(
            PhaseDecompositionInput {
                dimension,
                phase_table: popcount_table(dimension),
                terms: product,
            },
            limits,
            arithmetic,
        )?;
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capacity_boundary<T>() {
        let capacity = (isize::MAX as usize / std::mem::size_of::<T>()) as u128;
        assert!(std::alloc::Layout::array::<T>(capacity as usize).is_ok());
        assert!(vector_capacity::<T>("capacity", capacity).is_ok());
        assert!(std::alloc::Layout::array::<T>((capacity + 1) as usize).is_err());
        let e = vector_capacity::<T>("capacity", capacity + 1).unwrap_err();
        assert_eq!(e.field, "capacity");
        assert_eq!(
            e.message,
            format!(
                "needs at least {}; configured limit is {capacity}",
                capacity + 1
            )
        );
        assert!(vector_capacity::<T>("capacity", u128::MAX).is_err());
    }

    #[test]
    fn vector_capacity_matches_platform_layout_without_allocating() {
        capacity_boundary::<u8>();
        capacity_boundary::<CliffordIndicator>();
        capacity_boundary::<(BitVector, bool)>();
        capacity_boundary::<(BitVector, u8)>();
    }
}
