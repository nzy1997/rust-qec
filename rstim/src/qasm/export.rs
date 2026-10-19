//! Checked semantic export and bounded lifting of native Stim circuits.
use super::*;
use crate::ir::{StimInstr, StimTarget};

#[derive(Clone, Debug, Default, Serialize)]
pub struct ExportOptions {
    /// Explicitly discard tags, coordinates, and QEC annotations, with a report.
    pub omit_annotations: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OmittedAnnotation {
    pub name: String,
    /// Alternating instruction and repeat-iteration indices from the outer circuit.
    pub instruction_path: Vec<usize>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct QasmExport {
    pub text: String,
    pub classical_registers: Vec<Register>,
    pub omitted_annotations: Vec<OmittedAnnotation>,
}

#[derive(Clone, Debug)]
pub struct LiftedQasmCircuit {
    pub circuit: QasmCircuit,
    pub omitted_annotations: Vec<OmittedAnnotation>,
}

pub fn to_qasm(
    circuit: &QasmCircuit,
    _options: &ExportOptions,
    limits: &QasmLimits,
) -> Result<QasmExport, QasmError> {
    super::lower::validate(circuit, limits)?;
    let mut writer = Writer {
        text: String::new(),
        limit: limits.max_output_bytes,
    };
    writer.push("OPENQASM 2.0;\n")?;
    for (kind, regs) in [
        ("qreg", &circuit.quantum_registers),
        ("creg", &circuit.classical_registers),
    ] {
        for reg in regs {
            writer.push(&format!("{kind} {}[{}];\n", reg.name, reg.size))?;
        }
    }
    let mut omitted_annotations = Vec::new();
    let mut report_bytes = 0;
    for (index, op) in circuit.operations.iter().enumerate() {
        if let OperationKind::Gate { gate, .. } = &op.kind {
            let angles: &[&Angle] = match gate {
                Gate::Rx(angle) | Gate::Ry(angle) | Gate::Rz(angle) => &[angle],
                Gate::U { theta, phi, lambda } => &[theta, phi, lambda],
                _ => &[],
            };
            for angle in angles {
                if matches!(angle, Angle::Rational {denominator, ..} if *denominator > i64::MAX as u64)
                {
                    let mut error = QasmError::new(
                        ErrorCode::Unrepresentable,
                        "exact angle denominator exceeds the supported signed 64-bit expression integer range; choose a representable exact parameter",
                    );
                    error.origin = op.origin.clone();
                    return Err(error);
                }
            }
        }
        let result = match &op.kind {
            OperationKind::Gate { gate, targets } => {
                let operands = targets
                    .iter()
                    .map(|q| operand(&circuit.quantum_registers, *q as usize))
                    .collect::<Vec<_>>();
                writer.gate(gate, &operands, "")
            }
            OperationKind::Measure { qubit, bit } => writer.push(&format!(
                "measure {} -> {};\n",
                operand(&circuit.quantum_registers, *qubit as usize),
                operand(&circuit.classical_registers, *bit)
            )),
            OperationKind::Reset { qubit } => writer.push(&format!(
                "reset {};\n",
                operand(&circuit.quantum_registers, *qubit as usize)
            )),
            OperationKind::Barrier { qubits } => {
                if qubits.is_empty() {
                    push_omission(
                        &mut omitted_annotations,
                        &mut report_bytes,
                        OmittedAnnotation {
                            name: "EMPTY_BARRIER".into(),
                            instruction_path: vec![index],
                            source: "barrier;".into(),
                        },
                        limits.max_output_bytes,
                    )
                } else {
                    writer.push(&format!(
                        "barrier {};\n",
                        qubits
                            .iter()
                            .map(|q| operand(&circuit.quantum_registers, *q as usize))
                            .collect::<Vec<_>>()
                            .join(",")
                    ))
                }
            }
            OperationKind::Conditional {
                bit,
                value,
                gate,
                targets,
            } => {
                let reg = circuit
                    .classical_registers
                    .binary_search_by_key(bit, |r| r.offset)
                    .map(|index| &circuit.classical_registers[index])
                    .expect("validated singleton condition");
                let gate = match gate {
                    Pauli::X => Gate::X,
                    Pauli::Y => Gate::Y,
                    Pauli::Z => Gate::Z,
                };
                targets.iter().try_for_each(|target| {
                    writer.gate(
                        &gate,
                        &[operand(&circuit.quantum_registers, *target as usize)],
                        &format!("if({}=={}) ", reg.name, u8::from(*value)),
                    )
                })
            }
        };
        result.map_err(|mut e| {
            e.origin = op.origin.clone();
            e
        })?;
    }
    Ok(QasmExport {
        text: writer.text,
        classical_registers: circuit.classical_registers.clone(),
        omitted_annotations,
    })
}

pub fn export_stim(
    instructions: &[StimInstr],
    options: &ExportOptions,
    limits: &QasmLimits,
) -> Result<QasmExport, QasmError> {
    let lifted = lift(instructions, options, limits)?;
    let mut result = to_qasm(&lifted.circuit, options, limits)?;
    result
        .omitted_annotations
        .extend(lifted.omitted_annotations);
    Ok(result)
}

fn operand(registers: &[Register], index: usize) -> String {
    // Validation guarantees positive widths and contiguous, increasing offsets.
    let register_index = registers.partition_point(|r| r.offset <= index) - 1;
    let reg = &registers[register_index];
    format!("{}[{}]", reg.name, index - reg.offset)
}

struct Writer {
    text: String,
    limit: usize,
}
impl Writer {
    fn gate(&mut self, gate: &Gate, operands: &[String], prefix: &str) -> Result<(), QasmError> {
        match gate {
            Gate::CX => {
                return self.push(&format!("{prefix}CX {},{};\n", operands[0], operands[1]));
            }
            Gate::CY => {
                self.gate(&Gate::SDag, &operands[1..], prefix)?;
                self.gate(&Gate::CX, operands, prefix)?;
                return self.gate(&Gate::S, &operands[1..], prefix);
            }
            Gate::CZ => {
                self.gate(&Gate::H, &operands[1..], prefix)?;
                self.gate(&Gate::CX, operands, prefix)?;
                return self.gate(&Gate::H, &operands[1..], prefix);
            }
            Gate::SWAP => {
                self.gate(&Gate::CX, operands, prefix)?;
                self.gate(
                    &Gate::CX,
                    &[operands[1].clone(), operands[0].clone()],
                    prefix,
                )?;
                return self.gate(&Gate::CX, operands, prefix);
            }
            _ => {}
        }
        let parameters = match gate {
            Gate::I => "0,0,0".into(),
            Gate::H => "pi/2,0,pi".into(),
            Gate::X => "pi,0,pi".into(),
            Gate::Y => "pi,pi/2,pi/2".into(),
            Gate::Z => "0,0,pi".into(),
            Gate::S => "0,0,pi/2".into(),
            Gate::SDag => "0,0,-pi/2".into(),
            Gate::T => "0,0,pi/4".into(),
            Gate::TDag => "0,0,-pi/4".into(),
            Gate::SqrtX => "pi/2,-pi/2,pi/2".into(),
            Gate::SqrtXDag => "-pi/2,-pi/2,pi/2".into(),
            Gate::SqrtY => "pi/2,0,0".into(),
            Gate::SqrtYDag => "-pi/2,0,0".into(),
            Gate::Rx(a) => format!("{},-pi/2,pi/2", a.expression()),
            Gate::Ry(a) => format!("{},0,0", a.expression()),
            Gate::Rz(a) => format!("0,0,{}", a.expression()),
            Gate::U { theta, phi, lambda } => format!(
                "{},{},{}",
                theta.expression(),
                phi.expression(),
                lambda.expression()
            ),
            _ => unreachable!("two-qubit gates emitted above"),
        };
        self.push(&format!("{prefix}U({parameters}) {};\n", operands[0]))
    }

    fn push(&mut self, value: &str) -> Result<(), QasmError> {
        if value.len() > self.limit.saturating_sub(self.text.len()) {
            return Err(limit("QASM output byte limit exceeded"));
        }
        self.text.push_str(value);
        Ok(())
    }
}
fn limit(message: &str) -> QasmError {
    QasmError::new(ErrorCode::Limit, message)
}
fn invalid(message: impl Into<String>) -> QasmError {
    QasmError::new(ErrorCode::Type, message)
}
fn unsupported(message: impl Into<String>) -> QasmError {
    QasmError::new(ErrorCode::Unrepresentable, message)
}

fn push_omission(
    report: &mut Vec<OmittedAnnotation>,
    used_bytes: &mut usize,
    annotation: OmittedAnnotation,
    max_bytes: usize,
) -> Result<(), QasmError> {
    let bytes = annotation
        .name
        .len()
        .checked_add(annotation.source.len())
        .and_then(|n| {
            annotation
                .instruction_path
                .len()
                .checked_mul(std::mem::size_of::<usize>())
                .and_then(|path_bytes| n.checked_add(path_bytes))
        })
        .ok_or_else(|| limit("annotation report size overflow"))?;
    let total = used_bytes
        .checked_add(bytes)
        .ok_or_else(|| limit("annotation report size overflow"))?;
    if total > max_bytes {
        return Err(limit("annotation report byte limit exceeded"));
    }
    report.push(annotation);
    *used_bytes = total;
    Ok(())
}

/// Lift into ordered semantic operations without compiling or simulating.
pub fn lift(
    instructions: &[StimInstr],
    options: &ExportOptions,
    limits: &QasmLimits,
) -> Result<LiftedQasmCircuit, QasmError> {
    // Count repeat iterations, including empty bodies, before expansion. Recursive
    // descent is bounded independently from the expanded operation budget.
    let (_, width) = inspect(instructions, limits, 0)?;
    let mut builder = Lifter {
        circuit: QasmCircuit {
            quantum_registers: if width == 0 {
                Vec::new()
            } else {
                vec![Register {
                    name: "q".into(),
                    size: width,
                    offset: 0,
                }]
            },
            classical_registers: Vec::new(),
            operations: Vec::new(),
        },
        omitted: Vec::new(),
        options,
        limits,
        report_bytes: 0,
        target_work: 0,
    };
    builder.expand(instructions, &mut Vec::new())?;
    super::lower::validate(&builder.circuit, limits)?;
    Ok(LiftedQasmCircuit {
        circuit: builder.circuit,
        omitted_annotations: builder.omitted,
    })
}

fn inspect(
    instructions: &[StimInstr],
    limits: &QasmLimits,
    depth: usize,
) -> Result<(usize, usize), QasmError> {
    if depth > limits.max_gate_depth {
        return Err(limit("native repeat nesting limit exceeded"));
    }
    let (mut cost, mut width) = (0usize, 0usize);
    for instruction in instructions {
        let added = match instruction {
            StimInstr::Repeat { count, body } => {
                if *count == 0 {
                    return Err(invalid("REPEAT count must be positive"));
                }
                let (body_cost, body_width) = inspect(body, limits, depth + 1)?;
                width = width.max(body_width);
                let repetitions = usize::try_from(*count)
                    .map_err(|_| limit("repeat expansion exceeds platform range"))?;
                body_cost
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(repetitions))
                    .ok_or_else(|| limit("repeat expansion count overflow"))?
            }
            StimInstr::Op {
                name,
                tag,
                args,
                targets,
            } => {
                if name.len() > limits.max_source_bytes
                    || tag
                        .as_ref()
                        .is_some_and(|t| t.len() > limits.max_source_bytes)
                    || args.len() > limits.max_operations
                    || targets.len() > limits.max_operations
                {
                    return Err(limit("native instruction size exceeds limit"));
                }
                for t in targets {
                    if let Some(q) = t.qubit_index() {
                        let next = (q as usize)
                            .checked_add(1)
                            .ok_or_else(|| limit("native qubit index overflow"))?;
                        if next > limits.max_qubits {
                            return Err(limit("native qubit width exceeds limit"));
                        }
                        width = width.max(next);
                    }
                }
                targets.len().max(1)
            }
        };
        cost = cost
            .checked_add(added)
            .ok_or_else(|| limit("native instruction count overflow"))?;
        if cost > limits.max_operations {
            return Err(limit("native repeat expansion exceeds operation limit"));
        }
    }
    Ok((cost, width))
}

struct Lifter<'a> {
    circuit: QasmCircuit,
    omitted: Vec<OmittedAnnotation>,
    options: &'a ExportOptions,
    limits: &'a QasmLimits,
    report_bytes: usize,
    target_work: usize,
}
impl Lifter<'_> {
    fn checked_target_work(&self, additional: usize) -> Result<usize, QasmError> {
        self.target_work
            .checked_add(additional)
            .filter(|work| *work <= self.limits.max_operations)
            .ok_or_else(|| limit("lifted target work exceeds operation limit"))
    }
    fn push(&mut self, kind: OperationKind) -> Result<(), QasmError> {
        if self.circuit.operations.len() >= self.limits.max_operations {
            return Err(limit("lifted operation count exceeds limit"));
        }
        let targets = super::lower::target_work(&kind);
        self.target_work = self.checked_target_work(targets)?;
        self.circuit
            .operations
            .push(Operation { kind, origin: None });
        Ok(())
    }
    fn gate(&mut self, gate: Gate, targets: Vec<u32>) -> Result<(), QasmError> {
        self.push(OperationKind::Gate { gate, targets })
    }
    fn single(&mut self, gate: Gate, q: u32) -> Result<(), QasmError> {
        self.gate(gate, vec![q])
    }
    fn omit(
        &mut self,
        name: &str,
        path: &[usize],
        source: String,
        explicit: bool,
    ) -> Result<(), QasmError> {
        if explicit && !self.options.omit_annotations {
            return Err(unsupported(format!(
                "{name} requires explicit annotation omission"
            )));
        }
        push_omission(
            &mut self.omitted,
            &mut self.report_bytes,
            OmittedAnnotation {
                name: name.into(),
                instruction_path: path.into(),
                source,
            },
            self.limits.max_output_bytes,
        )
    }

    fn expand(
        &mut self,
        instructions: &[StimInstr],
        path: &mut Vec<usize>,
    ) -> Result<(), QasmError> {
        for (index, instruction) in instructions.iter().enumerate() {
            path.push(index);
            let result = match instruction {
                StimInstr::Repeat { count, body } => {
                    for iteration in 0..*count {
                        path.push(iteration as usize);
                        self.expand(body, path)?;
                        path.pop();
                    }
                    Ok(())
                }
                StimInstr::Op {
                    name,
                    tag,
                    args,
                    targets,
                } => {
                    self.operation(name, args, targets, path)?;
                    if let Some(tag) = tag {
                        self.omit("TAG", path, format!("{name}[{tag}]"), true)?;
                    }
                    Ok(())
                }
            };
            path.pop();
            result?;
        }
        Ok(())
    }
    fn record(&self, r: i32) -> Result<usize, QasmError> {
        if r >= 0 {
            return Err(invalid("record lookback must be negative"));
        }
        self.circuit
            .classical_registers
            .len()
            .checked_sub(r.unsigned_abs() as usize)
            .ok_or_else(|| invalid("record lookback precedes available measurements"))
    }
    fn operation(
        &mut self,
        name: &str,
        args: &[f64],
        targets: &[StimTarget],
        path: &[usize],
    ) -> Result<(), QasmError> {
        if args.iter().any(|a| !a.is_finite()) {
            return Err(invalid(format!("{name} has a non-finite argument")));
        }
        match name {
            "DETECTOR" | "OBSERVABLE_INCLUDE" | "QUBIT_COORDS" | "SHIFT_COORDS" => {
                match name {
                    "DETECTOR" | "OBSERVABLE_INCLUDE" => {
                        if name == "OBSERVABLE_INCLUDE"
                            && (args.len() != 1
                                || args[0] < 0.0
                                || args[0].fract() != 0.0
                                || args[0] > u32::MAX as f64)
                        {
                            return Err(invalid(
                                "OBSERVABLE_INCLUDE requires one nonnegative integer index",
                            ));
                        }
                        for t in targets {
                            if let StimTarget::Rec(r) = t {
                                self.record(*r)?;
                            } else {
                                return Err(invalid(format!("{name} requires record targets")));
                            }
                        }
                    }
                    "QUBIT_COORDS" => {
                        for t in targets {
                            plain_qubit(t)?;
                        }
                    }
                    "SHIFT_COORDS" => {
                        if !targets.is_empty() {
                            return Err(invalid("SHIFT_COORDS has no targets"));
                        }
                    }
                    _ => unreachable!(),
                }
                let source = crate::ir::circuit_to_string(&[StimInstr::new(
                    name,
                    args.to_vec(),
                    targets.to_vec(),
                )]);
                self.omit(name, path, source, true)
            }
            "TICK" => {
                no_args(name, args)?;
                if !targets.is_empty() {
                    return Err(invalid("TICK has no targets"));
                }
                let width = self.circuit.num_qubits();
                if width == 0 {
                    self.omit("TICK", path, "TICK\n".into(), false)
                } else {
                    // A compressed repeat of global ticks must not allocate
                    // width * repetitions targets before the writer budget runs.
                    self.checked_target_work(width)?;
                    self.push(OperationKind::Barrier {
                        qubits: (0..width as u32).collect(),
                    })
                }
            }
            "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" => {
                if !args.is_empty() {
                    return Err(unsupported(
                        "measurement noise arguments are outside the QASM export profile, including zero",
                    ));
                }
                let basis = if name.ends_with('X') {
                    Pauli::X
                } else if name.ends_with('Y') {
                    Pauli::Y
                } else {
                    Pauli::Z
                };
                for target in targets {
                    let (qubit, inverted) = match target {
                        StimTarget::Qubit(q) => (*q, false),
                        StimTarget::QubitInv(q) => (*q, true),
                        _ => {
                            return Err(invalid(
                                "measurement requires qubit or inverted-qubit targets",
                            ));
                        }
                    };
                    self.measure(qubit, basis, inverted)?;
                    if name.starts_with("MR") {
                        self.reset(qubit, basis)?;
                    }
                }
                Ok(())
            }
            "R" | "RZ" | "RX" | "RY" => {
                no_args(name, args)?;
                let basis = match name {
                    "RX" => Pauli::X,
                    "RY" => Pauli::Y,
                    _ => Pauli::Z,
                };
                for target in targets {
                    self.reset(plain_qubit(target)?, basis)?;
                }
                Ok(())
            }
            "CX" | "CNOT" | "ZCX" | "CY" | "ZCY" | "CZ" | "ZCZ" | "SWAP" => {
                no_args(name, args)?;
                if !targets.len().is_multiple_of(2) {
                    return Err(invalid(format!("{name} requires paired targets")));
                }
                for pair in targets.chunks_exact(2) {
                    match (&pair[0], &pair[1]) {
                        (StimTarget::Rec(r), StimTarget::Qubit(q)) if name != "SWAP" => {
                            self.feedback(name, *r, *q)?
                        }
                        (StimTarget::Qubit(q), StimTarget::Rec(r))
                            if matches!(name, "CZ" | "ZCZ") =>
                        {
                            self.feedback(name, *r, *q)?
                        }
                        (StimTarget::Qubit(a), StimTarget::Qubit(b)) => {
                            if a == b {
                                return Err(invalid(format!("{name} quantum operands alias")));
                            }
                            let gate = match name {
                                "CY" | "ZCY" => Gate::CY,
                                "CZ" | "ZCZ" => Gate::CZ,
                                "SWAP" => Gate::SWAP,
                                _ => Gate::CX,
                            };
                            self.gate(gate, vec![*a, *b])?;
                        }
                        _ => {
                            return Err(unsupported(format!(
                                "{name} target combination is outside the QASM export profile"
                            )));
                        }
                    }
                }
                Ok(())
            }
            "R_X" | "R_Y" | "R_Z" => {
                if args.len() != 1 {
                    return Err(invalid(format!("{name} requires one half-turn angle")));
                }
                if targets.is_empty() {
                    return Err(invalid(format!("{name} requires a quantum target")));
                }
                let angle = half_turn_angle(args[0])?;
                let gate = match name {
                    "R_X" => Gate::Rx(angle),
                    "R_Y" => Gate::Ry(angle),
                    _ => Gate::Rz(angle),
                };
                for target in targets {
                    self.single(gate.clone(), plain_qubit(target)?)?;
                }
                Ok(())
            }
            _ => {
                let gate = match name {
                    "I" => Gate::I,
                    "H" => Gate::H,
                    "X" => Gate::X,
                    "Y" => Gate::Y,
                    "Z" => Gate::Z,
                    "S" | "SQRT_Z" => Gate::S,
                    "S_DAG" | "SQRT_Z_DAG" => Gate::SDag,
                    "SQRT_X" => Gate::SqrtX,
                    "SQRT_X_DAG" => Gate::SqrtXDag,
                    "SQRT_Y" => Gate::SqrtY,
                    "SQRT_Y_DAG" => Gate::SqrtYDag,
                    "T" => Gate::T,
                    "T_DAG" => Gate::TDag,
                    _ => {
                        return Err(unsupported(format!(
                            "{name} is outside the QASM export profile"
                        )));
                    }
                };
                no_args(name, args)?;
                if matches!(gate, Gate::T | Gate::TDag) && targets.is_empty() {
                    return Err(invalid(format!("{name} requires a quantum target")));
                }
                for target in targets {
                    self.single(gate.clone(), plain_qubit(target)?)?;
                }
                Ok(())
            }
        }
    }
    fn feedback(&mut self, name: &str, r: i32, q: u32) -> Result<(), QasmError> {
        let bit = self.record(r)?;
        let gate = match name {
            "CY" | "ZCY" => Pauli::Y,
            "CZ" | "ZCZ" => Pauli::Z,
            _ => Pauli::X,
        };
        self.push(OperationKind::Conditional {
            bit,
            value: true,
            gate,
            targets: vec![q],
        })
    }
    fn measure(&mut self, q: u32, basis: Pauli, inverted: bool) -> Result<(), QasmError> {
        let bit = self.circuit.classical_registers.len();
        if bit >= self.limits.max_classical_bits {
            return Err(limit(
                "native measurement record exceeds classical bit limit",
            ));
        }
        self.circuit.classical_registers.push(Register {
            name: format!("m{bit}"),
            size: 1,
            offset: bit,
        });
        if basis == Pauli::Y {
            self.single(Gate::SDag, q)?;
        }
        if basis != Pauli::Z {
            self.single(Gate::H, q)?;
        }
        if inverted {
            self.single(Gate::X, q)?;
        }
        self.push(OperationKind::Measure { qubit: q, bit })?;
        if inverted {
            self.single(Gate::X, q)?;
        }
        if basis != Pauli::Z {
            self.single(Gate::H, q)?;
        }
        if basis == Pauli::Y {
            self.single(Gate::S, q)?;
        }
        Ok(())
    }
    fn reset(&mut self, q: u32, basis: Pauli) -> Result<(), QasmError> {
        self.push(OperationKind::Reset { qubit: q })?;
        if basis != Pauli::Z {
            self.single(Gate::H, q)?;
        }
        if basis == Pauli::Y {
            self.single(Gate::S, q)?;
        }
        Ok(())
    }
}
fn plain_qubit(target: &StimTarget) -> Result<u32, QasmError> {
    if let StimTarget::Qubit(q) = target {
        Ok(*q)
    } else {
        Err(invalid("gate requires plain quantum targets"))
    }
}
fn no_args(name: &str, args: &[f64]) -> Result<(), QasmError> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(invalid(format!("{name} does not accept arguments")))
    }
}

