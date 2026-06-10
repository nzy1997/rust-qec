use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type ArtifactMap = BTreeMap<String, String>;
pub type CaseSummary = BTreeMap<String, Value>;
pub type ParamMap = BTreeMap<String, Value>;
pub type MetricMap = BTreeMap<String, f64>;

pub trait PairMapExt<K, V>: Sized {
    fn from_pairs<I>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>;
}

impl<K> PairMapExt<K, String> for BTreeMap<String, String>
where
    K: Into<String>,
{
    fn from_pairs<I>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, String)>,
    {
        pairs
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect()
    }
}

impl<K> PairMapExt<K, Value> for BTreeMap<String, Value>
where
    K: Into<String>,
{
    fn from_pairs<I>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, Value)>,
    {
        pairs
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect()
    }
}

impl<K> PairMapExt<K, f64> for BTreeMap<String, f64>
where
    K: Into<String>,
{
    fn from_pairs<I>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, f64)>,
    {
        pairs
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunManifest {
    pub benchmark: String,
    pub version: u64,
    pub runner: String,
    pub language: String,
    pub artifact_dir: String,
}

impl RunManifest {
    pub fn new(
        benchmark: String,
        version: u64,
        runner: String,
        language: String,
        artifact_dir: String,
    ) -> Self {
        Self {
            benchmark,
            version,
            runner,
            language,
            artifact_dir,
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
