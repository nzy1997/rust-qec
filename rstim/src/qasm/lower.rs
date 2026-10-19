use super::*;
use crate::ir::{StimInstr, StimTarget};
use std::collections::HashSet;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../tests/support/qasm2_reference.rs"]
mod boundary_reference;

/// Final classical storage is distinct from the append-only quantum record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "measurement", rename_all = "snake_case")]
pub enum ClassicalBinding {
    Zero,
    Measurement(usize),
}

#[derive(Clone, Debug, Serialize)]
pub struct CapabilityReport {
    pub stabilizer: bool,
    pub near_clifford: bool,
    pub num_qubits: usize,
    pub num_classical_bits: usize,
    pub num_measurements: usize,
    pub stabilizer_diagnostic: Option<QasmError>,
    pub near_clifford_diagnostic: Option<QasmError>,
}

/// Executable projection plus the information needed to interpret its outputs.
#[derive(Clone, Debug)]
pub struct LoweredQasmCircuit {
    pub instructions: Vec<StimInstr>,
    pub num_qubits: usize,
    pub measurement_events: Vec<usize>,
    pub classical_outputs: Vec<ClassicalBinding>,
    pub instruction_origins: Vec<Option<SourceSpan>>,
    pub target: Backend,
}

impl LoweredQasmCircuit {
    /// Reconstruct declared classical bits, including unwritten zeros and overwrites.
    pub fn project_classical(&self, record: &[bool]) -> Result<Vec<bool>, QasmError> {
        if record.len() != self.measurement_events.len() {
            return Err(QasmError::new(
                ErrorCode::Type,
                format!(
                    "expected {} measurement bits, got {}",
                    self.measurement_events.len(),
                    record.len()
                ),
            ));
        }
        self.classical_outputs
            .iter()
            .map(|binding| match binding {
                ClassicalBinding::Zero => Ok(false),
                ClassicalBinding::Measurement(id) => self
                    .measurement_events
                    .get(*id)
                    .and_then(|index| record.get(*index))
                    .copied()
                    .ok_or_else(|| {
                        QasmError::new(
                            ErrorCode::InvalidDocument,
                            "classical output references an invalid measurement",
                        )
                    }),
            })
            .collect()
    }
}

/// Retained semantic operands, including wide scoped barriers and feedback.
pub(super) fn target_work(kind: &OperationKind) -> usize {
    match kind {
        OperationKind::Gate { targets, .. } | OperationKind::Conditional { targets, .. } => {
            targets.len()
        }
        OperationKind::Barrier { qubits } => qubits.len(),
        OperationKind::Measure { .. } | OperationKind::Reset { .. } => 1,
    }
}

pub(super) fn validate(circuit: &QasmCircuit, limits: &QasmLimits) -> Result<(), QasmError> {
    let mut names = HashSet::new();
    let qubits = validate_registers(&circuit.quantum_registers, limits.max_qubits, &mut names)?;
    let bits = validate_registers(
        &circuit.classical_registers,
        limits.max_classical_bits,
        &mut names,
    )?;
    if qubits > u32::MAX as usize {
        return Err(limit("qubit indices exceed the native target range"));
    }
    if circuit.operations.len() > limits.max_operations {
        return Err(limit("expanded operation count exceeds limit"));
    }
    let mut targets = 0usize;
    for operation in &circuit.operations {
        targets = targets
            .checked_add(target_work(&operation.kind))
            .ok_or_else(|| {
                let mut error = limit("expanded operand work overflows the supported range");
                error.origin = operation.origin.clone();
                error
            })?;
        if targets > limits.max_operations {
            let mut error = limit("expanded operand work exceeds limit");
            error.origin = operation.origin.clone();
            return Err(error);
        }
        validate_operation(&operation.kind, qubits, bits, &circuit.classical_registers).map_err(
            |mut e| {
                e.origin = operation.origin.clone();
                e
            },
        )?;
    }
    Ok(())
}