fn half_turn_angle(value: f64) -> Result<Angle, QasmError> {
    if !value.is_finite() {
        return Err(invalid("rotation half-turn angle must be finite"));
    }
    // The native parameter is already FP64 in half-turn units. Reduce its
    // exactly representable binary remainder before multiplying by pi; for
    // example, 1e20 half-turns is identity, not the rounded radian angle 1e20*pi.
    let value = value % 4.0;
    if value == 0.0 {
        return Angle::pi_ratio(0, 1);
    }
    // An f64 is an exact binary rational. Preserve that rational symbolically
    // whenever the bounded Angle representation can hold its numerator/denominator.
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mut mantissa = bits & ((1u64 << 52) - 1);
    let mut power = if exponent == 0 {
        -1074
    } else {
        mantissa |= 1u64 << 52;
        exponent - 1023 - 52
    };
    while mantissa.is_multiple_of(2) {
        mantissa >>= 1;
        power += 1;
    }
    let sign = if value.is_sign_negative() { -1i128 } else { 1 };
    let rational = if (0..=63).contains(&power) {
        mantissa
            .checked_shl(power as u32)
            .filter(|n| (*n >> power as u32) == mantissa)
            .map(|n| (sign * n as i128, 1u64))
    } else if (-63..0).contains(&power) {
        Some((sign * mantissa as i128, 1u64 << (-power as u32)))
    } else {
        None
    };
    if let Some((n, denominator)) = rational
        && let Ok(numerator) = i64::try_from(n)
    {
        return Angle::pi_ratio(numerator, denominator);
    }
    Angle::radians(value * std::f64::consts::PI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_semantic_fences_report_omission_with_a_bounded_payload_and_origin() {
        let origin = SourceSpan {
            source: "semantic-document".into(),
            line: 7,
            column: 1,
            offset: 12,
            length: 8,
            expansion: Vec::new(),
        };
        let mut circuit = from_qasm("OPENQASM 2.0;").unwrap();
        circuit.operations.push(Operation {
            kind: OperationKind::Barrier { qubits: Vec::new() },
            origin: Some(origin.clone()),
        });
        let output = to_qasm(&circuit, &ExportOptions::default(), &QasmLimits::default()).unwrap();
        assert_eq!(output.text, "OPENQASM 2.0;\n");
        assert_eq!(
            output.omitted_annotations,
            vec![OmittedAnnotation {
                name: "EMPTY_BARRIER".into(),
                instruction_path: vec![0],
                source: "barrier;".into(),
            }]
        );
        let limits = QasmLimits {
            // The QASM header fits exactly; only the structured omission report
            // exceeds this limit. Documents cannot bypass native report bounds.
            max_output_bytes: output.text.len(),
            ..QasmLimits::default()
        };
        let error = to_qasm(&circuit, &ExportOptions::default(), &limits).unwrap_err();
        assert_eq!(error.code, ErrorCode::Limit);
        assert!(error.message.contains("annotation report byte limit"));
        assert_eq!(error.origin, Some(origin.clone()));

        let bounded = QasmLimits {
            max_output_bytes: 32,
            ..QasmLimits::default()
        };
        to_qasm(&circuit, &ExportOptions::default(), &bounded).unwrap();
        let second_origin = SourceSpan { line: 8, ..origin };
        circuit.operations.push(Operation {
            kind: OperationKind::Barrier { qubits: Vec::new() },
            origin: Some(second_origin.clone()),
        });
        let error = to_qasm(&circuit, &ExportOptions::default(), &bounded).unwrap_err();
        assert_eq!(error.code, ErrorCode::Limit);
        assert_eq!(error.origin, Some(second_origin));
    }
}
