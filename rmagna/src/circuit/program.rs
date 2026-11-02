//! Checked owned input for the static affine quantum compiler.

use super::*;
use crate::probability::ProbabilityError;
use std::fmt;

/// Construction limits, separate from the coherent compiler's lowering limits.
#[derive(Clone, Copy, Debug)]
pub struct OperationProgramOptions {
    pub max_qubits: usize,
    pub max_operations: usize,
    pub max_records: usize,
}
impl Default for OperationProgramOptions {
    fn default() -> Self {
        Self {
            max_qubits: 65536,
            max_operations: 1_000_000,
            max_records: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationProgramErrorKind {
    ResourceLimit {
        option: &'static str,
        limit: usize,
        required: usize,
    },
    QubitOutOfBounds {
        qubit: usize,
        width: usize,
    },
    RepeatedQubit {
        qubit: usize,
    },
    GateArity {
        gate: Gate,
        expected: usize,
        actual: usize,
    },
    NoiseTargetCount {
        expected: usize,
        actual: usize,
    },
    NoiseProbabilityCount {
        expected: usize,
        actual: usize,
    },
    EmptyMeasurementProduct,
    ResetMeasurementArity {
        actual: usize,
    },
    RecordSequence {
        expected: usize,
        actual: usize,
    },
    DetectorSequence {
        expected: usize,
        actual: usize,
    },
    UnknownRecord {
        record: usize,
        available: usize,
    },
    ExternalDecision {
        call: usize,
        bit: usize,
    },
    UnsupportedControlGate {
        gate: Gate,
    },
    ControlOutputCount {
        actual: usize,
    },
    ControlInputCount {
        name: &'static str,
        expected: usize,
        actual: usize,
    },
    ControlOutputExpression {
        name: &'static str,
    },
    RegisterVersion {
        register: usize,
        expected: usize,
        actual: usize,
    },
    Unsupported {
        feature: &'static str,
    },
    NonFiniteCoordinates,
    Probability(ProbabilityError),
}

/// A construction error at a zero-based operation, or at whole-program admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationProgramError {
    pub operation: Option<usize>,
    pub kind: OperationProgramErrorKind,
}
impl fmt::Display for OperationProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(operation) = self.operation {
            write!(f, "operation {operation}: ")?;
        }
        use OperationProgramErrorKind::*;
        match &self.kind {
            ResourceLimit {
                option,
                limit,
                required,
            } => write!(f, "{option} = {limit} is insufficient; need {required}"),
            QubitOutOfBounds { qubit, width } => write!(
                f,
                "physical qubit {qubit} is outside declared width {width}"
            ),
            RepeatedQubit { qubit } => write!(
                f,
                "physical qubit {qubit} occurs more than once in this operation"
            ),
            GateArity {
                gate,
                expected,
                actual,
            } => write!(
                f,
                "{} requires {expected} distinct targets; got {actual}",
                gate.name()
            ),
            NoiseTargetCount { expected, actual } => write!(
                f,
                "noise operation requires {expected} distinct targets; got {actual}"
            ),
            NoiseProbabilityCount { expected, actual } => write!(
                f,
                "noise operation requires {expected} explicit probabilities; got {actual}"
            ),
            EmptyMeasurementProduct => write!(f, "measurement requires a nonempty Pauli product"),
            ResetMeasurementArity { actual } => write!(
                f,
                "measurement with reset requires one Pauli factor; got {actual}"
            ),
            RecordSequence { expected, actual } => write!(
                f,
                "measurement must write next record {expected}; got {actual}"
            ),
            DetectorSequence { expected, actual } => {
                write!(f, "detector must use next index {expected}; got {actual}")
            }
            UnknownRecord { record, available } => write!(
                f,
                "record {record} is unavailable; only earlier records 0..{available} may be read"
            ),
            ExternalDecision { call, bit } => write!(
                f,
                "decision {call}:{bit} requires the external causal runtime"
            ),
            UnsupportedControlGate { gate } => write!(
                f,
                "{} has no supported earlier-record coherent control",
                gate.name()
            ),
            ControlOutputCount { actual } => write!(
                f,
                "builtin control requires exactly one output; got {actual}"
            ),
            ControlInputCount {
                name,
                expected,
                actual,
            } => write!(f, "CONTROL[{name}] requires {expected} input; got {actual}"),
            ControlOutputExpression { name } => write!(
                f,
                "CONTROL[{name}] output disagrees with its resolved affine inputs"
            ),
            RegisterVersion {
                register,
                expected,
                actual,
            } => write!(
                f,
                "control register {register} must write version {expected}; got {actual}"
            ),
            Unsupported { feature } => write!(
                f,
                "{feature} is unsupported by owned static affine programs"
            ),
            NonFiniteCoordinates => write!(f, "detector coordinates must be finite"),
            Probability(error) => write!(
                f,
                "exclusive noise probabilities must sum to at most one: {error}"
            ),
        }
    }
}
impl std::error::Error for OperationProgramError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let OperationProgramErrorKind::Probability(error) = &self.kind {
            Some(error)
        } else {
            None
        }
    }
}