fn validate_registers(
    registers: &[Register],
    max: usize,
    names: &mut HashSet<String>,
) -> Result<usize, QasmError> {
    let mut offset = 0usize;
    for r in registers {
        if !valid_identifier(&r.name) || !names.insert(r.name.clone()) {
            return Err(QasmError::new(
                ErrorCode::Type,
                format!("invalid or duplicate register name {:?}", r.name),
            ));
        }
        if r.size == 0 || r.offset != offset {
            return Err(QasmError::new(
                ErrorCode::Type,
                format!(
                    "register {} must have positive size and a contiguous offset",
                    r.name
                ),
            ));
        }
        offset = offset
            .checked_add(r.size)
            .ok_or_else(|| limit("register width overflow"))?;
        if offset > max {
            return Err(limit("declared register width exceeds limit"));
        }
    }
    Ok(offset)
}

pub(super) fn valid_identifier(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !matches!(
            name,
            "include"
                | "qreg"
                | "creg"
                | "gate"
                | "opaque"
                | "measure"
                | "reset"
                | "barrier"
                | "if"
                | "pi"
                | "sin"
                | "cos"
                | "tan"
                | "exp"
                | "ln"
                | "sqrt"
        )
}

fn validate_operation(
    kind: &OperationKind,
    qubits: usize,
    bits: usize,
    regs: &[Register],
) -> Result<(), QasmError> {
    let q = |index: u32| {
        if (index as usize) < qubits {
            Ok(())
        } else {
            Err(QasmError::new(
                ErrorCode::Type,
                format!("qubit {index} is outside declared width {qubits}"),
            ))
        }
    };
    let bit = |index: usize| {
        if index < bits {
            Ok(())
        } else {
            Err(QasmError::new(
                ErrorCode::Type,
                format!("classical bit {index} is outside declared width {bits}"),
            ))
        }
    };
    match kind {
        OperationKind::Gate { gate, targets } => {
            let arity = if matches!(gate, Gate::CX | Gate::CY | Gate::CZ | Gate::SWAP) {
                2
            } else {
                1
            };
            if targets.len() != arity {
                return Err(QasmError::new(
                    ErrorCode::Type,
                    format!(
                        "gate requires {arity} quantum operands, got {}",
                        targets.len()
                    ),
                ));
            }
            validate_qubits(targets, &q)?;
            match gate {
                Gate::Rx(a) | Gate::Ry(a) | Gate::Rz(a) => a.validate()?,
                Gate::U { theta, phi, lambda } => {
                    theta.validate()?;
                    phi.validate()?;
                    lambda.validate()?;
                }
                _ => {}
            }
        }
        OperationKind::Measure { qubit, bit: b } => {
            q(*qubit)?;
            bit(*b)?;
        }
        OperationKind::Reset { qubit } => {
            q(*qubit)?;
        }
        OperationKind::Barrier { qubits: targets } => {
            for target in targets {
                q(*target)?;
            }
        }
        OperationKind::Conditional {
            bit: b, targets, ..
        } => {
            bit(*b)?;
            if !regs.iter().any(|r| r.offset == *b && r.size == 1) {
                return Err(QasmError::new(
                    ErrorCode::Unsupported,
                    "v1 feedback requires a one-bit classical register",
                ));
            }
            if targets.is_empty() {
                return Err(QasmError::new(
                    ErrorCode::Type,
                    "conditional Pauli requires a quantum operand",
                ));
            }
            validate_qubits(targets, &q)?;
        }
    }
    Ok(())
}

fn validate_qubits(
    targets: &[u32],
    check: &impl Fn(u32) -> Result<(), QasmError>,
) -> Result<(), QasmError> {
    let mut seen = HashSet::new();
    for target in targets {
        check(*target)?;
        if !seen.insert(*target) {
            return Err(QasmError::new(
                ErrorCode::Type,
                "quantum operands alias after register expansion",
            ));
        }
    }
    Ok(())
}

