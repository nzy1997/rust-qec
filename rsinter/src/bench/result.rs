use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};

use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::failure::{FailureKind, classify_error};

pub type ArtifactMap = BTreeMap<String, String>;
pub type CaseSummary = BTreeMap<String, Value>;
pub type ParamMap = BTreeMap<String, Value>;
pub type MetricMap = BTreeMap<String, f64>;

pub trait PairMapExt<K, V> {
    fn from_pairs<const N: usize>(pairs: [(K, V); N]) -> Self;
}

impl PairMapExt<&str, Value> for BTreeMap<String, Value> {
    fn from_pairs<const N: usize>(pairs: [(&str, Value); N]) -> Self {
        pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }
}

impl PairMapExt<&str, f64> for MetricMap {
    fn from_pairs<const N: usize>(pairs: [(&str, f64); N]) -> Self {
        pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunManifest {
    pub benchmark: String,
    pub benchmark_version: u64,
    pub runner: String,
    pub language: String,
    pub output_dir: String,
}

impl RunManifest {
    pub fn new(
        benchmark: String,
        benchmark_version: u64,
        runner: String,
        language: String,
        output_dir: String,
    ) -> Self {
        Self {
            benchmark,
            benchmark_version,
            runner,
            language,
            output_dir,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkResultRow {
    pub benchmark: String,
    pub runner: String,
    pub language: String,
    pub status: String,
    pub failure_kind: FailureKind,
    pub params: ParamMap,
    pub case_summary: CaseSummary,
    pub metrics: MetricMap,
    pub artifacts: ArtifactMap,
    pub error: Option<String>,
}

const ROW_IDENTITY_SCHEMA: &str = "rsinter.benchmark_result_row.v1";
const CASE_SUMMARY_ADDITIVE_KEYS: [&str; 1] = ["num_shots_generated"];

#[derive(Serialize)]
struct BenchmarkResultRowIdentityInput<'a> {
    schema: &'static str,
    benchmark: &'a str,
    runner: &'a str,
    language: &'a str,
    params: &'a ParamMap,
    case_summary: CaseSummary,
}

impl BenchmarkResultRow {
    pub fn identity(&self) -> Result<String, String> {
        let input = BenchmarkResultRowIdentityInput {
            schema: ROW_IDENTITY_SCHEMA,
            benchmark: &self.benchmark,
            runner: &self.runner,
            language: &self.language,
            params: &self.params,
            case_summary: stable_case_summary(&self.case_summary),
        };
        let bytes = serde_json::to_vec(&input).map_err(|error| error.to_string())?;
        let digest = Sha256::digest(bytes);
        Ok(format!("sha256:{}", lower_hex(&digest)))
    }
}

pub(crate) fn stable_case_summary(case_summary: &CaseSummary) -> CaseSummary {
    case_summary
        .iter()
        .filter(|(key, _)| !case_summary_additive_keys().contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(crate) fn case_summary_additive_keys() -> &'static [&'static str] {
    &CASE_SUMMARY_ADDITIVE_KEYS
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

impl Serialize for BenchmarkResultRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let identity = self.identity().map_err(serde::ser::Error::custom)?;
        let mut row = serializer.serialize_struct("BenchmarkResultRow", 11)?;
        row.serialize_field("identity", &identity)?;
        row.serialize_field("benchmark", &self.benchmark)?;
        row.serialize_field("runner", &self.runner)?;
        row.serialize_field("language", &self.language)?;
        row.serialize_field("status", &self.status)?;
        row.serialize_field("failure_kind", &self.failure_kind)?;
        row.serialize_field("params", &self.params)?;
        row.serialize_field("case_summary", &self.case_summary)?;
        row.serialize_field("metrics", &self.metrics)?;
        row.serialize_field("artifacts", &self.artifacts)?;
        row.serialize_field("error", &self.error)?;
        row.end()
    }
}

#[derive(Deserialize)]
struct RawBenchmarkResultRow {
    benchmark: String,
    runner: String,
    language: String,
    status: String,
    #[serde(default)]
    failure_kind: Option<FailureKind>,
    params: ParamMap,
    case_summary: CaseSummary,
    metrics: MetricMap,
    artifacts: ArtifactMap,
    error: Option<String>,
}

impl<'de> Deserialize<'de> for BenchmarkResultRow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawBenchmarkResultRow::deserialize(deserializer)?;
        let failure_kind = raw.failure_kind.unwrap_or_else(|| {
            infer_legacy_failure_kind(&raw.status, raw.error.as_deref(), &raw.params, &raw.metrics)
        });
        Ok(Self {
            benchmark: raw.benchmark,
            runner: raw.runner,
            language: raw.language,
            status: raw.status,
            failure_kind,
            params: raw.params,
            case_summary: raw.case_summary,
            metrics: raw.metrics,
            artifacts: raw.artifacts,
            error: raw.error,
        })
    }
}

fn infer_legacy_failure_kind(
    status: &str,
    error: Option<&str>,
    params: &ParamMap,
    metrics: &MetricMap,
) -> FailureKind {
    if status == "error" {
        return error
            .map(|message| classify_error(message, FailureKind::SolverFailure))
            .unwrap_or(FailureKind::SolverFailure);
    }
    if legacy_timed_out(params, metrics) {
        FailureKind::Timeout
    } else if metrics.get("logical_errors").copied().unwrap_or(0.0) > 0.0 {
        FailureKind::LogicalFailure
    } else {
        FailureKind::Ok
    }
}

fn legacy_timed_out(params: &ParamMap, metrics: &MetricMap) -> bool {
    let Some(max_wall_seconds) = params.get("max_wall_seconds").and_then(Value::as_f64) else {
        return false;
    };
    let Some(wall_seconds) = metrics.get("wall_seconds") else {
        return false;
    };
    wall_seconds.is_finite()
        && *wall_seconds >= max_wall_seconds
        && !legacy_reached_cap(params, metrics, "max_shots", "shots_used")
        && !legacy_reached_cap(params, metrics, "max_errors", "logical_errors")
}

fn legacy_reached_cap(
    params: &ParamMap,
    metrics: &MetricMap,
    param_key: &str,
    metric_key: &str,
) -> bool {
    match (
        params.get(param_key).and_then(Value::as_f64),
        metrics.get(metric_key).copied(),
    ) {
        (Some(cap), Some(value)) => value.is_finite() && value >= cap,
        _ => false,
    }
}

pub fn write_results_jsonl(rows: &[BenchmarkResultRow], out: &mut dyn Write) -> Result<(), String> {
    for row in rows {
        serde_json::to_writer(&mut *out, row).map_err(|e| e.to_string())?;
        out.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn read_results_jsonl(input: impl Read) -> Result<Vec<BenchmarkResultRow>, String> {
    let reader = BufReader::new(input);
    let mut rows = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let row = serde_json::from_str::<BenchmarkResultRow>(&line).map_err(|e| e.to_string())?;
        rows.push(row);
    }
    Ok(rows)
}
