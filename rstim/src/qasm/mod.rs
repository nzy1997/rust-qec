//! Checked OpenQASM 2.0 conversion with ordered measurements and classical outputs.
//!
//! The conversion document retains information that a plain Stim circuit cannot
//! represent, including register declarations and overwritten classical bits.
//! Choose an execution backend explicitly; conversion never silently removes
//! unsupported operations or falls back to a more expensive backend.
//!
//! ```
//! use rstim::qasm::{from_qasm, Backend, QasmLimits};
//! let circuit = from_qasm("OPENQASM 2.0; qreg q[1]; creg c[1]; measure q -> c;")?;
//! let plan = circuit.lower(Backend::Stabilizer, &QasmLimits::default())?;
//! assert_eq!(plan.project_classical(&[true])?, vec![true]);
//! # Ok::<(), rstim::qasm::QasmError>(())
//! ```

mod expr;
mod frontend;
mod lower;

pub use expr::Angle;
pub use lower::{CapabilityReport, ClassicalBinding, LoweredQasmCircuit};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A location in source, with the call/include context that produced it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpan {
    pub source: String,
    pub line: usize,
    pub column: usize,
    pub offset: usize,
    pub length: usize,
    pub expansion: Vec<String>,
}

/// Stable categories distinguish language errors from execution limitations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Parse,
    Type,
    Unsupported,
    BackendUnsupported,
    Unrepresentable,
    MissingLayout,
    Limit,
    Io,
    InvalidDocument,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "qasm_parse_error",
            Self::Type => "qasm_type_error",
            Self::Unsupported => "qasm_unsupported_construct",
            Self::BackendUnsupported => "qasm_backend_unsupported",
            Self::Unrepresentable => "qasm_unrepresentable_export",
            Self::MissingLayout => "qasm_missing_layout",
            Self::Limit => "qasm_limit_exceeded",
            Self::Io => "qasm_io_error",
            Self::InvalidDocument => "qasm_invalid_document",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QasmError {
    pub code: ErrorCode,
    pub message: String,
    pub origin: Option<SourceSpan>,
}

impl QasmError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            origin: None,
        }
    }

    pub fn at(mut self, origin: SourceSpan) -> Self {
        if self.origin.is_none() {
            self.origin = Some(origin);
        }
        self
    }
}

impl std::fmt::Display for QasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = &self.origin {
            write!(f, "{}:{}:{}: ", s.source, s.line, s.column)?;
        }
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}
impl std::error::Error for QasmError {}

/// Budgets checked before allocating or expanding untrusted circuit input.
#[derive(Clone, Debug)]
pub struct QasmLimits {
    pub max_source_bytes: usize,
    pub max_include_files: usize,
    pub max_include_depth: usize,
    pub max_gate_depth: usize,
    pub max_expr_depth: usize,
    pub max_tokens: usize,
    pub max_operations: usize,
    pub max_qubits: usize,
    pub max_classical_bits: usize,
    pub max_output_bytes: usize,
}

impl Default for QasmLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 4 * 1024 * 1024,
            max_include_files: 128,
            max_include_depth: 32,
            max_gate_depth: 64,
            max_expr_depth: 128,
            max_tokens: 1_000_000,
            max_operations: 1_000_000,
            max_qubits: 4096,
            max_classical_bits: 1_000_000,
            max_output_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedSource {
    pub name: String,
    pub text: String,
}

/// Resolves external includes. The bundled standard header takes precedence.
pub trait IncludeResolver {
    fn resolve(&self, requested: &str, from: &str) -> Result<ResolvedSource, QasmError>;
}

/// String imports cannot implicitly read the working directory or network.
pub struct RejectIncludes;
impl IncludeResolver for RejectIncludes {
    fn resolve(&self, requested: &str, _from: &str) -> Result<ResolvedSource, QasmError> {
        Err(QasmError::new(
            ErrorCode::Io,
            format!("include {requested:?} requires an explicit include resolver"),
        ))
    }
}

/// File includes search the including file's directory, then configured paths.
#[derive(Clone, Debug)]
pub struct FileResolver {
    pub include_dirs: Vec<PathBuf>,
    pub max_file_bytes: usize,
}

impl Default for FileResolver {
    fn default() -> Self {
        Self {
            include_dirs: Vec::new(),
            max_file_bytes: QasmLimits::default().max_source_bytes,
        }
    }
}

