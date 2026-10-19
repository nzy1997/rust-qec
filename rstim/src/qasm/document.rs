//! Versioned semantic documents. Executable plans and mutable classical bindings
//! are deliberately derived again rather than trusted as stored parallel state.
use super::{
    Angle, ErrorCode, Gate, Operation, OperationKind, Pauli, QasmCircuit, QasmError, QasmLimits,
    Register, SourceSpan,
};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

const SCHEMA: &str = "rstim.qasm2-program.v1";

#[derive(Serialize)]
struct DocumentRef<'a> {
    schema: &'static str,
    quantum_registers: &'a [Register],
    classical_registers: &'a [Register],
    operations: &'a [Operation],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    quantum_registers: Vec<Register>,
    classical_registers: Vec<Register>,
    operations: Vec<WireOperation>,
}

// Use private wire types so widening an execution enum cannot accidentally
// weaken the on-disk contract. Every tagged envelope rejects unknown fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireOperation {
    kind: WireOperationKind,
    origin: Option<SourceSpan>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireOperationKind {
    Gate {
        gate: WireGate,
        targets: Vec<u32>,
    },
    Measure {
        qubit: u32,
        bit: usize,
    },
    Reset {
        qubit: u32,
    },
    Barrier {
        qubits: Vec<u32>,
    },
    Conditional {
        bit: usize,
        value: bool,
        gate: Pauli,
        targets: Vec<u32>,
    },
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    content = "parameters",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum WireGate {
    I,
    H,
    X,
    Y,
    Z,
    S,
    SDag,
    SqrtX,
    SqrtXDag,
    SqrtY,
    SqrtYDag,
    T,
    TDag,
    CX,
    CY,
    CZ,
    #[serde(rename = "s_w_a_p")]
    Swap,
    Rx(Angle),
    Ry(Angle),
    Rz(Angle),
    U {
        theta: Angle,
        phi: Angle,
        lambda: Angle,
    },
}
impl From<WireGate> for Gate {
    fn from(gate: WireGate) -> Self {
        match gate {
            WireGate::I => Self::I,
            WireGate::H => Self::H,
            WireGate::X => Self::X,
            WireGate::Y => Self::Y,
            WireGate::Z => Self::Z,
            WireGate::S => Self::S,
            WireGate::SDag => Self::SDag,
            WireGate::SqrtX => Self::SqrtX,
            WireGate::SqrtXDag => Self::SqrtXDag,
            WireGate::SqrtY => Self::SqrtY,
            WireGate::SqrtYDag => Self::SqrtYDag,
            WireGate::T => Self::T,
            WireGate::TDag => Self::TDag,
            WireGate::CX => Self::CX,
            WireGate::CY => Self::CY,
            WireGate::CZ => Self::CZ,
            WireGate::Swap => Self::SWAP,
            WireGate::Rx(a) => Self::Rx(a),
            WireGate::Ry(a) => Self::Ry(a),
            WireGate::Rz(a) => Self::Rz(a),
            WireGate::U { theta, phi, lambda } => Self::U { theta, phi, lambda },
        }
    }
}
impl From<WireOperation> for Operation {
    fn from(operation: WireOperation) -> Self {
        let kind = match operation.kind {
            WireOperationKind::Gate { gate, targets } => OperationKind::Gate {
                gate: gate.into(),
                targets,
            },
            WireOperationKind::Measure { qubit, bit } => OperationKind::Measure { qubit, bit },
            WireOperationKind::Reset { qubit } => OperationKind::Reset { qubit },
            WireOperationKind::Barrier { qubits } => OperationKind::Barrier { qubits },
            WireOperationKind::Conditional {
                bit,
                value,
                gate,
                targets,
            } => OperationKind::Conditional {
                bit,
                value,
                gate,
                targets,
            },
        };
        Self {
            kind,
            origin: operation.origin,
        }
    }
}

impl QasmCircuit {
    /// Serialize a checked, self-contained semantic document.
    ///
    /// `max_output_bytes` bounds both the document writer and `from_json` input.
    /// Exact angle expressions, declaration order and optional provenance survive;
    /// backend plans and final classical bindings are recomputed after loading.
    pub fn to_json(&self, limits: &QasmLimits) -> Result<String, QasmError> {
        validate_document(self, limits)?;
        let wire = DocumentRef {
            schema: SCHEMA,
            quantum_registers: &self.quantum_registers,
            classical_registers: &self.classical_registers,
            operations: &self.operations,
        };
        let mut writer = BoundedWriter {
            bytes: Vec::new(),
            max: limits.max_output_bytes,
            exceeded: false,
        };
        if let Err(error) = serde_json::to_writer_pretty(&mut writer, &wire) {
            return Err(if writer.exceeded {
                limit("serialized document exceeds byte limit")
            } else {
                invalid(format!("serialize semantic document: {error}"))
            });
        }
        // serde_json emits UTF-8, but keep this public API fallible even if its
        // serializer implementation changes in the future.
        String::from_utf8(writer.bytes)
            .map_err(|_| invalid("document serializer emitted invalid UTF-8"))
    }

    /// Load the versioned semantic format and recheck every executable invariant.
    /// Malformed documents use `InvalidDocument`; configured budget failures use
    /// `Limit`. Loading a valid document does not select an execution backend.
    pub fn from_json(text: &str, limits: &QasmLimits) -> Result<Self, QasmError> {
        if text.len() > limits.max_output_bytes {
            return Err(limit("serialized document exceeds byte limit"));
        }
        // serde_json's default recursion limit remains enabled. There is no
        // recursive semantic data in this schema that requires disabling it.
        let document: Document = serde_json::from_str(text)
            .map_err(|error| invalid(format!("invalid semantic document: {error}")))?;
        if document.schema != SCHEMA {
            return Err(invalid(format!(
                "unsupported semantic document schema {:?}",
                document.schema
            )));
        }
        let circuit = Self {
            quantum_registers: document.quantum_registers,
            classical_registers: document.classical_registers,
            operations: document
                .operations
                .into_iter()
                .map(Operation::from)
                .collect(),
        };
        validate_document(&circuit, limits)?;
        Ok(circuit)
    }
}

fn validate_document(circuit: &QasmCircuit, limits: &QasmLimits) -> Result<(), QasmError> {
    super::lower::validate(circuit, limits).map_err(|mut error| {
        if error.code != ErrorCode::Limit {
            error.code = ErrorCode::InvalidDocument;
        }
        error
    })?;
    let mut provenance_bytes = 0usize;
    for operation in &circuit.operations {
        if let OperationKind::Barrier { qubits } = &operation.kind {
            let mut seen = std::collections::HashSet::new();
            if qubits.iter().any(|qubit| !seen.insert(*qubit)) {
                return Err(invalid("barrier operands alias"));
            }
        }
        if let Some(origin) = &operation.origin {
            validate_origin(origin, limits, &mut provenance_bytes)?;
        }
    }
    Ok(())
}
fn validate_origin(
    origin: &SourceSpan,
    limits: &QasmLimits,
    total: &mut usize,
) -> Result<(), QasmError> {
    if origin.source.is_empty()
        || origin.line == 0
        || origin.column == 0
        || origin.offset.checked_add(origin.length).is_none()
    {
        return Err(invalid(
            "invalid source provenance: expected a source name, positive line/column and nonoverflowing byte range",
        ));
    }
    if origin.expansion.len()
        > limits
            .max_include_depth
            .saturating_add(limits.max_gate_depth)
    {
        return Err(limit("source provenance expansion depth exceeds limit"));
    }
    for text in std::iter::once(&origin.source).chain(&origin.expansion) {
        if text.is_empty() {
            return Err(invalid(
                "source provenance expansion entries cannot be empty",
            ));
        }
        *total = total
            .checked_add(text.len())
            .ok_or_else(|| limit("source provenance byte count overflow"))?;
        if *total > limits.max_output_bytes {
            return Err(limit("source provenance exceeds document byte limit"));
        }
    }
    Ok(())
}

struct BoundedWriter {
    bytes: Vec<u8>,
    max: usize,
    exceeded: bool,
}
impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let end = self.bytes.len().checked_add(buffer.len());
        let Some(end) = end.filter(|end| *end <= self.max) else {
            self.exceeded = true;
            return Err(io::Error::other("document byte limit exceeded"));
        };
        // Geometric capacity growth is capped at the configured document size.
        // Never first build an unbounded string and check its size afterward.
        if end > self.bytes.capacity() {
            let capacity = self
                .bytes
                .capacity()
                .saturating_mul(2)
                .max(256)
                .max(end)
                .min(self.max);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(io::Error::other)?;
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn invalid(message: impl Into<String>) -> QasmError {
    QasmError::new(ErrorCode::InvalidDocument, message)
}
fn limit(message: impl Into<String>) -> QasmError {
    QasmError::new(ErrorCode::Limit, message)
}
