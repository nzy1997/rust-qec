//! Bounded immutable probability plans shared by instrument branches.
use super::*;
use crate::fibers::FiberOptions;
use crate::unitary::PreparedProbability;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Default)]
pub enum QueryPreparation {
    #[default]
    Cold,
    Trace,
    FixedFibers(FiberOptions),
    /// Prefer a matching built-in certificate, otherwise retain fixed fibers.
    FixedCatalogue(crate::phase::PhaseResourceOptions),
}
impl QueryPreparation {
    pub fn is_fixed(self) -> bool {
        matches!(self, Self::FixedFibers(_) | Self::FixedCatalogue(_))
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Trace => "trace",
            Self::FixedFibers(_) => "fixed",
            Self::FixedCatalogue(_) => "catalogue",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedQueryStatistics {
    pub cached_queries: usize,
    pub fixed_resources: usize,
    /// Actual selected catalogue resources; missing keys count as fibers.
    pub catalogue_resources: usize,
    /// Independent maxima over retained reference resources, not query quotas.
    pub maximum_quotient_rank: Option<usize>,
    pub maximum_terms: Option<usize>,
    pub maximum_phase_variables: Option<usize>,
}
impl PreparedQueryStatistics {
    pub(super) fn from_resources<'a>(
        cached_queries: usize,
        resources: impl Iterator<Item = &'a crate::phase::PreparedPhaseResource>,
    ) -> Self {
        let mut statistics = Self {
            cached_queries,
            fixed_resources: 0,
            catalogue_resources: 0,
            maximum_quotient_rank: None,
            maximum_terms: None,
            maximum_phase_variables: None,
        };
        for resource in resources {
            statistics.fixed_resources += 1;
            statistics.catalogue_resources +=
                usize::from(resource.kind() == crate::phase::PhaseResourceKind::CertifiedCatalogue);
            statistics.maximum_quotient_rank = Some(
                statistics
                    .maximum_quotient_rank
                    .unwrap_or(0)
                    .max(resource.quotient_rank()),
            );
            statistics.maximum_terms = Some(
                statistics
                    .maximum_terms
                    .unwrap_or(0)
                    .max(resource.num_terms()),
            );
            statistics.maximum_phase_variables = Some(
                statistics
                    .maximum_phase_variables
                    .unwrap_or(0)
                    .max(resource.num_variables()),
            );
        }
        statistics
    }
}
/// Compatibility names for the probability-only preparation interface.
pub use QueryPreparation as ProbabilityPreparation;
pub type PreparedProbabilityStatistics = PreparedQueryStatistics;
pub(super) struct ProbabilityQueries {
    mode: ProbabilityPreparation,
    limit: usize,
    plans: Mutex<HashMap<Vec<BitVector>, Arc<PreparedProbability>>>,
}
impl ProbabilityQueries {
    pub(super) fn new(mode: ProbabilityPreparation, limit: usize) -> Result<Self, InstrumentError> {
        if !matches!(mode, ProbabilityPreparation::Cold) && limit == 0 {
            return Err(InstrumentError::ResourceLimit {
                option: "max_prepared_probabilities",
                limit,
                required: 1,
            });
        }
        Ok(Self {
            mode,
            limit,
            plans: Mutex::new(HashMap::new()),
        })
    }
    pub(super) fn statistics(&self) -> Result<PreparedProbabilityStatistics, InstrumentError> {
        let plans = self
            .plans
            .lock()
            .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
        Ok(PreparedQueryStatistics::from_resources(
            plans.len(),
            plans
                .values()
                .filter_map(|plan| plan.fixed_phase_resource()),
        ))
    }
    pub(super) fn evaluate(
        &self,
        state: &CompiledUnitary,
        rows: &[BitVector],
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        if matches!(self.mode, ProbabilityPreparation::Cold) {
            return Ok(state.probability_z(rows, outcomes, max_terms, arithmetic)?);
        }
        let (plan, _) = self.plan(state, rows, max_terms, arithmetic)?;
        Ok(plan.evaluate(state, outcomes, max_terms, arithmetic)?)
    }
    pub(super) fn prepare(
        &self,
        state: &CompiledUnitary,
        rows: &[BitVector],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<QueryPreparationOutcome, InstrumentError> {
        if rows.iter().any(|r| r.width() != state.width()) {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        state
            .width()
            .checked_add(rows.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        if matches!(self.mode, ProbabilityPreparation::Cold) {
            return Ok(QueryPreparationOutcome::Cold);
        }
        let (plan, outcome) = self.plan(state, rows, max_terms, arithmetic)?;
        super::preparation::validate_resource(plan.fixed_phase_resource(), max_terms)?;
        Ok(outcome)
    }
    fn plan(
        &self,
        state: &CompiledUnitary,
        rows: &[BitVector],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<(Arc<PreparedProbability>, QueryPreparationOutcome), InstrumentError> {
        // One lock serializes preparation and enforces the bound across threads.
        // Evaluation runs after releasing it. Plans own programs, while programs
        // never own this cache, so retaining branches creates no reference cycle.
        let result = {
            let mut plans = self
                .plans
                .lock()
                .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
            if let Some(plan) = plans.get(rows) {
                (Arc::clone(plan), QueryPreparationOutcome::Reused)
            } else {
                if plans.len() >= self.limit {
                    return Err(InstrumentError::ResourceLimit {
                        option: "max_prepared_probabilities",
                        limit: self.limit,
                        required: plans.len().saturating_add(1),
                    });
                }
                let plan = match self.mode {
                    ProbabilityPreparation::Trace => state.prepare_probability(rows, arithmetic)?,
                    ProbabilityPreparation::FixedFibers(options) => state
                        .prepare_probability_with_fibers(rows, max_terms, options, arithmetic)?,
                    ProbabilityPreparation::FixedCatalogue(options) => state
                        .prepare_probability(rows, arithmetic)?
                        .with_phase_resource(
                            max_terms,
                            options,
                            Some(crate::catalogue::builtin_phase_catalogue()?),
                            arithmetic,
                        )?,
                    ProbabilityPreparation::Cold => unreachable!("cold queries returned above"),
                };
                let plan = Arc::new(plan);
                plans.insert(rows.to_vec(), Arc::clone(&plan));
                (plan, QueryPreparationOutcome::Created)
            }
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase::{ParityPhase, PhaseResourceOptions, PreparedPhaseResource};
    #[test]
    fn resource_peaks_are_independent_and_absent_for_nonfixed_families() {
        let a = ScalarArithmetic::default();
        let db = crate::catalogue::builtin_phase_catalogue().unwrap();
        let bits = |width: usize, mask: usize| {
            BitVector::from_indices(
                width,
                (0..width.min(usize::BITS as usize)).filter(|j| mask >> j & 1 != 0),
            )
            .unwrap()
        };
        let scalar = PreparedPhaseResource::new(
            &ParityPhase::new(0, 7, []).unwrap(),
            8,
            PhaseResourceOptions::default(),
            Some(db),
            &a,
        )
        .unwrap();
        let ccz = ParityPhase::new(
            3,
            0,
            (1_usize..8).map(|m| (bits(3, m), if m.count_ones() % 2 == 1 { 1 } else { 7 })),
        )
        .unwrap();
        let mut computational = PhaseResourceOptions::default();
        computational.fibers.attempts = 0;
        let ccz = PreparedPhaseResource::new(&ccz, 8, computational, None, &a).unwrap();
        let wide =
            ParityPhase::new(129, 2, [(BitVector::from_indices(129, [128]).unwrap(), 1)]).unwrap();
        let wide =
            PreparedPhaseResource::new(&wide, 8, PhaseResourceOptions::default(), Some(db), &a)
                .unwrap();
        assert_eq!(scalar.num_variables(), 0);
        assert_eq!(ccz.num_variables(), 3);
        assert_eq!(wide.num_variables(), 129);
        assert_eq!(scalar.catalogue_resource().unwrap().num_variables(), 0);
        assert_eq!(ccz.clifford_fibers().unwrap().num_variables(), 3);
        assert_eq!(wide.catalogue_resource().unwrap().num_variables(), 129);
        assert_eq!(
            PreparedQueryStatistics::from_resources(4, [&scalar, &ccz, &wide].into_iter()),
            PreparedQueryStatistics {
                cached_queries: 4,
                fixed_resources: 3,
                catalogue_resources: 2,
                maximum_quotient_rank: Some(3),
                maximum_terms: Some(8),
                maximum_phase_variables: Some(129),
            }
        );
        let empty = PreparedQueryStatistics::from_resources(4, std::iter::empty());
        assert_eq!(empty.cached_queries, 4);
        assert_eq!(empty.fixed_resources, 0);
        assert_eq!(empty.catalogue_resources, 0);
        assert_eq!(empty.maximum_quotient_rank, None);
        assert_eq!(empty.maximum_terms, None);
        assert_eq!(empty.maximum_phase_variables, None);
        assert_eq!(
            PreparedQueryStatistics::from_resources(1, [&scalar].into_iter()).maximum_quotient_rank,
            Some(0)
        );
        use crate::catalogue::{
            CatalogueLimits, CliffordIndicator, PhaseCatalogue, PhaseDecompositionInput,
        };
        let mut catalogue = PhaseCatalogue::new();
        let terms = (0..4)
            .map(|assignment| {
                let first = assignment & 1 != 0;
                let second = assignment & 2 != 0;
                CliffordIndicator {
                    coefficient: a.omega(i64::from(first) + i64::from(second), 0).unwrap(),
                    constraints: vec![(bits(4, 3), first), (bits(4, 12), second)],
                    phase: ParityPhase::new(
                        4,
                        0,
                        [(first, 1), (second, 4)]
                            .into_iter()
                            .filter(|(odd, _)| !odd)
                            .map(|(_, mask)| (bits(4, mask), 2)),
                    )
                    .unwrap(),
                }
            })
            .collect();
        catalogue
            .insert(
                PhaseDecompositionInput {
                    dimension: 4,
                    phase_table: (0_u8..16).map(|value| value.count_ones() as u8).collect(),
                    terms,
                },
                &CatalogueLimits::default(),
                &a,
            )
            .unwrap();
        let four_t = ParityPhase::new(4, 0, (0..4).map(|j| (bits(4, 1 << j), 1))).unwrap();
        let four_t = PreparedPhaseResource::new(
            &four_t,
            8,
            PhaseResourceOptions::default(),
            Some(&catalogue),
            &a,
        )
        .unwrap();
        assert_eq!(
            (
                four_t.quotient_rank(),
                four_t.num_terms(),
                four_t.num_variables()
            ),
            (4, 4, 4)
        );
        // Rank, term count and coordinate width have three different owners.
        assert_eq!(
            PreparedQueryStatistics::from_resources(5, [&scalar, &ccz, &wide, &four_t].into_iter()),
            PreparedQueryStatistics {
                cached_queries: 5,
                fixed_resources: 4,
                catalogue_resources: 3,
                maximum_quotient_rank: Some(4),
                maximum_terms: Some(8),
                maximum_phase_variables: Some(129),
            }
        );
    }
}
