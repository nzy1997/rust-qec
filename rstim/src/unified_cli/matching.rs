use std::fs;
use std::path::{Path, PathBuf};

use rmatching::Matching;
use serde::Serialize;

pub const COMMAND: &str = "circuit.decode";

#[derive(Serialize)]
pub struct Summary {
    pub decoder: &'static str,
    pub shots: usize,
    pub num_detectors: usize,
    pub num_observables: usize,
    pub out: PathBuf,
}

pub struct Failure {
    pub code: &'static str,
    pub message: String,
}

pub fn run(dem: &Path, input: &Path, out: &Path) -> Result<Summary, Failure> {
    let dem_text = fs::read_to_string(dem).map_err(|error| Failure {
        code: "input_error",
        message: format!("failed to read {}: {error}", dem.display()),
    })?;
    let mut matching = Matching::from_dem(&dem_text).map_err(|message| Failure {
        code: "invalid_dem",
        message,
    })?;
    let num_detectors = matching.num_detectors();
    let num_observables = matching.num_observables();
    let rows = fs::read_to_string(input).map_err(|error| Failure {
        code: "input_error",
        message: format!("failed to read {}: {error}", input.display()),
    })?;

    let mut predictions = String::new();
    let mut shots = 0;
    for (index, row) in rows.lines().enumerate() {
        if row.len() != num_detectors || !row.bytes().all(|bit| matches!(bit, b'0' | b'1')) {
            return Err(Failure {
                code: "invalid_detector_row",
                message: format!(
                    "row {} must contain exactly {num_detectors} detector bits (0 or 1); observable bits must not be included",
                    index + 1
                ),
            });
        }
        let syndrome: Vec<u8> = row.bytes().map(|bit| bit - b'0').collect();
        for bit in matching.decode(&syndrome) {
            predictions.push(char::from(b'0' + bit));
        }
        predictions.push('\n');
        shots += 1;
    }
    if shots == 0 {
        return Err(Failure {
            code: "invalid_detector_row",
            message: "detector input contains no rows".to_string(),
        });
    }
    fs::write(out, predictions).map_err(|error| Failure {
        code: "output_error",
        message: format!("failed to write {}: {error}", out.display()),
    })?;
    Ok(Summary {
        decoder: "rmatching",
        shots,
        num_detectors,
        num_observables,
        out: out.to_path_buf(),
    })
}
