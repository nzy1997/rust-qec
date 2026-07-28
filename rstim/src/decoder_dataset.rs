#![allow(dead_code)] // Manifest types and schema constants are used by later exporter stages.

use crate::sim::bit_table::BitTable;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub const LOGICAL_FLIP_MARKER: &str = "# RSTIM_LOGICAL_FLIP_POINT";
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

#[derive(Debug, Clone)]
pub struct ExportDecoderDatasetConfig {
    pub circuit_text: String,
    pub shots: usize,
    pub mode: DecoderDatasetMode,
    pub logical_x_qubits: Vec<u32>,
    pub public_out: PathBuf,
    pub private_out: PathBuf,
    pub seed: Option<u64>,
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

#[doc(hidden)]
pub fn parse_logical_x_qubits(value: &str) -> Result<Vec<u32>, String> {
    if value.trim().is_empty() {
        return Err("--logical_x_qubits must be non-empty".to_string());
    }

    let mut seen = BTreeSet::new();
    let mut qubits = Vec::new();
    for token in value.split(',') {
        let token = token.trim();
        let qubit = token
            .parse::<u32>()
            .map_err(|_| format!("--logical_x_qubits contains invalid qubit index {token:?}"))?;
        if !seen.insert(qubit) {
            return Err(format!(
                "--logical_x_qubits contains duplicate qubit index {qubit}"
            ));
        }
        qubits.push(qubit);
    }
    Ok(qubits)
}

fn marker_depth_before_line(line: &str, current_depth: usize) -> usize {
    let code = line.split('#').next().unwrap_or("").trim();
    if code == "}" {
        current_depth.saturating_sub(1)
    } else {
        current_depth
    }
}

fn marker_depth_after_line(line: &str, current_depth: usize) -> usize {
    let code = line.split('#').next().unwrap_or("").trim();
    if code.ends_with('{') {
        current_depth + 1
    } else if code == "}" {
        current_depth.saturating_sub(1)
    } else {
        current_depth
    }
}

#[doc(hidden)]
pub fn circuit_with_injected_logical_x(
    circuit_text: &str,
    logical_x_qubits: &[u32],
) -> Result<String, String> {
    let mut marker_count = 0;
    let mut marker_at_top_level = false;
    let mut depth = 0;
    for line in circuit_text.lines() {
        let depth_before = marker_depth_before_line(line, depth);
        if line.contains(LOGICAL_FLIP_MARKER) {
            if line.trim() != LOGICAL_FLIP_MARKER {
                return Err("logical flip marker must be standalone".to_string());
            }
            marker_count += 1;
            marker_at_top_level = depth_before == 0;
        }
        depth = marker_depth_after_line(line, depth);
    }

    if marker_count != 1 {
        return Err("logical flip marker must appear exactly once".to_string());
    }
    if !marker_at_top_level {
        return Err("logical flip marker must be top-level".to_string());
    }

    let injected = format!(
        "X {}\n",
        logical_x_qubits
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mut output = String::with_capacity(circuit_text.len() + injected.len());
    for line in circuit_text.split_inclusive('\n') {
        output.push_str(line);
        if line.trim() == LOGICAL_FLIP_MARKER {
            if !line.ends_with('\n') {
                output.push('\n');
            }
            output.push_str(&injected);
        }
    }
    Ok(output)
}

#[derive(Debug)]
struct ValidatedDecoderDatasetInput {
    public_circuit_text: String,
    public_instrs: Vec<crate::ir::StimInstr>,
    private_one_circuit_text: Option<String>,
    private_one_instrs: Option<Vec<crate::ir::StimInstr>>,
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

fn validate_logical_x_effect(
    public_instrs: &[crate::ir::StimInstr],
    private_instrs: &[crate::ir::StimInstr],
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
            return Err("injected logical X changes detector reference values".to_string());
        }
    }
    let flips = out0.observable_flips.get(0, 0) ^ out1.observable_flips.get(0, 0);
    if !flips {
        return Err("injected logical X does not flip observable 0".to_string());
    }
    Ok(())
}

#[doc(hidden)]
#[allow(private_interfaces)]
pub fn validate_decoder_dataset_inputs(
    config: &ExportDecoderDatasetConfig,
) -> Result<ValidatedDecoderDatasetInput, String> {
    if config.shots == 0 {
        return Err("--shots must be positive".to_string());
    }
    let public_instrs = crate::parser::parse_lines(&config.circuit_text)?;
    let stats = crate::stats::summarize(&public_instrs);
    if stats.num_observables != 1 {
        return Err(format!(
            "export_decoder_dataset requires exactly one observable, found {}",
            stats.num_observables
        ));
    }
    if stats.num_sweep_bits != 0 {
        return Err("export_decoder_dataset does not support sweep-bit circuits".to_string());
    }
    match config.mode {
        DecoderDatasetMode::Detectors if !config.logical_x_qubits.is_empty() => {
            return Err("detectors mode rejects --logical_x_qubits".to_string());
        }
        DecoderDatasetMode::MeasurementsBlinded if config.logical_x_qubits.is_empty() => {
            return Err("measurements_blinded mode requires --logical_x_qubits".to_string());
        }
        _ => {}
    }

    let (private_one_circuit_text, private_one_instrs) = match config.mode {
        DecoderDatasetMode::Detectors => (None, None),
        DecoderDatasetMode::MeasurementsBlinded => {
            for &qubit in &config.logical_x_qubits {
                if qubit as usize >= stats.num_qubits {
                    return Err(format!(
                        "--logical_x_qubits contains qubit {qubit}, but circuit has {} qubits",
                        stats.num_qubits
                    ));
                }
            }
            let circuit_text =
                circuit_with_injected_logical_x(&config.circuit_text, &config.logical_x_qubits)?;
            let instrs = crate::parser::parse_lines(&circuit_text)?;
            validate_logical_x_effect(&public_instrs, &instrs)?;
            (Some(circuit_text), Some(instrs))
        }
    };

    Ok(ValidatedDecoderDatasetInput {
        public_circuit_text: config.circuit_text.clone(),
        public_instrs,
        private_one_circuit_text,
        private_one_instrs,
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
    pub measurements: usize,
    pub detectors: usize,
    pub observables: usize,
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

#[doc(hidden)]
pub fn generate_decoder_dataset_artifacts(
    config: &ExportDecoderDatasetConfig,
) -> Result<DecoderDatasetArtifacts, String> {
    let validated = validate_decoder_dataset_inputs(config)?;
    let mut rngs = make_dataset_rngs(config.seed);
    match config.mode {
        DecoderDatasetMode::Detectors => {
            let result = crate::sampler::sample_batch_with_options(
                &validated.public_instrs,
                config.shots,
                &mut rngs.physical,
                crate::sampler::SampleOptions {
                    output_mode: crate::sampler::SampleOutputMode::Full,
                    ..crate::sampler::SampleOptions::default()
                },
            )?;
            Ok(DecoderDatasetArtifacts {
                public_circuit_text: validated.public_circuit_text,
                public_instrs: validated.public_instrs,
                public_row_kind: "detectors",
                public_shots: result.detections,
                answers: result.observable_flips,
                masks: None,
                measurements: validated.measurements,
                detectors: validated.detectors,
                observables: validated.observables,
            })
        }
        DecoderDatasetMode::MeasurementsBlinded => {
            let mut source_labels: Vec<bool> =
                (0..config.shots).map(|_| rngs.mask.r#gen()).collect();
            for index in (1..source_labels.len()).rev() {
                let replacement = rngs.permutation.gen_range(0..=index);
                source_labels.swap(index, replacement);
            }

            let zero_count = source_labels.iter().filter(|&&label| !label).count();
            let one_count = config.shots - zero_count;
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

            let mut measurements = BitTable::try_new(validated.measurements, config.shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            let mut masks = BitTable::try_new(1, config.shots)
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
            let mut answers = BitTable::try_new(1, config.shots)
                .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
            for shot in 0..config.shots {
                let public_observable = public_interpretation.observable_flips.get(0, shot);
                let mask_bit = masks.get(0, shot);
                answers.set(0, shot, public_observable ^ mask_bit);
            }

            Ok(DecoderDatasetArtifacts {
                public_circuit_text: validated.public_circuit_text,
                public_instrs: validated.public_instrs,
                public_row_kind: "measurements",
                public_shots: measurements,
                answers,
                masks: Some(masks),
                measurements: validated.measurements,
                detectors: validated.detectors,
                observables: validated.observables,
            })
        }
    }
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
    generation: PrivateGenerationManifest,
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
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
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

fn write_private_bundle(
    path: &Path,
    manifest: &PrivateManifest,
    answers: &[u8],
    masks: Option<&[u8]>,
) -> Result<(), String> {
    write_manifest(&path.join("manifest.json"), manifest)?;
    write_file(&path.join("answers.b8"), answers)?;
    if let Some(masks) = masks {
        write_file(&path.join("masks.b8"), masks)?;
    }
    Ok(())
}

fn write_public_bundle(
    path: &Path,
    manifest: &PublicManifest,
    circuit: &[u8],
    shots: &[u8],
) -> Result<(), String> {
    write_manifest(&path.join("manifest.json"), manifest)?;
    write_file(&path.join("circuit.stim"), circuit)?;
    write_file(&path.join("shots.b8"), shots)
}

pub fn export_decoder_dataset(
    config: ExportDecoderDatasetConfig,
) -> Result<DecoderDatasetSummary, String> {
    let mut publisher = FsDirectoryPublisher::from_env();
    export_decoder_dataset_with_publisher(config, &mut publisher)
}

#[doc(hidden)]
pub fn export_decoder_dataset_with_publisher(
    config: ExportDecoderDatasetConfig,
    publisher: &mut impl DirectoryPublisher,
) -> Result<DecoderDatasetSummary, String> {
    let validated_paths = validate_output_directories(&config.public_out, &config.private_out)?;
    let artifacts = generate_decoder_dataset_artifacts(&config)?;
    let public_shots_bytes = bit_table_to_b8_bytes(&artifacts.public_shots)?;
    let answers_bytes = bit_table_to_b8_bytes(&artifacts.answers)?;
    let masks_bytes = artifacts
        .masks
        .as_ref()
        .map(bit_table_to_b8_bytes)
        .transpose()?;
    let circuit_sha256 = sha256_hex(artifacts.public_circuit_text.as_bytes());
    let shots_sha256 = sha256_hex(&public_shots_bytes);
    let dataset_id = sha256_hex(&dataset_id_material(
        PUBLIC_SCHEMA_VERSION,
        config.mode,
        &circuit_sha256,
        config.shots,
        artifacts.public_shots.num_major(),
        &shots_sha256,
    ));
    let public_row_bits = artifacts.public_shots.num_major();
    let answers_bits = artifacts.answers.num_major();
    let masks_bits = artifacts.masks.as_ref().map(BitTable::num_major);
    let masks_file = match (&masks_bytes, masks_bits) {
        (Some(bytes), Some(bits)) => Some(FileManifest {
            file: "masks.b8",
            sha256: sha256_hex(bytes),
            bits,
            bytes_per_shot: bytes_per_shot(bits)?,
        }),
        (None, None) => None,
        _ => return Err("inconsistent blinded mask artifacts".to_string()),
    };
    let public_manifest = PublicManifest {
        format: DATASET_FORMAT,
        schema_version: PUBLIC_SCHEMA_VERSION,
        dataset_id: dataset_id.clone(),
        mode: config.mode,
        shots: config.shots,
        row: PublicRowManifest {
            kind: artifacts.public_row_kind,
            bits: public_row_bits,
            encoding: "b8",
            bit_order: "lsb_first",
            bytes_per_shot: bytes_per_shot(public_row_bits)?,
        },
        circuit: CircuitManifest {
            file: "circuit.stim",
            sha256: circuit_sha256,
            measurements: artifacts.measurements,
            detectors: artifacts.detectors,
            observables: artifacts.observables,
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
            sha256: sha256_hex(&answers_bytes),
            bits: answers_bits,
            bytes_per_shot: bytes_per_shot(answers_bits)?,
        },
        masks_file,
        generation: PrivateGenerationManifest {
            rstim_version: crate::version(),
            seed: config.seed,
        },
    };

    let mut private_stage = StagedBundle::create(&validated_paths.private, true)?;
    let mut public_stage = StagedBundle::create(&validated_paths.public, false)?;
    write_private_bundle(
        &private_stage.temp_path,
        &private_manifest,
        &answers_bytes,
        masks_bytes.as_deref(),
    )?;
    write_public_bundle(
        &public_stage.temp_path,
        &public_manifest,
        artifacts.public_circuit_text.as_bytes(),
        &public_shots_bytes,
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
        logical_x_qubits: Vec<u32>,
    ) -> ExportDecoderDatasetConfig {
        ExportDecoderDatasetConfig {
            circuit_text: circuit_text.to_string(),
            shots: 1,
            mode,
            logical_x_qubits,
            public_out: std::path::PathBuf::from("public-unused"),
            private_out: std::path::PathBuf::from("private-unused"),
            seed: Some(1),
        }
    }

    #[test]
    fn parse_logical_x_qubits_rejects_empty_duplicate_and_bad_tokens() {
        assert_eq!(parse_logical_x_qubits("0,2,4").unwrap(), vec![0, 2, 4]);
        assert!(parse_logical_x_qubits("")
            .unwrap_err()
            .contains("non-empty"));
        assert!(parse_logical_x_qubits("0,2,2")
            .unwrap_err()
            .contains("duplicate"));
        assert!(parse_logical_x_qubits("0,nope")
            .unwrap_err()
            .contains("invalid"));
    }

    #[test]
    fn marker_must_be_unique_standalone_and_top_level() {
        let good = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(circuit_with_injected_logical_x(good, &[0])
            .unwrap()
            .contains("\nX 0\n"));

        let marker_without_trailing_newline = "R 0\n# RSTIM_LOGICAL_FLIP_POINT";
        assert_eq!(
            circuit_with_injected_logical_x(marker_without_trailing_newline, &[0]).unwrap(),
            "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX 0\n"
        );

        let missing = "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(circuit_with_injected_logical_x(missing, &[0])
            .unwrap_err()
            .contains("marker"));

        let duplicate = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(circuit_with_injected_logical_x(duplicate, &[0])
            .unwrap_err()
            .contains("exactly once"));

        let nested =
            "R 0\nREPEAT 2 {\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(circuit_with_injected_logical_x(nested, &[0])
            .unwrap_err()
            .contains("top-level"));

        let inline = "R 0 # RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        assert!(circuit_with_injected_logical_x(inline, &[0])
            .unwrap_err()
            .contains("standalone"));
    }

    #[test]
    fn logical_validation_requires_observable_flip_without_detector_change() {
        let valid = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let config = test_config(valid, DecoderDatasetMode::MeasurementsBlinded, vec![0]);
        assert!(validate_decoder_dataset_inputs(&config).is_ok());

        let no_flip = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let config = test_config(no_flip, DecoderDatasetMode::MeasurementsBlinded, vec![]);
        assert!(validate_decoder_dataset_inputs(&config)
            .unwrap_err()
            .contains("--logical_x_qubits"));

        let changes_detector = "R 0 1\n# RSTIM_LOGICAL_FLIP_POINT\nM 0 1\nDETECTOR rec[-2] rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\n";
        let config = test_config(
            changes_detector,
            DecoderDatasetMode::MeasurementsBlinded,
            vec![0],
        );
        assert!(validate_decoder_dataset_inputs(&config)
            .unwrap_err()
            .contains("changes detector"));
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
        let config = test_config(circuit, DecoderDatasetMode::Detectors, vec![]);
        let artifacts = generate_decoder_dataset_artifacts(&config).unwrap();

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
            "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let mut config = test_config(circuit, DecoderDatasetMode::MeasurementsBlinded, vec![0]);
        config.shots = 16;
        config.seed = Some(0xdec0_de01);

        let artifacts = generate_decoder_dataset_artifacts(&config).unwrap();
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
            "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let mut config = test_config(circuit, DecoderDatasetMode::MeasurementsBlinded, vec![0]);
        config.shots = 32;
        config.seed = Some(123);

        let a = generate_decoder_dataset_artifacts(&config).unwrap();
        let b = generate_decoder_dataset_artifacts(&config).unwrap();
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
            "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let mut config = test_config(circuit, DecoderDatasetMode::MeasurementsBlinded, vec![0]);
        config.shots = 8;
        config.seed = Some(7);
        config.public_out = public_out.clone();
        config.private_out = private_out.clone();

        let summary = export_decoder_dataset(config).unwrap();
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
        assert!(std::fs::read_to_string(public_out.join("circuit.stim"))
            .unwrap()
            .contains(LOGICAL_FLIP_MARKER));
    }

    #[test]
    fn public_directory_is_not_visible_when_public_rename_fails() {
        let root = tempfile::tempdir().unwrap();
        let public_out = root.path().join("public");
        let private_out = root.path().join("private");
        let circuit = "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
        let mut config = test_config(circuit, DecoderDatasetMode::Detectors, vec![]);
        config.public_out = public_out.clone();
        config.private_out = private_out.clone();
        config.seed = Some(3);

        let mut publisher = FailingDirectoryPublisher::new(2);
        let err = export_decoder_dataset_with_publisher(config, &mut publisher).unwrap_err();

        assert!(err.contains("private bundle retained"));
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
