//! Explicit static family construction, without scalar responses or sampling.
use super::*;
use crate::fibers::PreparedCliffordFibers;
use crate::unitary::DensityLayout;

/// A query's fixed geometry. Outcome values and noise assignments are absent.
#[derive(Clone, Copy)]
pub enum InstrumentQueryLayout<'a> {
    Acceptance,
    ReportedProbability {
        parities: &'a [BitExpr],
    },
    AcceptedProbability {
        parities: &'a [BitExpr],
    },
    TargetDensity {
        outputs: &'a [usize],
        target: &'a CompiledUnitary,
        target_layout: DensityLayout<'a>,
    },
    PauliMoment {
        outputs: &'a [usize],
        word: &'a PauliWord,
    },
    /// Validate the phase-independent record transport; no numeric family is retained.
    ReportedSupport,
}
impl InstrumentQueryLayout<'_> {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Acceptance => "acceptance",
            Self::ReportedProbability { .. } => "reported_probability",
            Self::AcceptedProbability { .. } => "accepted_probability",
            Self::TargetDensity { .. } => "target_density",
            Self::PauliMoment { .. } => "pauli_moment",
            Self::ReportedSupport => "reported_support",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryPreparationOutcome {
    Created,
    Reused,
    /// Layout validation succeeded; this execution policy retains no numeric plan.
    Cold,
}
pub(super) fn validate_resource(
    resource: Option<&PreparedCliffordFibers>,
    max_terms: usize,
) -> Result<(), InstrumentError> {
    if let Some(r) = resource
        && r.num_terms() > max_terms
    {
        return Err(
            UnitaryError::Phase(crate::phase::PhaseError::FiberTermLimit {
                coordinates: r.enumerated_coordinates(),
                quotient_rank: r.quotient_rank(),
                limit: max_terms,
            })
            .into(),
        );
    }
    Ok(())
}
impl CompiledInstrument {
    /// Prepare the exact same immutable family used by a later query. No scalar
    /// response, noise draw, controller call, or postselected history is produced.
    /// Fixed-resource limits apply to reused plans as well as new construction.
    pub fn prepare_query(
        &self,
        layout: InstrumentQueryLayout<'_>,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<QueryPreparationOutcome, InstrumentError> {
        match layout {
            InstrumentQueryLayout::Acceptance => self.probability_queries.prepare(
                &self.unitary,
                &self.event_rows,
                max_terms,
                arithmetic,
            ),
            InstrumentQueryLayout::ReportedProbability { parities } => {
                let rows = parities
                    .iter()
                    .map(|p| self.parity_row(p))
                    .collect::<Result<Vec<_>, _>>()?;
                self.probability_queries
                    .prepare(&self.unitary, &rows, max_terms, arithmetic)
            }
            InstrumentQueryLayout::AcceptedProbability { parities } => {
                let mut rows = self.event_rows.as_ref().clone();
                rows.extend(
                    parities
                        .iter()
                        .map(|p| self.parity_row(p))
                        .collect::<Result<Vec<_>, _>>()?,
                );
                self.probability_queries
                    .prepare(&self.unitary, &rows, max_terms, arithmetic)
            }
            InstrumentQueryLayout::TargetDensity {
                outputs,
                target,
                target_layout,
            } => {
                let physical = self.output_wires(outputs)?;
                self.density_queries.prepare(
                    &self.unitary,
                    DensityLayout {
                        outputs: &physical,
                        event_rows: &self.event_rows,
                    },
                    target,
                    target_layout,
                    max_terms,
                    arithmetic,
                )
            }
            InstrumentQueryLayout::PauliMoment { outputs, word } => {
                let physical = self.physical_word(outputs, word)?;
                self.moment_queries.prepare(
                    &self.unitary,
                    &physical,
                    &self.event_rows,
                    max_terms,
                    arithmetic,
                )
            }
            InstrumentQueryLayout::ReportedSupport => {
                // Structural support has no phase-resource family to prime. Keep
                // the existing conservative support path and its resource behavior.
                if self.record_wires.iter().any(|&q| q >= self.unitary.width()) {
                    return Err(UnitaryError::DimensionMismatch.into());
                }
                Ok(QueryPreparationOutcome::Cold)
            }
        }
    }
}