/// Immutable owned operations, validated for static affine quantum compilation.
///
/// Physical width is explicit, including unused wires and width zero. Record and
/// detector indices start at zero and must be sequential. Expressions may read
/// only strictly earlier reported records. Each noise operation describes one
/// one- or two-qubit category, with exact residual identity probability.
/// Builtin XOR/NOT declarations retain checked resolved output expressions and
/// sequential per-register versions. They do not invoke an external callback.
///
/// This input has no parsed-source provenance, text serialization, external
/// controller callbacks or conversion to a public [`Circuit`]. Edited
/// [`CircuitView`] values continue to reject parsed/compiler access.
///
/// Use [`crate::instrument::compile_operation_program`] or
/// [`crate::instrument::compile_operation_program_stabilizer`] to compile.
///
/// ```
/// use rmagna::circuit::{Gate, Operation, OperationProgram};
/// let program = OperationProgram::new(1, vec![Operation::Unitary {
///     gate: Gate::H, qubits: vec![0], control: None,
/// }])?;
/// assert_eq!(program.num_qubits(), 1);
/// # Ok::<(), rmagna::circuit::OperationProgramError>(())
/// ```
///
/// Parsed-source APIs are deliberately unavailable:
/// ```compile_fail
/// use rmagna::circuit::OperationProgram;
/// let program = OperationProgram::new(0, vec![]).unwrap();
/// program.to_text();
/// ```
/// It also cannot be passed to parsed-only entry points:
/// ```compile_fail
/// use rmagna::circuit::OperationProgram;
/// use rmagna::instrument::{CompileOptions, compile_affine};
/// use rmagna::scalar::ScalarArithmetic;
/// let program = OperationProgram::new(0, vec![]).unwrap();
/// compile_affine(&program, &CompileOptions::default(), &ScalarArithmetic::default());
/// ```
#[derive(Clone, Debug)]
pub struct OperationProgram {
    operations: Vec<Operation>,
    num_qubits: usize,
    num_records: usize,
    detectors: Vec<BitExpr>,
    observables: BTreeMap<usize, BitExpr>,
}
impl OperationProgram {
    pub fn new(
        num_qubits: usize,
        operations: Vec<Operation>,
    ) -> Result<Self, OperationProgramError> {
        Self::with_options(num_qubits, operations, &OperationProgramOptions::default())
    }

