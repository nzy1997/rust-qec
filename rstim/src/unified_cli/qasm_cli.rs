//! Explicit QASM conversion and sampling profiles for the unified CLI.
use super::{CommandError, ErrorFormat, pipeline::PipelineFormat};
use crate::ir::StimInstr;
use crate::near_clifford::CompiledNearCliffordExecutor;
use crate::output::OutputFormat;
use crate::qasm::{
    self, Backend, IncludeResolver, QasmCircuit, QasmError, QasmLimits, ResolvedSource,
};
use crate::sim::bit_table::BitTable;
use clap::{Args, ValueEnum};
use serde_json::{Value, json};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const IMPORT_COMMAND: &str = "circuit.import-qasm";
pub const EXPORT_COMMAND: &str = "circuit.export-qasm";
const SAMPLE_COMMAND: &str = "circuit.sample";

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CliBackend {
    Stabilizer,
    NearClifford,
}
impl CliBackend {
    fn target(self) -> Backend {
        match self {
            Self::Stabilizer => Backend::Stabilizer,
            Self::NearClifford => Backend::NearClifford,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Stabilizer => "stabilizer",
            Self::NearClifford => "near-clifford",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ImportFormat {
    Qasm2Json,
    Stim,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum InputFormat {
    Stim,
    Qasm2Json,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ResultView {
    MeasurementRecord,
    Classical,
}
impl ResultView {
    fn name(self) -> &'static str {
        match self {
            Self::MeasurementRecord => "measurement-record",
            Self::Classical => "classical",
        }
    }
}

#[derive(Debug, Args)]
pub struct ImportOptions {
    #[arg(long = "in")]
    pub input: Option<PathBuf>,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, value_enum, default_value = "qasm2-json")]
    pub out_format: ImportFormat,
    #[arg(long, value_enum, default_value = "stabilizer")]
    pub backend: CliBackend,
    #[arg(long = "include-dir")]
    pub include_dirs: Vec<PathBuf>,
    #[arg(long,value_enum,default_value_t=PipelineFormat::Json)]
    pub format: PipelineFormat,
}
#[derive(Debug, Args)]
pub struct ExportOptionsCLI {
    #[arg(long = "in")]
    pub input: Option<PathBuf>,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, value_enum, default_value = "stim")]
    pub in_format: InputFormat,
    #[arg(long)]
    pub omit_annotations: bool,
    #[arg(long,value_enum,default_value_t=PipelineFormat::Json)]
    pub format: PipelineFormat,
}
#[derive(Debug)]
pub struct SampleOptions {
    pub shots: u64,
    pub input: Option<PathBuf>,
    pub out: PathBuf,
    pub out_format: String,
    pub seed: Option<u64>,
    pub skip_reference_sample: bool,
    pub format: PipelineFormat,
    pub in_format: InputFormat,
    pub backend: CliBackend,
    pub result_view: ResultView,
    pub max_active_rank: usize,
}
pub struct Report {
    pub result: Value,
    pub warnings: Vec<String>,
}

