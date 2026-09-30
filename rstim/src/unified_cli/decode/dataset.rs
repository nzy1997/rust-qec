use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::DecodeFailure;

/// Writes a complete public dataset bundle (`manifest.json`, `circuit.stim`,
/// `shots.b8`) into an existing directory, computing counts, SHA-256 hashes,
/// and the derived `dataset_id`. Shared by `dataset export`-style producers
/// and the `dataset import` verb so the wire format has exactly one writer.
pub(super) fn write_public_bundle(
    dir: &Path,
    circuit_text: &str,
    shots_b8: &[u8],
    shots: usize,
) -> Result<(), DecodeFailure> {
    let instrs = rstim::validation::parse_and_validate(circuit_text)
        .map_err(|message| DecodeFailure::new("invalid_dataset", message))?;
    let stats = rstim::stats::summarize(&instrs);
    let row_bits = stats.num_measurements;
    let row_bytes = row_bits.div_ceil(8);
    if shots_b8.len() != shots * row_bytes {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            format!(
                "shots payload has {} bytes, expected {shots} shots x {row_bytes} bytes",
                shots_b8.len()
            ),
        ));
    }
    let circuit_sha = sha256_hex(circuit_text.as_bytes());
    let shots_sha = sha256_hex(shots_b8);
    let manifest = serde_json::json!({
        "format": RSTIM_DATASET_FORMAT,
        "schema_version": RSTIM_DATASET_SCHEMA_VERSION,
        "dataset_id": sha256_hex(&dataset_id_material(
            RSTIM_DATASET_SCHEMA_VERSION,
            "measurements_blinded",
            &circuit_sha,
            shots,
            row_bits,
            &shots_sha,
        )),
        "mode": "measurements_blinded",
        "shots": shots,
        "row": {
            "kind": "measurements",
            "bits": row_bits,
            "encoding": "b8",
            "bit_order": "lsb_first",
            "bytes_per_shot": row_bytes,
        },
        "circuit": {
            "file": "circuit.stim",
            "sha256": circuit_sha,
            "measurements": stats.num_measurements,
            "detectors": stats.num_detectors,
            "observables": stats.num_observables,
            "sweep_bits": stats.num_sweep_bits,
        },
        "shots_file": {
            "file": "shots.b8",
            "sha256": shots_sha,
            "bits": row_bits,
            "bytes_per_shot": row_bytes,
        },
    });
    let write = |name: &str, bytes: &[u8]| -> Result<(), DecodeFailure> {
        fs::write(dir.join(name), bytes)
            .map_err(|error| DecodeFailure::new("missing_dataset_file", format!("{error}")))
    };
    write("circuit.stim", circuit_text.as_bytes())?;
    write("shots.b8", shots_b8)?;
    write(
        "manifest.json",
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| DecodeFailure::new("invalid_dataset", error.to_string()))?
            .as_slice(),
    )?;
    Ok(())
}

const RSTIM_DATASET_FORMAT: &str = "rstim_decoder_dataset";
const RSTIM_DATASET_SCHEMA_VERSION: u32 = 1;
const QUDE_DATASET_FORMAT: &str = "qude_decoder_dataset";
const QUDE_DATASET_SCHEMA_VERSION: u32 = 3;