    /// Admit an immutable program only after all shape, causality and quota checks pass.
    /// Compilation can still reject its independent path/gate/coherent budgets.
    pub fn with_options(
        num_qubits: usize,
        operations: Vec<Operation>,
        options: &OperationProgramOptions,
    ) -> Result<Self, OperationProgramError> {
        use OperationProgramErrorKind::*;
        let admission = |option, limit, required| {
            if required > limit {
                Err(OperationProgramError {
                    operation: None,
                    kind: ResourceLimit {
                        option,
                        limit,
                        required,
                    },
                })
            } else {
                Ok(())
            }
        };
        admission("max_qubits", options.max_qubits, num_qubits)?;
        admission("max_operations", options.max_operations, operations.len())?;
        let mut records = 0;
        let mut detectors = Vec::new();
        let mut observables = BTreeMap::<usize, BitExpr>::new();
        let mut register_versions = BTreeMap::<usize, usize>::new();
        for (index, operation) in operations.iter().enumerate() {
            let error = |kind| OperationProgramError {
                operation: Some(index),
                kind,
            };
            let targets = |qubits: &[usize]| -> Result<(), OperationProgramError> {
                let mut seen = BTreeSet::new();
                for &qubit in qubits {
                    if qubit >= num_qubits {
                        return Err(error(QubitOutOfBounds {
                            qubit,
                            width: num_qubits,
                        }));
                    }
                    if !seen.insert(qubit) {
                        return Err(error(RepeatedQubit { qubit }));
                    }
                }
                Ok(())
            };
            let expression = |expression: &BitExpr| -> Result<(), OperationProgramError> {
                for atom in &expression.terms {
                    match atom {
                        BitAtom::Record(record) if *record >= records => {
                            return Err(error(UnknownRecord {
                                record: *record,
                                available: records,
                            }));
                        }
                        BitAtom::Decision { call, bit } => {
                            return Err(error(ExternalDecision {
                                call: *call,
                                bit: *bit,
                            }));
                        }
                        _ => {}
                    }
                }
                Ok(())
            };
            match operation {
                Operation::Unitary {
                    gate,
                    qubits,
                    control,
                } => {
                    if qubits.len() != gate.arity() {
                        return Err(error(GateArity {
                            gate: *gate,
                            expected: gate.arity(),
                            actual: qubits.len(),
                        }));
                    }
                    targets(qubits)?;
                    if let Some(control) = control {
                        expression(control)?;
                        if !control.terms.is_empty() && gate.coherently_controlled().is_none() {
                            return Err(error(UnsupportedControlGate { gate: *gate }));
                        }
                    }
                }
                Operation::Reset { qubit, .. } => targets(&[*qubit])?,
                Operation::Measure {
                    product,
                    reset,
                    record,
                    ..
                } => {
                    if product.factors.is_empty() {
                        return Err(error(EmptyMeasurementProduct));
                    }
                    if *reset && product.factors.len() != 1 {
                        return Err(error(ResetMeasurementArity {
                            actual: product.factors.len(),
                        }));
                    }
                    targets(&product.factors.iter().map(|&(q, _)| q).collect::<Vec<_>>())?;
                    if *record != records {
                        return Err(error(RecordSequence {
                            expected: records,
                            actual: *record,
                        }));
                    }
                    if records >= options.max_records {
                        return Err(error(ResourceLimit {
                            option: "max_records",
                            limit: options.max_records,
                            required: records.saturating_add(1),
                        }));
                    }
                    records += 1;
                }
                Operation::Noise {
                    kind,
                    probabilities,
                    qubits,
                    product,
                    guard,
                } => {
                    if guard.is_some() {
                        return Err(error(Unsupported {
                            feature: "guarded noise",
                        }));
                    }
                    if matches!(kind, NoiseKind::Correlated { .. }) {
                        return Err(error(Unsupported {
                            feature: "correlated noise",
                        }));
                    }
                    if product.is_some() {
                        return Err(error(Unsupported {
                            feature: "Pauli product on noncorrelated noise",
                        }));
                    }
                    let count = match kind {
                        NoiseKind::Pauli1 => 3,
                        NoiseKind::Pauli2 => 15,
                        _ => 1,
                    };
                    if probabilities.len() != count {
                        return Err(error(NoiseProbabilityCount {
                            expected: count,
                            actual: probabilities.len(),
                        }));
                    }
                    let arity = if matches!(kind, NoiseKind::Depolarize2 | NoiseKind::Pauli2) {
                        2
                    } else {
                        1
                    };
                    if qubits.len() != arity {
                        return Err(error(NoiseTargetCount {
                            expected: arity,
                            actual: qubits.len(),
                        }));
                    }
                    targets(qubits)?;
                    crate::probability::Probability::checked_sum(probabilities.iter().cloned())
                        .map_err(|e| error(Probability(e)))?;
                }
                Operation::Detector {
                    index: detector,
                    expression: expr,
                    coordinates,
                } => {
                    if *detector != detectors.len() {
                        return Err(error(DetectorSequence {
                            expected: detectors.len(),
                            actual: *detector,
                        }));
                    }
                    expression(expr)?;
                    if !coordinates.iter().all(|v| v.is_finite()) {
                        return Err(error(NonFiniteCoordinates));
                    }
                    detectors.push(expr.clone());
                }
                Operation::Postselect { expression: expr } => expression(expr)?,
                Operation::Observable {
                    index,
                    expression: expr,
                } => {
                    expression(expr)?;
                    observables.entry(*index).or_default().xor_assign(expr);
                }
                Operation::Control {
                    name,
                    call,
                    inputs,
                    outputs,
                } => {
                    if call.is_some() {
                        return Err(error(Unsupported {
                            feature: "external control nodes",
                        }));
                    }
                    for input in inputs {
                        expression(input)?;
                    }
                    for output in outputs {
                        expression(&output.expression)?;
                    }
                    validate_builtin_control(name, inputs, outputs, &mut register_versions)
                        .map_err(error)?;
                }
                Operation::Annotation { .. } => {
                    return Err(error(Unsupported {
                        feature: "annotations",
                    }));
                }
            }
        }
        Ok(Self {
            operations,
            num_qubits,
            num_records: records,
            detectors,
            observables,
        })
    }

    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }
    pub fn num_records(&self) -> usize {
        self.num_records
    }
    pub fn detectors(&self) -> &[BitExpr] {
        &self.detectors
    }
    pub fn observables(&self) -> &BTreeMap<usize, BitExpr> {
        &self.observables
    }

    // Private adapter to the existing checked lowering, never a parsed input
    // exposed to callers. No source parser, text serialization or edited view
    // is used. Operation indices remain stable for compiler fault identities.
    pub(crate) fn compilation_input(&self) -> Circuit {
        Circuit {
            operations: self
                .operations
                .iter()
                .enumerate()
                .map(|(id, operation)| LocatedOperation {
                    id,
                    location: SourceLocation {
                        source: None,
                        line: 0,
                        column: 0,
                        repeat: Vec::new(),
                    },
                    operation: operation.clone(),
                })
                .collect(),
            num_qubits: self.num_qubits,
            num_records: self.num_records,
            num_instructions: self.operations.len(),
            detectors: self.detectors.clone(),
            observables: self.observables.clone(),
            external_calls: 0,
            has_guarded_noise: false,
            declarations_with_targets: BTreeSet::new(),
            declared_readout_noise: self
                .operations
                .iter()
                .enumerate()
                .filter_map(|(id, op)| matches!(op, Operation::Measure { .. }).then_some(id))
                .collect(),
            source_text: String::new(),
        }
    }
}