fn error(
    command: &'static str,
    code: &'static str,
    message: impl Into<String>,
    json: bool,
) -> CommandError {
    CommandError {
        command,
        code,
        message: message.into(),
        json,
        exit_code: 2,
    }
}
fn qasm_error(command: &'static str, cause: QasmError, json: bool) -> CommandError {
    error(command, cause.code.as_str(), cause.to_string(), json)
}
fn read_text(
    command: &'static str,
    path: Option<&Path>,
    stdin: &mut dyn Read,
    max: usize,
    json: bool,
) -> Result<String, CommandError> {
    let mut bytes = Vec::new();
    let count = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    let result = match path.filter(|path| *path != Path::new("-")) {
        Some(path) => File::open(path).and_then(|file| file.take(count).read_to_end(&mut bytes)),
        None => stdin.take(count).read_to_end(&mut bytes),
    };
    result.map_err(|cause| {
        error(
            command,
            "input_error",
            format!("read circuit input: {cause}"),
            json,
        )
    })?;
    if bytes.len() > max {
        return Err(error(
            command,
            "qasm_limit_exceeded",
            "circuit input exceeds byte limit",
            json,
        ));
    }
    String::from_utf8(bytes).map_err(|cause| {
        error(
            command,
            "input_error",
            format!("input is not UTF-8: {cause}"),
            json,
        )
    })
}
struct StdinResolver<'a> {
    dirs: &'a [PathBuf],
    max: usize,
}
impl IncludeResolver for StdinResolver<'_> {
    fn resolve(&self, requested: &str, from: &str) -> Result<ResolvedSource, QasmError> {
        let mut paths = Vec::new();
        // Nested includes may resolve beside a file already found through an
        // explicit directory. The root stdin name never implies the cwd.
        if Path::new(from).is_absolute()
            && let Some(parent) = Path::new(from).parent()
        {
            paths.push(parent.join(requested));
        }
        paths.extend(self.dirs.iter().map(|dir| dir.join(requested)));
        for path in paths {
            if path.is_file() {
                let path = path
                    .canonicalize()
                    .map_err(|cause| QasmError::new(qasm::ErrorCode::Io, cause.to_string()))?;
                let mut bytes = Vec::new();
                File::open(&path)
                    .and_then(|file| file.take(self.max as u64 + 1).read_to_end(&mut bytes))
                    .map_err(|cause| {
                        QasmError::new(
                            qasm::ErrorCode::Io,
                            format!("read include {}: {cause}", path.display()),
                        )
                    })?;
                if bytes.len() > self.max {
                    return Err(QasmError::new(
                        qasm::ErrorCode::Limit,
                        "included source exceeds byte limit",
                    ));
                }
                let text = String::from_utf8(bytes)
                    .map_err(|cause| QasmError::new(qasm::ErrorCode::Io, cause.to_string()))?;
                return Ok(ResolvedSource {
                    name: path.display().to_string(),
                    text,
                });
            }
        }
        Err(QasmError::new(
            qasm::ErrorCode::Io,
            format!("stdin include {requested:?} requires a matching explicit --include-dir"),
        ))
    }
}
fn publish(
    command: &'static str,
    out: &Path,
    bytes: &[u8],
    json: bool,
) -> Result<(), CommandError> {
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|cause| {
        error(
            command,
            "output_error",
            format!("create output temporary file: {cause}"),
            json,
        )
    })?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.flush())
        .map_err(|cause| {
            error(
                command,
                "output_error",
                format!("write {}: {cause}", out.display()),
                json,
            )
        })?;
    temporary.persist(out).map_err(|cause| {
        error(
            command,
            "output_error",
            format!("publish {}: {cause}", out.display()),
            json,
        )
    })?;
    Ok(())
}

