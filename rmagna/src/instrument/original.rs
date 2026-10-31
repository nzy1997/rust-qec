//! Owned original-operation instruments with explicit complete assignments.
//!
//! Wire labels are signed user identities. Physical boundary order is declared
//! outputs followed by chronological reset/discard environments. Destructive
//! readouts are contracted, never traced as extra environment legs. Callbacks
//! execute afresh per extraction; compiled branches contain no callback handles.
//!
//! ```
//! use rmagna::clifford::CliffordCircuit;
//! use rmagna::gf2::BitVector;
//! use rmagna::instrument::{OriginalAssignment, OriginalGate, OriginalInstrument,
//!     OriginalLimits, OriginalOperation};
//! use rmagna::scalar::{ExactScalar, ScalarArithmetic};
//! use rmagna::unitary::CompilerStrategy;
//! let a = ScalarArithmetic::default();
//! let input = CliffordCircuit::computational(BitVector::zeros(1)?)?;
//! let instrument = OriginalInstrument::new(
//!     vec![-9], vec![-9], input,
//!     vec![OriginalOperation { id: "reset".into(),
//!         gate: OriginalGate::Reset { wire: -9, plus: true } }],
//!     vec![], OriginalLimits::default(), &a)?;
//! let branch = instrument.compile(&OriginalAssignment::default(),
//!     CompilerStrategy::ParityCopies, &a)?;
//! assert_eq!(branch.outputs(), &[-9]);
//! assert_eq!(branch.environment()[0].wire, -9);
//! assert_eq!(branch.width(), 2);
//! assert_eq!(branch.probability(&a)?, ExactScalar::one());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::clifford::CliffordGate as C;
use crate::gf2::{BitVector, Gf2Error};
use crate::pauli::{PauliError, PauliWord};
use crate::records::{CompletedRecordError, CompletedRecordLimits, CompletedRecordSpec};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::stabilizer::{AffineKet, StabilizerError};
use crate::unitary::{
    CompilationDiagnostics, CompiledUnitary, CompilerStrategy, ExpansionLimits, UnitaryCircuit,
    UnitaryError, UnitaryGate as U, validate_stabilizer_input, with_zero_ancillas,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};
#[path = "original_trace.rs"]
mod trace;
pub use trace::{
    OriginalBoundaryDraw, OriginalBoundaryLimits, OriginalBoundaryOptions, OriginalBoundaryQuery,
    OriginalBoundaryResource, OriginalBoundaryService, OriginalBoundaryTask, OriginalNetworkLimits,
    OriginalNetworkMoment, OriginalNetworkOptions, OriginalNetworkProbability,
    OriginalNetworkResource, OriginalPreparedTrace, OriginalPreparedTraceResult,
    OriginalRecordEvent, OriginalRecordLayout, OriginalRecordParity, OriginalStaticTrace,
    OriginalTraceAssignment, OriginalTraceOptions, OriginalTracePreparationLimits,
    OriginalTraceResult, OriginalTraceView,
};

