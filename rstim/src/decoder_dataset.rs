#![allow(dead_code)] // Manifest types and schema constants are used by later exporter stages.

use crate::sim::bit_table::BitTable;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::PathBuf;

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

pub fn export_decoder_dataset(
    _config: ExportDecoderDatasetConfig,
) -> Result<DecoderDatasetSummary, String> {
    Err("export_decoder_dataset is not implemented".to_string())
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
}