pub(super) fn analyze(
    circuit: &QasmCircuit,
    limits: &QasmLimits,
) -> Result<CapabilityReport, QasmError> {
    validate(circuit, limits)?;
    let diagnostic = |result: Result<LoweredQasmCircuit, QasmError>| match result {
        Ok(_) => Ok(None),
        Err(e) if e.code == ErrorCode::BackendUnsupported => Ok(Some(e)),
        Err(e) => Err(e),
    };
    let stabilizer_diagnostic = diagnostic(lower(circuit, Backend::Stabilizer, limits))?;
    let near_clifford_diagnostic = diagnostic(lower(circuit, Backend::NearClifford, limits))?;
    Ok(CapabilityReport {
        stabilizer: stabilizer_diagnostic.is_none(),
        near_clifford: near_clifford_diagnostic.is_none(),
        num_qubits: circuit.num_qubits(),
        num_classical_bits: circuit.num_classical_bits(),
        num_measurements: circuit
            .operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::Measure { .. }))
            .count(),
        stabilizer_diagnostic,
        near_clifford_diagnostic,
    })
}

pub(super) fn lower(
    circuit: &QasmCircuit,
    target: Backend,
    limits: &QasmLimits,
) -> Result<LoweredQasmCircuit, QasmError> {
    validate(circuit, limits)?;
    let mut builder = Builder {
        plan: LoweredQasmCircuit {
            instructions: Vec::new(),
            num_qubits: circuit.num_qubits(),
            measurement_events: Vec::new(),
            classical_outputs: vec![ClassicalBinding::Zero; circuit.num_classical_bits()],
            instruction_origins: Vec::new(),
            target,
        },
        limits,
        origin: None,
    };
    if builder.plan.num_qubits > 0 {
        // Native execution infers width from targets; a declared idle qubit is real.
        builder.push(
            "I",
            Vec::new(),
            vec![StimTarget::Qubit((builder.plan.num_qubits - 1) as u32)],
        )?;
    }
    for operation in &circuit.operations {
        builder.origin = operation.origin.clone();
        builder.operation(&operation.kind).map_err(|mut e| {
            if e.origin.is_none() {
                e.origin = operation.origin.clone();
            }
            e
        })?;
    }
    match target {
        Backend::Stabilizer => crate::validation::validate_circuit(&builder.plan.instructions),
        Backend::NearClifford => {
            crate::near_clifford::NearCliffordExecutor::compile(builder.plan.instructions.clone())
                .map(|_| ())
        }
    }
    .map_err(|e| QasmError::new(ErrorCode::BackendUnsupported, e))?;
    Ok(builder.plan)
}

struct Builder<'a> {
    plan: LoweredQasmCircuit,
    limits: &'a QasmLimits,
    origin: Option<SourceSpan>,
}

