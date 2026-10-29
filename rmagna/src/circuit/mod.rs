//! Magna text circuits with exact probabilities and causal classical expressions.

mod parse;
use crate::probability::Probability;
pub use parse::{ParseError, ParseOptions, parse_circuit};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BitAtom {
    Record(usize),
    Decision { call: usize, bit: usize },
}

/// Canonical XOR over reported records and external decision bits. Registers
/// are resolved at their read site; overwriting one cannot change an old use.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BitExpr {
    pub constant: bool,
    pub terms: BTreeSet<BitAtom>,
}
impl BitExpr {
    pub fn atom(atom: BitAtom) -> Self {
        Self {
            constant: false,
            terms: BTreeSet::from([atom]),
        }
    }
    pub fn xor_assign(&mut self, other: &Self) {
        self.constant ^= other.constant;
        for atom in &other.terms {
            if !self.terms.remove(atom) {
                self.terms.insert(atom.clone());
            }
        }
    }
    pub fn inverted(mut self) -> Self {
        self.constant = !self.constant;
        self
    }
    pub fn is_affine(&self) -> bool {
        self.terms.iter().all(|a| matches!(a, BitAtom::Record(_)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Basis {
    X,
    Y,
    Z,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauliProduct {
    pub factors: Vec<(usize, Basis)>,
    pub inverted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    I,
    X,
    Y,
    Z,
    H,
    S,
    Sdag,
    T,
    Tdag,
    SqrtX,
    SqrtXdag,
    SqrtY,
    SqrtYdag,
    Hxy,
    Hyz,
    Cxyz,
    Czyx,
    Cx,
    Cy,
    Cz,
    Swap,
    Iswap,
    IswapDag,
    Cs,
    CsDag,
    Ch,
    Ccz,
    Ccx,
}
impl Gate {
    pub fn arity(&self) -> usize {
        match self {
            Self::Ccz | Self::Ccx => 3,
            Self::Cx
            | Self::Cy
            | Self::Cz
            | Self::Swap
            | Self::Iswap
            | Self::IswapDag
            | Self::Cs
            | Self::CsDag
            | Self::Ch => 2,
            _ => 1,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::I => "I",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::H => "H",
            Self::S => "S",
            Self::Sdag => "S_DAG",
            Self::T => "T",
            Self::Tdag => "T_DAG",
            Self::SqrtX => "SQRT_X",
            Self::SqrtXdag => "SQRT_X_DAG",
            Self::SqrtY => "SQRT_Y",
            Self::SqrtYdag => "SQRT_Y_DAG",
            Self::Hxy => "H_XY",
            Self::Hyz => "H_YZ",
            Self::Cxyz => "C_XYZ",
            Self::Czyx => "C_ZYX",
            Self::Cx => "CX",
            Self::Cy => "CY",
            Self::Cz => "CZ",
            Self::Swap => "SWAP",
            Self::Iswap => "ISWAP",
            Self::IswapDag => "ISWAP_DAG",
            Self::Cs => "CS",
            Self::CsDag => "CS_DAG",
            Self::Ch => "CH",
            Self::Ccz => "CCZ",
            Self::Ccx => "CCX",
        }
    }
    pub(super) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "I" => Self::I,
            "X" => Self::X,
            "Y" => Self::Y,
            "Z" => Self::Z,
            "H" | "H_XZ" => Self::H,
            "S" => Self::S,
            "S_DAG" => Self::Sdag,
            "T" => Self::T,
            "T_DAG" => Self::Tdag,
            "SQRT_X" => Self::SqrtX,
            "SQRT_X_DAG" => Self::SqrtXdag,
            "SQRT_Y" => Self::SqrtY,
            "SQRT_Y_DAG" => Self::SqrtYdag,
            "H_XY" => Self::Hxy,
            "H_YZ" => Self::Hyz,
            "C_XYZ" => Self::Cxyz,
            "C_ZYX" => Self::Czyx,
            "CX" | "CNOT" | "ZCX" => Self::Cx,
            "CY" | "ZCY" => Self::Cy,
            "CZ" | "ZCZ" => Self::Cz,
            "SWAP" => Self::Swap,
            "ISWAP" => Self::Iswap,
            "ISWAP_DAG" => Self::IswapDag,
            "CS" => Self::Cs,
            "CS_DAG" => Self::CsDag,
            "CH" => Self::Ch,
            "CCZ" => Self::Ccz,
            "CCX" => Self::Ccx,
            _ => return None,
        })
    }
    pub(super) fn classical_target(&self) -> Option<Self> {
        match self {
            Self::Cx => Some(Self::X),
            Self::Cy => Some(Self::Y),
            Self::Cz => Some(Self::Z),
            Self::Cs => Some(Self::S),
            Self::CsDag => Some(Self::Sdag),
            Self::Ch => Some(Self::H),
            Self::Ccz => Some(Self::Cz),
            Self::Ccx => Some(Self::Cx),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoiseKind {
    X,
    Y,
    Z,
    Depolarize1,
    Depolarize2,
    Pauli1,
    Pauli2,
    Correlated { chain: usize, alternative: usize },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterWrite {
    pub register: usize,
    pub version: usize,
    pub expression: BitExpr,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Unitary {
        gate: Gate,
        qubits: Vec<usize>,
        control: Option<BitExpr>,
    },
    Reset {
        qubit: usize,
        basis: Basis,
    },
    Measure {
        product: PauliProduct,
        reset: bool,
        report_flip: Probability,
        record: usize,
    },
    Noise {
        kind: NoiseKind,
        probabilities: Vec<Probability>,
        qubits: Vec<usize>,
        product: Option<PauliProduct>,
        guard: Option<BitExpr>,
    },
    Control {
        name: String,
        call: Option<usize>,
        inputs: Vec<BitExpr>,
        outputs: Vec<RegisterWrite>,
    },
    Detector {
        index: usize,
        expression: BitExpr,
        coordinates: Vec<f64>,
    },
    Postselect {
        expression: BitExpr,
    },
    Observable {
        index: usize,
        expression: BitExpr,
    },
    Annotation {
        name: String,
        coordinates: Vec<f64>,
        qubits: Vec<usize>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub source: Option<String>,
    pub line: usize,
    pub column: usize,
    pub repeat: Vec<u64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LocatedOperation {
    pub id: usize,
    pub location: SourceLocation,
    pub operation: Operation,
}

#[derive(Clone, Debug)]
pub struct Circuit {
    operations: Vec<LocatedOperation>,
    num_qubits: usize,
    num_records: usize,
    num_instructions: usize,
    detectors: Vec<BitExpr>,
    observables: BTreeMap<usize, BitExpr>,
    external_calls: usize,
    has_guarded_noise: bool,
    // Retain syntax lost when classical expressions cancel or resolve to zero.
    declarations_with_targets: BTreeSet<usize>,
    // Operation IDs with a declared readout probability, including zero.
    declared_readout_noise: BTreeSet<usize>,
    source_text: String,
}
impl Circuit {
    pub fn operations(&self) -> &[LocatedOperation] {
        &self.operations
    }
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }
    pub fn num_records(&self) -> usize {
        self.num_records
    }
    pub fn num_instructions(&self) -> usize {
        self.num_instructions
    }
    pub fn detectors(&self) -> &[BitExpr] {
        &self.detectors
    }
    pub fn observables(&self) -> &BTreeMap<usize, BitExpr> {
        &self.observables
    }
    pub fn external_calls(&self) -> usize {
        self.external_calls
    }
    pub fn to_text(&self) -> &str {
        &self.source_text
    }
    pub fn format_version(&self) -> &'static str {
        "magna-0.1"
    }
    pub fn needs_history_sampling(&self) -> bool {
        self.external_calls != 0 || self.has_guarded_noise
    }
    /// Whether this measurement had an explicit readout probability argument.
    /// Omitted arguments and non-measurement operation IDs return false.
    pub fn readout_noise_declared(&self, operation_id: usize) -> bool {
        self.declared_readout_noise.contains(&operation_id)
    }
    pub(crate) fn declaration_has_targets(&self, operation_id: usize) -> bool {
        self.declarations_with_targets.contains(&operation_id)
    }
}

impl Circuit {
    // Only the causal runtime constructs these views from validated parsed
    // operations. Original operation IDs and physical wire labels stay stable.
    pub(crate) fn runtime_view(&self, operations: Vec<LocatedOperation>) -> Self {
        let num_records = operations
            .iter()
            .filter_map(|op| match op.operation {
                Operation::Measure { record, .. } => Some(record + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let detectors = operations
            .iter()
            .filter_map(|op| match &op.operation {
                Operation::Detector { expression, .. } => Some(expression.clone()),
                _ => None,
            })
            .collect();
        let mut observables = BTreeMap::<usize, BitExpr>::new();
        for op in &operations {
            if let Operation::Observable { index, expression } = &op.operation {
                observables
                    .entry(*index)
                    .or_default()
                    .xor_assign(expression);
            }
        }
        let external_calls = operations
            .iter()
            .filter(|op| matches!(op.operation, Operation::Control { call: Some(_), .. }))
            .count();
        let has_guarded_noise = operations
            .iter()
            .any(|op| matches!(op.operation, Operation::Noise { guard: Some(_), .. }));
        Self {
            num_instructions: operations.len(),
            operations,
            num_qubits: self.num_qubits,
            num_records,
            detectors,
            observables,
            external_calls,
            has_guarded_noise,
            declarations_with_targets: self.declarations_with_targets.clone(),
            declared_readout_noise: self.declared_readout_noise.clone(),
            source_text: self.source_text.clone(),
        }
    }
}

impl Circuit {
    pub(crate) fn minimum_width(&self, width: usize) -> Self {
        let mut circuit = self.clone();
        circuit.num_qubits = circuit.num_qubits.max(width);
        circuit
    }
}

#[cfg(test)]
mod declaration_metadata_tests {
    use super::*;
    use crate::noise::{OriginalNoiseAction, OriginalNoiseOptions, original_noise_categories};

    #[test]
    fn runtime_view_retains_owned_declarations_and_original_anchors() {
        let circuit =
            parse_circuit("M 0\nM(0) 0\nMX 1\nMPP(0) Z0 X1", &ParseOptions::default()).unwrap();
        let selected = circuit
            .operations()
            .iter()
            .filter(|op| [1, 3, 4].contains(&op.id))
            .cloned()
            .collect();
        let view = circuit.runtime_view(selected);
        drop(circuit);
        let table = original_noise_categories(&view, &OriginalNoiseOptions::default()).unwrap();
        assert_eq!(table.len(), 3);
        for (site, (category, operation)) in table.iter().zip([1, 3, 4]).enumerate() {
            assert_eq!(category.internal_site, site);
            assert_eq!(category.source_operations[0].operation, operation);
            assert_eq!(
                category.channels[0].action,
                OriginalNoiseAction::ReportFlip { record: operation }
            );
            assert!(view.readout_noise_declared(operation));
        }
    }
}
