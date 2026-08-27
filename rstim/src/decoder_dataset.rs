use crate::sim::bit_table::BitTable;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub const LOGICAL_FLIP_MARKER_TAG: &str = "rstim:logical_flip_point";
pub const LOGICAL_FLIP_MARKER: &str = "TICK[rstim:logical_flip_point]";
pub const DEFAULT_DECODER_DATASET_BATCH_SHOTS: usize = 10_000;
const PUBLIC_SCHEMA_VERSION: u32 = 1;
const DATASET_FORMAT: &str = "rstim_decoder_dataset";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecoderDatasetMode {
    Detectors,
    MeasurementsBlinded,
}

impl DecoderDatasetMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "detectors" => Ok(Self::Detectors),
            "measurements_blinded" => Ok(Self::MeasurementsBlinded),
            other => Err(format!("unknown decoder dataset mode: {other}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detectors => "detectors",
            Self::MeasurementsBlinded => "measurements_blinded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalPauli {
    X,
    Z,
}

impl LogicalPauli {
    fn gate_name(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Z => "Z",
        }
    }

    fn option_name(self) -> &'static str {
        match self {
            Self::X => "--logical_x_qubits",
            Self::Z => "--logical_z_qubits",
        }
    }

    fn as_str(self) -> &'static str {
        self.gate_name()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalFlip {
    pub pauli: LogicalPauli,
    pub qubits: Vec<u32>,
}

impl LogicalFlip {
    pub fn parse(pauli: LogicalPauli, value: &str) -> Result<Self, String> {
        Ok(Self {
            pauli,
            qubits: parse_logical_qubits(pauli.option_name(), value)?,
        })
    }
}

#[derive(Debug, Clone)]
/// Backward-compatible X-only export configuration from rstim 0.2.0.
pub struct ExportDecoderDatasetConfig {
    pub circuit_text: String,
    pub shots: usize,
    pub mode: DecoderDatasetMode,
    pub logical_x_qubits: Vec<u32>,
    pub public_out: PathBuf,
    pub private_out: PathBuf,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone)]
/// Export configuration supporting either an X or Z logical flip.
pub struct ExportDecoderDatasetLogicalFlipConfig {
    pub circuit_text: String,
    pub shots: usize,
    pub mode: DecoderDatasetMode,
    pub logical_flip: Option<LogicalFlip>,
    pub public_out: PathBuf,
    pub private_out: PathBuf,
    pub seed: Option<u64>,
    /// Also write a per-shot `trace.jsonl` sidecar (schema `rstim.error-trace.v1`)
    /// into the private bundle, recording every noise realization (Pauli branch
    /// or loss onset) behind each shot. Traced sampling is per-shot and slower,
    /// and produces a different batch than untraced sampling for the same seed.
    pub error_trace: bool,
}

impl From<ExportDecoderDatasetConfig> for ExportDecoderDatasetLogicalFlipConfig {
    fn from(config: ExportDecoderDatasetConfig) -> Self {
        let logical_flip = if config.logical_x_qubits.is_empty() {
            None
        } else {
            Some(LogicalFlip {
                pauli: LogicalPauli::X,
                qubits: config.logical_x_qubits,
            })
        };
        Self {
            circuit_text: config.circuit_text,
            shots: config.shots,
            mode: config.mode,
            logical_flip,
            public_out: config.public_out,
            private_out: config.private_out,
            seed: config.seed,
            error_trace: false,
        }
    }
}

impl From<&ExportDecoderDatasetConfig> for ExportDecoderDatasetLogicalFlipConfig {
    fn from(config: &ExportDecoderDatasetConfig) -> Self {
        config.clone().into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderDatasetSummary {
    pub dataset_id: String,
    pub mode: DecoderDatasetMode,
    pub shots: usize,
    pub row_bits: usize,
    pub public_out: PathBuf,
    pub private_out: PathBuf,
}

#[doc(hidden)]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("write hex into String");
    }
    out
}

#[doc(hidden)]
pub fn bit_table_to_b8_bytes(table: &BitTable) -> Result<Vec<u8>, String> {
    let bytes_per_shot = table
        .num_major()
        .checked_add(7)
        .ok_or_else(|| "b8 row width overflows".to_string())?
        / 8;
    let total = bytes_per_shot
        .checked_mul(table.num_minor())
        .ok_or_else(|| "b8 output size overflows".to_string())?;
    let mut bytes = Vec::with_capacity(total);
    crate::output::write_shots_b8(table, &mut bytes)
        .map_err(|error| format!("write error: {error}"))?;
    Ok(bytes)
}

#[doc(hidden)]
pub fn dataset_id_material(
    schema_version: u32,
    mode: DecoderDatasetMode,
    circuit_sha256: &str,
    shots: usize,
    row_bits: usize,
    shots_b8_sha256: &str,
) -> Vec<u8> {
    format!(
        "format={DATASET_FORMAT}\nschema_version={schema_version}\nmode={}\ncircuit_sha256={circuit_sha256}\nshots={shots}\nrow_bits={row_bits}\nshots_b8_sha256={shots_b8_sha256}\n",
        mode.as_str(),
    )
    .into_bytes()
}

fn parse_logical_qubits(option_name: &str, value: &str) -> Result<Vec<u32>, String> {
    if value.trim().is_empty() {
        return Err(format!("{option_name} must be non-empty"));
    }

    let mut seen = BTreeSet::new();
    let mut qubits = Vec::new();
    for token in value.split(',') {
        let token = token.trim();
        let qubit = token
            .parse::<u32>()
            .map_err(|_| format!("{option_name} contains invalid qubit index {token:?}"))?;
        if !seen.insert(qubit) {
            return Err(format!(
                "{option_name} contains duplicate qubit index {qubit}"
            ));
        }
        qubits.push(qubit);
    }
    Ok(qubits)
}

#[doc(hidden)]
pub fn parse_logical_x_qubits(value: &str) -> Result<Vec<u32>, String> {
    LogicalFlip::parse(LogicalPauli::X, value).map(|flip| flip.qubits)
}

pub(crate) fn logical_flip_marker_instruction() -> crate::ir::StimInstr {
    crate::ir::StimInstr::Op {
        name: "TICK".to_string(),
        tag: Some(LOGICAL_FLIP_MARKER_TAG.to_string()),
        args: Vec::new(),
        targets: Vec::new(),
    }
}