fn validate_builtin_control(
    name: &str,
    inputs: &[BitExpr],
    outputs: &[RegisterWrite],
    versions: &mut BTreeMap<usize, usize>,
) -> Result<(), OperationProgramErrorKind> {
    use OperationProgramErrorKind::*;
    let name: &'static str = match name {
        "XOR" => "XOR",
        "NOT" => "NOT",
        _ => {
            return Err(Unsupported {
                feature: "control names other than builtin XOR/NOT",
            });
        }
    };
    if outputs.len() != 1 {
        return Err(ControlOutputCount {
            actual: outputs.len(),
        });
    }
    if name == "NOT" && inputs.len() != 1 {
        return Err(ControlInputCount {
            name,
            expected: 1,
            actual: inputs.len(),
        });
    }
    let mut expected = BitExpr::default();
    for input in inputs {
        expected.xor_assign(input);
    }
    if name == "NOT" {
        expected.constant ^= true;
    }
    let output = &outputs[0];
    // XOR's optional boolean constant has no separate syntax field in an
    // owned Operation. Its resolved output constant retains that choice.
    if output.expression.terms != expected.terms
        || (name == "NOT" && output.expression.constant != expected.constant)
    {
        return Err(ControlOutputExpression { name });
    }
    // A validated version is bounded by the number of preceding operations,
    // so adding one cannot overflow while the next Vec element exists.
    let version = versions.get(&output.register).map_or(0, |v| v + 1);
    if output.version != version {
        return Err(RegisterVersion {
            register: output.register,
            expected: version,
            actual: output.version,
        });
    }
    versions.insert(output.register, output.version);
    Ok(())
}