impl IncludeResolver for FileResolver {
    fn resolve(&self, requested: &str, from: &str) -> Result<ResolvedSource, QasmError> {
        let mut paths = Vec::new();
        if let Some(parent) = Path::new(from).parent() {
            paths.push(parent.join(requested));
        }
        paths.extend(self.include_dirs.iter().map(|p| p.join(requested)));
        for p in paths {
            if p.is_file() {
                let p = p.canonicalize().map_err(|e| {
                    QasmError::new(
                        ErrorCode::Io,
                        format!("resolve include {}: {e}", p.display()),
                    )
                })?;
                let text = read_source(&p, self.max_file_bytes)?;
                return Ok(ResolvedSource {
                    name: p.display().to_string(),
                    text,
                });
            }
        }
        Err(QasmError::new(
            ErrorCode::Io,
            format!("cannot resolve include {requested:?} from {from:?}"),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Stabilizer,
    NearClifford,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Register {
    pub name: String,
    pub size: usize,
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pauli {
    X,
    Y,
    Z,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "parameters", rename_all = "snake_case")]
pub enum Gate {
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
    SWAP,
    Rx(Angle),
    Ry(Angle),
    Rz(Angle),
    U {
        theta: Angle,
        phi: Angle,
        lambda: Angle,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationKind {
    Gate {
        gate: Gate,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub kind: OperationKind,
    pub origin: Option<SourceSpan>,
}

/// A checked conversion document. Its executable operations stay in source order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QasmCircuit {
    pub(super) quantum_registers: Vec<Register>,
    pub(super) classical_registers: Vec<Register>,
    pub(super) operations: Vec<Operation>,
}

impl QasmCircuit {
    pub fn quantum_registers(&self) -> &[Register] {
        &self.quantum_registers
    }
    pub fn classical_registers(&self) -> &[Register] {
        &self.classical_registers
    }
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
    pub fn num_qubits(&self) -> usize {
        self.quantum_registers.iter().map(|r| r.size).sum()
    }
    pub fn num_classical_bits(&self) -> usize {
        self.classical_registers.iter().map(|r| r.size).sum()
    }
    pub fn lower(
        &self,
        target: Backend,
        limits: &QasmLimits,
    ) -> Result<LoweredQasmCircuit, QasmError> {
        lower::lower(self, target, limits)
    }
    pub fn analyze(&self, limits: &QasmLimits) -> Result<CapabilityReport, QasmError> {
        lower::analyze(self, limits)
    }
}

/// Parse and type-check a source string without implicit external file access.
pub fn from_qasm(source: &str) -> Result<QasmCircuit, QasmError> {
    parse_and_check(source, "<input>", &RejectIncludes, &QasmLimits::default())
}

pub fn parse_and_check(
    source: &str,
    name: &str,
    resolver: &dyn IncludeResolver,
    limits: &QasmLimits,
) -> Result<QasmCircuit, QasmError> {
    let circuit = frontend::parse(source, name, resolver, limits)?;
    lower::validate(&circuit, limits)?;
    Ok(circuit)
}

pub fn from_qasm_file(
    path: &Path,
    resolver: &FileResolver,
    limits: &QasmLimits,
) -> Result<QasmCircuit, QasmError> {
    let path = path
        .canonicalize()
        .map_err(|e| QasmError::new(ErrorCode::Io, format!("resolve {}: {e}", path.display())))?;
    let source = read_source(&path, limits.max_source_bytes)?;
    parse_and_check(&source, &path.display().to_string(), resolver, limits)
}

fn read_source(path: &Path, max: usize) -> Result<String, QasmError> {
    use std::io::Read;
    let file = std::fs::File::open(path)
        .map_err(|e| QasmError::new(ErrorCode::Io, format!("open {}: {e}", path.display())))?;
    if file
        .metadata()
        .map_err(|e| QasmError::new(ErrorCode::Io, e.to_string()))?
        .len()
        > max as u64
    {
        return Err(QasmError::new(
            ErrorCode::Limit,
            format!("source {} exceeds {max} byte limit", path.display()),
        ));
    }
    let cap = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    let mut source = String::new();
    file.take(cap)
        .read_to_string(&mut source)
        .map_err(|e| QasmError::new(ErrorCode::Io, format!("read {}: {e}", path.display())))?;
    if source.len() > max {
        return Err(QasmError::new(
            ErrorCode::Limit,
            "source exceeds byte limit",
        ));
    }
    Ok(source)
}
