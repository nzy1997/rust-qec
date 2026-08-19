use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::DecodeFailure;

const DATASET_FORMAT: &str = "rstim_decoder_dataset";
const DATASET_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicManifest {
    pub(super) format: String,
    pub(super) schema_version: u32,
    pub(super) dataset_id: String,
    pub(super) mode: String,
    pub(super) shots: usize,
    pub(super) row: RowManifest,
    pub(super) circuit: CircuitManifest,
    pub(super) shots_file: FileManifest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RowManifest {
    pub(super) kind: String,
    pub(super) bits: usize,
    pub(super) encoding: String,
    pub(super) bit_order: String,
    pub(super) bytes_per_shot: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CircuitManifest {
    pub(super) file: String,
    pub(super) sha256: String,
    pub(super) measurements: usize,
    pub(super) detectors: usize,
    pub(super) observables: usize,
    pub(super) sweep_bits: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileManifest {
    pub(super) file: String,
    pub(super) sha256: String,
    pub(super) bits: usize,
    pub(super) bytes_per_shot: usize,
}

pub(super) struct Dataset {
    pub(super) manifest: PublicManifest,
    pub(super) circuit_text: String,
    pub(super) shots_path: PathBuf,
}

pub(super) fn read_dataset(path: &Path) -> Result<Dataset, DecodeFailure> {
    let manifest_path = path.join("manifest.json");
    let circuit_path = path.join("circuit.stim");
    let shots_path = path.join("shots.b8");
    for required in [&manifest_path, &circuit_path, &shots_path] {
        if !required.is_file() {
            return Err(DecodeFailure::new(
                "missing_dataset_file",
                format!("missing public dataset file {}", required.display()),
            ));
        }
    }
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|error| DecodeFailure::new("missing_dataset_file", format!("{error}")))?;
    let manifest: PublicManifest = serde_json::from_slice(&manifest_bytes).map_err(|error| {
        DecodeFailure::new("invalid_dataset", format!("invalid manifest.json: {error}"))
    })?;
    if manifest.format != DATASET_FORMAT || manifest.schema_version != DATASET_SCHEMA_VERSION {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "unsupported decoder dataset format or schema version",
        ));
    }
    if manifest.mode != "measurements_blinded" || manifest.row.kind != "measurements" {
        return Err(DecodeFailure::new(
            "unsupported_dataset_mode",
            "decode requires a measurements_blinded dataset",
        ));
    }
    if manifest.row.encoding != "b8" || manifest.row.bit_order != "lsb_first" {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "measurement rows must use lsb_first b8 encoding",
        ));
    }
    if manifest.circuit.file != "circuit.stim" || manifest.shots_file.file != "shots.b8" {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "manifest must name circuit.stim and shots.b8",
        ));
    }
    let expected_row_bytes = manifest.row.bits.div_ceil(8);
    if manifest.row.bytes_per_shot != expected_row_bytes
        || manifest.shots_file.bits != manifest.row.bits
        || manifest.shots_file.bytes_per_shot != expected_row_bytes
    {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "manifest b8 row widths are inconsistent",
        ));
    }
    let circuit_bytes = fs::read(&circuit_path)
        .map_err(|error| DecodeFailure::new("missing_dataset_file", error.to_string()))?;
    if sha256_hex(&circuit_bytes) != manifest.circuit.sha256
        || sha256_file(&shots_path)? != manifest.shots_file.sha256
    {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "dataset file SHA256 does not match manifest",
        ));
    }
    let expected_dataset_id = sha256_hex(&dataset_id_material(
        manifest.schema_version,
        &manifest.mode,
        &manifest.circuit.sha256,
        manifest.shots,
        manifest.row.bits,
        &manifest.shots_file.sha256,
    ));
    if manifest.dataset_id != expected_dataset_id {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "dataset_id does not match public material",
        ));
    }
    let expected_len = manifest
        .shots
        .checked_mul(expected_row_bytes)
        .ok_or_else(|| DecodeFailure::new("invalid_dataset", "shots.b8 size overflows"))?;
    let actual_len = fs::metadata(&shots_path)
        .map_err(|error| DecodeFailure::new("missing_dataset_file", error.to_string()))?
        .len();
    if actual_len != expected_len as u64 {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            format!("shots.b8 has {actual_len} bytes, expected {expected_len}"),
        ));
    }
    let circuit_text = String::from_utf8(circuit_bytes).map_err(|error| {
        DecodeFailure::new(
            "invalid_dataset",
            format!("circuit.stim is not UTF-8: {error}"),
        )
    })?;
    Ok(Dataset {
        manifest,
        circuit_text,
        shots_path,
    })
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sha256_file(path: &Path) -> Result<String, DecodeFailure> {
    let file = File::open(path)
        .map_err(|error| DecodeFailure::new("missing_dataset_file", error.to_string()))?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| DecodeFailure::new("invalid_dataset", error.to_string()))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn dataset_id_material(
    schema_version: u32,
    mode: &str,
    circuit_sha256: &str,
    shots: usize,
    row_bits: usize,
    shots_sha256: &str,
) -> Vec<u8> {
    format!(
        "format={DATASET_FORMAT}\nschema_version={schema_version}\nmode={mode}\ncircuit_sha256={circuit_sha256}\nshots={shots}\nrow_bits={row_bits}\nshots_b8_sha256={shots_sha256}\n"
    )
    .into_bytes()
}
