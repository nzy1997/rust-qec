use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use rstim::output::OutputFormat;
use rstim::result_stream::ResultBlockReader;
use serde::{Deserialize, Serialize};

use super::dataset::{FileManifest, read_dataset_for_scoring, sha256_file};
use super::{DecodeFailure, STATS_SCHEMA_VERSION};

pub const COMMAND: &str = "dataset.score";

pub struct ScoreOptions {
    pub dataset: PathBuf,
    pub private: PathBuf,
    pub predictions: PathBuf,
    pub decode_stats: Option<PathBuf>,
}

#[derive(Serialize)]
pub struct ScoreResult {
    pub dataset_id: String,
    pub shot_count: usize,
    pub observables: usize,
    pub logical_errors: usize,
    pub logical_error_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct_loss_patterns: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct_loss_patterns_exact: Option<bool>,
}

impl ScoreResult {
    pub fn human(&self) -> String {
        format!(
            "Scored shots: {}\nLogical errors: {} / {}\n",
            self.shot_count, self.logical_errors, self.shot_count
        )
    }
}

#[derive(Deserialize)]
struct PrivateManifest {
    format: String,
    schema_version: u32,
    dataset_id: String,
    mode: String,
    shots: usize,
    answers_file: FileManifest,
}

#[derive(Deserialize)]
struct DecodeStats {
    schema_version: String,
    circuit_sha256: String,
    shot_count: usize,
    attempted_shot_count: usize,
    timeout_count: usize,
    infeasible_shot_count: usize,
    distinct_loss_patterns: usize,
    distinct_loss_patterns_exact: bool,
}

fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    code: &'static str,
) -> Result<T, DecodeFailure> {
    let bytes = fs::read(path).map_err(|error| {
        DecodeFailure::new(
            "input_error",
            format!("failed to read {}: {error}", path.display()),
        )
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| DecodeFailure::new(code, format!("invalid {}: {error}", path.display())))
}

fn rows(
    path: &Path,
    bits: usize,
    shots: usize,
    code: &'static str,
) -> Result<ResultBlockReader<BufReader<File>>, DecodeFailure> {
    let expected_len = shots
        .checked_mul(bits.div_ceil(8))
        .ok_or_else(|| DecodeFailure::new(code, "b8 file size overflows"))?;
    let file = File::open(path).map_err(|error| {
        DecodeFailure::new(
            "input_error",
            format!("failed to open {}: {error}", path.display()),
        )
    })?;
    if file
        .metadata()
        .map_err(|error| DecodeFailure::new("input_error", error.to_string()))?
        .len()
        != expected_len as u64
    {
        return Err(DecodeFailure::new(
            code,
            format!(
                "Incomplete rows: {} must contain {shots} shots ({expected_len} bytes)",
                path.display()
            ),
        ));
    }
    ResultBlockReader::new(
        BufReader::new(file),
        bits,
        shots as u64,
        OutputFormat::B8,
        1024,
    )
    .map_err(|error| DecodeFailure::new(code, error.to_string()))
}

/// Scores complete prediction rows against a matching private answer bundle.
/// Any wrong observable counts as one logical error for that shot. Private
/// input masks have already been accounted for in answers.b8.
pub fn run(options: &ScoreOptions) -> Result<ScoreResult, DecodeFailure> {
    let dataset = read_dataset_for_scoring(&options.dataset)?;
    let public = &dataset.manifest;
    let dataset_id = public.dataset_id.as_ref().ok_or_else(|| {
        DecodeFailure::new(
            "invalid_dataset",
            "score requires an rstim dataset with a dataset_id",
        )
    })?;
    let private: PrivateManifest =
        read_json(&options.private.join("manifest.json"), "invalid_dataset")?;
    if private.format != "rstim_decoder_dataset" || private.schema_version != 1 {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "unsupported private dataset format or schema version",
        ));
    }
    if &private.dataset_id != dataset_id
        || private.mode != public.mode
        || private.shots != public.shots
    {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "Mismatched datasets: public and private identity, mode and shot count must agree",
        ));
    }
    let bits = public.circuit.observables;
    let circuit_stats = rstim::stats::summarize_text(&dataset.circuit_text)
        .map_err(|message| DecodeFailure::new("invalid_dataset", message))?;
    if bits == 0 || bits != circuit_stats.num_observables {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "score requires a matching, nonzero observable count",
        ));
    }
    if private.answers_file.file != "answers.b8"
        || private.answers_file.bits != bits
        || private.answers_file.bytes_per_shot != bits.div_ceil(8)
    {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "private answers must name answers.b8 with the observable row width",
        ));
    }
    let answers_path = options.private.join("answers.b8");
    if sha256_file(&answers_path)? != private.answers_file.sha256 {
        return Err(DecodeFailure::new(
            "invalid_dataset",
            "answers.b8 SHA256 does not match private manifest",
        ));
    }
    let stats: Option<DecodeStats> = options
        .decode_stats
        .as_ref()
        .map(|path| read_json(path, "invalid_decode_stats"))
        .transpose()?;
    if let Some(stats) = &stats {
        if stats.schema_version != STATS_SCHEMA_VERSION
            || stats.circuit_sha256 != public.circuit.sha256
            || stats.shot_count != public.shots
            || stats.attempted_shot_count != public.shots
            || stats.timeout_count != 0
            || stats.infeasible_shot_count != 0
        {
            return Err(DecodeFailure::new(
                "invalid_decode_stats",
                "decode statistics must match the circuit and show a complete, successful run",
            ));
        }
    }
    let mut predictions = rows(
        &options.predictions,
        bits,
        public.shots,
        "invalid_predictions",
    )?;
    let mut answers = rows(&answers_path, bits, public.shots, "invalid_dataset")?;
    let mut logical_errors = 0;
    while let Some(predicted) = predictions
        .next_block()
        .map_err(|error| DecodeFailure::new("invalid_predictions", error.to_string()))?
    {
        let answer = answers
            .next_block()
            .map_err(|error| DecodeFailure::new("invalid_dataset", error.to_string()))?
            .ok_or_else(|| DecodeFailure::new("invalid_dataset", "Incomplete answer rows"))?;
        for shot in 0..predicted.num_minor() {
            if (0..bits).any(|bit| predicted.get(bit, shot) != answer.get(bit, shot)) {
                logical_errors += 1;
            }
        }
    }
    // Also checks EOF for zero-shot bundles.
    answers
        .next_block()
        .map_err(|error| DecodeFailure::new("invalid_dataset", error.to_string()))?;
    Ok(ScoreResult {
        dataset_id: dataset_id.clone(),
        shot_count: public.shots,
        observables: bits,
        logical_errors,
        logical_error_rate: (public.shots > 0).then(|| logical_errors as f64 / public.shots as f64),
        distinct_loss_patterns: stats.as_ref().map(|stats| stats.distinct_loss_patterns),
        distinct_loss_patterns_exact: stats
            .as_ref()
            .map(|stats| stats.distinct_loss_patterns_exact),
    })
}