pub fn run_import(
    options: &ImportOptions,
    error_format: Option<ErrorFormat>,
    stdin: &mut dyn Read,
) -> Result<Report, CommandError> {
    let json = options.format.is_json(error_format);
    let limits = QasmLimits::default();
    let circuit = if let Some(path) = options.input.as_deref().filter(|p| *p != Path::new("-")) {
        qasm::from_qasm_file(
            path,
            &qasm::FileResolver {
                include_dirs: options.include_dirs.clone(),
                max_file_bytes: limits.max_source_bytes,
            },
            &limits,
        )
    } else {
        let text = read_text(IMPORT_COMMAND, None, stdin, limits.max_source_bytes, json)?;
        qasm::parse_and_check(
            &text,
            "<stdin>",
            &StdinResolver {
                dirs: &options.include_dirs,
                max: limits.max_source_bytes,
            },
            &limits,
        )
    }
    .map_err(|cause| qasm_error(IMPORT_COMMAND, cause, json))?;
    let plan = circuit
        .lower(options.backend.target(), &limits)
        .map_err(|cause| qasm_error(IMPORT_COMMAND, cause, json))?;
    let (text, format, warnings) = match options.out_format {
        ImportFormat::Qasm2Json => (
            circuit
                .to_json(&limits)
                .map_err(|cause| qasm_error(IMPORT_COMMAND, cause, json))?,
            "qasm2-json",
            Vec::new(),
        ),
        ImportFormat::Stim => {
            let text = crate::ir::circuit_to_string(&plan.instructions);
            if text.len() > limits.max_output_bytes {
                return Err(error(
                    IMPORT_COMMAND,
                    "qasm_limit_exceeded",
                    "Stim projection exceeds output byte limit",
                    json,
                ));
            }
            (text,"stim",vec!["Stim execution projection omits declared register layout and final classical output bindings; use qasm2-json for a complete semantic document".to_owned()])
        }
    };
    publish(IMPORT_COMMAND, &options.out, text.as_bytes(), json)?;
    Ok(Report {
        result: json!({"backend":options.backend.name(),"num_qubits":plan.num_qubits,"num_measurements":plan.measurement_events.len(),"num_output_bits":circuit.num_classical_bits(),"output_layout":{"kind":"classical-registers","registers":circuit.classical_registers(),"order":"declaration-then-index"},"out":options.out.display().to_string(),"out_format":format}),
        warnings,
    })
}

pub fn run_export(
    options: &ExportOptionsCLI,
    error_format: Option<ErrorFormat>,
    stdin: &mut dyn Read,
) -> Result<Report, CommandError> {
    let json = options.format.is_json(error_format);
    let limits = QasmLimits::default();
    let max = match options.in_format {
        InputFormat::Stim => limits.max_source_bytes,
        InputFormat::Qasm2Json => limits.max_output_bytes,
    };
    let text = read_text(EXPORT_COMMAND, options.input.as_deref(), stdin, max, json)?;
    let export_options = qasm::ExportOptions {
        omit_annotations: options.omit_annotations,
    };
    let exported = match options.in_format {
        InputFormat::Stim => {
            let instructions = parse_native(EXPORT_COMMAND, &text, &limits, json)?;
            qasm::export_stim(&instructions, &export_options, &limits)
        }
        InputFormat::Qasm2Json => {
            let circuit = QasmCircuit::from_json(&text, &limits)
                .map_err(|cause| qasm_error(EXPORT_COMMAND, cause, json))?;
            qasm::to_qasm(&circuit, &export_options, &limits)
        }
    }
    .map_err(|cause| qasm_error(EXPORT_COMMAND, cause, json))?;
    let warnings = exported
        .omitted_annotations
        .iter()
        .map(|item| {
            format!(
                "Omitted annotation {} at native instruction path {:?}",
                item.name, item.instruction_path
            )
        })
        .collect();
    publish(EXPORT_COMMAND, &options.out, exported.text.as_bytes(), json)?;
    Ok(Report {
        result: json!({"out":options.out.display().to_string(),"out_format":"qasm2","output_layout":{"kind":"classical-registers","registers":exported.classical_registers,"order":"declaration-then-index"},"omitted_annotations":exported.omitted_annotations}),
        warnings,
    })
}

