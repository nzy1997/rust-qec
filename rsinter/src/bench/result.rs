use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkResultRow {
    pub benchmark: String,
    pub runner: String,
    pub language: String,
    pub status: String,
    pub params: ParamMap,
    pub case_summary: CaseSummary,
    pub metrics: MetricMap,
    pub artifacts: ArtifactMap,
    pub error: Option<String>,
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
