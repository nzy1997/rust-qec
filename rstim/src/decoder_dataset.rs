#![allow(dead_code)] // Manifest types and schema constants are used by later exporter stages.

use crate::sim::bit_table::BitTable;
use serde::Serialize;
use sha2::{Digest, Sha256};
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