fn parse_native(
    command: &'static str,
    text: &str,
    limits: &QasmLimits,
    json: bool,
) -> Result<Vec<StimInstr>, CommandError> {
    // Cap syntactic nesting before constructing a deeply recursive AST whose
    // destruction alone could overflow the stack. Match the Stim parser's line
    // block delimiters; syntax validity still belongs to the actual parser.
    let mut depth = 0usize;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.ends_with('{') {
            depth += 1;
            if depth > limits.max_gate_depth {
                return Err(error(
                    command,
                    "qasm_limit_exceeded",
                    "native repeat depth exceeds limit",
                    json,
                ));
            }
        } else if line == "}" {
            depth = depth.saturating_sub(1);
        }
    }
    let instructions = crate::parser::parse_lines(text)
        .map_err(|message| error(command, "invalid_circuit", message, json))?;
    native_layout(command, &instructions, limits, json)?;
    Ok(instructions)
}
fn native_layout(
    command: &'static str,
    instructions: &[StimInstr],
    limits: &QasmLimits,
    json: bool,
) -> Result<(usize, usize), CommandError> {
    let fail = |message| error(command, "qasm_limit_exceeded", message, json);
    let mut pending = vec![(instructions, 1usize, 0usize)];
    let (mut qubits, mut measurements, mut work) = (0usize, 0usize, 0usize);
    while let Some((body, multiplier, depth)) = pending.pop() {
        if depth > limits.max_gate_depth {
            return Err(fail("native repeat depth exceeds limit"));
        }
        for instruction in body {
            match instruction {
                StimInstr::Repeat { count, body } => {
                    if *count == 0 {
                        return Err(fail("zero repeat count is unsupported"));
                    }
                    let count = usize::try_from(*count)
                        .map_err(|_| fail("repeat count exceeds platform capacity"))?;
                    let expanded = multiplier
                        .checked_mul(count)
                        .ok_or_else(|| fail("repeat count multiplication overflows"))?;
                    // Charge each iteration even when its body is empty.
                    work = work
                        .checked_add(expanded)
                        .ok_or_else(|| fail("native expansion work overflows"))?;
                    if work > limits.max_operations {
                        return Err(fail("native expanded work exceeds limit"));
                    }
                    pending.push((body, expanded, depth + 1));
                }
                StimInstr::Op { targets, .. } => {
                    work = work
                        .checked_add(multiplier)
                        .ok_or_else(|| fail("native expansion work overflows"))?;
                    if work > limits.max_operations {
                        return Err(fail("native expanded work exceeds limit"));
                    }
                    for target in targets {
                        if let Some(q) = target.qubit_index() {
                            let width = (q as usize)
                                .checked_add(1)
                                .ok_or_else(|| fail("native qubit width overflows"))?;
                            if width > limits.max_qubits {
                                return Err(fail(
                                    "native qubit width exceeds QASM CLI profile limit",
                                ));
                            }
                            qubits = qubits.max(width);
                        }
                    }
                    // A flat Op slice cannot recurse or hide repeat overflow.
                    let count = crate::stats::num_measurements(std::slice::from_ref(instruction));
                    let count = count
                        .checked_mul(multiplier)
                        .ok_or_else(|| fail("native measurement count overflows"))?;
                    measurements = measurements
                        .checked_add(count)
                        .ok_or_else(|| fail("native measurement count overflows"))?;
                    if measurements > limits.max_operations {
                        return Err(fail("native measurement count exceeds limit"));
                    }
                }
            }
        }
    }
    Ok((qubits, measurements))
}
fn sample_budget(
    shots: usize,
    raw: usize,
    output: usize,
    qubits: usize,
    format: OutputFormat,
    limits: &QasmLimits,
    json: bool,
) -> Result<(), CommandError> {
    let fail = || {
        error(
            SAMPLE_COMMAND,
            "qasm_limit_exceeded",
            "requested shots exceed bounded sampling/output work; reduce --shots",
            json,
        )
    };
    let width = raw.max(output).max(qubits).max(1);
    if shots
        .checked_mul(width)
        .is_none_or(|work| work > limits.max_output_bytes)
    {
        return Err(fail());
    }
    // Include bit-table word padding, including one-shot very-wide outputs.
    if output
        .checked_mul(shots.div_ceil(64))
        .and_then(|words| words.checked_mul(8))
        .is_none_or(|bytes| bytes > limits.max_output_bytes)
    {
        return Err(fail());
    }
    let bytes = match format {
        OutputFormat::Format01 => output
            .checked_add(1)
            .and_then(|width| shots.checked_mul(width)),
        OutputFormat::B8 => output.div_ceil(8).checked_mul(shots),
        OutputFormat::Ptb64 => output
            .checked_mul(shots.div_ceil(64))
            .and_then(|words| words.checked_mul(8)),
        OutputFormat::Hits => output
            .checked_mul(output.max(1).to_string().len() + 1)
            .and_then(|width| width.checked_add(1))
            .and_then(|width| width.checked_mul(shots)),
        OutputFormat::R8 => output
            .checked_add(1)
            .and_then(|width| width.checked_mul(shots)),
        OutputFormat::Dets => None,
    };
    if bytes.is_none_or(|n| n > limits.max_output_bytes) {
        return Err(fail());
    }
    Ok(())
}

