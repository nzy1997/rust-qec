//! Unitary third-hierarchy circuits lowered to Clifford boundaries and phases.

mod insertions;
mod observations;
mod phase_map;
pub use insertions::{InsertionPreparationOptions, PreparedInsertions};
mod prepared;
mod shared;
mod support;
pub use prepared::{
    DensityLayout, PreparedDensityOverlap, PreparedPauliMoment, PreparedProbability,
};
pub use support::PreparedProbabilitySupport;

use crate::clifford::{CliffordCircuit, CliffordError, CliffordGate};
use crate::gf2::{AffineSupport, BitVector, Gf2Error};
use crate::pauli::{PauliError, PauliWord};
use crate::phase::{ClosedPhaseSum, ParityPhase, PhaseError};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::stabilizer::{AffineKet, PreparedZProjection, StabilizerError};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitaryError {
    DimensionMismatch,
    DimensionOverflow,
    InvalidTurns,
    RawPathLimit {
        limit: usize,
        required: usize,
    },
    RepeatedQubit,
    RepeatedOutput,
    IncompatibleProgram,
    InvalidInsertion {
        position: usize,
        gate_count: usize,
    },
    InvalidPreparedInsertion {
        index: usize,
        count: usize,
    },
    InsertionResourceLimit {
        option: &'static str,
        limit: usize,
        required: u128,
    },
    Binary(Gf2Error),
    Clifford(CliffordError),
    Phase(PhaseError),
    Scalar(ScalarError),
    Stabilizer(StabilizerError),
    Pauli(PauliError),
}
impl fmt::Display for UnitaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch => {
                write!(f, "unitary input, event, or target dimensions disagree")
            }
            Self::DimensionOverflow => {
                write!(f, "unitary network width exceeds the platform index range")
            }
            Self::RawPathLimit { limit, required } => write!(
                f,
                "max_path_bits = {limit} is insufficient; raw Clifford network needs {required}"
            ),
            Self::InvalidTurns => write!(
                f,
                "T turns must be below eight and controlled-S turns below four"
            ),
            Self::RepeatedQubit => write!(f, "multi-qubit diagonal gate repeats a qubit"),
            Self::RepeatedOutput => write!(f, "target output list contains duplicate qubits"),
            Self::IncompatibleProgram => write!(
                f,
                "prepared plan requires a view of its original compiled program"
            ),
            Self::InvalidPreparedInsertion { index, count } => write!(
                f,
                "prepared insertion index {index} is outside catalog size {count}"
            ),
            Self::InsertionResourceLimit {
                option,
                limit,
                required,
            } => write!(
                f,
                "insertion preparation needs {option} >= {required}; configured limit is {limit}"
            ),
            Self::InvalidInsertion {
                position,
                gate_count,
            } => write!(
                f,
                "Pauli insertion position {position} exceeds the circuit's {gate_count} gates"
            ),
            Self::Binary(e) => e.fmt(f),
            Self::Clifford(e) => e.fmt(f),
            Self::Phase(e) => e.fmt(f),
            Self::Scalar(e) => e.fmt(f),
            Self::Stabilizer(e) => e.fmt(f),
            Self::Pauli(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for UnitaryError {}
impl From<Gf2Error> for UnitaryError {
    fn from(e: Gf2Error) -> Self {
        Self::Binary(e)
    }
}
impl From<CliffordError> for UnitaryError {
    fn from(e: CliffordError) -> Self {
        Self::Clifford(e)
    }
}
impl From<PhaseError> for UnitaryError {
    fn from(e: PhaseError) -> Self {
        Self::Phase(e)
    }
}
impl From<ScalarError> for UnitaryError {
    fn from(e: ScalarError) -> Self {
        Self::Scalar(e)
    }
}
impl From<StabilizerError> for UnitaryError {
    fn from(e: StabilizerError) -> Self {
        Self::Stabilizer(e)
    }
}
impl From<PauliError> for UnitaryError {
    fn from(e: PauliError) -> Self {
        Self::Pauli(e)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompilerStrategy {
    #[default]
    ParityCopies,
    SharedCommutingLayer,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompilationDiagnostics {
    pub strategy: CompilerStrategy,
    pub layer_slots: Vec<usize>,
    pub reference_layer: Option<usize>,
    pub reference_layer_slots: usize,
    pub phase_ancillas: usize,
    pub shared_phase_coordinates: usize,
    pub coherent_width: usize,
    pub reference_coherent_width: usize,
    pub shared_observations_before_reduction: usize,
    pub clifford_gates: usize,
    /// Variables retained after local Hadamard contraction.
    pub path_variables: usize,
    /// Computational input support plus Hadamards before any contraction.
    pub raw_path_variables: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitaryGate {
    Clifford(CliffordGate),
    T { qubit: usize, turns: u8 },
    Cs { a: usize, b: usize, turns: u8 },
    Ccz { a: usize, b: usize, c: usize },
}

/// A coherent fixed-input circuit. Parsing, measurement records and stochastic
/// noise belong to the instrument layer; this type describes unitary gates.
#[derive(Clone, Debug)]
pub struct UnitaryCircuit {
    width: usize,
    gates: Vec<UnitaryGate>,
}
impl UnitaryCircuit {
    pub fn new(
        width: usize,
        gates: impl IntoIterator<Item = UnitaryGate>,
    ) -> Result<Self, UnitaryError> {
        let gates = gates.into_iter().collect::<Vec<_>>();
        for gate in &gates {
            match *gate {
                UnitaryGate::Clifford(c) => {
                    CliffordCircuit::new(width, [c])?;
                }
                UnitaryGate::T { qubit, turns } => {
                    BitVector::from_indices(width, [qubit])?;
                    if turns > 7 {
                        return Err(UnitaryError::InvalidTurns);
                    }
                }
                UnitaryGate::Cs { a, b, turns } => {
                    BitVector::from_indices(width, [a, b])?;
                    if a == b {
                        return Err(UnitaryError::RepeatedQubit);
                    }
                    if turns > 3 {
                        return Err(UnitaryError::InvalidTurns);
                    }
                }
                UnitaryGate::Ccz { a, b, c } => {
                    BitVector::from_indices(width, [a, b, c])?;
                    if a == b || a == c || b == c {
                        return Err(UnitaryError::RepeatedQubit);
                    }
                }
            }
        }
        Ok(Self { width, gates })
    }
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn gates(&self) -> &[UnitaryGate] {
        &self.gates
    }

    pub fn compile_with_strategy(
        &self,
        input: &BitVector,
        arithmetic: &ScalarArithmetic,
        strategy: CompilerStrategy,
    ) -> Result<CompiledUnitary, UnitaryError> {
        self.compile_with_path_limit(input, arithmetic, strategy, usize::MAX)
    }

    /// Bound raw paths of the selected Clifford lowering before contraction.
    /// Computational inputs contribute no support variables; each lowered H
    /// contributes one path even when later local contractions remove it.
    pub fn compile_with_path_limit(
        &self,
        input: &BitVector,
        arithmetic: &ScalarArithmetic,
        strategy: CompilerStrategy,
        max_path_bits: usize,
    ) -> Result<CompiledUnitary, UnitaryError> {
        match strategy {
            CompilerStrategy::ParityCopies => self.compile_parity(input, arithmetic, max_path_bits),
            CompilerStrategy::SharedCommutingLayer => {
                shared::compile(self, input, arithmetic, max_path_bits)
            }
        }
    }

    /// Copy each diagonal parity to a fresh |0> ancilla using CNOTs. Its phase
    /// is applied only at the final scalar boundary. Summing that ancilla with
    /// an unnormalized <+| reproduces the original gate exactly, including
    /// noncommuting gates on either side. The physical state is never expanded.
    pub fn compile(
        &self,
        input: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompiledUnitary, UnitaryError> {
        self.compile_parity(input, arithmetic, usize::MAX)
    }

    fn compile_parity(
        &self,
        input: &BitVector,
        arithmetic: &ScalarArithmetic,
        max_path_bits: usize,
    ) -> Result<CompiledUnitary, UnitaryError> {
        if input.width() != self.width {
            return Err(UnitaryError::DimensionMismatch);
        }
        let mut lowered = Vec::<CliffordGate>::new();
        let mut coefficients = Vec::new();
        let mut positions = Vec::new();
        for &gate in &self.gates {
            positions.push(lowered.len());
            match gate {
                UnitaryGate::Clifford(c) => {
                    lowered.push(c);
                    continue;
                }
                UnitaryGate::T { qubit, turns } if turns & 1 == 0 => {
                    lowered.push(CliffordGate::S {
                        qubit,
                        turns: turns / 2,
                    });
                    continue;
                }
                UnitaryGate::Cs { turns: 0, .. } => continue,
                UnitaryGate::Cs { a, b, turns: 2 } => {
                    lowered.push(CliffordGate::Cz { a, b });
                    continue;
                }
                _ => {}
            }
            for (qubits, coefficient) in diagonal_terms(gate) {
                if coefficient == 0 {
                    continue;
                }
                let target = self
                    .width
                    .checked_add(coefficients.len())
                    .ok_or(UnitaryError::DimensionOverflow)?;
                for control in qubits {
                    lowered.push(CliffordGate::Cx { control, target });
                }
                coefficients.push(coefficient);
            }
        }
        positions.push(lowered.len());
        let width = self
            .width
            .checked_add(coefficients.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        let raw_path_variables = check_raw_paths(&lowered, max_path_bits)?;
        let initial =
            CliffordCircuit::computational(BitVector::from_indices(width, input.ones())?)?;
        let circuit = CliffordCircuit::new(width, lowered)?;
        let boundary = Arc::new(
            circuit
                .evolve(&initial, arithmetic)?
                .expect("unitary Clifford gates preserve a nonzero basis input"),
        );
        let phase = Arc::new(ParityPhase::new(
            coefficients.len(),
            0,
            coefficients
                .into_iter()
                .enumerate()
                .map(|(j, c)| Ok((BitVector::from_indices(width - self.width, [j])?, c)))
                .collect::<Result<Vec<_>, Gf2Error>>()?,
        )?);
        let compilation = Arc::new(CompilationDiagnostics {
            strategy: CompilerStrategy::ParityCopies,
            layer_slots: vec![],
            reference_layer: None,
            reference_layer_slots: 0,
            phase_ancillas: phase.num_variables(),
            shared_phase_coordinates: 0,
            coherent_width: width,
            reference_coherent_width: width,
            shared_observations_before_reduction: 0,
            clifford_gates: circuit.gates().len(),
            path_variables: boundary.columns().len(),
            raw_path_variables,
        });
        let program = Arc::new(InsertionProgram {
            reference: Arc::clone(&boundary),
            query_reference: Arc::clone(&boundary),
            observation_reduction: None,
            reference_phase: Arc::clone(&phase),
            query_reference_phase: Arc::clone(&phase),
            shared: None,
            circuit,
            positions,
        });
        Ok(CompiledUnitary {
            physical_width: self.width,
            boundary,
            phase,
            program,
            compilation,
        })
    }
}

fn diagonal_terms(gate: UnitaryGate) -> Vec<(Vec<usize>, u8)> {
    match gate {
        UnitaryGate::T { qubit, turns } => vec![(vec![qubit], turns)],
        UnitaryGate::Cs { a, b, turns } => vec![
            (vec![a], turns),
            (vec![b], turns),
            (vec![a, b], (8 - turns) & 7),
        ],
        UnitaryGate::Ccz { a, b, c } => vec![
            (vec![a], 1),
            (vec![b], 1),
            (vec![c], 1),
            (vec![a, b], 7),
            (vec![a, c], 7),
            (vec![b, c], 7),
            (vec![a, b, c], 1),
        ],
        UnitaryGate::Clifford(_) => Vec::new(),
    }
}

/// An exact query result with the resource size used for this closed network.
#[derive(Clone, Debug)]
pub struct NetworkValue {
    pub value: ExactScalar,
    pub quotient_rank: usize,
    pub terms: usize,
}

/// The Clifford circuit is built once. Queries close the requested boundaries
/// before restricting the phase and constructing its computational quotient.
/// The selected compiler strategy determines the frozen observations. Final
/// phase resources are still prepared separately for distinct queries.
pub struct CompiledUnitary {
    physical_width: usize,
    boundary: Arc<AffineKet>,
    phase: Arc<ParityPhase>,
    program: Arc<InsertionProgram>,
    compilation: Arc<CompilationDiagnostics>,
}
struct InsertionProgram {
    reference: Arc<AffineKet>,
    query_reference: Arc<AffineKet>,
    observation_reduction: Option<observations::ObservationReduction>,
    reference_phase: Arc<ParityPhase>,
    query_reference_phase: Arc<ParityPhase>,
    shared: Option<shared::PhaseUpdates>,
    circuit: CliffordCircuit,
    positions: Vec<usize>,
}

/// Insert a physical Pauli before gate `before_gate`. Position equal to the
/// gate count is the final circuit boundary. Equal-position insertions act in
/// their supplied order; different positions are always processed in time order.
#[derive(Clone, Debug)]
pub struct PauliInsertion {
    pub before_gate: usize,
    pub word: PauliWord,
}
/// Selected density outputs and simultaneous physical Z-parity events.
#[derive(Clone, Copy, Debug)]
pub struct DensityBoundary<'a> {
    pub outputs: &'a [usize],
    pub event_rows: &'a [BitVector],
    pub outcomes: &'a BitVector,
}

impl<'a> DensityBoundary<'a> {
    pub fn layout(&self) -> DensityLayout<'a> {
        DensityLayout {
            outputs: self.outputs,
            event_rows: self.event_rows,
        }
    }
}

impl CompiledUnitary {
    pub fn width(&self) -> usize {
        self.physical_width
    }
    pub fn compilation(&self) -> &CompilationDiagnostics {
        &self.compilation
    }
    pub fn phase_legs(&self) -> usize {
        self.phase.num_variables()
    }

    /// A conservative computational support image, without non-Clifford phases.
    /// Summing the frozen phase legs cannot create a physical label outside
    /// this family, but interference can remove labels inside it.
    pub fn structural_z_support(&self, rows: &[BitVector]) -> Result<AffineSupport, UnitaryError> {
        if rows.iter().any(|row| row.width() != self.width()) {
            return Err(UnitaryError::DimensionMismatch);
        }
        let lifted = rows
            .iter()
            .map(|row| BitVector::from_indices(self.boundary.width(), row.ones()))
            .collect::<Result<Vec<_>, _>>()?;
        let map = |value: &BitVector| -> Result<BitVector, Gf2Error> {
            let mut result = BitVector::zeros(rows.len())?;
            for (j, row) in lifted.iter().enumerate() {
                result.set(j, row.dot(value)?)?;
            }
            Ok(result)
        };
        Ok(AffineSupport::new(
            map(self.boundary.offset())?,
            self.boundary
                .columns()
                .iter()
                .map(map)
                .collect::<Result<Vec<_>, _>>()?,
        )?)
    }

    /// Prepare fixed physical trace and event rows against the original
    /// reference, including when called on a perturbed insertion view.
    pub fn prepare_probability(
        &self,
        rows: &[BitVector],
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedProbability, UnitaryError> {
        PreparedProbability::prepare(self, rows, arithmetic)
    }

    /// Freeze a certified Clifford-fiber resource before adaptive radical
    /// absorption. The reported resource rank/term counts can be larger.
    pub fn prepare_probability_with_fibers(
        &self,
        rows: &[BitVector],
        max_terms: usize,
        options: crate::fibers::FiberOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedProbability, UnitaryError> {
        self.prepare_probability(rows, arithmetic)?
            .with_phase_resource(
                max_terms,
                crate::phase::PhaseResourceOptions {
                    fibers: options,
                    ..Default::default()
                },
                None,
                arithmetic,
            )
    }

    /// Prepare exact `<psi|Pi P Pi|psi>` for a fixed Pauli word and Z-event
    /// layout. Outcomes and Pauli insertion updates remain evaluation inputs.
    pub fn prepare_pauli_moment(
        &self,
        word: &PauliWord,
        rows: &[BitVector],
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedPauliMoment, UnitaryError> {
        PreparedPauliMoment::prepare(self, word, rows, arithmetic)
    }

    /// Additionally retain a certified fixed phase resource. Anticommuting
    /// event/Pauli pairs certify zero without requiring any enumerated terms.
    pub fn prepare_pauli_moment_with_fibers(
        &self,
        word: &PauliWord,
        rows: &[BitVector],
        max_terms: usize,
        options: crate::fibers::FiberOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedPauliMoment, UnitaryError> {
        self.prepare_pauli_moment(word, rows, arithmetic)?
            .with_phase_resource(
                max_terms,
                crate::phase::PhaseResourceOptions {
                    fibers: options,
                    ..Default::default()
                },
                None,
                arithmetic,
            )
    }

    /// Prepare ordered output and event layouts for reduced density overlap.
    /// Outcome values remain independent evaluation arguments on both sides.
    pub fn prepare_density_overlap(
        &self,
        boundary: DensityLayout<'_>,
        target: &Self,
        target_boundary: DensityLayout<'_>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedDensityOverlap, UnitaryError> {
        PreparedDensityOverlap::prepare(self, boundary, target, target_boundary, arithmetic)
    }

    /// Add one fixed certified phase resource to a density-overlap family.
    pub fn prepare_density_overlap_with_fibers(
        &self,
        boundary: DensityLayout<'_>,
        target: &Self,
        target_boundary: DensityLayout<'_>,
        max_terms: usize,
        options: crate::fibers::FiberOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedDensityOverlap, UnitaryError> {
        self.prepare_density_overlap(boundary, target, target_boundary, arithmetic)?
            .with_phase_resource(
                max_terms,
                crate::phase::PhaseResourceOptions {
                    fibers: options,
                    ..Default::default()
                },
                None,
                arithmetic,
            )
    }

    /// Replace the insertion assignment relative to the original compiled
    /// circuit, even when called on a previously perturbed view. The reference
    /// Clifford ket and program are immutable and shared; gates are not evolved
    /// again. Queries still prepare their phase resource as documented above.
    pub fn with_insertions(
        &self,
        insertions: &[PauliInsertion],
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        if insertions.is_empty() {
            // Preserve the identity-frame coefficient budget check while
            // sharing the already reduced immutable no-fault query boundary.
            arithmetic.multiply(self.program.reference.coefficient(), &ExactScalar::one())?;
            return Ok(Self {
                physical_width: self.physical_width,
                boundary: Arc::clone(&self.program.query_reference),
                phase: Arc::clone(&self.program.query_reference_phase),
                compilation: Arc::clone(&self.compilation),
                program: Arc::clone(&self.program),
            });
        }
        let count = self.program.positions.len() - 1;
        let mut grouped = BTreeMap::<usize, Vec<&PauliWord>>::new();
        for insertion in insertions {
            if insertion.before_gate > count {
                return Err(UnitaryError::InvalidInsertion {
                    position: insertion.before_gate,
                    gate_count: count,
                });
            }
            if insertion.word.width() != self.width() {
                return Err(UnitaryError::DimensionMismatch);
            }
            grouped
                .entry(insertion.before_gate)
                .or_default()
                .push(&insertion.word);
        }
        let total = self.program.reference.width();
        let mut frame = PauliWord::identity(total)?;
        for position in 0..=count {
            if let Some(words) = grouped.get(&position) {
                for word in words {
                    let extended = PauliWord::new(
                        BitVector::from_indices(total, word.x().ones())?,
                        BitVector::from_indices(total, word.z().ones())?,
                        word.phase(),
                    )?;
                    frame = extended.multiply(&frame)?;
                }
            }
            if position < count {
                for &gate in &self.program.circuit.gates()
                    [self.program.positions[position]..self.program.positions[position + 1]]
                {
                    frame = frame.conjugated(gate)?;
                }
            }
        }
        self.with_insertion_frame(&frame, &grouped, None, arithmetic)
    }
    fn with_insertion_frame(
        &self,
        frame: &PauliWord,
        grouped: &BTreeMap<usize, Vec<&PauliWord>>,
        phase_override: Option<Arc<ParityPhase>>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        let boundary = Arc::new(frame.apply(&self.program.reference, arithmetic)?);
        let phase = if let Some(phase) = phase_override {
            phase
        } else if let Some(updates) = &self.program.shared {
            Arc::new(updates.phase(grouped)?)
        } else {
            Arc::clone(&self.program.reference_phase)
        };
        let (boundary, phase) = if let Some(reduction) = &self.program.observation_reduction {
            reduction.apply(&boundary, &phase)?
        } else {
            (boundary, phase)
        };
        Ok(Self {
            physical_width: self.physical_width,
            boundary,
            phase,
            compilation: Arc::clone(&self.compilation),
            program: Arc::clone(&self.program),
        })
    }

    pub fn amplitude(
        &self,
        output: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if output.width() != self.width() {
            return Err(UnitaryError::DimensionMismatch);
        }
        let total = self.boundary.width();
        let rows = (0..self.width())
            .map(|j| BitVector::from_indices(total, [j]))
            .collect::<Result<Vec<_>, _>>()?;
        let ket = self.boundary.constrain_z(&rows, output)?;
        close(
            ket,
            &[(self.width()..total).collect::<Vec<_>>()],
            &[(&self.phase, false)],
            max_terms,
            arithmetic,
        )
    }

    /// Exact probability of simultaneous Z-parity events on the output.
    /// Empty rows yield the total norm. Contradictory events yield exact zero.
    pub fn probability_z(
        &self,
        rows: &[BitVector],
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if rows.len() != outcomes.width() || rows.iter().any(|r| r.width() != self.width()) {
            return Err(UnitaryError::DimensionMismatch);
        }
        let half = self.boundary.width();
        let joint = self
            .boundary
            .tensor(&self.boundary.conjugate(), arithmetic)?;
        let total = joint.width();
        let mut constraints = (0..self.width())
            .map(|j| BitVector::from_indices(total, [j, half + j]))
            .collect::<Result<Vec<_>, _>>()?;
        let mut rhs = BitVector::zeros(
            self.width()
                .checked_add(rows.len())
                .ok_or(UnitaryError::DimensionOverflow)?,
        )?;
        for (j, row) in rows.iter().enumerate() {
            constraints.push(BitVector::from_indices(total, row.ones())?);
            rhs.set(self.width() + j, outcomes.get(j)?)?;
        }
        let ket = joint.constrain_z(&constraints, &rhs)?;
        let legs = [
            (self.width()..half).collect::<Vec<_>>(),
            (half + self.width()..total).collect::<Vec<_>>(),
        ];
        close(
            ket,
            &legs,
            &[(&self.phase, false), (&self.phase, true)],
            max_terms,
            arithmetic,
        )
    }

    /// Exact <psi|Pi P Pi|psi>, retaining complex phase for a general Pauli word.
    pub fn pauli_moment(
        &self,
        word: &PauliWord,
        rows: &[BitVector],
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if word.width() != self.width()
            || rows.len() != outcomes.width()
            || rows.iter().any(|row| row.width() != self.width())
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        let half = self.boundary.width();
        let lifted = rows
            .iter()
            .map(|row| BitVector::from_indices(half, row.ones()))
            .collect::<Result<Vec<_>, _>>()?;
        let left = self.boundary.conjugate().constrain_z(&lifted, outcomes)?;
        let right = self.boundary.constrain_z(&lifted, outcomes)?;
        let (Some(left), Some(right)) = (left, right) else {
            return Ok(NetworkValue {
                value: ExactScalar::zero(),
                quotient_rank: 0,
                terms: 0,
            });
        };
        let word = PauliWord::new(
            BitVector::from_indices(half, word.x().ones())?,
            BitVector::from_indices(half, word.z().ones())?,
            word.phase(),
        )?;
        let joint = left.tensor(&word.apply(&right, arithmetic)?, arithmetic)?;
        let total = joint.width();
        let equality = (0..self.width())
            .map(|q| BitVector::from_indices(total, [q, half + q]))
            .collect::<Result<Vec<_>, _>>()?;
        let ket = joint.constrain_z(&equality, &BitVector::zeros(self.width())?)?;
        let legs = [
            (self.width()..half).collect(),
            (half + self.width()..total).collect(),
        ];
        close(
            ket,
            &legs,
            &[(&self.phase, true), (&self.phase, false)],
            max_terms,
            arithmetic,
        )
    }

    /// Exact `<self|target>` for fixed pure unitary preparations.
    pub fn overlap(
        &self,
        target: &Self,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if target.width() != self.width() {
            return Err(UnitaryError::DimensionMismatch);
        }
        let half = self.boundary.width();
        let joint = self
            .boundary
            .conjugate()
            .tensor(&target.boundary, arithmetic)?;
        let total = joint.width();
        let rows = (0..self.width())
            .map(|j| BitVector::from_indices(total, [j, half + j]))
            .collect::<Result<Vec<_>, _>>()?;
        let ket = joint.constrain_z(&rows, &BitVector::zeros(self.width())?)?;
        let legs = [
            (self.width()..half).collect::<Vec<_>>(),
            (half + target.width()..total).collect::<Vec<_>>(),
        ];
        close(
            ket,
            &legs,
            &[(&self.phase, true), (&target.phase, false)],
            max_terms,
            arithmetic,
        )
    }

    /// Exact unnormalized mass after Z events and projection of selected
    /// physical outputs onto a pure target. Every other physical wire is traced,
    /// so outputs may remain entangled with records or discarded environments.
    pub fn target_weight(
        &self,
        outputs: &[usize],
        target: &Self,
        event_rows: &[BitVector],
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        let target_outputs = (0..target.width()).collect::<Vec<_>>();
        let target_outcomes = BitVector::zeros(0)?;
        self.density_overlap(
            DensityBoundary {
                outputs,
                event_rows,
                outcomes,
            },
            target,
            DensityBoundary {
                outputs: &target_outputs,
                event_rows: &[],
                outcomes: &target_outcomes,
            },
            max_terms,
            arithmetic,
        )
    }

    /// Unnormalized Hilbert-Schmidt overlap of reduced, projected density
    /// operators. Outputs are paired in the supplied order; all other physical
    /// indices are traced independently in each preparation.
    pub fn density_overlap(
        &self,
        boundary: DensityBoundary<'_>,
        target: &Self,
        target_boundary: DensityBoundary<'_>,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if boundary.event_rows.len() != boundary.outcomes.width()
            || target_boundary.event_rows.len() != target_boundary.outcomes.width()
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        let prepared::DensityNetwork {
            rows,
            trace_count,
            legs,
        } = prepared::density_rows(
            self.width(),
            self.boundary.width(),
            boundary.layout(),
            target.width(),
            target.boundary.width(),
            target_boundary.layout(),
        )?;
        let joint = prepared::density_joint(&self.boundary, &target.boundary, arithmetic)?;
        let rhs = prepared::density_rhs(trace_count, boundary.outcomes, target_boundary.outcomes)?;
        close(
            joint.constrain_z(&rows, &rhs)?,
            &legs,
            &[
                (&self.phase, false),
                (&self.phase, true),
                (&target.phase, true),
                (&target.phase, false),
            ],
            max_terms,
            arithmetic,
        )
    }
}
fn zero_network_value() -> NetworkValue {
    NetworkValue {
        value: ExactScalar::zero(),
        quotient_rank: 0,
        terms: 0,
    }
}

fn compose_phases(
    widths: &[usize],
    phases: &[(&ParityPhase, bool)],
) -> Result<ParityPhase, UnitaryError> {
    let width = widths
        .iter()
        .try_fold(0_usize, |a, &b| a.checked_add(b))
        .ok_or(UnitaryError::DimensionOverflow)?;
    if phases.len() != widths.len()
        || phases
            .iter()
            .zip(widths)
            .any(|((p, _), &n)| p.num_variables() != n)
    {
        return Err(UnitaryError::DimensionMismatch);
    }
    let mut constant = 0;
    let mut terms = Vec::new();
    let mut offset = 0;
    for ((phase, conjugate), &length) in phases.iter().zip(widths) {
        let sign = |c: u8| if *conjugate { (8 - c) & 7 } else { c };
        constant = (constant + sign(phase.constant())) & 7;
        for (axis, c) in phase.terms() {
            terms.push((
                BitVector::from_indices(width, axis.ones().map(|j| offset + j))?,
                sign(*c),
            ));
        }
        offset += length;
    }
    Ok(ParityPhase::new(width, constant, terms)?)
}

fn close(
    ket: Option<AffineKet>,
    legs: &[Vec<usize>],
    phases: &[(&ParityPhase, bool)],
    max_terms: usize,
    arithmetic: &ScalarArithmetic,
) -> Result<NetworkValue, UnitaryError> {
    let keep = legs.iter().flatten().copied().collect::<Vec<_>>();
    let retained = match ket {
        Some(ket) => ket.sum_boundary(&keep, arithmetic)?,
        None => None,
    };
    close_retained(
        retained,
        &legs.iter().map(Vec::len).collect::<Vec<_>>(),
        phases,
        max_terms,
        arithmetic,
    )
}

/// Close an already retained and contracted boundary in phase-family order.
fn close_retained(
    ket: Option<AffineKet>,
    widths: &[usize],
    phases: &[(&ParityPhase, bool)],
    max_terms: usize,
    arithmetic: &ScalarArithmetic,
) -> Result<NetworkValue, UnitaryError> {
    let Some(ket) = ket else {
        return Ok(zero_network_value());
    };
    let width = widths
        .iter()
        .try_fold(0_usize, |a, &b| a.checked_add(b))
        .ok_or(UnitaryError::DimensionOverflow)?;
    if ket.width() != width {
        return Err(UnitaryError::DimensionMismatch);
    }
    let phase = compose_phases(widths, phases)?;
    let restricted = phase.restrict(ket.offset(), ket.columns())?;
    let prepared = ClosedPhaseSum::new_adaptive(
        &restricted,
        ket.phase(),
        ket.coefficient(),
        max_terms,
        arithmetic,
        crate::fibers::FiberOptions::default(),
    )?;
    Ok(NetworkValue {
        value: prepared.evaluate(arithmetic)?,
        quotient_rank: prepared.quotient_rank(),
        terms: prepared.num_terms(),
    })
}

// Check before Clifford evolution, which deliberately contracts paths at each H.
fn check_raw_paths(gates: &[CliffordGate], limit: usize) -> Result<usize, UnitaryError> {
    let required = gates
        .iter()
        .filter(|gate| matches!(gate, CliffordGate::H(_)))
        .count();
    if required > limit {
        return Err(UnitaryError::RawPathLimit { limit, required });
    }
    Ok(required)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadratic::QuadraticForm;

    #[test]
    fn shared_phase_closure_matches_literal_complex_polynomials() {
        let arithmetic = ScalarArithmetic::default();
        let columns = (0..5)
            .map(|j| BitVector::from_indices(5, [j]).unwrap())
            .collect();
        let mut upper = vec![BitVector::zeros(5).unwrap(); 5];
        upper[1].set(4, true).unwrap();
        let ket = AffineKet::new(
            BitVector::zeros(5).unwrap(),
            columns,
            QuadraticForm::new(3, vec![1, 0, 0, 0, 0], upper).unwrap(),
            arithmetic.omega(3, -2).unwrap(),
        )
        .unwrap();
        let left = ParityPhase::new(
            2,
            2,
            [(vec![0], 1), (vec![1], 3), (vec![0, 1], 7)]
                .into_iter()
                .map(|(indices, c)| (BitVector::from_indices(2, indices).unwrap(), c)),
        )
        .unwrap();
        let right =
            ParityPhase::new(1, 5, [(BitVector::from_indices(1, [0]).unwrap(), 5)]).unwrap();
        for conjugate_left in [false, true] {
            for conjugate_right in [false, true] {
                let mut expected = ExactScalar::zero();
                for x in 0_i64..32 {
                    let bit = |j: u32| -> i64 { (x >> j) & 1 };
                    let quadratic = 3 + bit(0) + 2 * bit(1) * bit(4);
                    let a = 2 + bit(2) + 3 * bit(0) + 7 * (bit(2) ^ bit(0));
                    let b = 5 + 5 * bit(4);
                    let angle = 3
                        + 2 * quadratic
                        + if conjugate_left { -a } else { a }
                        + if conjugate_right { -b } else { b };
                    expected = arithmetic
                        .add(&expected, &arithmetic.omega(angle, -2).unwrap())
                        .unwrap();
                }
                let actual = close(
                    Some(ket.clone()),
                    &[vec![2, 0], vec![4]],
                    &[(&left, conjugate_left), (&right, conjugate_right)],
                    8,
                    &arithmetic,
                )
                .unwrap();
                assert_eq!(actual.value, expected);
            }
        }
    }
}