pub type OriginalWire = i64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginalPauli {
    I,
    X,
    Y,
    Z,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadoutBasis {
    X,
    Z,
}

/// Phase exponents use pi/4 units for the parity of the complete support.
/// Thus S-dagger is `Phase { turns: -2, .. }`. Empty parity is zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginalGate {
    Identity(Vec<OriginalWire>),
    H(OriginalWire),
    Phase {
        wires: Vec<OriginalWire>,
        turns: i64,
    },
    Pauli {
        wire: OriginalWire,
        pauli: OriginalPauli,
    },
    Cx(OriginalWire, OriginalWire),
    Cz(OriginalWire, OriginalWire),
    Ccz(OriginalWire, OriginalWire, OriginalWire),
    Prepare {
        wire: OriginalWire,
        plus: bool,
    },
    Reset {
        wire: OriginalWire,
        plus: bool,
    },
    Discard(OriginalWire),
    Measure {
        factors: Vec<(OriginalWire, OriginalPauli)>,
        inverted: bool,
        record: String,
    },
    Destructive {
        wire: OriginalWire,
        basis: ReadoutBasis,
        record: String,
    },
    Control,
}
impl OriginalGate {
    fn wire_count(&self) -> usize {
        match self {
            Self::Identity(v) | Self::Phase { wires: v, .. } => v.len(),
            Self::Measure { factors, .. } => factors.len(),
            Self::Control => 0,
            Self::Cx(..) | Self::Cz(..) => 2,
            Self::Ccz(..) => 3,
            _ => 1,
        }
    }
    fn wires(&self) -> Vec<OriginalWire> {
        match self {
            Self::Identity(v) | Self::Phase { wires: v, .. } => v.clone(),
            Self::Measure { factors, .. } => factors.iter().map(|v| v.0).collect(),
            Self::Control => vec![],
            Self::Cx(a, b) | Self::Cz(a, b) => vec![*a, *b],
            Self::Ccz(a, b, c) => vec![*a, *b, *c],
            Self::H(w)
            | Self::Discard(w)
            | Self::Pauli { wire: w, .. }
            | Self::Prepare { wire: w, .. }
            | Self::Reset { wire: w, .. }
            | Self::Destructive { wire: w, .. } => vec![*w],
        }
    }
    fn record(&self) -> Option<&str> {
        match self {
            Self::Measure { record, .. } | Self::Destructive { record, .. } => Some(record),
            _ => None,
        }
    }
    fn bytes(&self) -> u128 {
        32 + 16 * self.wire_count() as u128 + self.record().map_or(0, |r| r.len() as u128)
    }
    fn validate(&self) -> Result<(), OriginalError> {
        let wires = self.wires();
        if wires.iter().copied().collect::<BTreeSet<_>>().len() != wires.len() {
            return Err(invalid("repeated operation wire", ""));
        }
        if let Self::Measure { factors, .. } = self
            && factors.iter().any(|(_, p)| *p == OriginalPauli::I)
        {
            return Err(invalid("measurement factors must be X, Y or Z", ""));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct OriginalOperation {
    pub id: String,
    pub gate: OriginalGate,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginalOrigin {
    Operation(String),
    ControlEmission { control: String, ordinal: usize },
    Fault { after: String, ordinal: usize },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginalCallbackError {
    UndeclaredParent(String),
    Failed(String),
}
/// Read-only, ordered declared parents. Duplicate declarations remain visible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalControlInputs {
    entries: Vec<(String, bool)>,
}
impl OriginalControlInputs {
    pub fn entries(&self) -> &[(String, bool)] {
        &self.entries
    }
    pub fn get(&self, name: &str) -> Result<bool, OriginalCallbackError> {
        self.entries
            .iter()
            .find(|v| v.0 == name)
            .map(|v| v.1)
            .ok_or_else(|| OriginalCallbackError::UndeclaredParent(name.into()))
    }
}
pub type OriginalCallback = dyn Fn(&OriginalControlInputs) -> Result<Vec<OriginalGate>, OriginalCallbackError>
    + Send
    + Sync;
#[derive(Clone)]
pub struct OriginalControl {
    pub operation: String,
    pub parents: Vec<String>,
    pub version: String,
    pub callback: Arc<OriginalCallback>,
}
#[derive(Clone, Debug)]
pub struct OriginalFault {
    pub after: String,
    pub wire: OriginalWire,
    pub pauli: OriginalPauli,
}
#[derive(Clone, Debug, Default)]
pub struct OriginalAssignment {
    /// Exactly one true value per source record, in any order.
    pub true_values: Vec<(String, bool)>,
    /// An XOR list: repeated names cancel.
    pub report_flips: Vec<String>,
    pub faults: Vec<OriginalFault>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalDecision {
    pub operation: String,
    pub version: String,
    pub inputs: OriginalControlInputs,
    pub emitted_operations: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalRecord {
    pub name: String,
    pub true_value: bool,
    pub reported_value: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalEnvironment {
    pub wire: OriginalWire,
    pub retired_by: OriginalOrigin,
}

/// Structural limits exclude existing caller-owned objects and scalar limbs.
/// Callbacks allocate their returned vector themselves; it is admitted before
/// cloning or lowering. These limits cannot bound arbitrary callback work.
#[derive(Clone, Copy, Debug)]
pub struct OriginalLimits {
    pub max_metadata_bytes: usize,
    /// Source operations plus control declarations.
    pub max_operations: usize,
    pub max_parents: usize,
    pub max_faults: usize,
    pub max_emitted_operations: usize,
    pub max_physical_slots: usize,
    pub max_gates: usize,
    pub max_path_bits: usize,
    /// Also bounds the normalized-input validator's geometry before callbacks.
    pub expansion: ExpansionLimits,
    pub completed: CompletedRecordLimits,
}
impl Default for OriginalLimits {
    fn default() -> Self {
        Self {
            max_metadata_bytes: 1 << 24,
            max_operations: 1 << 20,
            max_parents: 1 << 20,
            max_faults: 1 << 20,
            max_emitted_operations: 1 << 20,
            max_physical_slots: 65536,
            max_gates: 1 << 20,
            max_path_bits: 1 << 20,
            expansion: ExpansionLimits::default(),
            completed: CompletedRecordLimits::default(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginalError {
    Metadata {
        kind: &'static str,
        identity: String,
    },
    Lifetime {
        wire: OriginalWire,
        expected_live: bool,
    },
    OutputMismatch,
    DimensionMismatch,
    Callback {
        operation: String,
        error: OriginalCallbackError,
    },
    ResourceLimit {
        option: &'static str,
        limit: usize,
        required: u128,
    },
    Binary(Gf2Error),
    Unitary(UnitaryError),
    Stabilizer(StabilizerError),
    Scalar(ScalarError),
    Pauli(PauliError),
    Completed(CompletedRecordError),
}
impl fmt::Display for OriginalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "original instrument: {self:?}")
    }
}
impl std::error::Error for OriginalError {}
macro_rules! from_error { ($($ty:ty => $v:ident),* $(,)?) => {$(impl From<$ty> for OriginalError {
    fn from(e:$ty)->Self {Self::$v(e)}
})*}; }
from_error!(Gf2Error=>Binary, UnitaryError=>Unitary, StabilizerError=>Stabilizer,
    ScalarError=>Scalar, PauliError=>Pauli, CompletedRecordError=>Completed);
fn invalid(kind: &'static str, id: &str) -> OriginalError {
    OriginalError::Metadata {
        kind,
        identity: id.into(),
    }
}
fn admit(option: &'static str, limit: usize, required: u128) -> Result<(), OriginalError> {
    if required > limit as u128 {
        Err(OriginalError::ResourceLimit {
            option,
            limit,
            required,
        })
    } else {
        Ok(())
    }
}
fn distinct(wires: &[OriginalWire]) -> bool {
    wires.iter().copied().collect::<BTreeSet<_>>().len() == wires.len()
}

/// Immutable owned metadata. Construction validates all identities and initial
/// state before any callback. Assignment admission also precedes every callback.
#[derive(Clone)]
pub struct OriginalInstrument {
    inputs: Vec<OriginalWire>,
    outputs: Vec<OriginalWire>,
    initial: AffineKet,
    operations: Vec<OriginalOperation>,
    controls: BTreeMap<String, OriginalControl>,
    records: Vec<String>,
    limits: OriginalLimits,
}
impl OriginalInstrument {
    pub fn new(
        inputs: Vec<OriginalWire>,
        outputs: Vec<OriginalWire>,
        initial: AffineKet,
        operations: Vec<OriginalOperation>,
        controls: Vec<OriginalControl>,
        limits: OriginalLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, OriginalError> {
        admit(
            "max_operations",
            limits.max_operations,
            operations.len() as u128 + controls.len() as u128,
        )?;
        admit(
            "max_physical_slots",
            limits.max_physical_slots,
            inputs.len() as u128,
        )?;
        let bytes = 16 * (inputs.len() as u128 + outputs.len() as u128)
            + operations
                .iter()
                .map(|o| o.id.len() as u128 + o.gate.bytes() + 32)
                .sum::<u128>()
            + controls
                .iter()
                .map(|c| {
                    64 + c.operation.len() as u128
                        + c.version.len() as u128
                        + c.parents.iter().map(|p| 32 + p.len() as u128).sum::<u128>()
                })
                .sum::<u128>();
        admit("max_metadata_bytes", limits.max_metadata_bytes, bytes)?;
        admit(
            "max_parents",
            limits.max_parents,
            controls.iter().map(|c| c.parents.len() as u128).sum(),
        )?;
        if !distinct(&inputs) || !distinct(&outputs) {
            return Err(invalid("duplicate boundary wire", ""));
        }
        admit(
            "max_path_bits",
            limits.max_path_bits,
            initial.columns().len() as u128,
        )?;
        let rank = initial.columns().len() as u128;
        // Bound the input validator's transposition and elimination before it
        // allocates. Existing caller-owned affine columns are excluded.
        admit(
            "max_geometry_words",
            limits.expansion.max_geometry_words,
            16 * ((initial.width() as u128 + rank) * rank.div_ceil(64)
                + initial.width() as u128
                + rank
                + 128),
        )?;
        validate_stabilizer_input(&initial, inputs.len(), arithmetic)?;
        let mut control_map = BTreeMap::new();
        for c in controls {
            if c.version.is_empty() {
                return Err(invalid("empty control version", &c.operation));
            }
            if control_map.contains_key(&c.operation) {
                return Err(invalid("duplicate control", &c.operation));
            }
            control_map.insert(c.operation.clone(), c);
        }
        let mut ids = BTreeSet::new();
        let mut records = Vec::new();
        let mut record_set = BTreeSet::new();
        let mut used = BTreeSet::new();
        for op in &operations {
            if op.id.is_empty() || !ids.insert(op.id.clone()) {
                return Err(invalid("empty or duplicate operation", &op.id));
            }
            op.gate.validate()?;
            if op.gate == OriginalGate::Control {
                let c = control_map
                    .get(&op.id)
                    .ok_or_else(|| invalid("missing control", &op.id))?;
                for p in &c.parents {
                    if !record_set.contains(p) {
                        return Err(invalid("unknown or future parent", p));
                    }
                }
                used.insert(op.id.clone());
            }
            if let Some(r) = op.gate.record() {
                if r.is_empty() || !record_set.insert(r.to_owned()) {
                    return Err(invalid("empty or duplicate record", r));
                }
                records.push(r.to_owned());
            }
        }
        if let Some(name) = control_map.keys().find(|k| !used.contains(*k)) {
            return Err(invalid("unused control", name));
        }
        Ok(Self {
            inputs,
            outputs,
            initial,
            operations,
            controls: control_map,
            records,
            limits,
        })
    }
    pub fn compile(
        &self,
        assignment: &OriginalAssignment,
        strategy: CompilerStrategy,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OriginalBranch, OriginalError> {
        let l = self.limits;
        admit("max_faults", l.max_faults, assignment.faults.len() as u128)?;
        let bytes = assignment
            .true_values
            .iter()
            .map(|v| 32 + v.0.len() as u128)
            .sum::<u128>()
            + assignment
                .report_flips
                .iter()
                .map(|v| 32 + v.len() as u128)
                .sum::<u128>()
            + assignment
                .faults
                .iter()
                .map(|v| 48 + v.after.len() as u128)
                .sum::<u128>();
        admit("max_metadata_bytes", l.max_metadata_bytes, bytes)?;
        let known = self
            .records
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut values = BTreeMap::new();
        for (r, v) in &assignment.true_values {
            if !known.contains(r.as_str()) {
                return Err(invalid("unknown true record", r));
            }
            if values.insert(r.as_str(), *v).is_some() {
                return Err(invalid("duplicate true record", r));
            }
        }
        for r in &self.records {
            if !values.contains_key(r.as_str()) {
                return Err(invalid("missing true record", r));
            }
        }
        let mut reports = values.clone();
        for r in &assignment.report_flips {
            let value = reports
                .get_mut(r.as_str())
                .ok_or_else(|| invalid("unknown report flip", r))?;
            *value = !*value;
        }
        let ids = self
            .operations
            .iter()
            .map(|o| o.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut faults: BTreeMap<&str, Vec<(usize, &OriginalFault)>> = BTreeMap::new();
        for (i, f) in assignment.faults.iter().enumerate() {
            if !ids.contains(f.after.as_str()) {
                return Err(invalid("unknown fault anchor", &f.after));
            }
            faults.entry(&f.after).or_default().push((i, f));
        }
        let mut lower = BranchLowering::new(&self.inputs, l);
        let mut decisions = Vec::new();
        let mut origins = Vec::new();
        let mut emitted = 0usize;
        let mut emitted_bytes = 0u128;
        for op in &self.operations {
            let origin = OriginalOrigin::Operation(op.id.clone());
            origins.push(origin.clone());
            if op.gate == OriginalGate::Control {
                let c = &self.controls[&op.id];
                let inputs = OriginalControlInputs {
                    entries: c
                        .parents
                        .iter()
                        .map(|p| (p.clone(), reports[p.as_str()]))
                        .collect(),
                };
                let gates = (c.callback)(&inputs).map_err(|error| OriginalError::Callback {
                    operation: op.id.clone(),
                    error,
                })?;
                emitted = emitted
                    .checked_add(gates.len())
                    .ok_or(OriginalError::DimensionMismatch)?;
                admit(
                    "max_emitted_operations",
                    l.max_emitted_operations,
                    emitted as u128,
                )?;
                emitted_bytes += gates
                    .iter()
                    .map(|g| g.bytes() + op.id.len() as u128 + 64)
                    .sum::<u128>();
                admit("max_metadata_bytes", l.max_metadata_bytes, emitted_bytes)?;
                // Admit the entire emission before applying any emitted gate.
                for g in &gates {
                    g.validate()?;
                    if g.record().is_some() || *g == OriginalGate::Control {
                        return Err(invalid("callback must be measurement-free", &op.id));
                    }
                }
                decisions.push(OriginalDecision {
                    operation: op.id.clone(),
                    version: c.version.clone(),
                    inputs,
                    emitted_operations: gates.len(),
                });
                for (ordinal, g) in gates.iter().enumerate() {
                    let origin = OriginalOrigin::ControlEmission {
                        control: op.id.clone(),
                        ordinal,
                    };
                    lower.gate(g, &origin, Some(&values))?;
                    origins.push(origin);
                }
            } else {
                lower.gate(&op.gate, &origin, Some(&values))?;
            }
            for &(ordinal, f) in faults.get(op.id.as_str()).into_iter().flatten() {
                let origin = OriginalOrigin::Fault {
                    after: op.id.clone(),
                    ordinal,
                };
                // Native identity faults retain order but have no lifetime effect.
                if f.pauli != OriginalPauli::I {
                    lower.gate(
                        &OriginalGate::Pauli {
                            wire: f.wire,
                            pauli: f.pauli,
                        },
                        &origin,
                        Some(&values),
                    )?;
                }
                origins.push(origin);
            }
        }
        if lower.live.len() != self.outputs.len()
            || self.outputs.iter().any(|w| !lower.live.contains_key(w))
        {
            return Err(OriginalError::OutputMismatch);
        }
        let keep = self
            .outputs
            .iter()
            .map(|w| lower.live[w])
            .chain(lower.environment_slots.iter().copied())
            .collect();
        let initial = with_zero_ancillas(&self.initial, lower.slots)?;
        let unitary = UnitaryCircuit::new(lower.slots, lower.gates)?
            .compile_stabilizer_with_path_limit(&initial, arithmetic, strategy, l.max_path_bits)?;
        let records = self
            .records
            .iter()
            .map(|r| OriginalRecord {
                name: r.clone(),
                true_value: values[r.as_str()],
                reported_value: reports[r.as_str()],
            })
            .collect();
        Ok(OriginalBranch {
            unitary,
            keep,
            fixed: lower.fixed,
            outputs: self.outputs.clone(),
            environment: lower.environment,
            records,
            decisions,
            origins,
            limits: l,
        })
    }
}
struct BranchLowering {
    live: BTreeMap<OriginalWire, usize>,
    slots: usize,
    gates: Vec<U>,
    fixed: Vec<(usize, bool)>,
    record_slots: Vec<(usize, bool)>,
    environment_slots: Vec<usize>,
    environment: Vec<OriginalEnvironment>,
    limits: OriginalLimits,
}
impl BranchLowering {
    fn new(inputs: &[OriginalWire], limits: OriginalLimits) -> Self {
        Self {
            live: inputs.iter().enumerate().map(|(i, &w)| (w, i)).collect(),
            slots: inputs.len(),
            gates: vec![],
            fixed: vec![],
            record_slots: vec![],
            environment_slots: vec![],
            environment: vec![],
            limits,
        }
    }
    fn live(&self, w: OriginalWire) -> Result<usize, OriginalError> {
        self.live.get(&w).copied().ok_or(OriginalError::Lifetime {
            wire: w,
            expected_live: true,
        })
    }
    fn fresh(&mut self) -> Result<usize, OriginalError> {
        admit(
            "max_physical_slots",
            self.limits.max_physical_slots,
            self.slots as u128 + 1,
        )?;
        let slot = self.slots;
        self.slots += 1;
        Ok(slot)
    }
    fn push(&mut self, g: U) -> Result<(), OriginalError> {
        admit(
            "max_gates",
            self.limits.max_gates,
            self.gates.len() as u128 + 1,
        )?;
        self.gates.push(g);
        Ok(())
    }
    fn c(&mut self, g: C) -> Result<(), OriginalError> {
        self.push(U::Clifford(g))
    }
    fn basis(&mut self, q: usize, p: OriginalPauli, inverse: bool) -> Result<(), OriginalError> {
        if inverse {
            if p == OriginalPauli::X || p == OriginalPauli::Y {
                self.c(C::H(q))?;
            }
            if p == OriginalPauli::Y {
                self.c(C::S { qubit: q, turns: 1 })?;
            }
        } else {
            if p == OriginalPauli::Y {
                self.c(C::S { qubit: q, turns: 3 })?;
            }
            if p == OriginalPauli::X || p == OriginalPauli::Y {
                self.c(C::H(q))?;
            }
        }
        Ok(())
    }
    fn gate(
        &mut self,
        g: &OriginalGate,
        origin: &OriginalOrigin,
        values: Option<&BTreeMap<&str, bool>>,
    ) -> Result<(), OriginalError> {
        match g {
            OriginalGate::Prepare { wire, plus } => {
                if self.live.contains_key(wire) {
                    return Err(OriginalError::Lifetime {
                        wire: *wire,
                        expected_live: false,
                    });
                }
                let q = self.fresh()?;
                self.live.insert(*wire, q);
                if *plus {
                    self.c(C::H(q))?;
                }
            }
            OriginalGate::Reset { wire, .. } | OriginalGate::Discard(wire) => {
                let old = self.live(*wire)?;
                self.live.remove(wire);
                self.environment_slots.push(old);
                self.environment.push(OriginalEnvironment {
                    wire: *wire,
                    retired_by: origin.clone(),
                });
                if let OriginalGate::Reset { plus, .. } = g {
                    let q = self.fresh()?;
                    self.live.insert(*wire, q);
                    if *plus {
                        self.c(C::H(q))?;
                    }
                }
            }
            OriginalGate::Destructive {
                wire,
                basis,
                record,
            } => {
                let q = self.live(*wire)?;
                if *basis == ReadoutBasis::X {
                    self.c(C::H(q))?;
                }
                if let Some(values) = values {
                    self.fixed.push((q, values[record.as_str()]));
                } else {
                    self.record_slots.push((q, false));
                }
                self.live.remove(wire);
            }
            OriginalGate::Measure {
                factors,
                inverted,
                record,
            } => {
                let slots = factors
                    .iter()
                    .map(|(w, p)| Ok((self.live(*w)?, *p)))
                    .collect::<Result<Vec<_>, OriginalError>>()?;
                let ancilla = self.fresh()?;
                for &(q, p) in &slots {
                    self.basis(q, p, false)?;
                    self.c(C::Cx {
                        control: q,
                        target: ancilla,
                    })?;
                    self.basis(q, p, true)?;
                }
                if let Some(values) = values {
                    self.fixed
                        .push((ancilla, values[record.as_str()] ^ inverted));
                } else {
                    self.record_slots.push((ancilla, *inverted));
                }
            }
            OriginalGate::Control => return Err(invalid("uninstantiated control", "")),
            _ => {
                let slots = g
                    .wires()
                    .into_iter()
                    .map(|w| self.live(w))
                    .collect::<Result<Vec<_>, _>>()?;
                match g {
                    OriginalGate::Identity(_) => {}
                    OriginalGate::H(_) => self.c(C::H(slots[0]))?,
                    OriginalGate::Cx(..) => self.c(C::Cx {
                        control: slots[0],
                        target: slots[1],
                    })?,
                    OriginalGate::Cz(..) => self.c(C::Cz {
                        a: slots[0],
                        b: slots[1],
                    })?,
                    OriginalGate::Ccz(..) => self.push(U::Ccz {
                        a: slots[0],
                        b: slots[1],
                        c: slots[2],
                    })?,
                    OriginalGate::Phase { turns, .. } => {
                        if let Some(&pivot) = slots.last() {
                            for &q in &slots[..slots.len() - 1] {
                                self.c(C::Cx {
                                    control: q,
                                    target: pivot,
                                })?;
                            }
                            self.push(U::T {
                                qubit: pivot,
                                turns: turns.rem_euclid(8) as u8,
                            })?;
                            for &q in slots[..slots.len() - 1].iter().rev() {
                                self.c(C::Cx {
                                    control: q,
                                    target: pivot,
                                })?;
                            }
                        }
                    }
                    OriginalGate::Pauli { pauli, .. } => match pauli {
                        OriginalPauli::I => {}
                        OriginalPauli::X => self.c(C::X(slots[0]))?,
                        OriginalPauli::Z => self.c(C::Z(slots[0]))?,
                        OriginalPauli::Y => {
                            self.c(C::S {
                                qubit: slots[0],
                                turns: 3,
                            })?;
                            self.c(C::X(slots[0]))?;
                            self.c(C::S {
                                qubit: slots[0],
                                turns: 1,
                            })?;
                        }
                    },
                    _ => unreachable!("handled lifetime/record operations"),
                }
            }
        }
        Ok(())
    }
}

/// Complete coherent boundary, with no callbacks or mutable extraction state.
pub struct OriginalBranch {
    unitary: CompiledUnitary,
    keep: Vec<usize>,
    fixed: Vec<(usize, bool)>,
    outputs: Vec<OriginalWire>,
    environment: Vec<OriginalEnvironment>,
    records: Vec<OriginalRecord>,
    decisions: Vec<OriginalDecision>,
    origins: Vec<OriginalOrigin>,
    limits: OriginalLimits,
}
impl OriginalBranch {
    pub fn outputs(&self) -> &[OriginalWire] {
        &self.outputs
    }
    pub fn environment(&self) -> &[OriginalEnvironment] {
        &self.environment
    }
    pub fn records(&self) -> &[OriginalRecord] {
        &self.records
    }
    pub fn decisions(&self) -> &[OriginalDecision] {
        &self.decisions
    }
    pub fn origins(&self) -> &[OriginalOrigin] {
        &self.origins
    }
    /// Actual diagnostics of the underlying fixed-slot coherent compiler.
    pub fn compilation(&self) -> &CompilationDiagnostics {
        self.unitary.compilation()
    }
    pub fn width(&self) -> usize {
        self.keep.len()
    }
    /// Independent direct coherent-network amplitude, in outputs/environment
    /// order. `max_terms` bounds the network's own exact phase-sum route.
    pub fn amplitude(
        &self,
        value: &BitVector,
        max_terms: usize,
        a: &ScalarArithmetic,
    ) -> Result<ExactScalar, OriginalError> {
        if value.width() != self.width() {
            return Err(OriginalError::DimensionMismatch);
        }
        let mut full = BitVector::zeros(self.unitary.width())?;
        for (i, &q) in self.keep.iter().enumerate() {
            full.set(q, value.get(i)?)?;
        }
        for &(q, b) in &self.fixed {
            full.set(q, b)?;
        }
        Ok(self.unitary.amplitude(&full, max_terms, a)?.value)
    }
    /// Owned projected side kets. Global phases and coherent interference are
    /// retained; these arbitrary completed terms carry no resource certificate.
    pub fn expansion(&self, a: &ScalarArithmetic) -> Result<Vec<AffineKet>, OriginalError> {
        let expansion = self
            .unitary
            .stabilizer_expansion(self.limits.expansion, a)?;
        let mut terms = Vec::new();
        for k in expansion.entries() {
            self.geometry(k.width(), k.columns().len(), self.fixed.len())?;
            let mut rows = Vec::with_capacity(self.fixed.len());
            let mut rhs = BitVector::zeros(self.fixed.len())?;
            for (i, &(q, b)) in self.fixed.iter().enumerate() {
                let mut row = BitVector::zeros(k.columns().len())?;
                for (j, c) in k.columns().iter().enumerate() {
                    row.set(j, c.get(q)?)?;
                }
                rows.push(row);
                rhs.set(i, k.offset().get(q)? ^ b)?;
            }
            if let Some(k) = k.constrain_paths(&rows, &rhs)?
                && let Some(k) = k.retain_boundary(&self.keep)?.contract(a)?
            {
                terms.push(k);
            }
        }
        Ok(terms)
    }
    fn geometry(&self, width: usize, paths: usize, rows: usize) -> Result<(), OriginalError> {
        admit(
            "max_path_bits",
            self.limits.expansion.max_path_bits,
            paths as u128,
        )?;
        let p = paths as u128;
        let n = width as u128;
        let r = rows as u128;
        // Tensor/constraint/restriction payload, headers and elimination scratch.
        let words =
            16 * (n * (p + 1).div_ceil(64) + (p + r) * (p + 1).div_ceil(64) + n + p + r + 128);
        admit(
            "max_geometry_words",
            self.limits.expansion.max_geometry_words,
            words,
        )
    }
    fn spec(&self, width: usize) -> Result<CompletedRecordSpec, OriginalError> {
        Ok(CompletedRecordSpec::new(
            width,
            &[],
            &[],
            &BitVector::zeros(0)?,
            self.limits.completed,
        )?)
    }
    pub fn probability(&self, a: &ScalarArithmetic) -> Result<ExactScalar, OriginalError> {
        Ok(self
            .spec(self.width())?
            .probability(&self.expansion(a)?, a)?
            .total(a)?)
    }
    /// Unnormalized physical-output moment. Environment legs carry identity.
    pub fn output_moment(
        &self,
        word: &PauliWord,
        a: &ScalarArithmetic,
    ) -> Result<ExactScalar, OriginalError> {
        if word.width() != self.outputs.len() {
            return Err(OriginalError::DimensionMismatch);
        }
        let word = PauliWord::new(
            BitVector::from_indices(self.width(), word.x().ones())?,
            BitVector::from_indices(self.width(), word.z().ones())?,
            word.phase(),
        )?;
        Ok(self
            .spec(self.width())?
            .moment(&self.expansion(a)?, &word, a)?
            .total(a)?)
    }
    /// Exact `<target|rho_outputs|target>` for an arbitrary coherent target.
    /// Targets may be subnormalized or cancelling; environments are traced.
    pub fn target_weight(
        &self,
        target: &[AffineKet],
        a: &ScalarArithmetic,
    ) -> Result<ExactScalar, OriginalError> {
        admit(
            "max_source_entries",
            self.limits.completed.max_source_entries,
            target.len() as u128,
        )?;
        if target.iter().any(|k| k.width() != self.outputs.len()) {
            return Err(OriginalError::DimensionMismatch);
        }
        let terms = self.expansion(a)?;
        let count = terms.len() as u128 * target.len() as u128;
        admit(
            "max_source_entries",
            self.limits.completed.max_source_entries,
            count,
        )?;
        admit("max_pairs", self.limits.completed.max_pairs, count * count)?;
        for l in &terms {
            for r in target {
                let paths = l
                    .columns()
                    .len()
                    .checked_add(r.columns().len())
                    .ok_or(OriginalError::DimensionMismatch)?;
                let width = l
                    .width()
                    .checked_add(r.width())
                    .ok_or(OriginalError::DimensionMismatch)?;
                self.geometry(width, paths, self.outputs.len())?;
            }
        }
        let mut projected = Vec::new();
        for l in &terms {
            for r in target {
                let joint = l.tensor(&r.conjugate(), a)?;
                let paths = joint.columns().len();
                let mut rows = Vec::new();
                let mut rhs = BitVector::zeros(self.outputs.len())?;
                for q in 0..self.outputs.len() {
                    let mut row = BitVector::zeros(paths)?;
                    for (i, c) in joint.columns().iter().enumerate() {
                        row.set(i, c.get(q)? ^ c.get(l.width() + q)?)?;
                    }
                    rows.push(row);
                    rhs.set(
                        q,
                        joint.offset().get(q)? ^ joint.offset().get(l.width() + q)?,
                    )?;
                }
                if let Some(k) = joint.constrain_paths(&rows, &rhs)? {
                    let keep = (self.outputs.len()..self.width()).collect::<Vec<_>>();
                    if let Some(k) = k.retain_boundary(&keep)?.contract(a)? {
                        projected.push(k);
                    }
                }
            }
        }
        Ok(self
            .spec(self.environment.len())?
            .probability(&projected, a)?
            .total(a)?)
    }
}
