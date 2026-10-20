//! Coherent lowering and exact conditional masses for affine instruments.

mod gates;
mod lower;
use crate::circuit::{BitAtom, BitExpr, Circuit};
use crate::gf2::{BitVector, Gf2Error};
use crate::pauli::{PauliError, PauliWord};
use crate::probability::{Probability, ProbabilityError};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::unitary::{CompiledUnitary, NetworkValue, PauliInsertion, UnitaryError};
use num_rational::BigRational;
use num_traits::One;
use std::{collections::BTreeSet, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstrumentError {
    NeedsHistory {
        operation: usize,
    },
    NonAffineQuery,
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
            Self::NeedsHistory { operation } => {
                write!(f, "operation {operation} requires a causal history runtime")
            }
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
    pub max_coherent_qubits: usize,
    pub max_gates: usize,
    pub max_noise_sites: usize,
}
impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            max_coherent_qubits: 65536,
            max_gates: 1_000_000,
            max_noise_sites: 1_000_000,
        }
    }
}
#[derive(Clone, Debug)]
pub struct FaultAlternative {
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
    alternatives: Vec<FaultAlternative>,
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

pub struct CompiledInstrument {
    unitary: CompiledUnitary,
    live_wires: Vec<usize>,
    record_wires: Vec<usize>,
    event_rows: Vec<BitVector>,
    outcomes: BitVector,
    sites: Vec<NoiseSite>,
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
    lower::compile(circuit, options, arithmetic)
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

    fn inserted(
        &self,
        faults: &[FaultSelection],
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompiledUnitary, InstrumentError> {
        let mut seen = BTreeSet::new();
        let mut insertions = Vec::new();
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
            insertions.push(PauliInsertion {
                before_gate: site.before_gate,
                word: alternative.word.clone(),
            });
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
        let mut total = BigRational::one();
        for (index, site) in self.sites.iter().enumerate() {
            total *= if let Some(&alternative) = chosen.get(&index) {
                site.alternatives[alternative].probability.value()
            } else {
                site.identity.value()
            };
        }
        Ok(total)
    }
    pub fn acceptance(
        &self,
        faults: &[FaultSelection],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        Ok(self.inserted(faults, arithmetic)?.probability_z(
            &self.event_rows,
            &self.outcomes,
            max_terms,
            arithmetic,
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
        let physical = outputs
            .iter()
            .map(|&qubit| {
                self.live_wires
                    .get(qubit)
                    .copied()
                    .ok_or(InstrumentError::QubitOutOfBounds {
                        qubit,
                        width: self.num_qubits(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let unitary = self.inserted(faults, arithmetic)?;
        let accepted =
            unitary.probability_z(&self.event_rows, &self.outcomes, max_terms, arithmetic)?;
        let target_success = unitary.target_weight(
            &physical,
            target,
            &self.event_rows,
            &self.outcomes,
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
        Ok(self
            .inserted(faults, arithmetic)?
            .probability_z(&rows, &rhs, max_terms, arithmetic)?)
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
