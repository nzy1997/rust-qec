use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::bench::result::{BenchmarkResultRow, CaseSummary, MetricMap, ParamMap};
use crate::failure::{classify_completed, classify_error, FailureKind};

#[derive(Debug, Deserialize)]
struct SurfaceCompareCsvRow {
    tier: String,
    decoder: String,
    backend: String,
    distance: u64,
    rounds: u64,
    p: f64,
    seed: u64,
    num_dets: u64,
    num_obs: u64,
    shots_budget: u64,
    errors_budget: u64,
    shots_used: u64,
    logical_errors: u64,
    logical_error_rate: f64,
    compile_us: f64,
    total_decode_us: f64,
    decode_us_per_shot: f64,
    status: String,
    error: String,
}

pub fn read_surface_compare_csv(
    path: &Path,
    benchmark_name: &str,
) -> Result<Vec<BenchmarkResultRow>, String> {
    let mut reader = csv::Reader::from_path(path).map_err(|e| e.to_string())?;
    reader
        .deserialize::<SurfaceCompareCsvRow>()
        .map(|row| row.map_err(|e| e.to_string()))
        .map(|row| row.map(|row| row.into_benchmark_row(benchmark_name)))
        .collect()
}

impl SurfaceCompareCsvRow {
    fn into_benchmark_row(self, benchmark_name: &str) -> BenchmarkResultRow {
        let error = if self.error.trim().is_empty() {
            None
        } else {
            Some(self.error)
        };
        let failure_kind = if self.status == "ok" {
            classify_completed(self.logical_errors, false)
        } else {
            error
                .as_deref()
                .map(|message| classify_error(message, FailureKind::SolverFailure))
                .unwrap_or(FailureKind::SolverFailure)
        };

        BenchmarkResultRow {
            benchmark: benchmark_name.into(),
            runner: self.decoder.clone(),
            language: language_for_decoder(&self.decoder).into(),
            status: self.status,
            failure_kind,
            params: ParamMap::from([
                ("backend".into(), serde_json::json!(self.backend)),
                ("distance".into(), serde_json::json!(self.distance)),
                (
                    "errors_budget".into(),
                    serde_json::json!(self.errors_budget),
                ),
                ("max_errors".into(), serde_json::json!(self.errors_budget)),
                ("max_shots".into(), serde_json::json!(self.shots_budget)),
                ("p".into(), serde_json::json!(self.p)),
                ("rounds".into(), serde_json::json!(self.rounds)),
                ("seed".into(), serde_json::json!(self.seed)),
                ("shots_budget".into(), serde_json::json!(self.shots_budget)),
                ("tier".into(), serde_json::json!(self.tier)),
            ]),
            case_summary: CaseSummary::from([
                (
                    "logical_observable_count".into(),
                    serde_json::json!(self.num_obs),
                ),
                ("num_dets".into(), serde_json::json!(self.num_dets)),
                ("num_obs".into(), serde_json::json!(self.num_obs)),
            ]),
            metrics: MetricMap::from([
                ("compile_us".into(), self.compile_us),
                ("decode_us_per_shot".into(), self.decode_us_per_shot),
                ("logical_error_rate".into(), self.logical_error_rate),
                ("logical_errors".into(), self.logical_errors as f64),
                ("shots_used".into(), self.shots_used as f64),
                ("total_decode_us".into(), self.total_decode_us),
            ]),
            artifacts: BTreeMap::new(),
            error,
        }
    }
}

fn language_for_decoder(decoder: &str) -> &'static str {
    match decoder {
        "rmatching" | "rbposd" | "rilpqec" => "rust",
        "pymatching" | "ilpqec" | "ldpc" => "python",
        _ => "legacy",
    }
}
