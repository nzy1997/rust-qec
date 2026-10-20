//! Unitary third-hierarchy circuits lowered to Clifford boundaries and phases.

mod observations;
mod shared;

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
    RepeatedQubit,
    RepeatedOutput,
    IncompatibleProgram,
    InvalidInsertion { position: usize, gate_count: usize },
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
            Self::InvalidTurns => write!(
                f,
                "T turns must be below eight and controlled-S turns below four"
            ),
            Self::RepeatedQubit => write!(f, "multi-qubit diagonal gate repeats a qubit"),
            Self::RepeatedOutput => write!(f, "target output list contains duplicate qubits"),
            Self::IncompatibleProgram => write!(
                f,
                "prepared query requires a view of its original compiled program"
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
    pub path_variables: usize,
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
        match strategy {
            CompilerStrategy::ParityCopies => self.compile(input, arithmetic),
            CompilerStrategy::SharedCommutingLayer => shared::compile(self, input, arithmetic),
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
        });
        let program = Arc::new(InsertionProgram {
            reference: Arc::clone(&boundary),
            query_reference: Arc::clone(&boundary),
            observation_reduction: None,
            reference_phase: Arc::clone(&phase),
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

/// Fixed trace/event constraint elimination, shared across one reference
/// program's Pauli assignments. Final phase closure is still query specific.
pub struct PreparedProbability {
    program: Arc<InsertionProgram>,
    physical_width: usize,
    event_count: usize,
    projection: PreparedZProjection,
    legs: [Vec<usize>; 2],
}

impl PreparedProbability {
    pub fn evaluate(
        &self,
        state: &CompiledUnitary,
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if !Arc::ptr_eq(&self.program, &state.program) {
            return Err(UnitaryError::IncompatibleProgram);
        }
        if outcomes.width() != self.event_count {
            return Err(UnitaryError::DimensionMismatch);
        }
        let rhs = BitVector::from_indices(
            self.physical_width + self.event_count,
            outcomes.ones().map(|j| self.physical_width + j),
        )?;
        let joint = state
            .boundary
            .tensor(&state.boundary.conjugate(), arithmetic)?;
        close(
            self.projection.project(&joint, &rhs)?,
            &self.legs,
            &[(&state.phase, false), (&state.phase, true)],
            max_terms,
            arithmetic,
        )
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
        if rows.iter().any(|row| row.width() != self.width()) {
            return Err(UnitaryError::DimensionMismatch);
        }
        self.width()
            .checked_add(rows.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        let reference = &self.program.query_reference;
        let half = reference.width();
        let joint = reference.tensor(&reference.conjugate(), arithmetic)?;
        let total = joint.width();
        let mut constraints = (0..self.width())
            .map(|j| BitVector::from_indices(total, [j, half + j]))
            .collect::<Result<Vec<_>, _>>()?;
        constraints.extend(
            rows.iter()
                .map(|row| BitVector::from_indices(total, row.ones()))
                .collect::<Result<Vec<_>, _>>()?,
        );
        Ok(PreparedProbability {
            program: Arc::clone(&self.program),
            physical_width: self.width(),
            event_count: rows.len(),
            projection: PreparedZProjection::new(&joint, &constraints)?,
            legs: [
                (self.width()..half).collect(),
                (half + self.width()..total).collect(),
            ],
        })
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
        let boundary = Arc::new(frame.apply(&self.program.reference, arithmetic)?);
        let phase = if let Some(updates) = &self.program.shared {
            Arc::new(updates.phase(&grouped)?)
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
        if boundary.outputs.len() != target_boundary.outputs.len() {
            return Err(UnitaryError::DimensionMismatch);
        }
        for (width, boundary) in [
            (self.width(), &boundary),
            (target.width(), &target_boundary),
        ] {
            if boundary.event_rows.len() != boundary.outcomes.width()
                || boundary.event_rows.iter().any(|row| row.width() != width)
            {
                return Err(UnitaryError::DimensionMismatch);
            }
            if boundary
                .outputs
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != boundary.outputs.len()
            {
                return Err(UnitaryError::RepeatedOutput);
            }
            BitVector::from_indices(width, boundary.outputs.iter().copied())?;
        }
        let positions = boundary
            .outputs
            .iter()
            .enumerate()
            .map(|(i, &q)| (q, target_boundary.outputs[i]))
            .collect::<BTreeMap<_, _>>();
        let n = self.boundary.width();
        let m = target.boundary.width();
        let joint = self
            .boundary
            .tensor(&self.boundary.conjugate(), arithmetic)?
            .tensor(&target.boundary.conjugate(), arithmetic)?
            .tensor(&target.boundary, arithmetic)?;
        let total = joint.width();
        let mut rows = Vec::new();
        for qubit in 0..self.width() {
            if let Some(&target_index) = positions.get(&qubit) {
                rows.push(BitVector::from_indices(
                    total,
                    [qubit, 2 * n + target_index],
                )?);
                rows.push(BitVector::from_indices(
                    total,
                    [n + qubit, 2 * n + m + target_index],
                )?);
            } else {
                rows.push(BitVector::from_indices(total, [qubit, n + qubit])?);
            }
        }
        let target_kept = target_boundary
            .outputs
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        for qubit in 0..target.width() {
            if !target_kept.contains(&qubit) {
                rows.push(BitVector::from_indices(
                    total,
                    [2 * n + qubit, 2 * n + m + qubit],
                )?);
            }
        }
        let mut bits = vec![false; rows.len()];
        for (row_index, row) in boundary.event_rows.iter().enumerate() {
            rows.push(BitVector::from_indices(total, row.ones())?);
            bits.push(boundary.outcomes.get(row_index)?);
            rows.push(BitVector::from_indices(total, row.ones().map(|j| n + j))?);
            bits.push(boundary.outcomes.get(row_index)?);
        }
        for (row_index, row) in target_boundary.event_rows.iter().enumerate() {
            rows.push(BitVector::from_indices(
                total,
                row.ones().map(|j| 2 * n + j),
            )?);
            bits.push(target_boundary.outcomes.get(row_index)?);
            rows.push(BitVector::from_indices(
                total,
                row.ones().map(|j| 2 * n + m + j),
            )?);
            bits.push(target_boundary.outcomes.get(row_index)?);
        }
        let rhs = BitVector::from_indices(
            bits.len(),
            bits.iter()
                .enumerate()
                .filter_map(|(i, &bit)| bit.then_some(i)),
        )?;
        let ket = joint.constrain_z(&rows, &rhs)?;
        let legs = [
            (self.width()..n).collect::<Vec<_>>(),
            (n + self.width()..2 * n).collect(),
            (2 * n + target.width()..2 * n + m).collect(),
            (2 * n + m + target.width()..total).collect(),
        ];
        close(
            ket,
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

fn close(
    ket: Option<AffineKet>,
    legs: &[Vec<usize>],
    phases: &[(&ParityPhase, bool)],
    max_terms: usize,
    arithmetic: &ScalarArithmetic,
) -> Result<NetworkValue, UnitaryError> {
    let zero = || NetworkValue {
        value: ExactScalar::zero(),
        quotient_rank: 0,
        terms: 0,
    };
    let Some(ket) = ket else {
        return Ok(zero());
    };
    let keep = legs.iter().flatten().copied().collect::<Vec<_>>();
    let Some(ket) = ket.sum_boundary(&keep, arithmetic)? else {
        return Ok(zero());
    };
    if phases.len() != legs.len()
        || phases
            .iter()
            .zip(legs)
            .any(|((p, _), l)| p.num_variables() != l.len())
    {
        return Err(UnitaryError::DimensionMismatch);
    }
    let mut constant = 0;
    let mut terms = Vec::new();
    let mut offset = 0;
    for ((phase, conjugate), indices) in phases.iter().zip(legs) {
        let sign = |c: u8| if *conjugate { (8 - c) & 7 } else { c };
        constant = (constant + sign(phase.constant())) & 7;
        for (axis, c) in phase.terms() {
            terms.push((
                BitVector::from_indices(keep.len(), axis.ones().map(|j| offset + j))?,
                sign(*c),
            ));
        }
        offset += indices.len();
    }
    let phase = ParityPhase::new(keep.len(), constant, terms)?;
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
