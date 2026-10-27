//! Exact, immutable certificates for constrained Clifford phase decompositions.
mod builtin;
mod injection;
mod json;
mod product;
mod resource;
mod selection;
mod walsh;
use crate::gf2::BitVector;
use crate::phase::ParityPhase;
use crate::scalar::{ExactScalar, ScalarArithmetic};
pub use builtin::builtin_phase_catalogue;
pub use injection::{PhaseInjectionOptions, PhaseInjectionReceipt};
pub use resource::{CataloguePreparationOptions, PreparedCataloguePhase};
pub use selection::{
    FrozenPhasePlan, PhaseSelectionOptions, PhaseSelectionReceipt, PhaseSelectionRoute,
};
use std::{collections::BTreeMap, fmt, sync::Arc};

/// One coefficient times a Clifford phase and an affine support indicator.
/// Supports may overlap, be inconsistent, or cancel against other terms.
#[derive(Clone, Debug)]
pub struct CliffordIndicator {
    pub coefficient: ExactScalar,
    pub constraints: Vec<(BitVector, bool)>,
    pub phase: ParityPhase,
}
#[derive(Clone, Debug)]
pub struct PhaseDecompositionInput {
    pub dimension: usize,
    /// Lexicographic integer assignments with coordinate zero least significant.
    pub phase_table: Vec<u8>,
    pub terms: Vec<CliffordIndicator>,
}
#[derive(Clone, Debug)]
pub struct CertifiedPhaseDecomposition {
    input: PhaseDecompositionInput,
}
impl CertifiedPhaseDecomposition {
    pub fn dimension(&self) -> usize {
        self.input.dimension
    }
    pub fn phase_table(&self) -> &[u8] {
        &self.input.phase_table
    }
    pub fn terms(&self) -> &[CliffordIndicator] {
        &self.input.terms
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CatalogueLimits {
    pub max_json_bytes: usize,
    pub max_entries: usize,
    pub max_dimension: usize,
    pub max_table_values: usize,
    pub max_terms: usize,
    pub max_constraint_rows: usize,
    /// Stored normalized rows. JSON raw rows must also fit the remaining
    /// capacity before an entry's phases are normalized.
    pub max_parity_rows: usize,
    /// Assignment comparisons plus indicator and constraint/parity checks.
    pub max_verification_work: u128,
}
impl Default for CatalogueLimits {
    fn default() -> Self {
        Self {
            max_json_bytes: 16 << 20,
            max_entries: 1024,
            max_dimension: 20,
            max_table_values: 1 << 20,
            max_terms: 1 << 20,
            max_constraint_rows: 1 << 20,
            max_parity_rows: 1 << 20,
            max_verification_work: 16 << 20,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CatalogueUsage {
    pub entries: usize,
    pub table_values: usize,
    pub terms: usize,
    pub constraint_rows: usize,
    pub parity_rows: usize,
    pub verification_work: u128,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogueError {
    pub field: String,
    pub message: String,
}
impl fmt::Display for CatalogueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}
impl std::error::Error for CatalogueError {}
fn error(field: impl Into<String>, message: impl fmt::Display) -> CatalogueError {
    CatalogueError {
        field: field.into(),
        message: message.to_string(),
    }
}
fn limit(field: &str, required: u128, configured: u128) -> Result<(), CatalogueError> {
    if required > configured {
        Err(error(
            field,
            format!("needs at least {required}; configured limit is {configured}"),
        ))
    } else {
        Ok(())
    }
}
fn assignments(dimension: usize, limits: &CatalogueLimits) -> Result<usize, CatalogueError> {
    limit(
        "catalogue.max_dimension",
        dimension as u128,
        limits.max_dimension as u128,
    )?;
    let count = u32::try_from(dimension)
        .ok()
        .and_then(|n| 1_usize.checked_shl(n))
        .ok_or_else(|| {
            error(
                "catalogue.dimension",
                "phase table size exceeds platform index range",
            )
        })?;
    limit(
        "catalogue.max_table_values",
        count as u128,
        limits.max_table_values as u128,
    )?;
    Ok(count)
}
fn vector_capacity<T>(field: &str, count: u128) -> Result<(), CatalogueError> {
    let capacity = (isize::MAX as usize / std::mem::size_of::<T>()) as u128;
    limit(field, count, capacity)
}
/// A complete-table lookup whose entries have been reconstructed exactly.
/// Clones share certified immutable entries; insertion commits only on success.
#[derive(Clone, Debug, Default)]
pub struct PhaseCatalogue {
    entries: BTreeMap<usize, BTreeMap<Vec<u8>, Arc<CertifiedPhaseDecomposition>>>,
    usage: CatalogueUsage,
}
impl PhaseCatalogue {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn usage(&self) -> CatalogueUsage {
        self.usage
    }
    pub fn get(
        &self,
        dimension: usize,
        phase_table: &[u8],
    ) -> Option<&CertifiedPhaseDecomposition> {
        // Dimension is part of the certificate key, including the zero-dimensional case.
        self.entries
            .get(&dimension)?
            .get(phase_table)
            .map(Arc::as_ref)
    }
    pub fn entries(&self) -> impl Iterator<Item = &CertifiedPhaseDecomposition> {
        self.entries
            .values()
            .flat_map(|entries| entries.values().map(Arc::as_ref))
    }
    fn check_existing(&self, limits: &CatalogueLimits) -> Result<(), CatalogueError> {
        let u = self.usage;
        for (field, used, cap) in [
            ("catalogue.max_entries", u.entries, limits.max_entries),
            (
                "catalogue.max_table_values",
                u.table_values,
                limits.max_table_values,
            ),
            ("catalogue.max_terms", u.terms, limits.max_terms),
            (
                "catalogue.max_constraint_rows",
                u.constraint_rows,
                limits.max_constraint_rows,
            ),
            (
                "catalogue.max_parity_rows",
                u.parity_rows,
                limits.max_parity_rows,
            ),
            (
                "catalogue.max_dimension",
                self.entries.last_key_value().map_or(0, |(n, _)| *n),
                limits.max_dimension,
            ),
        ] {
            limit(field, used as u128, cap as u128)?;
        }
        limit(
            "catalogue.max_verification_work",
            u.verification_work,
            limits.max_verification_work,
        )
    }
    pub fn insert(
        &mut self,
        input: PhaseDecompositionInput,
        limits: &CatalogueLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<(), CatalogueError> {
        self.insert_at(input, limits, arithmetic, self.usage.entries)
    }
    fn insert_at(
        &mut self,
        input: PhaseDecompositionInput,
        limits: &CatalogueLimits,
        arithmetic: &ScalarArithmetic,
        entry_index: usize,
    ) -> Result<(), CatalogueError> {
        let usage = self.validate_at(&input, limits, arithmetic, entry_index)?;
        self.entries.entry(input.dimension).or_default().insert(
            input.phase_table.clone(),
            Arc::new(CertifiedPhaseDecomposition { input }),
        );
        self.usage = usage;
        Ok(())
    }
    fn validate_at(
        &self,
        input: &PhaseDecompositionInput,
        limits: &CatalogueLimits,
        arithmetic: &ScalarArithmetic,
        entry_index: usize,
    ) -> Result<CatalogueUsage, CatalogueError> {
        self.check_existing(limits)?;
        let prefix = format!("catalogue.entries.{entry_index}");
        let count = assignments(input.dimension, limits)?;
        if input.phase_table.len() != count {
            return Err(error(
                format!("{prefix}.core_phase_mod8"),
                format!(
                    "dimension {} needs {count} phase values, found {}",
                    input.dimension,
                    input.phase_table.len()
                ),
            ));
        }
        if let Some((j, _)) = input.phase_table.iter().enumerate().find(|(_, v)| **v > 7) {
            return Err(error(
                format!("{prefix}.core_phase_mod8.{j}"),
                "phase must be an integer from zero through seven",
            ));
        }
        if self.get(input.dimension, &input.phase_table).is_some() {
            return Err(error(&prefix, "duplicate dimension/phase-table entry"));
        }
        let constraints = input
            .terms
            .iter()
            .map(|t| t.constraints.len() as u128)
            .sum::<u128>();
        let parities = input
            .terms
            .iter()
            .map(|t| t.phase.terms().len() as u128)
            .sum::<u128>();
        let totals = [
            (
                "catalogue.max_entries",
                self.usage.entries as u128 + 1,
                limits.max_entries,
            ),
            (
                "catalogue.max_table_values",
                self.usage.table_values as u128 + count as u128,
                limits.max_table_values,
            ),
            (
                "catalogue.max_terms",
                self.usage.terms as u128 + input.terms.len() as u128,
                limits.max_terms,
            ),
            (
                "catalogue.max_constraint_rows",
                self.usage.constraint_rows as u128 + constraints,
                limits.max_constraint_rows,
            ),
            (
                "catalogue.max_parity_rows",
                self.usage.parity_rows as u128 + parities,
                limits.max_parity_rows,
            ),
        ];
        for (name, required, configured) in totals {
            limit(name, required, configured as u128)?;
        }
        let work = (count as u128) * (1 + input.terms.len() as u128 + constraints + parities);
        let total_work = self
            .usage
            .verification_work
            .checked_add(work)
            .ok_or_else(|| {
                error(
                    "catalogue.max_verification_work",
                    "verification work exceeds u128",
                )
            })?;
        limit(
            "catalogue.max_verification_work",
            total_work,
            limits.max_verification_work,
        )?;
        for (j, term) in input.terms.iter().enumerate() {
            let field = format!("{prefix}.terms.{j}");
            if term.phase.num_variables() != input.dimension
                || term
                    .constraints
                    .iter()
                    .any(|(row, _)| row.width() != input.dimension)
            {
                return Err(error(
                    &field,
                    "constraint or Clifford phase dimension differs from entry dimension",
                ));
            }
            term.phase
                .clifford()
                .map_err(|e| error(format!("{field}.clifford_parities"), e))?;
            arithmetic
                .multiply(&term.coefficient, &ExactScalar::one())
                .map_err(|e| error(format!("{field}.coefficient"), e))?;
        }
        for word in 0..count {
            let x = BitVector::from_indices(
                input.dimension,
                (0..input.dimension).filter(|j| word >> j & 1 != 0),
            )
            .map_err(|e| error(&prefix, e))?;
            let mut actual = ExactScalar::zero();
            for (j, term) in input.terms.iter().enumerate() {
                let field = format!("{prefix}.terms.{j}.assignment.{word}");
                let present = term
                    .constraints
                    .iter()
                    .try_fold(true, |present, (row, bit)| {
                        row.dot(&x).map(|v| present && (v == *bit))
                    })
                    .map_err(|e| error(&field, e))?;
                if present {
                    let phase = term.phase.evaluate(&x).map_err(|e| error(&field, e))?;
                    let value = arithmetic
                        .multiply(
                            &term.coefficient,
                            &arithmetic
                                .omega(i64::from(phase), 0)
                                .map_err(|e| error(&field, e))?,
                        )
                        .map_err(|e| error(&field, e))?;
                    actual = arithmetic
                        .add(&actual, &value)
                        .map_err(|e| error(&field, e))?;
                }
            }
            let expected = arithmetic
                .omega(i64::from(input.phase_table[word]), 0)
                .map_err(|e| error(&prefix, e))?;
            if actual != expected {
                return Err(error(
                    format!("{prefix}.assignment.{word}"),
                    format!(
                        "decomposition does not reconstruct omega^{}; actual {actual:?}",
                        input.phase_table[word]
                    ),
                ));
            }
        }
        Ok(CatalogueUsage {
            entries: self.usage.entries + 1,
            table_values: self.usage.table_values + count,
            terms: self.usage.terms + input.terms.len(),
            constraint_rows: self.usage.constraint_rows + constraints as usize,
            parity_rows: self.usage.parity_rows + parities as usize,
            verification_work: total_work,
        })
    }
    fn shared_entry(
        &self,
        dimension: usize,
        phase_table: &[u8],
    ) -> Option<Arc<CertifiedPhaseDecomposition>> {
        self.entries.get(&dimension)?.get(phase_table).cloned()
    }
    fn from_shared(
        entry: Arc<CertifiedPhaseDecomposition>,
        limits: &CatalogueLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, CatalogueError> {
        let mut result = Self::new();
        let usage = result.validate_at(&entry.input, limits, arithmetic, 0)?;
        result
            .entries
            .entry(entry.dimension())
            .or_default()
            .insert(entry.phase_table().to_vec(), entry);
        result.usage = usage;
        Ok(result)
    }
}
