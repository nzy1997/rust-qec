//! Conservative positive affine-mixture certificates for completed masses.
use super::*;
use crate::gf2::{AffineSupport, LinearFactorization};
use std::collections::BTreeMap;

pub struct CompletedAffineComponent {
    support: AffineSupport,
    mass_per_label: ExactScalar,
    total_mass: ExactScalar,
}
impl CompletedAffineComponent {
    pub fn support(&self) -> &AffineSupport {
        &self.support
    }
    pub fn mass_per_label(&self) -> &ExactScalar {
        &self.mass_per_label
    }
    pub fn total_mass(&self) -> &ExactScalar {
        &self.total_mass
    }
}
/// An owned certificate. Components may overlap; their positive masses add.
/// An empty certificate denotes an identically zero completed marginal.
pub struct CompletedAffineMixture {
    components: Vec<CompletedAffineComponent>,
    total_mass: ExactScalar,
}
impl CompletedAffineMixture {
    pub fn components(&self) -> &[CompletedAffineComponent] {
        &self.components
    }
    pub fn total_mass(&self) -> &ExactScalar {
        &self.total_mass
    }
}
impl CompletedProbability {
    /// Certify a positive mixture of uniform affine laws. Signed or complex
    /// pieces on the same affine set are combined before checking positivity.
    /// Variable phases or remaining nonpositive groups return `None`, which
    /// does not prove that the complete marginal is nonpositive. This prepares
    /// no sampler and does not change the existing sampling stream.
    pub fn affine_mixture(
        &self,
        a: &ScalarArithmetic,
    ) -> Result<Option<CompletedAffineMixture>, CompletedRecordError> {
        type Key = (Vec<u64>, Vec<Vec<u64>>);
        let mut groups: BTreeMap<Key, (AffineSupport, ExactScalar)> = BTreeMap::new();
        for source in &self.0.terms {
            let Some(term) = source.contract(a)? else {
                continue;
            };
            if term.phase().linear().iter().any(|&c| c != 0)
                || term.phase().upper().iter().any(|row| !row.is_zero())
            {
                return Ok(None);
            }
            let mass = a.multiply(
                term.coefficient(),
                &a.omega(2 * i64::from(term.phase().constant()), 0)?,
            )?;
            let factor = LinearFactorization::new(self.width(), term.columns())?;
            let columns = factor.row_space_basis().to_vec();
            let mut offset = term.offset().clone();
            for (row, &pivot) in columns.iter().zip(factor.pivot_columns()) {
                if offset.get(pivot)? {
                    offset.xor_assign(row)?;
                }
            }
            let key = (
                offset.words().to_vec(),
                columns.iter().map(|row| row.words().to_vec()).collect(),
            );
            match groups.entry(key) {
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().1 = a.add(&entry.get().1, &mass)?;
                }
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((AffineSupport::new(offset, columns)?, mass));
                }
            }
        }
        let mut components = Vec::with_capacity(groups.len());
        let mut total_mass = ExactScalar::zero();
        for (support, mass_per_label) in groups.into_values() {
            if mass_per_label.is_zero() {
                continue;
            }
            if !mass_per_label.is_real() || mass_per_label.real_sign()? != Ordering::Greater {
                return Ok(None);
            }
            let dimension = i64::try_from(support.dimension())
                .map_err(|_| CompletedRecordError::DimensionOverflow)?;
            let count = a.scalar([1.into(), 0.into(), 0.into(), 0.into()], dimension)?;
            let mass = a.multiply(&mass_per_label, &count)?;
            total_mass = a.add(&total_mass, &mass)?;
            components.push(CompletedAffineComponent {
                support,
                mass_per_label,
                total_mass: mass,
            });
        }
        Ok(Some(CompletedAffineMixture {
            components,
            total_mass,
        }))
    }
}