impl Builder<'_> {
    fn push(
        &mut self,
        name: &str,
        args: Vec<f64>,
        targets: Vec<StimTarget>,
    ) -> Result<(), QasmError> {
        if self.plan.instructions.len() >= self.limits.max_operations {
            return Err(limit("lowered instruction count exceeds limit"));
        }
        self.plan
            .instructions
            .push(StimInstr::new(name, args, targets));
        self.plan.instruction_origins.push(self.origin.clone());
        Ok(())
    }

    fn single(&mut self, name: &str, q: u32) -> Result<(), QasmError> {
        self.push(name, Vec::new(), vec![StimTarget::Qubit(q)])
    }

    fn operation(&mut self, op: &OperationKind) -> Result<(), QasmError> {
        match op {
            OperationKind::Gate { gate, targets } => self.gate(gate, targets),
            OperationKind::Measure { qubit, bit } => {
                self.single("M", *qubit)?;
                let id = self.plan.measurement_events.len();
                if id >= i32::MAX as usize {
                    return Err(limit("measurement record exceeds native lookback range"));
                }
                self.plan.measurement_events.push(id);
                self.plan.classical_outputs[*bit] = ClassicalBinding::Measurement(id);
                Ok(())
            }
            OperationKind::Reset { qubit } => self.single("R", *qubit),
            OperationKind::Barrier { .. } => self.push("TICK", Vec::new(), Vec::new()),
            OperationKind::Conditional {
                bit,
                value,
                gate,
                targets,
            } => {
                let name = match gate {
                    Pauli::X => "X",
                    Pauli::Y => "Y",
                    Pauli::Z => "Z",
                };
                let binding = self.plan.classical_outputs[*bit].clone();
                for &q in targets {
                    match binding {
                        ClassicalBinding::Zero => {
                            if !value {
                                self.single(name, q)?;
                            }
                        }
                        ClassicalBinding::Measurement(id) => {
                            if !value {
                                self.single(name, q)?;
                            }
                            let distance = self.plan.measurement_events.len() - id;
                            let lookback = i32::try_from(distance).map_err(|_| {
                                limit("feedback record lookback exceeds native range")
                            })?;
                            self.push(
                                match gate {
                                    Pauli::X => "CX",
                                    Pauli::Y => "CY",
                                    Pauli::Z => "CZ",
                                },
                                Vec::new(),
                                vec![StimTarget::Rec(-lookback), StimTarget::Qubit(q)],
                            )?;
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn gate(&mut self, gate: &Gate, targets: &[u32]) -> Result<(), QasmError> {
        let q = targets[0];
        let name = match gate {
            Gate::I => "I",
            Gate::H => "H",
            Gate::X => "X",
            Gate::Y => "Y",
            Gate::Z => "Z",
            Gate::S => "S",
            Gate::SDag => "S_DAG",
            Gate::SqrtX => "SQRT_X",
            Gate::SqrtXDag => "SQRT_X_DAG",
            Gate::SqrtY => "SQRT_Y",
            Gate::SqrtYDag => "SQRT_Y_DAG",
            Gate::CX => "CX",
            Gate::CY => "CY",
            Gate::CZ => "CZ",
            Gate::SWAP => "SWAP",
            Gate::T | Gate::TDag => {
                self.needs_near("T/T_DAG")?;
                if matches!(gate, Gate::T) {
                    "T"
                } else {
                    "T_DAG"
                }
            }
            Gate::Rx(a) => return self.rotation("X", a, q),
            Gate::Ry(a) => return self.rotation("Y", a, q),
            Gate::Rz(a) => return self.rotation("Z", a, q),
            Gate::U { theta, phi, lambda } => {
                self.rotation("Z", lambda, q)?;
                self.rotation("Y", theta, q)?;
                return self.rotation("Z", phi, q);
            }
        };
        self.push(
            name,
            Vec::new(),
            targets.iter().map(|q| StimTarget::Qubit(*q)).collect(),
        )
    }

    fn needs_near(&self, gate: &str) -> Result<(), QasmError> {
        if self.plan.target == Backend::Stabilizer {
            Err(QasmError::new(
                ErrorCode::BackendUnsupported,
                format!(
                    "{gate} requires the explicit near-clifford backend; it is not a proven Clifford operation"
                ),
            ))
        } else {
            Ok(())
        }
    }

    fn rotation(&mut self, axis: &str, angle: &Angle, q: u32) -> Result<(), QasmError> {
        if let Some(quarters) = angle.clifford_quarters() {
            let name = match (axis, quarters.rem_euclid(4)) {
                (_, 0) => return Ok(()),
                ("X", 1) => "SQRT_X",
                ("X", 2) => "X",
                ("X", 3) => "SQRT_X_DAG",
                ("Y", 1) => "SQRT_Y",
                ("Y", 2) => "Y",
                ("Y", 3) => "SQRT_Y_DAG",
                ("Z", 1) => "S",
                ("Z", 2) => "Z",
                ("Z", 3) => "S_DAG",
                _ => unreachable!(),
            };
            self.single(name, q)
        } else {
            self.needs_near(&format!("R_{axis}"))?;
            self.push(
                &format!("R_{axis}"),
                vec![angle.execution_half_turns()],
                vec![StimTarget::Qubit(q)],
            )
        }
    }
}

fn limit(message: &str) -> QasmError {
    QasmError::new(ErrorCode::Limit, message)
}

#[cfg(test)]
mod target_budget_tests {
    use super::*;

    #[test]
    fn semantic_target_budget_is_shared_across_all_operation_kinds() {
        let limits = QasmLimits {
            max_operations: 8,
            ..Default::default()
        };
        let mut circuit = QasmCircuit {
            quantum_registers: vec![Register {
                name: "q".into(),
                size: 4,
                offset: 0,
            }],
            classical_registers: vec![Register {
                name: "c".into(),
                size: 1,
                offset: 0,
            }],
            operations: vec![
                Operation {
                    kind: OperationKind::Barrier {
                        qubits: vec![0, 1, 2],
                    },
                    origin: None,
                },
                Operation {
                    kind: OperationKind::Gate {
                        gate: Gate::CX,
                        targets: vec![0, 1],
                    },
                    origin: None,
                },
                Operation {
                    kind: OperationKind::Measure { qubit: 0, bit: 0 },
                    origin: None,
                },
                Operation {
                    kind: OperationKind::Reset { qubit: 2 },
                    origin: None,
                },
                Operation {
                    kind: OperationKind::Conditional {
                        bit: 0,
                        value: true,
                        gate: Pauli::X,
                        targets: vec![1],
                    },
                    origin: None,
                },
            ],
        };
        validate(&circuit, &limits).unwrap();
        circuit.operations.push(Operation {
            kind: OperationKind::Reset { qubit: 3 },
            origin: None,
        });
        let error = validate(&circuit, &limits).unwrap_err();
        assert_eq!(error.code, ErrorCode::Limit);
        assert!(error.message.contains("operand work"));
    }
}

#[cfg(test)]
mod checked_semantic_boundary_tests {
    use super::*;

    fn circuit(kind: OperationKind) -> QasmCircuit {
        QasmCircuit {
            quantum_registers: vec![Register {
                name: "q".into(),
                size: 2,
                offset: 0,
            }],
            classical_registers: vec![Register {
                name: "c".into(),
                size: 1,
                offset: 0,
            }],
            operations: vec![Operation {
                kind,
                origin: Some(SourceSpan {
                    source: "document".into(),
                    line: 1,
                    column: 1,
                    offset: 0,
                    length: 1,
                    expansion: vec![],
                }),
            }],
        }
    }

    #[test]
    fn malformed_semantic_operations_never_reach_a_backend() {
        for (kind, code) in [
            (
                OperationKind::Gate {
                    gate: Gate::CX,
                    targets: vec![0],
                },
                ErrorCode::Type,
            ),
            (
                OperationKind::Gate {
                    gate: Gate::CX,
                    targets: vec![0, 0],
                },
                ErrorCode::Type,
            ),
            (
                OperationKind::Gate {
                    gate: Gate::X,
                    targets: vec![2],
                },
                ErrorCode::Type,
            ),
            (OperationKind::Measure { qubit: 0, bit: 1 }, ErrorCode::Type),
            (OperationKind::Reset { qubit: 2 }, ErrorCode::Type),
            (OperationKind::Barrier { qubits: vec![2] }, ErrorCode::Type),
            (
                OperationKind::Conditional {
                    bit: 1,
                    value: true,
                    gate: Pauli::X,
                    targets: vec![0],
                },
                ErrorCode::Type,
            ),
            (
                OperationKind::Conditional {
                    bit: 0,
                    value: true,
                    gate: Pauli::X,
                    targets: vec![],
                },
                ErrorCode::Type,
            ),
            (
                OperationKind::Gate {
                    gate: Gate::Rx(Angle::Rational {
                        numerator: 1,
                        denominator: 0,
                        pi: true,
                    }),
                    targets: vec![0],
                },
                ErrorCode::Type,
            ),
            (
                OperationKind::Gate {
                    gate: Gate::Ry(Angle::Radians {
                        value: f64::INFINITY,
                    }),
                    targets: vec![0],
                },
                ErrorCode::Type,
            ),
        ] {
            let program = circuit(kind);
            let error = lower(&program, Backend::NearClifford, &QasmLimits::default()).unwrap_err();
            assert_eq!(error.code, code);
            assert_eq!(error.origin.unwrap().source, "document");
        }
        let mut program = circuit(OperationKind::Conditional {
            bit: 0,
            value: true,
            gate: Pauli::X,
            targets: vec![0],
        });
        program.classical_registers[0].size = 2;
        assert_eq!(
            validate(&program, &QasmLimits::default()).unwrap_err().code,
            ErrorCode::Unsupported
        );
    }

    #[test]
    fn semantic_registers_must_have_unique_valid_names_and_contiguous_positive_widths() {
        for registers in [
            vec![Register {
                name: "if".into(),
                size: 1,
                offset: 0,
            }],
            vec![Register {
                name: "Upper".into(),
                size: 1,
                offset: 0,
            }],
            vec![Register {
                name: "q".into(),
                size: 0,
                offset: 0,
            }],
            vec![Register {
                name: "q".into(),
                size: 1,
                offset: 1,
            }],
            vec![
                Register {
                    name: "q".into(),
                    size: 1,
                    offset: 0,
                },
                Register {
                    name: "q".into(),
                    size: 1,
                    offset: 1,
                },
            ],
            vec![
                Register {
                    name: "q".into(),
                    size: 1,
                    offset: 0,
                },
                Register {
                    name: "other".into(),
                    size: 1,
                    offset: 0,
                },
            ],
        ] {
            let program = QasmCircuit {
                quantum_registers: registers,
                classical_registers: vec![],
                operations: vec![],
            };
            assert_eq!(
                validate(&program, &QasmLimits::default()).unwrap_err().code,
                ErrorCode::Type
            );
        }
        let mut program = circuit(OperationKind::Reset { qubit: 0 });
        program.classical_registers[0].name = "q".into();
        assert_eq!(
            validate(&program, &QasmLimits::default()).unwrap_err().code,
            ErrorCode::Type
        );
        let limits = QasmLimits {
            max_qubits: usize::MAX,
            max_classical_bits: usize::MAX,
            ..Default::default()
        };
        for registers in [
            vec![
                Register {
                    name: "q".into(),
                    size: usize::MAX,
                    offset: 0,
                },
                Register {
                    name: "other".into(),
                    size: 1,
                    offset: usize::MAX,
                },
            ],
            vec![Register {
                name: "q".into(),
                size: u32::MAX as usize + 1,
                offset: 0,
            }],
        ] {
            let program = QasmCircuit {
                quantum_registers: registers,
                classical_registers: vec![],
                operations: vec![],
            };
            assert_eq!(
                validate(&program, &limits).unwrap_err().code,
                ErrorCode::Limit
            );
        }
        let mut program = circuit(OperationKind::Reset { qubit: 0 });
        assert_eq!(
            validate(
                &program,
                &QasmLimits {
                    max_qubits: 1,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::Limit
        );
        program.operations.push(Operation {
            kind: OperationKind::Reset { qubit: 1 },
            origin: None,
        });
        assert_eq!(
            validate(
                &program,
                &QasmLimits {
                    max_operations: 1,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::Limit
        );
    }

    #[test]
    fn typed_clifford_variants_match_independent_full_operators() {
        use super::boundary_reference::{Primitive as P, assert_unitary};
        use std::f64::consts::{FRAC_PI_2, PI};
        for (gate, expected) in [
            (Gate::S, vec![P::U(0, 0., 0., FRAC_PI_2)]),
            (Gate::SDag, vec![P::U(0, 0., 0., -FRAC_PI_2)]),
            (Gate::SqrtX, vec![P::U(0, FRAC_PI_2, -FRAC_PI_2, FRAC_PI_2)]),
            (
                Gate::SqrtXDag,
                vec![P::U(0, -FRAC_PI_2, -FRAC_PI_2, FRAC_PI_2)],
            ),
            (Gate::SqrtY, vec![P::U(0, FRAC_PI_2, 0., 0.)]),
            (Gate::SqrtYDag, vec![P::U(0, -FRAC_PI_2, 0., 0.)]),
            (Gate::SWAP, vec![P::CX(0, 1), P::CX(1, 0), P::CX(0, 1)]),
            (Gate::Z, vec![P::U(0, 0., 0., PI)]),
        ] {
            let targets = if matches!(gate, Gate::SWAP) {
                vec![0, 1]
            } else {
                vec![0]
            };
            let program = circuit(OperationKind::Gate { gate, targets });
            let plan = lower(&program, Backend::Stabilizer, &QasmLimits::default()).unwrap();
            // All input columns share one phase comparison. Replacing any of
            // these gates by identity changes this independently specified map.
            assert_unitary(&expected, &plan);
        }
    }
}
