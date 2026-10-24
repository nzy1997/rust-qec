//! Coherent lowering and exact conditional masses for affine instruments.

mod densities;
mod gates;
mod insertions;
mod lower;
mod moments;
mod preparation;
pub use insertions::{InsertionPreparation, PreparedInsertionStatistics};
pub use preparation::{InstrumentQueryLayout, QueryPreparationOutcome};
mod probabilities;
mod support;
use crate::circuit::{BitAtom, BitExpr, Circuit};
use crate::gf2::{AffineSupport, BitVector, Gf2Error};
use crate::pauli::{PauliError, PauliWord};
use crate::probability::{Probability, ProbabilityError};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::unitary::{CompiledUnitary, NetworkValue, PauliInsertion, UnitaryError};
use num_rational::BigRational;
pub use probabilities::{
    PreparedProbabilityStatistics, PreparedQueryStatistics, ProbabilityPreparation,
    QueryPreparation,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};
pub use support::PreparedSupportStatistics;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstrumentError {
    NeedsHistory {
        operation: usize,
    },
    NonAffineQuery,
    PreparedQueryCacheUnavailable,
    PreparedInsertionCatalogUnavailable,
    UncapturedSupportQuery,
    UnknownDecision {
        call: usize,
        bit: usize,
    },
    Unsupported {
        operation: usize,
        reason: &'static str,
    },
    ResourceLimit {
        option: &'static str,
        limit: usize,
        required: usize,
    },
    InvalidFault {
        site: usize,
        alternative: usize,
    },
    RepeatedFault {
        site: usize,
    },
    QubitOutOfBounds {
        qubit: usize,
        width: usize,
    },
    RecordOutOfBounds {
        record: usize,
        count: usize,
    },
    Binary(Gf2Error),
    Pauli(PauliError),
    Probability(ProbabilityError),
    Scalar(ScalarError),
    Unitary(UnitaryError),
}
impl fmt::Display for InstrumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UncapturedSupportQuery => write!(
                f,
                "reported query is outside the captured support transport; declare all required observation parities"
            ),
            Self::PreparedInsertionCatalogUnavailable => {
                write!(f, "compiled insertion catalog metadata is unavailable")
            }
            Self::NeedsHistory { operation } => {
                write!(f, "operation {operation} requires a causal history runtime")
            }
            Self::UnknownDecision { call, bit } => write!(
                f,
                "decision slot {bit} of controller call {call} is absent from the template"
            ),
            Self::PreparedQueryCacheUnavailable => write!(
                f,
                "prepared query cache is unavailable after an interrupted preparation"
            ),
            Self::NonAffineQuery => write!(
                f,
                "reported marginals require expressions over records; external decisions require the causal history runtime"
            ),
            Self::Unsupported { operation, reason } => write!(f, "operation {operation}: {reason}"),
            Self::ResourceLimit {
                option,
                limit,
                required,
            } => write!(f, "{option} = {limit} is insufficient; need {required}"),
            Self::InvalidFault { site, alternative } => {
                write!(f, "noise site {site} has no alternative {alternative}")
            }
            Self::RepeatedFault { site } => {
                write!(f, "noise site {site} was selected more than once")
            }
            Self::QubitOutOfBounds { qubit, width } => {
                write!(f, "output qubit {qubit} is outside physical width {width}")
            }
            Self::RecordOutOfBounds { record, count } => {
                write!(f, "record {record} is outside record count {count}")
            }
            Self::Binary(e) => e.fmt(f),
            Self::Pauli(e) => e.fmt(f),
            Self::Probability(e) => e.fmt(f),
            Self::Scalar(e) => e.fmt(f),
            Self::Unitary(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for InstrumentError {}
impl From<Gf2Error> for InstrumentError {
    fn from(e: Gf2Error) -> Self {
        Self::Binary(e)
    }
}
impl From<PauliError> for InstrumentError {
    fn from(e: PauliError) -> Self {
        Self::Pauli(e)
    }
}
impl From<ProbabilityError> for InstrumentError {
    fn from(e: ProbabilityError) -> Self {
        Self::Probability(e)
    }
}
impl From<ScalarError> for InstrumentError {
    fn from(e: ScalarError) -> Self {
        Self::Scalar(e)
    }
}
impl From<UnitaryError> for InstrumentError {
    fn from(e: UnitaryError) -> Self {
        Self::Unitary(e)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CompileOptions {
    pub insertion_preparation: InsertionPreparation,
    pub compiler_strategy: crate::unitary::CompilerStrategy,
    pub probability_preparation: ProbabilityPreparation,
    pub max_prepared_probabilities: usize,
    pub density_preparation: QueryPreparation,
    pub max_prepared_densities: usize,
    pub moment_preparation: QueryPreparation,
    pub max_prepared_moments: usize,
    pub max_coherent_qubits: usize,
    /// Raw paths in the chosen Clifford lowering, before local contraction.
    pub max_path_bits: usize,
    pub max_gates: usize,
    pub max_noise_sites: usize,
}
impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            insertion_preparation: InsertionPreparation::Direct,
            compiler_strategy: crate::unitary::CompilerStrategy::ParityCopies,
            probability_preparation: ProbabilityPreparation::Cold,
            max_prepared_probabilities: 64,
            density_preparation: QueryPreparation::Cold,
            max_prepared_densities: 16,
            moment_preparation: QueryPreparation::Cold,
            max_prepared_moments: 64,
            max_coherent_qubits: 65536,
            max_path_bits: 1_000_000,
            max_gates: 1_000_000,
            max_noise_sites: 1_000_000,
        }
    }
}
#[derive(Clone, Debug)]
pub struct FaultAlternative {
    catalog_index: Option<usize>,
    probability: Probability,
    word: PauliWord,
}
impl FaultAlternative {
    pub fn probability(&self) -> &Probability {
        &self.probability
    }
    pub fn word(&self) -> &PauliWord {
        &self.word
    }
}
#[derive(Clone, Debug)]
pub struct NoiseSite {
    source_operations: Vec<usize>,
    before_gate: usize,
    alternatives: Arc<Vec<FaultAlternative>>,
    identity: Probability,
}
impl NoiseSite {
    pub fn source_operations(&self) -> &[usize] {
        &self.source_operations
    }
    pub fn alternatives(&self) -> &[FaultAlternative] {
        &self.alternatives
    }
    pub fn identity_probability(&self) -> &Probability {
        &self.identity
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaultSelection {
    pub site: usize,
    pub alternative: usize,
}

#[derive(Clone)]
pub struct CompiledInstrument {
    insertion_catalog: Option<Arc<insertions::InsertionCatalog>>,
    initial_prepared_insertions: Vec<usize>,
    unitary: Arc<CompiledUnitary>,
    probability_queries: Arc<probabilities::ProbabilityQueries>,
    density_queries: Arc<densities::DensityQueries>,
    moment_queries: Arc<moments::MomentQueries>,
    support_queries: Arc<support::SupportQueries>,
    live_wires: Arc<Vec<usize>>,
    record_wires: Arc<Vec<usize>>,
    event_rows: Arc<Vec<BitVector>>,
    outcomes: BitVector,
    sites: Arc<Vec<NoiseSite>>,
    decision_wires: Arc<BTreeMap<(usize, usize), usize>>,
    initial_insertions: Vec<PauliInsertion>,
}
pub struct ExactMasses {
    pub accepted: NetworkValue,
    pub target_success: NetworkValue,
    pub error: ExactScalar,
}

pub fn compile_affine(
    circuit: &Circuit,
    options: &CompileOptions,
    arithmetic: &ScalarArithmetic,
) -> Result<CompiledInstrument, InstrumentError> {
    lower::compile(circuit, options, arithmetic, false)
}
pub(crate) fn compile_causal(
    circuit: &Circuit,
    options: &CompileOptions,
    arithmetic: &ScalarArithmetic,
) -> Result<CompiledInstrument, InstrumentError> {
    lower::compile(circuit, options, arithmetic, true)
}
/// Compile a measurement-free pure target from its text frontend. Gate phases
/// follow the standard matrices. Instrument operations are rejected explicitly.
pub fn compile_unitary(
    circuit: &Circuit,
    options: &CompileOptions,
    arithmetic: &ScalarArithmetic,
) -> Result<CompiledUnitary, InstrumentError> {
    lower::unitary(circuit, options, arithmetic)
}

impl CompiledInstrument {
    /// Restrict a cloned instrument's probability-parent transport to the span
    /// of explicitly declared observation parities. Subsequent support queries
    /// outside that span fail instead of silently choosing another proposal.
    /// This does not change physical probability or acceptance queries.
    pub fn with_support_transport(&self, parities: &[BitExpr]) -> Result<Self, InstrumentError> {
        let indices = (0..self.num_records()).collect::<Vec<_>>();
        let rows = parities
            .iter()
            .map(|p| record_row(p, &indices, self.num_records()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut result = self.clone();
        result.support_queries = Arc::new(
            self.support_queries
                .with_transport(self.num_records(), &rows)?,
        );
        Ok(result)
    }
    pub fn prepared_support_statistics(
        &self,
    ) -> Result<Option<PreparedSupportStatistics>, InstrumentError> {
        self.support_queries.statistics(self.num_records())
    }
    /// Exact Clifford-term support certificates from the unselected diagonal
    /// record probability parent. Condition each term before spanning a union;
    /// a hull taken before conditioning can retain unnecessary affine zeros.
    pub fn reported_support_terms(
        &self,
        parities: &[BitExpr],
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Vec<AffineSupport>, InstrumentError> {
        let indices = (0..self.num_records()).collect::<Vec<_>>();
        let rows = parities
            .iter()
            .map(|p| record_row(p, &indices, self.num_records()))
            .collect::<Result<Vec<_>, _>>()?;
        let rows = self.support_queries.map_rows(&rows)?;
        let constants = BitVector::from_indices(
            parities.len(),
            parities
                .iter()
                .enumerate()
                .filter_map(|(j, p)| p.constant.then_some(j)),
        )?;
        let terms = self.support_queries.evaluate(
            &self.inserted(faults, arithmetic)?,
            &self.record_wires,
            max_terms,
            arithmetic,
        )?;
        terms
            .into_iter()
            .map(|term| {
                Ok(
                    AffineSupport::new(term.offset().clone(), term.columns().to_vec())?
                        .linear_image(&rows)?
                        .translated(&constants)?,
                )
            })
            .collect()
    }
    pub fn prepared_insertion_statistics(&self) -> Option<PreparedInsertionStatistics> {
        self.insertion_catalog
            .as_ref()
            .map(|c| PreparedInsertionStatistics {
                catalog_entries: c.plan.len(),
                estimated_peak_bytes: c.plan.estimated_peak_bytes(),
            })
    }
    pub fn compilation(&self) -> &crate::unitary::CompilationDiagnostics {
        self.unitary.compilation()
    }
    pub fn prepared_probability_statistics(
        &self,
    ) -> Result<PreparedProbabilityStatistics, InstrumentError> {
        self.probability_queries.statistics()
    }
    pub fn prepared_density_statistics(&self) -> Result<PreparedQueryStatistics, InstrumentError> {
        self.density_queries.statistics()
    }
    pub fn prepared_moment_statistics(&self) -> Result<PreparedQueryStatistics, InstrumentError> {
        self.moment_queries.statistics()
    }
    fn probability(
        &self,
        rows: &[BitVector],
        outcomes: &BitVector,
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        let state = self.inserted(faults, arithmetic)?;
        self.probability_queries
            .evaluate(&state, rows, outcomes, max_terms, arithmetic)
    }

    pub fn num_qubits(&self) -> usize {
        self.live_wires.len()
    }
    pub fn num_records(&self) -> usize {
        self.record_wires.len()
    }
    pub fn coherent_qubits(&self) -> usize {
        self.unitary.width()
    }
    pub fn noise_sites(&self) -> &[NoiseSite] {
        &self.sites
    }

    pub(crate) fn branch(
        &self,
        decisions: &BTreeMap<(usize, usize), bool>,
        active_sites: &BTreeSet<usize>,
    ) -> Result<Self, InstrumentError> {
        let mut branch = self.clone();
        let mut x = BitVector::zeros(self.coherent_qubits())?;
        branch.initial_prepared_insertions.clear();
        for (&(call, bit), &value) in decisions {
            let &wire = self
                .decision_wires
                .get(&(call, bit))
                .ok_or(InstrumentError::UnknownDecision { call, bit })?;
            x.set(wire, value)?;
            if value && let Some(catalog) = &self.insertion_catalog {
                branch.initial_prepared_insertions.push(
                    *catalog
                        .decisions
                        .get(&(call, bit))
                        .ok_or(InstrumentError::PreparedInsertionCatalogUnavailable)?,
                );
            }
        }
        branch.initial_insertions = if x.is_zero() {
            vec![]
        } else {
            vec![PauliInsertion {
                before_gate: 0,
                word: PauliWord::new(x, BitVector::zeros(self.coherent_qubits())?, 0)?,
            }]
        };
        let mut sites = Vec::with_capacity(active_sites.len());
        for &site in active_sites {
            sites.push(
                self.sites
                    .get(site)
                    .ok_or(InstrumentError::InvalidFault {
                        site,
                        alternative: 0,
                    })?
                    .clone(),
            );
        }
        branch.sites = Arc::new(sites);
        Ok(branch)
    }
    pub(crate) fn conditioned(
        &self,
        conditions: &[(BitExpr, bool)],
    ) -> Result<Self, InstrumentError> {
        let mut result = self.clone();
        let mut rows = self.event_rows.as_ref().clone();
        let mut outcomes =
            BitVector::from_indices(rows.len() + conditions.len(), self.outcomes.ones())?;
        for (j, (expression, value)) in conditions.iter().enumerate() {
            rows.push(self.parity_row(expression)?);
            outcomes.set(self.event_rows.len() + j, expression.constant ^ value)?;
        }
        result.event_rows = Arc::new(rows);
        result.outcomes = outcomes;
        Ok(result)
    }
    fn inserted(
        &self,
        faults: &[FaultSelection],
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompiledUnitary, InstrumentError> {
        let mut seen = BTreeSet::new();
        let mut indices = self
            .insertion_catalog
            .as_ref()
            .map(|_| self.initial_prepared_insertions.clone());
        let mut insertions = if indices.is_some() {
            vec![]
        } else {
            self.initial_insertions.clone()
        };
        for fault in faults {
            if !seen.insert(fault.site) {
                return Err(InstrumentError::RepeatedFault { site: fault.site });
            }
            let site = self
                .sites
                .get(fault.site)
                .ok_or(InstrumentError::InvalidFault {
                    site: fault.site,
                    alternative: fault.alternative,
                })?;
            let alternative =
                site.alternatives
                    .get(fault.alternative)
                    .ok_or(InstrumentError::InvalidFault {
                        site: fault.site,
                        alternative: fault.alternative,
                    })?;
            if let Some(indices) = &mut indices {
                indices.push(
                    alternative
                        .catalog_index
                        .ok_or(InstrumentError::PreparedInsertionCatalogUnavailable)?,
                );
            } else {
                insertions.push(PauliInsertion {
                    before_gate: site.before_gate,
                    word: alternative.word.clone(),
                });
            }
        }
        if let (Some(catalog), Some(indices)) = (&self.insertion_catalog, indices) {
            return Ok(self.unitary.with_prepared_insertions(
                &catalog.plan,
                &indices,
                arithmetic,
            )?);
        }
        Ok(self.unitary.with_insertions(&insertions, arithmetic)?)
    }

    /// Exact physical mass of the prescribed mutually exclusive noise choices.
    /// Omitted sites select identity; a zero-probability choice remains exactly zero.
    pub fn fault_probability(
        &self,
        faults: &[FaultSelection],
    ) -> Result<BigRational, InstrumentError> {
        let mut chosen = std::collections::BTreeMap::new();
        for fault in faults {
            if chosen.insert(fault.site, fault.alternative).is_some() {
                return Err(InstrumentError::RepeatedFault { site: fault.site });
            }
            self.sites
                .get(fault.site)
                .and_then(|s| s.alternatives.get(fault.alternative))
                .ok_or(InstrumentError::InvalidFault {
                    site: fault.site,
                    alternative: fault.alternative,
                })?;
        }
        Ok(crate::noise::probability_product(
            self.sites.iter().enumerate().map(|(index, site)| {
                if let Some(&alternative) = chosen.get(&index) {
                    site.alternatives[alternative].probability.value()
                } else {
                    site.identity.value()
                }
            }),
        ))
    }
    pub fn acceptance(
        &self,
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        self.probability(
            &self.event_rows,
            &self.outcomes,
            faults,
            max_terms,
            arithmetic,
        )
    }
    fn output_wires(&self, outputs: &[usize]) -> Result<Vec<usize>, InstrumentError> {
        let mut seen = BTreeSet::new();
        outputs
            .iter()
            .map(|&qubit| {
                if !seen.insert(qubit) {
                    return Err(InstrumentError::Unitary(UnitaryError::RepeatedOutput));
                }
                self.live_wires
                    .get(qubit)
                    .copied()
                    .ok_or(InstrumentError::QubitOutOfBounds {
                        qubit,
                        width: self.num_qubits(),
                    })
            })
            .collect()
    }
    /// Accepted mass with additional affine reported conditions, unlike physical
    /// reported_probability which deliberately excludes the selection predicates.
    pub fn accepted_probability(
        &self,
        parities: &[BitExpr],
        outcomes: &BitVector,
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        if parities.len() != outcomes.width() {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        let mut rows = self.event_rows.as_ref().clone();
        let count = rows
            .len()
            .checked_add(parities.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        let mut rhs = BitVector::zeros(count)?;
        for j in 0..self.event_rows.len() {
            rhs.set(j, self.outcomes.get(j)?)?;
        }
        for (j, parity) in parities.iter().enumerate() {
            rows.push(self.parity_row(parity)?);
            rhs.set(
                self.event_rows.len() + j,
                outcomes.get(j)? ^ parity.constant,
            )?;
        }
        self.probability(&rows, &rhs, faults, max_terms, arithmetic)
    }
    pub fn target_density_weight(
        &self,
        outputs: &[usize],
        target: &CompiledUnitary,
        target_boundary: crate::unitary::DensityBoundary<'_>,
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        let physical = self.output_wires(outputs)?;
        let state = self.inserted(faults, arithmetic)?;
        self.density_queries.evaluate(
            &state,
            crate::unitary::DensityBoundary {
                outputs: &physical,
                event_rows: &self.event_rows,
                outcomes: &self.outcomes,
            },
            target,
            target_boundary,
            max_terms,
            arithmetic,
        )
    }
    pub fn pauli_moment(
        &self,
        outputs: &[usize],
        word: &PauliWord,
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        let word = self.physical_word(outputs, word)?;
        self.moment_queries.evaluate(
            &self.inserted(faults, arithmetic)?,
            &word,
            &self.event_rows,
            &self.outcomes,
            max_terms,
            arithmetic,
        )
    }
    fn physical_word(
        &self,
        outputs: &[usize],
        word: &PauliWord,
    ) -> Result<PauliWord, InstrumentError> {
        if word.width() != outputs.len() {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        let physical = self.output_wires(outputs)?;
        let map = |mask: &BitVector| {
            BitVector::from_indices(self.unitary.width(), mask.ones().map(|j| physical[j]))
        };
        Ok(PauliWord::new(
            map(word.x())?,
            map(word.z())?,
            word.phase(),
        )?)
    }
    pub fn masses(
        &self,
        outputs: &[usize],
        target: &CompiledUnitary,
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactMasses, InstrumentError> {
        let physical = self.output_wires(outputs)?;
        let unitary = self.inserted(faults, arithmetic)?;
        let accepted = self.probability_queries.evaluate(
            &unitary,
            &self.event_rows,
            &self.outcomes,
            max_terms,
            arithmetic,
        )?;
        let target_outputs = (0..target.width()).collect::<Vec<_>>();
        let empty = BitVector::zeros(0)?;
        let target_success = self.density_queries.evaluate(
            &unitary,
            crate::unitary::DensityBoundary {
                outputs: &physical,
                event_rows: &self.event_rows,
                outcomes: &self.outcomes,
            },
            target,
            crate::unitary::DensityBoundary {
                outputs: &target_outputs,
                event_rows: &[],
                outcomes: &empty,
            },
            max_terms,
            arithmetic,
        )?;
        let error = arithmetic.subtract(&accepted.value, &target_success.value)?;
        Ok(ExactMasses {
            accepted,
            target_success,
            error,
        })
    }

    /// Conservative support for reported parities, including their inversions
    /// and declared faults, and excluding every postselection predicate.
    pub fn reported_support(
        &self,
        parities: &[BitExpr],
        faults: &[FaultSelection],
        arithmetic: &ScalarArithmetic,
    ) -> Result<AffineSupport, InstrumentError> {
        let rows = parities
            .iter()
            .map(|p| self.parity_row(p))
            .collect::<Result<Vec<_>, _>>()?;
        let constants = BitVector::from_indices(
            parities.len(),
            parities
                .iter()
                .enumerate()
                .filter_map(|(j, p)| p.constant.then_some(j)),
        )?;
        Ok(self
            .inserted(faults, arithmetic)?
            .structural_z_support(&rows)?
            .translated(&constants)?)
    }

    /// Query physical reported-record marginals, without applying postselection.
    /// This distinction is required by the later causal history sampler.
    pub fn reported_probability(
        &self,
        parities: &[BitExpr],
        outcomes: &BitVector,
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        if outcomes.width() != parities.len() {
            return Err(InstrumentError::Unitary(UnitaryError::DimensionMismatch));
        }
        let mut rows = Vec::new();
        let mut rhs = outcomes.clone();
        for (i, parity) in parities.iter().enumerate() {
            rows.push(self.parity_row(parity)?);
            rhs.set(i, rhs.get(i)? ^ parity.constant)?;
        }
        self.probability(&rows, &rhs, faults, max_terms, arithmetic)
    }
    fn parity_row(&self, expression: &BitExpr) -> Result<BitVector, InstrumentError> {
        record_row(expression, &self.record_wires, self.unitary.width())
    }
}
fn record_row(
    expression: &BitExpr,
    records: &[usize],
    width: usize,
) -> Result<BitVector, InstrumentError> {
    let mut row = BitVector::zeros(width)?;
    for term in &expression.terms {
        let BitAtom::Record(record) = term else {
            return Err(InstrumentError::NonAffineQuery);
        };
        let wire = *records
            .get(*record)
            .ok_or(InstrumentError::RecordOutOfBounds {
                record: *record,
                count: records.len(),
            })?;
        row.set(wire, !row.get(wire)?)?;
    }
    Ok(row)
}
