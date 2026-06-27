use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::bench::result::{BenchmarkResultRow, CaseSummary, MetricMap, ParamMap};
use crate::failure::{classify_completed, classify_error, FailureKind};

#[derive(Debug, Deserialize)]
struct BbCompareCsvRow {
    case_id: String,
    runner: String,
    decoder_impl: String,
    code_id: String,
    p: f64,
    num_cycles: u64,
    shots_budget: u64,
    errors_budget: Option<u64>,
    shots_used: u64,
    seed: u64,
    bp_method: String,
    max_iter: u64,
    osd_method: String,
    osd_order: u64,
    batch_size: u64,
    batches_completed: u64,
    setup_seconds: f64,
    sample_seconds: f64,
    decode_seconds: f64,
    run_seconds: f64,
    logical_errors: u64,
    logical_error_rate: f64,
    bp_seconds: Option<f64>,
    osd_seconds: Option<f64>,
    decode_call_count: Option<f64>,
    bp_iteration_count: Option<f64>,
    osd_use_count: Option<f64>,
    osd_candidate_count: Option<f64>,
    gf2_solve_count: Option<f64>,
    gf2_full_elimination_count: Option<f64>,
    status: String,
    stop_reason: String,
    error: String,
}

pub fn read_bb_compare_csv(
    path: &Path,
    benchmark_name: &str,
) -> Result<Vec<BenchmarkResultRow>, String> {
    let mut reader = csv::Reader::from_path(path).map_err(|e| e.to_string())?;
    reader
        .deserialize::<BbCompareCsvRow>()
        .map(|row| row.map_err(|e| e.to_string()))
        .map(|row| row.map(|row| row.into_benchmark_row(benchmark_name)))
        .collect()
}

impl BbCompareCsvRow {
    fn into_benchmark_row(self, benchmark_name: &str) -> BenchmarkResultRow {
        let error = if self.error.trim().is_empty() {
            None
        } else {
            Some(self.error.clone())
        };
        let plottable = matches!(self.status.as_str(), "ok" | "partial") && self.shots_used > 0;
        let timed_out = self.stop_reason == "wall_budget_exhausted" || self.status == "partial";
        let failure_kind = if plottable {
            classify_completed(self.logical_errors, timed_out)
        } else if self.status == "skipped" {
            FailureKind::Unsupported
        } else {
            error
                .as_deref()
                .map(|message| classify_error(message, FailureKind::SolverFailure))
                .unwrap_or(FailureKind::SolverFailure)
        };

        let mut metrics = MetricMap::from([
            ("setup_seconds".into(), self.setup_seconds),
            ("sample_seconds".into(), self.sample_seconds),
            ("decode_seconds".into(), self.decode_seconds),
            ("run_seconds".into(), self.run_seconds),
            ("logical_error_rate".into(), self.logical_error_rate),
            ("logical_errors".into(), self.logical_errors as f64),
            ("shots_used".into(), self.shots_used as f64),
            (
                "run_seconds_per_shot".into(),
                per_shot(self.run_seconds, self.shots_used),
            ),
            (
                "decode_seconds_per_shot".into(),
                per_shot(self.decode_seconds, self.shots_used),
            ),
        ]);
        insert_optional_metric(&mut metrics, "bp_seconds", self.bp_seconds);
        insert_optional_metric(&mut metrics, "osd_seconds", self.osd_seconds);
        insert_optional_metric(&mut metrics, "decode_call_count", self.decode_call_count);
        insert_optional_metric(&mut metrics, "bp_iteration_count", self.bp_iteration_count);
        insert_optional_metric(&mut metrics, "osd_use_count", self.osd_use_count);
        insert_optional_metric(
            &mut metrics,
            "osd_candidate_count",
            self.osd_candidate_count,
        );
        insert_optional_metric(&mut metrics, "gf2_solve_count", self.gf2_solve_count);
        insert_optional_metric(
            &mut metrics,
            "gf2_full_elimination_count",
            self.gf2_full_elimination_count,
        );

        BenchmarkResultRow {
            benchmark: benchmark_name.into(),
            runner: self.decoder_impl.clone(),
            language: language_for_decoder(&self.decoder_impl).into(),
            status: failure_kind.status().into(),
            failure_kind,
            params: ParamMap::from([
                ("batch_size".into(), serde_json::json!(self.batch_size)),
                (
                    "batches_completed".into(),
                    serde_json::json!(self.batches_completed),
                ),
                ("bp_method".into(), serde_json::json!(self.bp_method)),
                ("case_id".into(), serde_json::json!(self.case_id)),
                ("code_id".into(), serde_json::json!(self.code_id)),
                ("csv_runner".into(), serde_json::json!(self.runner)),
                ("max_iter".into(), serde_json::json!(self.max_iter)),
                ("num_cycles".into(), serde_json::json!(self.num_cycles)),
                ("osd_method".into(), serde_json::json!(self.osd_method)),
                ("osd_order".into(), serde_json::json!(self.osd_order)),
                ("p".into(), serde_json::json!(self.p)),
                ("raw_status".into(), serde_json::json!(self.status)),
                ("rounds".into(), serde_json::json!(self.num_cycles)),
                ("seed".into(), serde_json::json!(self.seed)),
                ("shots_budget".into(), serde_json::json!(self.shots_budget)),
                (
                    "errors_budget".into(),
                    serde_json::json!(self.errors_budget),
                ),
                ("stop_reason".into(), serde_json::json!(self.stop_reason)),
            ]),
            case_summary: CaseSummary::from([
                ("logical_observable_count".into(), serde_json::json!(1)),
                ("num_obs".into(), serde_json::json!(1)),
            ]),
            metrics,
            artifacts: BTreeMap::new(),
            error,
        }
    }
}

fn insert_optional_metric(metrics: &mut MetricMap, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        metrics.insert(key.into(), value);
    }
}

fn per_shot(seconds: f64, shots: u64) -> f64 {
    if shots == 0 {
        0.0
    } else {
        seconds / shots as f64
    }
}

fn language_for_decoder(decoder: &str) -> &'static str {
    match decoder {
        "rbposd" => "rust",
        "ldpc_bposd" => "python",
        _ => "legacy",
    }
}