fn logical_flip_marker_index(instrs: &[crate::ir::StimInstr]) -> Result<usize, String> {
    fn scan(
        instrs: &[crate::ir::StimInstr],
        depth: usize,
        marker_count: &mut usize,
        top_level_index: &mut Option<usize>,
    ) -> Result<(), String> {
        for (index, instr) in instrs.iter().enumerate() {
            match instr {
                crate::ir::StimInstr::Op { name, tag, .. }
                    if tag.as_deref() == Some(LOGICAL_FLIP_MARKER_TAG) =>
                {
                    if name != "TICK" {
                        return Err(format!(
                            "logical flip tag [{LOGICAL_FLIP_MARKER_TAG}] must annotate TICK"
                        ));
                    }
                    *marker_count += 1;
                    if depth == 0 {
                        *top_level_index = Some(index);
                    }
                }
                crate::ir::StimInstr::Repeat { body, .. } => {
                    scan(body, depth + 1, marker_count, top_level_index)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    let mut marker_count = 0;
    let mut top_level_index = None;
    scan(instrs, 0, &mut marker_count, &mut top_level_index)?;
    if marker_count != 1 {
        return Err(format!(
            "logical flip marker {LOGICAL_FLIP_MARKER} must appear exactly once"
        ));
    }
    top_level_index.ok_or_else(|| "logical flip marker must be top-level".to_string())
}

fn positive_probability_noise_name(instrs: &[crate::ir::StimInstr]) -> Option<&str> {
    for instr in instrs {
        match instr {
            crate::ir::StimInstr::Op { name, args, .. }
                if matches!(
                    name.as_str(),
                    "I_ERROR"
                        | "II_ERROR"
                        | "LOSS"
                        | "X_ERROR"
                        | "Y_ERROR"
                        | "Z_ERROR"
                        | "DEPOLARIZE1"
                        | "DEPOLARIZE2"
                        | "PAULI_CHANNEL_1"
                        | "PAULI_CHANNEL_2"
                        | "HERALDED_ERASE"
                        | "HERALDED_PAULI_CHANNEL_1"
                        | "CORRELATED_ERROR"
                        | "E"
                        | "ELSE_CORRELATED_ERROR"
                        | "M"
                        | "MZ"
                        | "MX"
                        | "MY"
                        | "MR"
                        | "MRZ"
                        | "MRX"
                        | "MRY"
                        | "ML"
                        | "MZL"
                        | "MXL"
                        | "MYL"
                        | "MRL"
                        | "MRZL"
                        | "MRXL"
                        | "MRYL"
                        | "MPAD"
                        | "MPP"
                        | "MXX"
                        | "MYY"
                        | "MZZ"
                ) && args.iter().any(|probability| *probability > 0.0) =>
            {
                return Some(name);
            }
            crate::ir::StimInstr::Repeat { count, body } if *count > 0 => {
                if let Some(name) = positive_probability_noise_name(body) {
                    return Some(name);
                }
            }
            _ => {}
        }
    }
    None
}

#[doc(hidden)]
pub fn circuit_with_injected_logical_flip(
    circuit_text: &str,
    logical_flip: &LogicalFlip,
) -> Result<String, String> {
    let instrs = crate::validation::parse_and_validate(circuit_text)?;
    let marker_index = logical_flip_marker_index(&instrs)?;
    if let Some(noise_name) = positive_probability_noise_name(&instrs[..marker_index]) {
        return Err(format!(
            "logical flip marker {LOGICAL_FLIP_MARKER} must appear before the first positive-probability noise instruction; found {noise_name} before the marker"
        ));
    }

    let injected = format!(
        "{} {}\n",
        logical_flip.pauli.gate_name(),
        logical_flip
            .qubits
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mut output = String::with_capacity(circuit_text.len() + injected.len());
    let mut inserted = false;
    for line in circuit_text.split_inclusive('\n') {
        output.push_str(line);
        let code = line.split('#').next().unwrap_or("").trim();
        if code == LOGICAL_FLIP_MARKER {
            if !line.ends_with('\n') {
                output.push('\n');
            }
            output.push_str(&injected);
            inserted = true;
        }
    }
    if !inserted {
        return Err(format!(
            "logical flip marker must use the canonical source spelling {LOGICAL_FLIP_MARKER}"
        ));
    }
    Ok(output)
}

#[doc(hidden)]
pub fn circuit_with_injected_logical_x(
    circuit_text: &str,
    logical_x_qubits: &[u32],
) -> Result<String, String> {
    circuit_with_injected_logical_flip(
        circuit_text,
        &LogicalFlip {
            pauli: LogicalPauli::X,
            qubits: logical_x_qubits.to_vec(),
        },
    )
}

#[derive(Debug)]
struct ValidatedDecoderDatasetInput {
    public_circuit_text: String,
    public_instrs: Vec<crate::ir::StimInstr>,
    private_one_instrs: Option<Vec<crate::ir::StimInstr>>,
    logical_flip: Option<LogicalFlip>,
    measurements: usize,
    detectors: usize,
    observables: usize,
}

fn one_shot_measurement_table(bits: &[bool]) -> Result<BitTable, String> {
    let mut table = BitTable::try_new(bits.len(), 1)
        .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
    for (bit, value) in bits.iter().copied().enumerate() {
        if value {
            table.set(bit, 0, true);
        }
    }
    Ok(table)
}

fn validate_logical_flip_effect(
    public_instrs: &[crate::ir::StimInstr],
    private_instrs: &[crate::ir::StimInstr],
    pauli: LogicalPauli,
) -> Result<(), String> {
    let m0 = crate::data_path::build_reference_sample(
        public_instrs,
        crate::data_path::ReferenceSampleMode::SimulateNoiseless,
    )?;
    let m1 = crate::data_path::build_reference_sample(
        private_instrs,
        crate::data_path::ReferenceSampleMode::SimulateNoiseless,
    )?;
    let t0 = one_shot_measurement_table(&m0)?;
    let t1 = one_shot_measurement_table(&m1)?;
    let out0 = crate::m2d::measurements_to_detections(public_instrs, &t0)?;
    let out1 = crate::m2d::measurements_to_detections(public_instrs, &t1)?;
    for detector in 0..out0.detections.num_major() {
        if out0.detections.get(detector, 0) != out1.detections.get(detector, 0) {
            return Err(format!(
                "injected logical {} changes detector reference values",
                pauli.gate_name()
            ));
        }
    }
    let flips = out0.observable_flips.get(0, 0) ^ out1.observable_flips.get(0, 0);
    if !flips {
        return Err(format!(
            "injected logical {} does not flip observable 0",
            pauli.gate_name()
        ));
    }
    Ok(())
}

#[doc(hidden)]
#[allow(private_interfaces)]
pub fn validate_decoder_dataset_inputs(
    config: &ExportDecoderDatasetConfig,
) -> Result<ValidatedDecoderDatasetInput, String> {
    let config = ExportDecoderDatasetLogicalFlipConfig::from(config);
    validate_decoder_dataset_logical_flip_inputs(&config)
}

#[doc(hidden)]
#[allow(private_interfaces)]
pub fn validate_decoder_dataset_logical_flip_inputs(
    config: &ExportDecoderDatasetLogicalFlipConfig,
) -> Result<ValidatedDecoderDatasetInput, String> {
    if config.shots == 0 {
        return Err("--shots must be positive".to_string());
    }
    let public_instrs = crate::parser::parse_lines(&config.circuit_text)?;
    let stats = crate::stats::summarize(&public_instrs);
    if stats.num_observables == 0 {
        return Err(format!(
            "export_decoder_dataset requires at least one observable, found {}",
            stats.num_observables
        ));
    }
    if config.mode == DecoderDatasetMode::MeasurementsBlinded && stats.num_observables != 1 {
        return Err(format!(
            "measurements_blinded mode requires exactly one observable, found {}",
            stats.num_observables
        ));
    }
    if stats.num_sweep_bits != 0 {
        return Err("export_decoder_dataset does not support sweep-bit circuits".to_string());
    }
    match (config.mode, config.logical_flip.as_ref()) {
        (DecoderDatasetMode::Detectors, Some(logical_flip)) => {
            return Err(format!(
                "detectors mode rejects {}",
                logical_flip.pauli.option_name()
            ));
        }
        (DecoderDatasetMode::MeasurementsBlinded, None) => {
            return Err(
                "measurements_blinded mode requires exactly one of --logical_x_qubits or --logical_z_qubits"
                    .to_string(),
            );
        }
        _ => {}
    }

    let private_one_instrs = match config.mode {
        DecoderDatasetMode::Detectors => None,
        DecoderDatasetMode::MeasurementsBlinded => {
            let logical_flip = config
                .logical_flip
                .as_ref()
                .expect("measurements_blinded logical flip checked above");
            if logical_flip.qubits.is_empty() {
                return Err(format!(
                    "{} must be non-empty",
                    logical_flip.pauli.option_name()
                ));
            }
            let mut unique_qubits = BTreeSet::new();
            for &qubit in &logical_flip.qubits {
                if !unique_qubits.insert(qubit) {
                    return Err(format!(
                        "{} contains duplicate qubit index {qubit}",
                        logical_flip.pauli.option_name()
                    ));
                }
                if qubit as usize >= stats.num_qubits {
                    return Err(format!(
                        "{} contains qubit {qubit}, but circuit has {} qubits",
                        logical_flip.pauli.option_name(),
                        stats.num_qubits
                    ));
                }
            }
            let circuit_text =
                circuit_with_injected_logical_flip(&config.circuit_text, logical_flip)?;
            let instrs = crate::parser::parse_lines(&circuit_text)?;
            validate_logical_flip_effect(&public_instrs, &instrs, logical_flip.pauli)?;
            Some(instrs)
        }
    };

    Ok(ValidatedDecoderDatasetInput {
        public_circuit_text: config.circuit_text.clone(),
        public_instrs,
        private_one_instrs,
        logical_flip: config.logical_flip.clone(),
        measurements: stats.num_measurements,
        detectors: stats.num_detectors,
        observables: stats.num_observables,
    })
}

#[doc(hidden)]
pub struct DecoderDatasetArtifacts {
    pub public_circuit_text: String,
    pub public_instrs: Vec<crate::ir::StimInstr>,
    pub public_row_kind: &'static str,
    pub public_shots: BitTable,
    pub answers: BitTable,
    pub masks: Option<BitTable>,
    pub error_trace: Option<Vec<u8>>,
    pub measurements: usize,
    pub detectors: usize,
    pub observables: usize,
}

struct DecoderDatasetChunk {
    public_shots: BitTable,
    answers: BitTable,
    masks: Option<BitTable>,
    error_trace: Option<Vec<u8>>,
}

struct DatasetRngs {
    physical: rand::rngs::StdRng,
    mask: rand::rngs::StdRng,
    permutation: rand::rngs::StdRng,
}

fn make_dataset_rngs(seed: Option<u64>) -> DatasetRngs {
    match seed {
        Some(seed) => DatasetRngs {
            physical: domain_rng(seed, b"physical-sampling"),
            mask: domain_rng(seed, b"logical-mask"),
            permutation: domain_rng(seed, b"row-permutation"),
        },
        None => DatasetRngs {
            physical: rand::rngs::StdRng::from_entropy(),
            mask: rand::rngs::StdRng::from_entropy(),
            permutation: rand::rngs::StdRng::from_entropy(),
        },
    }
}

fn domain_rng(seed: u64, domain: &[u8]) -> rand::rngs::StdRng {
    let mut hasher = Sha256::new();
    hasher.update(b"rstim-decoder-dataset-v1\n");
    hasher.update(domain);
    hasher.update(b"\n");
    hasher.update(seed.to_le_bytes());
    rand::rngs::StdRng::from_seed(hasher.finalize().into())
}

fn copy_shot(src: &BitTable, src_shot: usize, dst: &mut BitTable, dst_shot: usize) {
    for row in 0..src.num_major() {
        if src.get(row, src_shot) {
            dst.set(row, dst_shot, true);
        }
    }
}

const ERROR_TRACE_SCHEMA: &str = "rstim.error-trace.v1";

fn append_error_trace_line(
    buffer: &mut Vec<u8>,
    shot: usize,
    trace: &crate::sample_trace::SampleTrace,
    logical_input: Option<(&LogicalFlip, bool)>,
) -> Result<(), String> {
    #[derive(Serialize)]
    struct TraceEvent<'a> {
        op: &'a str,
        targets: &'a [u32],
        branch: Option<&'a str>,
        path: &'a [usize],
        iterations: &'a [u64],
    }
    #[derive(Serialize)]
    struct TraceLine<'a> {
        schema_version: &'static str,
        shot: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        logical_input: Option<LogicalInputTrace<'a>>,
        events: Vec<TraceEvent<'a>>,
    }
    #[derive(Serialize)]
    struct LogicalInputTrace<'a> {
        bit: u8,
        applied: bool,
        pauli: &'static str,
        support: &'a [u32],
    }
    let line = TraceLine {
        schema_version: ERROR_TRACE_SCHEMA,
        shot,
        logical_input: logical_input.map(|(logical_flip, bit)| LogicalInputTrace {
            bit: u8::from(bit),
            applied: bit,
            pauli: logical_flip.pauli.as_str(),
            support: &logical_flip.qubits,
        }),
        events: trace
            .noise_events
            .iter()
            .map(|event| TraceEvent {
                op: &event.instr_name,
                targets: &event.target_qubits,
                branch: event.branch_label.as_deref(),
                path: &event.op_path,
                iterations: &event.repeat_iterations,
            })
            .collect(),
    };
    serde_json::to_writer(&mut *buffer, &line)
        .map_err(|error| format!("failed to serialize error trace line: {error}"))?;
    buffer.push(b'\n');
    Ok(())
}

/// Aggregates an observable bit per observable index. Stim allows several
/// `OBSERVABLE_INCLUDE(k)` instructions whose contributions XOR into the same
/// observable, so every entry with the same index must be combined.
fn observable_flips(
    output: &crate::executor::ExecOutput,
    num_observables: usize,
    context: &str,
) -> Result<Vec<bool>, String> {
    let mut seen = vec![false; num_observables];
    let mut flips = vec![false; num_observables];
    for (index, bit) in &output.observables {
        let observable = *index as usize;
        if observable >= num_observables {
            return Err(format!(
                "traced {context} shot produced observable {observable}, expected {num_observables}"
            ));
        }
        seen[observable] = true;
        flips[observable] ^= bit;
    }
    if let Some(missing) = seen.iter().position(|present| !present) {
        return Err(format!(
            "traced {context} shot produced no observable {missing}"
        ));
    }
    Ok(flips)
}

/// Samples a chunk shot-by-shot with tracing enabled so every shot carries the
/// noise realization (Pauli branch or loss onset) that produced it. `shot_offset`
/// is the global index of the chunk's first shot, so trace lines stay aligned
/// with `shots.b8` across chunk boundaries.
fn generate_traced_decoder_dataset_chunk(
    validated: &ValidatedDecoderDatasetInput,
    mode: DecoderDatasetMode,
    shots: usize,
    shot_offset: usize,
    rngs: &mut DatasetRngs,
) -> Result<DecoderDatasetChunk, String> {
    match mode {
        DecoderDatasetMode::Detectors => {
            let mut executor =
                crate::executor::Executor::from_instrs(validated.public_instrs.clone())?;
            let mut detections = BitTable::try_new(validated.detectors, shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            let mut answers = BitTable::try_new(validated.observables, shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            let mut trace_bytes = Vec::new();
            for shot in 0..shots {
                let (output, trace) = executor.run_with_trace(&mut rngs.physical)?;
                debug_assert_eq!(
                    output.detectors.len(),
                    validated.detectors,
                    "traced shot detector count matches the validated circuit"
                );
                for (detector, bit) in output.detectors.iter().copied().enumerate() {
                    if bit {
                        detections.set(detector, shot, true);
                    }
                }
                for (observable, bit) in
                    observable_flips(&output, validated.observables, "detectors")?
                        .iter()
                        .copied()
                        .enumerate()
                {
                    if bit {
                        answers.set(observable, shot, true);
                    }
                }
                append_error_trace_line(&mut trace_bytes, shot_offset + shot, &trace, None)?;
            }
            Ok(DecoderDatasetChunk {
                public_shots: detections,
                answers,
                masks: None,
                error_trace: Some(trace_bytes),
            })
        }
        DecoderDatasetMode::MeasurementsBlinded => {
            let private_one_instrs = validated
                .private_one_instrs
                .as_ref()
                .ok_or_else(|| "missing blinded logical-one circuit".to_string())?;
            let logical_flip = validated
                .logical_flip
                .as_ref()
                .ok_or_else(|| "missing blinded logical-input metadata".to_string())?;
            let mut zero_executor =
                crate::executor::Executor::from_instrs(validated.public_instrs.clone())?;
            let mut one_executor =
                crate::executor::Executor::from_instrs(private_one_instrs.clone())?;

            let mut source_labels: Vec<bool> = (0..shots).map(|_| rngs.mask.r#gen()).collect();
            for index in (1..source_labels.len()).rev() {
                let replacement = rngs.permutation.gen_range(0..=index);
                source_labels.swap(index, replacement);
            }

            let mut measurements = BitTable::try_new(validated.measurements, shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            let mut masks = BitTable::try_new(1, shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            let mut answers = BitTable::try_new(1, shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            let mut trace_bytes = Vec::new();
            for (shot, label) in source_labels.iter().copied().enumerate() {
                let executor = if label {
                    &mut one_executor
                } else {
                    &mut zero_executor
                };
                let (output, trace) = executor.run_with_trace(&mut rngs.physical)?;
                debug_assert_eq!(
                    output.measurements.len(),
                    validated.measurements,
                    "traced shot measurement count matches the validated circuit"
                );
                for (row, bit) in output.measurements.iter().copied().enumerate() {
                    if bit {
                        measurements.set(row, shot, true);
                    }
                }
                if label {
                    masks.set(0, shot, true);
                }
                // The injected logical flip only changes measurement values, not
                // the observable's record-bit structure, so the executed
                // observable equals the public interpretation; unmask it here.
                let flips = observable_flips(&output, validated.observables, "blinded")?;
                if flips[0] ^ label {
                    answers.set(0, shot, true);
                }
                append_error_trace_line(
                    &mut trace_bytes,
                    shot_offset + shot,
                    &trace,
                    Some((logical_flip, label)),
                )?;
            }
            Ok(DecoderDatasetChunk {
                public_shots: measurements,
                answers,
                masks: Some(masks),
                error_trace: Some(trace_bytes),
            })
        }
    }
}

fn generate_decoder_dataset_chunk(
    validated: &ValidatedDecoderDatasetInput,
    mode: DecoderDatasetMode,
    shots: usize,
    shot_offset: usize,
    error_trace: bool,
    rngs: &mut DatasetRngs,
) -> Result<DecoderDatasetChunk, String> {
    if error_trace {
        return generate_traced_decoder_dataset_chunk(validated, mode, shots, shot_offset, rngs);
    }
    match mode {
        DecoderDatasetMode::Detectors => {
            let result = crate::sampler::sample_batch_with_options(
                &validated.public_instrs,
                shots,
                &mut rngs.physical,
                crate::sampler::SampleOptions {
                    output_mode: crate::sampler::SampleOutputMode::Full,
                    ..crate::sampler::SampleOptions::default()
                },
            )?;
            Ok(DecoderDatasetChunk {
                public_shots: result.detections,
                answers: result.observable_flips,
                masks: None,
                error_trace: None,
            })
        }
        DecoderDatasetMode::MeasurementsBlinded => {
            let mut source_labels: Vec<bool> = (0..shots).map(|_| rngs.mask.r#gen()).collect();
            for index in (1..source_labels.len()).rev() {
                let replacement = rngs.permutation.gen_range(0..=index);
                source_labels.swap(index, replacement);
            }

            let zero_count = source_labels.iter().filter(|&&label| !label).count();
            let one_count = shots - zero_count;
            let zero_samples = crate::sampler::sample_batch_with_options(
                &validated.public_instrs,
                zero_count,
                &mut rngs.physical,
                crate::sampler::SampleOptions {
                    output_mode: crate::sampler::SampleOutputMode::Full,
                    ..crate::sampler::SampleOptions::default()
                },
            )?;
            let private_one_instrs = validated
                .private_one_instrs
                .as_ref()
                .ok_or_else(|| "missing blinded logical-one circuit".to_string())?;
            let one_samples = crate::sampler::sample_batch_with_options(
                private_one_instrs,
                one_count,
                &mut rngs.physical,
                crate::sampler::SampleOptions {
                    output_mode: crate::sampler::SampleOutputMode::Full,
                    ..crate::sampler::SampleOptions::default()
                },
            )?;

            let mut measurements = BitTable::try_new(validated.measurements, shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            let mut masks = BitTable::try_new(1, shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            let mut next_zero = 0;
            let mut next_one = 0;
            for (shot, label) in source_labels.iter().copied().enumerate() {
                if label {
                    copy_shot(&one_samples.measurements, next_one, &mut measurements, shot);
                    masks.set(0, shot, true);
                    next_one += 1;
                } else {
                    copy_shot(
                        &zero_samples.measurements,
                        next_zero,
                        &mut measurements,
                        shot,
                    );
                    next_zero += 1;
                }
            }

            let public_interpretation =
                crate::m2d::measurements_to_detections(&validated.public_instrs, &measurements)?;
            let mut answers = BitTable::try_new(1, shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            for shot in 0..shots {
                let public_observable = public_interpretation.observable_flips.get(0, shot);
                let mask_bit = masks.get(0, shot);
                answers.set(0, shot, public_observable ^ mask_bit);
            }

            Ok(DecoderDatasetChunk {
                public_shots: measurements,
                answers,
                masks: Some(masks),
                error_trace: None,
            })
        }
    }
}

#[doc(hidden)]
pub fn generate_decoder_dataset_artifacts(
    config: &ExportDecoderDatasetConfig,
) -> Result<DecoderDatasetArtifacts, String> {
    let config = ExportDecoderDatasetLogicalFlipConfig::from(config);
    generate_decoder_dataset_artifacts_with_logical_flip(&config)
}

#[doc(hidden)]
pub fn generate_decoder_dataset_artifacts_with_logical_flip(
    config: &ExportDecoderDatasetLogicalFlipConfig,
) -> Result<DecoderDatasetArtifacts, String> {
    let validated = validate_decoder_dataset_logical_flip_inputs(config)?;
    let mut rngs = make_dataset_rngs(config.seed);
    let chunk = generate_decoder_dataset_chunk(
        &validated,
        config.mode,
        config.shots,
        0,
        config.error_trace,
        &mut rngs)?;
    Ok(DecoderDatasetArtifacts {
        public_circuit_text: validated.public_circuit_text,
        public_instrs: validated.public_instrs,
        public_row_kind: match config.mode {
            DecoderDatasetMode::Detectors => "detectors",
            DecoderDatasetMode::MeasurementsBlinded => "measurements",
        },
        public_shots: chunk.public_shots,
        answers: chunk.answers,
        masks: chunk.masks,
        error_trace: chunk.error_trace,
        measurements: validated.measurements,
        detectors: validated.detectors,
        observables: validated.observables,
    })
}

#[derive(Debug, Serialize)]
struct PublicManifest {
    format: &'static str,
    schema_version: u32,
    dataset_id: String,
    mode: DecoderDatasetMode,
    shots: usize,
    row: PublicRowManifest,
    circuit: CircuitManifest,
    shots_file: FileManifest,
}

#[derive(Debug, Serialize)]
struct PrivateManifest {
    format: &'static str,
    schema_version: u32,
    dataset_id: String,
    mode: DecoderDatasetMode,
    shots: usize,
    answers_file: FileManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    masks_file: Option<FileManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_file: Option<TraceFileManifest>,
    generation: PrivateGenerationManifest,
}

#[derive(Debug, Serialize)]
struct TraceFileManifest {
    file: &'static str,
    sha256: String,
    schema: &'static str,
    lines: usize,
}

#[derive(Debug, Serialize)]
struct PublicRowManifest {
    kind: &'static str,
    bits: usize,
    encoding: &'static str,
    bit_order: &'static str,
    bytes_per_shot: usize,
}

#[derive(Debug, Serialize)]
struct CircuitManifest {
    file: &'static str,
    sha256: String,
    measurements: usize,
    detectors: usize,
    observables: usize,
    sweep_bits: usize,
}

#[derive(Debug, Serialize)]
struct FileManifest {
    file: &'static str,
    sha256: String,
    bits: usize,
    bytes_per_shot: usize,
}

#[derive(Debug, Serialize)]
struct PrivateGenerationManifest {
    rstim_version: &'static str,
    batch_shots: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u64>,
}

struct NewDirectoryPath {
    final_path: PathBuf,
    parent: PathBuf,
    name: OsString,
}

struct ValidatedOutputDirectories {
    public: NewDirectoryPath,
    private: NewDirectoryPath,
}

fn resolve_new_output_directory(path: &Path) -> Result<NewDirectoryPath, String> {
    let name = path
        .file_name()
        .ok_or_else(|| "output directory must have a final path component".to_string())?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|error| {
        format!(
            "failed to resolve output parent {}: {error}",
            parent.display()
        )
    })?;
    if !parent.is_dir() {
        return Err(format!(
            "output parent is not a directory: {}",
            parent.display()
        ));
    }
    let final_path = parent.join(name);
    if final_path
        .try_exists()
        .map_err(|error| format!("failed to inspect {}: {error}", final_path.display()))?
    {
        return Err(format!(
            "output directory already exists: {}",
            final_path.display()
        ));
    }
    Ok(NewDirectoryPath {
        final_path,
        parent,
        name: name.to_os_string(),
    })
}

fn validate_output_directories(
    public_out: &Path,
    private_out: &Path,
) -> Result<ValidatedOutputDirectories, String> {
    let public = resolve_new_output_directory(public_out)?;
    let private = resolve_new_output_directory(private_out)?;
    if public.final_path == private.final_path {
        return Err("--public_out and --private_out resolve to the same directory".to_string());
    }
    if public.final_path.starts_with(&private.final_path)
        || private.final_path.starts_with(&public.final_path)
    {
        return Err("--public_out and --private_out must not be nested".to_string());
    }
    Ok(ValidatedOutputDirectories { public, private })
}

#[doc(hidden)]
pub trait DirectoryPublisher {
    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()>;
}

struct FsDirectoryPublisher {
    #[cfg(debug_assertions)]
    fail_rename_at: Option<usize>,
    calls: usize,
}

impl FsDirectoryPublisher {
    fn from_env() -> Self {
        Self {
            #[cfg(debug_assertions)]
            fail_rename_at: std::env::var("RSTIM_TEST_DECODER_DATASET_FAIL_RENAME_AT")
                .ok()
                .and_then(|value| value.parse().ok()),
            calls: 0,
        }
    }
}

impl DirectoryPublisher for FsDirectoryPublisher {
    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        self.calls += 1;
        #[cfg(debug_assertions)]
        if self.fail_rename_at == Some(self.calls) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "injected decoder dataset rename failure",
            ));
        }
        fs::rename(from, to)
    }
}

struct StagedBundle {
    final_path: PathBuf,
    temp_path: PathBuf,
    published: bool,
}

impl StagedBundle {
    fn create(path: &NewDirectoryPath, private: bool) -> Result<Self, String> {
        let pid = std::process::id();
        for retry in 0usize..1000 {
            let temp_path = path.parent.join(format!(
                ".{}.rstim-decoder-dataset-{pid}-{retry}.tmp",
                path.name.to_string_lossy()
            ));
            let created = if private {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    fs::DirBuilder::new().mode(0o700).create(&temp_path)
                }
                #[cfg(not(unix))]
                {
                    fs::create_dir(&temp_path)
                }
            } else {
                fs::create_dir(&temp_path)
            };
            match created {
                Ok(()) => {
                    return Ok(Self {
                        final_path: path.final_path.clone(),
                        temp_path,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "failed to create staging directory {}: {error}",
                        temp_path.display()
                    ));
                }
            }
        }
        Err(format!(
            "failed to create unique staging directory beside {}",
            path.final_path.display()
        ))
    }

    fn publish_with(&mut self, publisher: &mut impl DirectoryPublisher) -> Result<(), String> {
        publisher
            .rename(&self.temp_path, &self.final_path)
            .map_err(|error| {
                format!(
                    "failed to publish bundle {}: {error}",
                    self.final_path.display()
                )
            })?;
        self.published = true;
        Ok(())
    }
}

impl Drop for StagedBundle {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.temp_path);
        }
    }
}

fn bytes_per_shot(bits: usize) -> Result<usize, String> {
    bits.checked_add(7)
        .ok_or_else(|| "b8 row width overflows".to_string())
        .map(|bits| bits / 8)
}

struct Sha256Writer<W> {
    inner: W,
    digest: Sha256,
}

impl<W: Write> Sha256Writer<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            digest: Sha256::new(),
        }
    }

    fn finish(mut self) -> Result<String, String> {
        self.inner
            .flush()
            .map_err(|error| format!("failed to flush dataset artifact: {error}"))?;
        let digest = self.digest.finalize();
        let mut output = String::with_capacity(64);
        for byte in digest {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("write hex into String");
        }
        Ok(output)
    }
}