#[derive(Debug)]
pub(super) struct PublicManifest {
    pub(super) schema_version: u32,
    pub(super) dataset_id: Option<String>,
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

#[derive(Debug, Deserialize)]
struct ManifestHeader {
    format: String,
    schema_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RstimPublicManifest {
    #[serde(rename = "format")]
    _format: String,
    schema_version: u32,
    dataset_id: String,
    mode: String,
    shots: usize,
    row: RowManifest,
    circuit: CircuitManifest,
    shots_file: FileManifest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QudePublicManifest {
    #[serde(rename = "format")]
    _format: String,
    schema_version: u32,
    #[serde(rename = "benchmark_id")]
    _benchmark_id: String,
    #[serde(rename = "code_family")]
    _code_family: String,
    #[serde(rename = "task")]
    _task: String,
    #[serde(rename = "code_params")]
    _code_params: Value,
    #[serde(rename = "noise_model")]
    _noise_model: String,
    #[serde(rename = "p")]
    _p: f64,
    #[serde(rename = "rounds")]
    _rounds: usize,
    mode: String,
    shots: usize,
    num_detectors: usize,
    num_measurements: usize,
    num_observables: usize,
    row: RowManifest,
    circuit: QudeCircuitManifest,
    shots_file: QudeFileManifest,
    predictions: QudePredictionsManifest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QudeCircuitManifest {
    file: String,
    sha256: String,
    measurements: usize,
    detectors: usize,
    observables: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QudeFileManifest {
    file: String,
    sha256: String,
    encoding: String,
    bit_order: String,
    bits_per_shot: usize,
    bytes_per_shot: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QudePredictionsManifest {
    encoding: String,
    bit_order: String,
    bits_per_shot: usize,
    bytes_per_shot: usize,
    padding: String,
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
    let manifest = parse_manifest(&manifest_bytes)?;
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
    if let Some(dataset_id) = &manifest.dataset_id {
        let expected_dataset_id = sha256_hex(&dataset_id_material(
            manifest.schema_version,
            &manifest.mode,
            &manifest.circuit.sha256,
            manifest.shots,
            manifest.row.bits,
            &manifest.shots_file.sha256,
        ));
        if dataset_id != &expected_dataset_id {
            return Err(DecodeFailure::new(
                "invalid_dataset",
                "dataset_id does not match public material",
            ));
        }
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

fn parse_manifest(bytes: &[u8]) -> Result<PublicManifest, DecodeFailure> {
    let invalid =
        |error| DecodeFailure::new("invalid_dataset", format!("invalid manifest.json: {error}"));
    let header: ManifestHeader = serde_json::from_slice(bytes).map_err(invalid)?;
    match (header.format.as_str(), header.schema_version) {
        (RSTIM_DATASET_FORMAT, RSTIM_DATASET_SCHEMA_VERSION) => {
            let value: RstimPublicManifest = serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(PublicManifest {
                schema_version: value.schema_version,
                dataset_id: Some(value.dataset_id),
                mode: value.mode,
                shots: value.shots,
                row: value.row,
                circuit: value.circuit,
                shots_file: value.shots_file,
            })
        }
        (QUDE_DATASET_FORMAT, QUDE_DATASET_SCHEMA_VERSION) => {
            let value: QudePublicManifest = serde_json::from_slice(bytes).map_err(invalid)?;
            normalize_qude_manifest(value)
        }
        _ => Err(DecodeFailure::new(
            "invalid_dataset",
            "unsupported decoder dataset format or schema version",
        )),
    }
}

fn normalize_qude_manifest(value: QudePublicManifest) -> Result<PublicManifest, DecodeFailure> {
    if value.num_measurements != value.circuit.measurements
        || value.num_detectors != value.circuit.detectors
        || value.num_observables != value.circuit.observables
    {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "Decoder-Server top-level and circuit counts disagree",
        ));
    }
    if value.row.bit_order != "little_endian"
        || value.shots_file.encoding != "b8"
        || value.shots_file.bit_order != "little_endian"
        || value.predictions.encoding != "b8"
        || value.predictions.bit_order != "little_endian"
        || value.predictions.padding != "zero"
    {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "Decoder-Server rows must use little_endian b8 encoding with zero-padded predictions",
        ));
    }
    let expected_prediction_bytes = value.num_observables.div_ceil(8);
    if value.predictions.bits_per_shot != value.num_observables
        || value.predictions.bytes_per_shot != expected_prediction_bytes
    {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "Decoder-Server prediction row width is inconsistent",
        ));
    }
    Ok(PublicManifest {
        schema_version: value.schema_version,
        dataset_id: None,
        mode: value.mode,
        shots: value.shots,
        row: RowManifest {
            bit_order: "lsb_first".to_string(),
            ..value.row
        },
        circuit: CircuitManifest {
            file: value.circuit.file,
            sha256: value.circuit.sha256,
            measurements: value.circuit.measurements,
            detectors: value.circuit.detectors,
            observables: value.circuit.observables,
            sweep_bits: 0,
        },
        shots_file: FileManifest {
            file: value.shots_file.file,
            sha256: value.shots_file.sha256,
            bits: value.shots_file.bits_per_shot,
            bytes_per_shot: value.shots_file.bytes_per_shot,
        },
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
        "format={RSTIM_DATASET_FORMAT}\nschema_version={schema_version}\nmode={mode}\ncircuit_sha256={circuit_sha256}\nshots={shots}\nrow_bits={row_bits}\nshots_b8_sha256={shots_sha256}\n"
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn write_valid_dataset(root: &Path) -> Value {
        let circuit = b"R 0\n";
        let shots = b"\0";
        fs::write(root.join("circuit.stim"), circuit).unwrap();
        fs::write(root.join("shots.b8"), shots).unwrap();
        let circuit_sha = sha256_hex(circuit);
        let shots_sha = sha256_hex(shots);
        let manifest = json!({
            "format": RSTIM_DATASET_FORMAT,
            "schema_version": RSTIM_DATASET_SCHEMA_VERSION,
            "dataset_id": sha256_hex(&dataset_id_material(
                RSTIM_DATASET_SCHEMA_VERSION,
                "measurements_blinded",
                &circuit_sha,
                1,
                1,
                &shots_sha,
            )),
            "mode": "measurements_blinded",
            "shots": 1,
            "row": {
                "kind": "measurements",
                "bits": 1,
                "encoding": "b8",
                "bit_order": "lsb_first",
                "bytes_per_shot": 1,
            },
            "circuit": {
                "file": "circuit.stim",
                "sha256": circuit_sha,
                "measurements": 0,
                "detectors": 0,
                "observables": 0,
                "sweep_bits": 0,
            },
            "shots_file": {
                "file": "shots.b8",
                "sha256": shots_sha,
                "bits": 1,
                "bytes_per_shot": 1,
            },
        });
        write_manifest(root, &manifest);
        manifest
    }

    fn write_manifest(root: &Path, manifest: &Value) {
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(manifest).unwrap(),
        )
        .unwrap();
    }

    fn refresh_dataset_id(manifest: &mut Value) {
        manifest["dataset_id"] = json!(sha256_hex(&dataset_id_material(
            manifest["schema_version"].as_u64().unwrap() as u32,
            manifest["mode"].as_str().unwrap(),
            manifest["circuit"]["sha256"].as_str().unwrap(),
            manifest["shots"].as_u64().unwrap() as usize,
            manifest["row"]["bits"].as_u64().unwrap() as usize,
            manifest["shots_file"]["sha256"].as_str().unwrap(),
        )));
    }

    fn assert_error(root: &Path, code: &str, message: &str) {
        let error = read_dataset(root).err().expect("dataset must be rejected");
        assert_eq!(error.code, code);
        assert!(error.message.contains(message), "{}", error.message);
    }

    #[test]
    fn manifest_structure_and_public_contract_are_strict() {
        let root = tempfile::tempdir().unwrap();
        write_valid_dataset(root.path());
        fs::write(root.path().join("manifest.json"), b"{").unwrap();
        assert_error(root.path(), "invalid_dataset", "invalid manifest.json");

        for (pointer, value, code, message) in [
            ("format", json!("other"), "invalid_dataset", "format"),
            (
                "schema_version",
                json!(2),
                "invalid_dataset",
                "schema version",
            ),
            (
                "mode",
                json!("detectors"),
                "unsupported_dataset_mode",
                "measurements_blinded",
            ),
            (
                "row.kind",
                json!("detectors"),
                "unsupported_dataset_mode",
                "measurements_blinded",
            ),
            ("row.encoding", json!("01"), "invalid_dataset", "b8"),
            (
                "row.bit_order",
                json!("msb_first"),
                "invalid_dataset",
                "lsb_first",
            ),
            (
                "circuit.file",
                json!("other.stim"),
                "invalid_dataset",
                "circuit.stim",
            ),
            (
                "shots_file.file",
                json!("other.b8"),
                "invalid_dataset",
                "shots.b8",
            ),
            (
                "row.bytes_per_shot",
                json!(2),
                "invalid_dataset",
                "row widths",
            ),
            ("shots_file.bits", json!(2), "invalid_dataset", "row widths"),
        ] {
            let case = tempfile::tempdir().unwrap();
            let mut manifest = write_valid_dataset(case.path());
            let (parent, field) = pointer.split_once('.').unwrap_or(("", pointer));
            if parent.is_empty() {
                manifest[field] = value;
            } else {
                manifest[parent][field] = value;
            }
            write_manifest(case.path(), &manifest);
            assert_error(case.path(), code, message);
        }
    }

    #[test]
    fn hashes_identity_size_and_utf8_are_verified_independently() {
        let root = tempfile::tempdir().unwrap();
        write_valid_dataset(root.path());
        fs::write(root.path().join("shots.b8"), b"\x01").unwrap();
        assert_error(root.path(), "invalid_dataset", "SHA256");

        let root = tempfile::tempdir().unwrap();
        let mut manifest = write_valid_dataset(root.path());
        manifest["dataset_id"] = json!("wrong");
        write_manifest(root.path(), &manifest);
        assert_error(root.path(), "invalid_dataset", "dataset_id");

        let root = tempfile::tempdir().unwrap();
        let mut manifest = write_valid_dataset(root.path());
        fs::write(root.path().join("shots.b8"), b"\0\0").unwrap();
        manifest["shots_file"]["sha256"] = json!(sha256_hex(b"\0\0"));
        refresh_dataset_id(&mut manifest);
        write_manifest(root.path(), &manifest);
        assert_error(root.path(), "invalid_dataset", "2 bytes, expected 1");

        let root = tempfile::tempdir().unwrap();
        let mut manifest = write_valid_dataset(root.path());
        fs::write(root.path().join("circuit.stim"), [0xff]).unwrap();
        manifest["circuit"]["sha256"] = json!(sha256_hex(&[0xff]));
        refresh_dataset_id(&mut manifest);
        write_manifest(root.path(), &manifest);
        assert_error(root.path(), "invalid_dataset", "not UTF-8");

        let root = tempfile::tempdir().unwrap();
        let mut manifest = write_valid_dataset(root.path());
        fs::write(root.path().join("shots.b8"), []).unwrap();
        manifest["shots"] = json!(u64::MAX);
        manifest["row"]["bits"] = json!(9);
        manifest["row"]["bytes_per_shot"] = json!(2);
        manifest["shots_file"]["bits"] = json!(9);
        manifest["shots_file"]["bytes_per_shot"] = json!(2);
        manifest["shots_file"]["sha256"] = json!(sha256_hex(&[]));
        refresh_dataset_id(&mut manifest);
        write_manifest(root.path(), &manifest);
        assert_error(root.path(), "invalid_dataset", "size overflows");
    }
}