pub fn run_sample(
    options: &SampleOptions,
    error_format: Option<ErrorFormat>,
    stdin: &mut dyn Read,
) -> Result<Report, CommandError> {
    let json = options.format.is_json(error_format);
    let limits = QasmLimits::default();
    if options.backend == CliBackend::Stabilizer && options.max_active_rank != 16 {
        return Err(error(
            SAMPLE_COMMAND,
            "invalid_arguments",
            "--max-active-rank requires --backend near-clifford",
            json,
        ));
    }
    if options.skip_reference_sample {
        return Err(error(
            SAMPLE_COMMAND,
            "invalid_arguments",
            "--skip-reference-sample is unavailable for QASM documents and explicit near-Clifford execution",
            json,
        ));
    }
    if options.in_format == InputFormat::Stim && options.result_view == ResultView::Classical {
        return Err(error(
            SAMPLE_COMMAND,
            "qasm_missing_layout",
            "classical result view requires a qasm2-json document with declared output layout",
            json,
        ));
    }
    let format = OutputFormat::from_str(&options.out_format)
        .map_err(|message| error(SAMPLE_COMMAND, "invalid_arguments", message, json))?;
    if format == OutputFormat::Dets {
        return Err(error(
            SAMPLE_COMMAND,
            "invalid_arguments",
            "measurement sampling does not support dets output",
            json,
        ));
    }
    let shots = usize::try_from(options.shots).map_err(|_| {
        error(
            SAMPLE_COMMAND,
            "qasm_limit_exceeded",
            "--shots exceeds platform capacity",
            json,
        )
    })?;
    let max = match options.in_format {
        InputFormat::Stim => limits.max_source_bytes,
        InputFormat::Qasm2Json => limits.max_output_bytes,
    };
    let text = read_text(SAMPLE_COMMAND, options.input.as_deref(), stdin, max, json)?;
    let circuit = if options.in_format == InputFormat::Qasm2Json {
        Some(
            QasmCircuit::from_json(&text, &limits)
                .map_err(|cause| qasm_error(SAMPLE_COMMAND, cause, json))?,
        )
    } else {
        None
    };
    let plan = circuit
        .as_ref()
        .map(|c| c.lower(options.backend.target(), &limits))
        .transpose()
        .map_err(|cause| qasm_error(SAMPLE_COMMAND, cause, json))?;
    let native;
    let (instructions, qubits, raw_width) = if let Some(plan) = &plan {
        (
            &plan.instructions,
            plan.num_qubits,
            plan.measurement_events.len(),
        )
    } else {
        native = parse_native(SAMPLE_COMMAND, &text, &limits, json)?;
        let (qubits, raw) = native_layout(SAMPLE_COMMAND, &native, &limits, json)?;
        (&native, qubits, raw)
    };
    let width = if options.result_view == ResultView::Classical {
        circuit.as_ref().map_or(0, QasmCircuit::num_classical_bits)
    } else {
        raw_width
    };
    sample_budget(shots, raw_width, width, qubits, format, &limits, json)?;
    let mut rng = crate::operations::make_rng(options.seed);
    let raw = match options.backend {
        CliBackend::Stabilizer => {
            crate::validation::validate_circuit(instructions).map_err(|message| {
                error(SAMPLE_COMMAND, "qasm_backend_unsupported", message, json)
            })?;
            crate::sampler::sample_batch_with_options(
                instructions,
                shots,
                &mut rng,
                crate::sampler::SampleOptions {
                    output_mode: crate::sampler::SampleOutputMode::MeasurementsOnly,
                    ..Default::default()
                },
            )
            .map_err(|message| error(SAMPLE_COMMAND, "execution_error", message, json))?
            .measurements
        }
        CliBackend::NearClifford => {
            let compiled = CompiledNearCliffordExecutor::compile_with_limit(
                instructions.clone(),
                options.max_active_rank,
            )
            .map_err(|message| {
                error(
                    SAMPLE_COMMAND,
                    "qasm_backend_unsupported",
                    format!(
                        "near-Clifford plan (--max-active-rank {}): {message}",
                        options.max_active_rank
                    ),
                    json,
                )
            })?;
            let batch = compiled
                .prepare_sampler()
                .and_then(|mut sampler| sampler.sample_measurements_u8(shots, &mut rng))
                .map_err(|message| {
                    error(
                        SAMPLE_COMMAND,
                        "execution_error",
                        format!(
                            "near-Clifford execution (--max-active-rank {}): {message}",
                            options.max_active_rank
                        ),
                        json,
                    )
                })?;
            if batch.measurements_per_shot != raw_width {
                return Err(error(
                    SAMPLE_COMMAND,
                    "execution_error",
                    "backend measurement width disagrees with checked layout",
                    json,
                ));
            }
            let mut table = BitTable::try_new(raw_width, shots).map_err(|cause| {
                error(
                    SAMPLE_COMMAND,
                    "qasm_limit_exceeded",
                    format!("measurement allocation: {cause:?}"),
                    json,
                )
            })?;
            for shot in 0..shots {
                for bit in 0..raw_width {
                    table.set(bit, shot, batch.measurements[shot * raw_width + bit] != 0);
                }
            }
            table
        }
    };
    let output = if options.result_view == ResultView::Classical {
        let mut table = BitTable::try_new(width, shots).map_err(|cause| {
            error(
                SAMPLE_COMMAND,
                "qasm_limit_exceeded",
                format!("classical output allocation: {cause:?}"),
                json,
            )
        })?;
        let plan = plan.as_ref().ok_or_else(|| {
            error(
                SAMPLE_COMMAND,
                "qasm_missing_layout",
                "missing classical output layout",
                json,
            )
        })?;
        for (bit, binding) in plan.classical_outputs.iter().enumerate() {
            if let qasm::ClassicalBinding::Measurement(id) = binding {
                let event = plan.measurement_events[*id];
                for shot in 0..shots {
                    table.set(bit, shot, raw.get(event, shot));
                }
            }
        }
        table
    } else {
        raw
    };
    let mut bytes = Vec::new();
    crate::operations::write_format(format, &output, &mut bytes)
        .map_err(|message| error(SAMPLE_COMMAND, "output_error", message, json))?;
    if bytes.len() > limits.max_output_bytes {
        return Err(error(
            SAMPLE_COMMAND,
            "qasm_limit_exceeded",
            "sample output exceeds byte limit",
            json,
        ));
    }
    publish(SAMPLE_COMMAND, &options.out, &bytes, json)?;
    let layout = if let Some(circuit) = &circuit {
        if options.result_view == ResultView::Classical {
            json!({"kind":"classical-registers","registers":circuit.classical_registers(),"order":"declaration-then-index"})
        } else {
            json!({"kind":"measurement-record","bits":raw_width,"order":"ascending-record-index"})
        }
    } else {
        json!({"kind":"measurement-record","bits":raw_width,"order":"ascending-record-index"})
    };
    Ok(Report {
        result: json!({"backend":options.backend.name(),"result_view":options.result_view.name(),"shots":options.shots,"num_measurements":raw_width,"num_output_bits":width,"output_layout":layout,"out":options.out.display().to_string(),"out_format":options.out_format}),
        warnings: Vec::new(),
    })
}