impl<W: Write> Write for Sha256Writer<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.digest.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn create_hashed_file(path: &Path) -> Result<Sha256Writer<BufWriter<File>>, String> {
    let file = File::create(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    Ok(Sha256Writer::new(BufWriter::new(file)))
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let file = File::create(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(bytes)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    writer
        .flush()
        .map_err(|error| format!("failed to flush {}: {error}", path.display()))
}

fn write_manifest(path: &Path, manifest: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("failed to serialize manifest: {error}"))?;
    bytes.push(b'\n');
    write_file(path, &bytes)
}

/// Exports using the backward-compatible X-only configuration.
pub fn export_decoder_dataset(
    config: ExportDecoderDatasetConfig,
) -> Result<DecoderDatasetSummary, String> {
    let mut publisher = FsDirectoryPublisher::from_env();
    export_decoder_dataset_with_publisher(config, &mut publisher)
}

/// Exports using the generalized X-or-Z logical-flip configuration.
pub fn export_decoder_dataset_with_logical_flip(
    config: ExportDecoderDatasetLogicalFlipConfig,
) -> Result<DecoderDatasetSummary, String> {
    let mut publisher = FsDirectoryPublisher::from_env();
    export_decoder_dataset_with_logical_flip_and_publisher(config, &mut publisher)
}

/// Exports in bounded batches so peak memory does not scale with total shots.
pub fn export_decoder_dataset_with_logical_flip_in_batches(
    config: ExportDecoderDatasetLogicalFlipConfig,
    batch_shots: usize,
) -> Result<DecoderDatasetSummary, String> {
    let mut publisher = FsDirectoryPublisher::from_env();
    export_decoder_dataset_with_logical_flip_and_publisher_in_batches(
        config,
        &mut publisher,
        batch_shots,
    )
}

#[doc(hidden)]
pub fn export_decoder_dataset_with_publisher(
    config: ExportDecoderDatasetConfig,
    publisher: &mut impl DirectoryPublisher,
) -> Result<DecoderDatasetSummary, String> {
    export_decoder_dataset_with_logical_flip_and_publisher(config.into(), publisher)
}

#[doc(hidden)]
pub fn export_decoder_dataset_with_logical_flip_and_publisher(
    config: ExportDecoderDatasetLogicalFlipConfig,
    publisher: &mut impl DirectoryPublisher,
) -> Result<DecoderDatasetSummary, String> {
    export_decoder_dataset_with_logical_flip_and_publisher_in_batches(
        config,
        publisher,
        DEFAULT_DECODER_DATASET_BATCH_SHOTS,
    )
}

fn export_decoder_dataset_with_logical_flip_and_publisher_in_batches(
    config: ExportDecoderDatasetLogicalFlipConfig,
    publisher: &mut impl DirectoryPublisher,
    batch_shots: usize,
) -> Result<DecoderDatasetSummary, String> {
    if batch_shots == 0 {
        return Err("--batch_shots must be positive".to_string());
    }
    let validated_paths = validate_output_directories(&config.public_out, &config.private_out)?;
    let validated = validate_decoder_dataset_logical_flip_inputs(&config)?;
    let public_row_kind = match config.mode {
        DecoderDatasetMode::Detectors => "detectors",
        DecoderDatasetMode::MeasurementsBlinded => "measurements",
    };
    let public_row_bits = match config.mode {
        DecoderDatasetMode::Detectors => validated.detectors,
        DecoderDatasetMode::MeasurementsBlinded => validated.measurements,
    };
    let answers_bits = validated.observables;
    let masks_bits = (config.mode == DecoderDatasetMode::MeasurementsBlinded).then_some(1);
    let circuit_sha256 = sha256_hex(validated.public_circuit_text.as_bytes());

    let mut private_stage = StagedBundle::create(&validated_paths.private, true)?;
    let mut public_stage = StagedBundle::create(&validated_paths.public, false)?;
    let mut shots_writer = create_hashed_file(&public_stage.temp_path.join("shots.b8"))?;
    let mut answers_writer = create_hashed_file(&private_stage.temp_path.join("answers.b8"))?;
    let mut masks_writer = masks_bits
        .map(|_| create_hashed_file(&private_stage.temp_path.join("masks.b8")))
        .transpose()?;
    let mut trace_writer = config
        .error_trace
        .then(|| create_hashed_file(&private_stage.temp_path.join("trace.jsonl")))
        .transpose()?;
    let mut rngs = make_dataset_rngs(config.seed);
    let mut remaining = config.shots;
    let mut shot_offset = 0;
    while remaining > 0 {
        let current_shots = remaining.min(batch_shots);
        let chunk = generate_decoder_dataset_chunk(
            &validated,
            config.mode,
            current_shots,
            shot_offset,
            config.error_trace,
            &mut rngs)?;
        crate::output::write_shots_b8(&chunk.public_shots, &mut shots_writer)
            .map_err(|error| format!("failed to write public shots: {error}"))?;
        crate::output::write_shots_b8(&chunk.answers, &mut answers_writer)
            .map_err(|error| format!("failed to write private answers: {error}"))?;
        match (&chunk.masks, masks_writer.as_mut()) {
            (Some(masks), Some(writer)) => crate::output::write_shots_b8(masks, writer)
                .map_err(|error| format!("failed to write private masks: {error}"))?,
            (None, None) => {}
            _ => return Err("inconsistent blinded mask artifacts".to_string()),
        }
        if let Some(writer) = trace_writer.as_mut() {
            let trace = chunk
                .error_trace
                .as_ref()
                .expect("traced chunk carries trace bytes");
            writer
                .write_all(trace)
                .map_err(|error| format!("failed to write private error trace: {error}"))?;
        }
        remaining -= current_shots;
        shot_offset += current_shots;
    }
    let shots_sha256 = shots_writer.finish()?;
    let answers_sha256 = answers_writer.finish()?;
    let masks_sha256 = masks_writer.map(Sha256Writer::finish).transpose()?;
    let trace_sha256 = trace_writer.map(Sha256Writer::finish).transpose()?;
    let dataset_id = sha256_hex(&dataset_id_material(
        PUBLIC_SCHEMA_VERSION,
        config.mode,
        &circuit_sha256,
        config.shots,
        public_row_bits,
        &shots_sha256,
    ));
    let masks_file = match (masks_sha256, masks_bits) {
        (Some(sha256), Some(bits)) => Some(FileManifest {
            file: "masks.b8",
            sha256,
            bits,
            bytes_per_shot: bytes_per_shot(bits)?,
        }),
        (None, None) => None,
        _ => return Err("inconsistent blinded mask artifacts".to_string()),
    };
    let trace_file = trace_sha256.map(|sha256| TraceFileManifest {
        file: "trace.jsonl",
        sha256,
        schema: ERROR_TRACE_SCHEMA,
        lines: config.shots,
    });
    let public_manifest = PublicManifest {
        format: DATASET_FORMAT,
        schema_version: PUBLIC_SCHEMA_VERSION,
        dataset_id: dataset_id.clone(),
        mode: config.mode,
        shots: config.shots,
        row: PublicRowManifest {
            kind: public_row_kind,
            bits: public_row_bits,
            encoding: "b8",
            bit_order: "lsb_first",
            bytes_per_shot: bytes_per_shot(public_row_bits)?,
        },
        circuit: CircuitManifest {
            file: "circuit.stim",
            sha256: circuit_sha256,
            measurements: validated.measurements,
            detectors: validated.detectors,
            observables: validated.observables,
            sweep_bits: 0,
        },
        shots_file: FileManifest {
            file: "shots.b8",
            sha256: shots_sha256,
            bits: public_row_bits,
            bytes_per_shot: bytes_per_shot(public_row_bits)?,
        },
    };
    let private_manifest = PrivateManifest {
        format: DATASET_FORMAT,
        schema_version: PUBLIC_SCHEMA_VERSION,
        dataset_id: dataset_id.clone(),
        mode: config.mode,
        shots: config.shots,
        answers_file: FileManifest {
            file: "answers.b8",
            sha256: answers_sha256,
            bits: answers_bits,
            bytes_per_shot: bytes_per_shot(answers_bits)?,
        },
        masks_file,
        trace_file,
        generation: PrivateGenerationManifest {
            rstim_version: crate::version(),
            batch_shots,
            seed: config.seed,
        },
    };

    write_manifest(
        &private_stage.temp_path.join("manifest.json"),
        &private_manifest)?;
    write_manifest(
        &public_stage.temp_path.join("manifest.json"),
        &public_manifest)?;
    write_file(
        &public_stage.temp_path.join("circuit.stim"),
        validated.public_circuit_text.as_bytes(),
    )?;
    private_stage.publish_with(publisher)?;
    match public_stage.publish_with(publisher) {
        Ok(()) => Ok(DecoderDatasetSummary {
            dataset_id,
            mode: config.mode,
            shots: config.shots,
            row_bits: public_row_bits,
            public_out: config.public_out,
            private_out: config.private_out,
        }),
        Err(error) => Err(format!(
            "{error}; private bundle retained at {}",
            private_stage.final_path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::bit_table::BitTable;

    fn test_config(
        circuit_text: &str,
        mode: DecoderDatasetMode,
        logical_flip: Option<LogicalFlip>,
    ) -> ExportDecoderDatasetLogicalFlipConfig {
        ExportDecoderDatasetLogicalFlipConfig {
            circuit_text: circuit_text.to_string(),
            shots: 1,
            mode,
            logical_flip,
            public_out: std::path::PathBuf::from("public-unused"),
            private_out: std::path::PathBuf::from("private-unused"),
            seed: Some(1),
            error_trace: false,
        }
    }

    fn logical_x(qubits: Vec<u32>) -> Option<LogicalFlip> {
        Some(LogicalFlip {
            pauli: LogicalPauli::X,
            qubits,
        })
    }

    fn exec_output_with_observables(observables: Vec<(u32, bool)>) -> crate::executor::ExecOutput {
        crate::executor::ExecOutput {
            measurements: Vec::new(),
            detectors: Vec::new(),
            detector_coords: Vec::new(),
            observables,
            observable_events: Vec::new(),
            inapplicable_noise_events: Vec::new(),
            qubit_coords: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn observable_flips_rejects_out_of_range_index() {
        let output = exec_output_with_observables(vec![(1, false)]);
        let error = observable_flips(&output, 1, "detectors").unwrap_err();
        assert!(
            error.contains("produced observable 1"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn observable_flips_rejects_missing_observable() {
        let output = exec_output_with_observables(vec![(0, true)]);
        let error = observable_flips(&output, 2, "detectors").unwrap_err();
        assert!(
            error.contains("no observable 1"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_logical_flip_rejects_empty_duplicate_and_bad_tokens() {
        assert_eq!(
            LogicalFlip::parse(LogicalPauli::Z, "0,2,4").unwrap(),
            LogicalFlip {
                pauli: LogicalPauli::Z,
                qubits: vec![0, 2, 4],
            }
        );
        assert!(
            LogicalFlip::parse(LogicalPauli::Z, "")
                .unwrap_err()
                .contains("--logical_z_qubits must be non-empty")
        );
        assert!(
            LogicalFlip::parse(LogicalPauli::X, "0,2,2")
                .unwrap_err()
                .contains("--logical_x_qubits contains duplicate")
        );
        assert!(
            LogicalFlip::parse(LogicalPauli::Z, "0,nope")
                .unwrap_err()
                .contains("--logical_z_qubits contains invalid")
        );
        assert_eq!(parse_logical_x_qubits("1,3").unwrap(), vec![1, 3]);
    }

    #[test]
    fn marker_must_be_a_unique_top_level_tagged_tick() {
        let good = "R 0\nTICK[rstim:logical_flip_point]\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let logical_z = LogicalFlip {
            pauli: LogicalPauli::Z,
            qubits: vec![0],
        };
        assert!(
            circuit_with_injected_logical_flip(good, &logical_z)
                .unwrap()
                .contains("\nZ 0\n")
        );

        let marker_without_trailing_newline = "R 0\nTICK[rstim:logical_flip_point]";
        assert_eq!(
            circuit_with_injected_logical_flip(marker_without_trailing_newline, &logical_z)
                .unwrap(),
            "R 0\nTICK[rstim:logical_flip_point]\nZ 0\n"
        );

        let missing = "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(
            circuit_with_injected_logical_x(missing, &[0])
                .unwrap_err()
                .contains("marker")
        );

        let duplicate = "R 0\nTICK[rstim:logical_flip_point]\nTICK[rstim:logical_flip_point]\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(
            circuit_with_injected_logical_x(duplicate, &[0])
                .unwrap_err()
                .contains("exactly once")
        );

        let nested =
            "R 0\nREPEAT 2 {\nTICK[rstim:logical_flip_point]\nM 0\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(
            circuit_with_injected_logical_x(nested, &[0])
                .unwrap_err()
                .contains("top-level")
        );

        let legacy_comment =
            "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(
            circuit_with_injected_logical_x(legacy_comment, &[0])
                .unwrap_err()
                .contains("exactly once")
        );

        let wrong_instruction =
            "R 0\nH[rstim:logical_flip_point] 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(
            circuit_with_injected_logical_x(wrong_instruction, &[0])
                .unwrap_err()
                .contains("must annotate TICK")
        );
    }

    #[test]
    fn logical_validation_requires_observable_flip_without_detector_change() {
        let valid = "R 0\nTICK[rstim:logical_flip_point]\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let config = test_config(
            valid,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        assert!(validate_decoder_dataset_logical_flip_inputs(&config).is_ok());

        let no_flip = "R 0\nTICK[rstim:logical_flip_point]\nR 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let config = test_config(
            no_flip,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        assert!(
            validate_decoder_dataset_logical_flip_inputs(&config)
                .unwrap_err()
                .contains("injected logical X does not flip observable 0")
        );

        let changes_detector = "R 0 1\nTICK[rstim:logical_flip_point]\nM 0 1\nDETECTOR rec[-2] rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\n";
        let config = test_config(
            changes_detector,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        assert!(
            validate_decoder_dataset_logical_flip_inputs(&config)
                .unwrap_err()
                .contains("changes detector")
        );
    }

    #[test]
    fn input_validation_rejects_observable_sweep_and_qubit_contract_violations() {
        let no_observable = "R 0\nM 0\n";
        let config = test_config(no_observable, DecoderDatasetMode::Detectors, None);
        assert!(
            validate_decoder_dataset_logical_flip_inputs(&config)
                .unwrap_err()
                .contains("at least one observable, found 0")
        );

        let multiple_observables =
            "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(1) rec[-1]\n";
        let config = test_config(multiple_observables, DecoderDatasetMode::Detectors, None);
        assert!(validate_decoder_dataset_logical_flip_inputs(&config).is_ok());

        let config = test_config(
            multiple_observables,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        assert!(
            validate_decoder_dataset_logical_flip_inputs(&config)
                .unwrap_err()
                .contains("measurements_blinded mode requires exactly one observable, found 2")
        );

        let sweep_bit = "R 0\nCX sweep[0] 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let config = test_config(sweep_bit, DecoderDatasetMode::Detectors, None);
        assert!(
            validate_decoder_dataset_logical_flip_inputs(&config)
                .unwrap_err()
                .contains("does not support sweep-bit circuits")
        );

        let one_qubit = "R 0\nTICK[rstim:logical_flip_point]\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let config = test_config(
            one_qubit,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![1]),
        );
        assert!(
            validate_decoder_dataset_logical_flip_inputs(&config)
                .unwrap_err()
                .contains("contains qubit 1, but circuit has 1 qubits")
        );

        let config = test_config(
            one_qubit,
            DecoderDatasetMode::MeasurementsBlinded,
            Some(LogicalFlip {
                pauli: LogicalPauli::Z,
                qubits: Vec::new(),
            }),
        );
        assert_eq!(
            validate_decoder_dataset_logical_flip_inputs(&config).unwrap_err(),
            "--logical_z_qubits must be non-empty"
        );

        let config = test_config(
            one_qubit,
            DecoderDatasetMode::MeasurementsBlinded,
            Some(LogicalFlip {
                pauli: LogicalPauli::X,
                qubits: vec![0, 0],
            }),
        );
        assert_eq!(
            validate_decoder_dataset_logical_flip_inputs(&config).unwrap_err(),
            "--logical_x_qubits contains duplicate qubit index 0"
        );
    }

    #[test]
    fn blinded_validation_requires_marker_before_first_positive_probability_noise() {
        let unsafe_circuit = "R 0 1\nLOSS(0.1) 0\nTICK[rstim:logical_flip_point]\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-2]\n";
        let config = test_config(
            unsafe_circuit,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        let error = validate_decoder_dataset_logical_flip_inputs(&config).unwrap_err();
        assert!(error.contains("positive-probability noise"), "{error}");
        assert!(error.contains("LOSS"), "{error}");
        assert!(error.contains(LOGICAL_FLIP_MARKER), "{error}");

        let loss_in_repeat = "R 0 1\nREPEAT 2 {\n  LOSS(0.1) 0\n}\nTICK[rstim:logical_flip_point]\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-2]\n";
        let config = test_config(
            loss_in_repeat,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        let error = validate_decoder_dataset_logical_flip_inputs(&config).unwrap_err();
        assert!(error.contains("LOSS"), "{error}");

        let zero_probability_loss =
            "R 0 1\nLOSS(0) 0\nTICK[rstim:logical_flip_point]\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-2]\n";
        let config = test_config(
            zero_probability_loss,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        assert!(validate_decoder_dataset_logical_flip_inputs(&config).is_ok());

        let loss_off_support = "R 0 1\nLOSS(0.1) 1\nTICK[rstim:logical_flip_point]\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-2]\n";
        let config = test_config(
            loss_off_support,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        let error = validate_decoder_dataset_logical_flip_inputs(&config).unwrap_err();
        assert!(error.contains("LOSS"), "{error}");

        for (name, circuit) in [
            (
                "Pauli error",
                "R 0\nX_ERROR(0.1) 0\nTICK[rstim:logical_flip_point]\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
            ),
            (
                "noisy measurement",
                "R 0\nM(0.1) 0\nTICK[rstim:logical_flip_point]\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
            ),
        ] {
            let config = test_config(
                circuit,
                DecoderDatasetMode::MeasurementsBlinded,
                logical_x(vec![0]),
            );
            let error = validate_decoder_dataset_logical_flip_inputs(&config).unwrap_err();
            assert!(error.contains("positive-probability noise"), "{name}: {error}");
        }

        let zero_probability_error = "R 0\nX_ERROR(0) 0\nTICK[rstim:logical_flip_point]\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let config = test_config(
            zero_probability_error,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        assert!(validate_decoder_dataset_logical_flip_inputs(&config).is_ok());

        let noise_after_marker = "R 0\nTICK[rstim:logical_flip_point]\nX_ERROR(0.1) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let config = test_config(
            noise_after_marker,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        assert!(validate_decoder_dataset_logical_flip_inputs(&config).is_ok());
    }

    #[test]
    fn deterministic_midswap_blinded_rows_cover_both_hidden_flip_paths() {
        let circuit = crate::codegen::rotated_memory_z_midswap(crate::codegen::MidSwapConfig {
            distance: 3,
            rounds: 1,
            before_round_data_depolarization: 0.0,
            before_round_data_loss_probability: 0.0,
            after_clifford_depolarization: 0.0,
            before_measure_flip_probability: 0.0,
            after_reset_flip_probability: 0.0,
            operation_loss_probability: 0.0,
            measurement_loss_probability: 0.0,
        })
        .unwrap();
        let mut config = test_config(
            &circuit,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![1, 8, 15]),
        );
        config.shots = 16;
        config.seed = Some(0x629);

        let artifacts = generate_decoder_dataset_artifacts_with_logical_flip(&config).unwrap();
        let masks = artifacts.masks.as_ref().unwrap();
        assert_eq!(artifacts.measurements, 34);
        assert_eq!(artifacts.public_shots.num_major(), 34);
        assert_eq!(artifacts.public_shots.num_minor(), 16);
        assert_eq!(artifacts.answers.num_major(), 1);
        assert_eq!(masks.num_major(), 1);
        assert!((0..config.shots).any(|shot| !masks.get(0, shot)));
        assert!((0..config.shots).any(|shot| masks.get(0, shot)));

        let public_interpretation = crate::m2d::measurements_to_detections(
            &artifacts.public_instrs,
            &artifacts.public_shots,
        )
        .unwrap();
        for shot in 0..config.shots {
            for loss_flag in (0..artifacts.measurements).step_by(2) {
                assert!(
                    !artifacts.public_shots.get(loss_flag, shot),
                    "loss flag {loss_flag}, shot {shot}"
                );
            }
            assert_eq!(
                artifacts.answers.get(0, shot),
                public_interpretation.observable_flips.get(0, shot) ^ masks.get(0, shot),
                "shot {shot}"
            );
        }
    }

    #[test]
    fn b8_bytes_are_lsb_first_and_zero_padded() {
        let mut table = BitTable::new(10, 2);
        table.set(0, 0, true);
        table.set(7, 0, true);
        table.set(9, 0, true);
        table.set(1, 1, true);
        table.set(8, 1, true);

        assert_eq!(
            bit_table_to_b8_bytes(&table).unwrap(),
            vec![0b1000_0001, 0b0000_0010, 0b0000_0010, 0b0000_0001]
        );
    }

    #[test]
    fn dataset_id_uses_only_public_material() {
        let left = dataset_id_material(
            1,
            DecoderDatasetMode::Detectors,
            "circuit-a",
            3,
            5,
            "shots-a",
        );
        let right = dataset_id_material(
            1,
            DecoderDatasetMode::Detectors,
            "circuit-a",
            3,
            5,
            "shots-a",
        );
        let changed_seed_would_not_be_an_argument = dataset_id_material(
            1,
            DecoderDatasetMode::Detectors,
            "circuit-a",
            3,
            5,
            "shots-b",
        );

        assert_eq!(left, right);
        assert_ne!(left, changed_seed_would_not_be_an_argument);
        assert!(!String::from_utf8(left).unwrap().contains("seed"));
    }

    #[test]
    fn detector_artifacts_publish_detections_and_private_answers() {
        let circuit = "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let config = test_config(circuit, DecoderDatasetMode::Detectors, None);
        let artifacts = generate_decoder_dataset_artifacts_with_logical_flip(&config).unwrap();

        assert_eq!(artifacts.public_row_kind, "detectors");
        assert_eq!(artifacts.public_shots.num_major(), 1);
        assert_eq!(artifacts.answers.num_major(), 1);
        assert!(artifacts.public_shots.get(0, 0));
        assert!(artifacts.answers.get(0, 0));
        assert!(artifacts.masks.is_none());
    }

    #[test]
    fn blinded_measurement_answers_are_public_observable_xor_mask() {
        let circuit =
            "R 0\nTICK[rstim:logical_flip_point]\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let mut config = test_config(
            circuit,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        config.shots = 16;
        config.seed = Some(0xdec0_de01);

        let artifacts = generate_decoder_dataset_artifacts_with_logical_flip(&config).unwrap();
        let public_interpretation = crate::m2d::measurements_to_detections(
            &artifacts.public_instrs,
            &artifacts.public_shots,
        )
        .unwrap();
        let masks = artifacts.masks.as_ref().unwrap();

        let mut saw_zero = false;
        let mut saw_one = false;
        for shot in 0..config.shots {
            let recomputed =
                public_interpretation.observable_flips.get(0, shot) ^ masks.get(0, shot);
            assert_eq!(artifacts.answers.get(0, shot), recomputed);
            saw_zero |= !masks.get(0, shot);
            saw_one |= masks.get(0, shot);
        }
        assert!(saw_zero);
        assert!(saw_one);
    }

    #[test]
    fn fixed_seed_reproduces_artifacts_byte_for_byte() {
        let circuit =
            "R 0\nTICK[rstim:logical_flip_point]\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let mut config = test_config(
            circuit,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        config.shots = 32;
        config.seed = Some(123);

        let a = generate_decoder_dataset_artifacts_with_logical_flip(&config).unwrap();
        let b = generate_decoder_dataset_artifacts_with_logical_flip(&config).unwrap();
        assert_eq!(
            bit_table_to_b8_bytes(&a.public_shots).unwrap(),
            bit_table_to_b8_bytes(&b.public_shots).unwrap()
        );
        assert_eq!(
            bit_table_to_b8_bytes(&a.answers).unwrap(),
            bit_table_to_b8_bytes(&b.answers).unwrap()
        );
        assert_eq!(
            bit_table_to_b8_bytes(a.masks.as_ref().unwrap()).unwrap(),
            bit_table_to_b8_bytes(b.masks.as_ref().unwrap()).unwrap()
        );
    }

    #[test]
    fn export_writes_exact_public_and_private_files() {
        let root = tempfile::tempdir().unwrap();
        let public_out = root.path().join("public");
        let private_out = root.path().join("private");
        let circuit =
            "R 0\nTICK[rstim:logical_flip_point]\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let mut config = test_config(
            circuit,
            DecoderDatasetMode::MeasurementsBlinded,
            logical_x(vec![0]),
        );
        config.shots = 8;
        config.seed = Some(7);
        config.public_out = public_out.clone();
        config.private_out = private_out.clone();

        let summary = export_decoder_dataset_with_logical_flip(config).unwrap();
        assert_eq!(summary.public_out, public_out);
        assert_eq!(
            sorted_entries(&public_out),
            vec!["circuit.stim", "manifest.json", "shots.b8"]
        );
        assert_eq!(
            sorted_entries(&private_out),
            vec!["answers.b8", "manifest.json", "masks.b8"]
        );

        let public_manifest = std::fs::read_to_string(public_out.join("manifest.json")).unwrap();
        assert_no_public_secret_words(&public_manifest);
        assert!(public_manifest.contains("\"mode\": \"measurements_blinded\""));
        assert!(
            std::fs::read_to_string(public_out.join("circuit.stim"))
                .unwrap()
                .contains(LOGICAL_FLIP_MARKER)
        );
    }

    #[test]
    fn batched_export_streams_cross_batch_files_and_hashes() {
        let root = tempfile::tempdir().unwrap();
        let public_out = root.path().join("public");
        let private_out = root.path().join("private");
        let circuit = concat!(
            "R 0\nX_ERROR(0.5) 0\nM 0\nDETECTOR rec[-1]\n",
            "OBSERVABLE_INCLUDE(0) rec[-1]\n",
            "OBSERVABLE_INCLUDE(1) rec[-1]\n",
        );
        let mut config = test_config(circuit, DecoderDatasetMode::Detectors, None);
        config.shots = 5;
        config.seed = Some(19);
        config.public_out = public_out.clone();
        config.private_out = private_out.clone();

        export_decoder_dataset_with_logical_flip_in_batches(config, 2).unwrap();

        let shots = std::fs::read(public_out.join("shots.b8")).unwrap();
        let answers = std::fs::read(private_out.join("answers.b8")).unwrap();
        assert_eq!(shots.len(), 5);
        assert_eq!(answers.len(), 5);

        let public_manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(public_out.join("manifest.json")).unwrap())
                .unwrap();
        let private_manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(private_out.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(public_manifest["shots"], 5);
        assert_eq!(private_manifest["answers_file"]["bits"], 2);
        assert_eq!(private_manifest["generation"]["batch_shots"], 2);
        assert_eq!(public_manifest["shots_file"]["sha256"], sha256_hex(&shots));
        assert_eq!(
            private_manifest["answers_file"]["sha256"],
            sha256_hex(&answers)
        );
    }

    #[test]
    fn batched_export_rejects_zero_batch_before_creating_outputs() {
        let root = tempfile::tempdir().unwrap();
        let public_out = root.path().join("public");
        let private_out = root.path().join("private");
        let mut config = test_config(
            "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
            DecoderDatasetMode::Detectors,
            None,
        );
        config.public_out = public_out.clone();
        config.private_out = private_out.clone();

        let error = export_decoder_dataset_with_logical_flip_in_batches(config, 0).unwrap_err();

        assert_eq!(error, "--batch_shots must be positive");
        assert!(!public_out.exists());
        assert!(!private_out.exists());
    }

    #[test]
    fn public_directory_is_not_visible_when_public_rename_fails() {
        let root = tempfile::tempdir().unwrap();
        let public_out = root.path().join("public");
        let private_out = root.path().join("private");
        let circuit = "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let mut config = test_config(circuit, DecoderDatasetMode::Detectors, None);
        config.public_out = public_out.clone();
        config.private_out = private_out.clone();
        config.seed = Some(3);

        let mut publisher = FailingDirectoryPublisher::new(2);
        let err = export_decoder_dataset_with_logical_flip_and_publisher(config, &mut publisher)
            .unwrap_err();

        assert!(err.contains("private bundle retained"));
        assert!(private_out.exists());
        assert!(!public_out.exists());
        assert_no_decoder_dataset_temps(root.path());
    }

    #[test]
    fn released_x_only_validation_and_publisher_wrappers_delegate() {
        let root = tempfile::tempdir().unwrap();
        let public_out = root.path().join("public");
        let private_out = root.path().join("private");
        let config = ExportDecoderDatasetConfig {
            circuit_text: "R 0\nTICK[rstim:logical_flip_point]\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n"
                .to_string(),
            shots: 1,
            mode: DecoderDatasetMode::MeasurementsBlinded,
            logical_x_qubits: vec![0],
            public_out: public_out.clone(),
            private_out: private_out.clone(),
            seed: Some(3),
        };

        assert!(validate_decoder_dataset_inputs(&config).is_ok());
        let mut publisher = FailingDirectoryPublisher::new(2);
        let error = export_decoder_dataset_with_publisher(config, &mut publisher).unwrap_err();

        assert!(error.contains("private bundle retained"));
        assert!(private_out.exists());
        assert!(!public_out.exists());
        assert_no_decoder_dataset_temps(root.path());
    }

    fn sorted_entries(path: &std::path::Path) -> Vec<String> {
        let mut entries: Vec<String> = std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        entries
    }

    fn assert_no_public_secret_words(text: &str) {
        for forbidden in [
            "seed",
            "mask",
            "answer",
            "private",
            "producer",
            "permutation",
        ] {
            assert!(
                !text.to_ascii_lowercase().contains(forbidden),
                "public manifest leaked {forbidden}: {text}"
            );
        }
    }

    fn assert_no_decoder_dataset_temps(path: &std::path::Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            assert!(
                !name.contains(".rstim-decoder-dataset-"),
                "temporary directory leaked: {name}"
            );
        }
    }

    struct FailingDirectoryPublisher {
        fail_at: usize,
        calls: usize,
    }

    impl FailingDirectoryPublisher {
        fn new(fail_at: usize) -> Self {
            Self { fail_at, calls: 0 }
        }
    }

    impl DirectoryPublisher for FailingDirectoryPublisher {
        fn rename(&mut self, from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
            self.calls += 1;
            if self.calls == self.fail_at {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "injected decoder dataset rename failure",
                ));
            }
            std::fs::rename(from, to)
        }
    }
}
