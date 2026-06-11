use std::collections::BTreeMap;

use toml::Value;

use crate::bench::result::BenchmarkResultRow;
use crate::bench::runners::rbposd::RbposdRunner;
use crate::bench::runners::rilpqec::RilpqecRunner;
use crate::bench::runners::rmatching::RmatchingRunner;

pub struct BenchCasePoint {
    pub distance: usize,
    pub rounds: usize,
    pub p: f64,
    pub max_shots: u64,
    pub max_errors: u64,
    pub batch_size: usize,
}

pub struct BenchRunContext {
    pub benchmark_name: String,
    pub runner_name: String,
    pub language: String,
    pub seed: u64,
}

pub trait RustBenchRunner: Send + Sync {
    fn name(&self) -> &'static str;
    fn run_point(
        &self,
        point: &BenchCasePoint,
        ctx: &BenchRunContext,
    ) -> Result<BenchmarkResultRow, String>;
}

pub type RustRunnerRegistry = BTreeMap<String, Box<dyn RustBenchRunner>>;

pub fn default_rust_runner_names() -> Vec<String> {
    ["rmatching", "rbposd", "rilpqec"]
        .into_iter()
        .map(|name| name.to_string())
        .collect()
}

pub fn build_default_rust_runner_registry() -> RustRunnerRegistry {
    let mut registry: RustRunnerRegistry = BTreeMap::new();
    registry.insert("rmatching".into(), Box::new(RmatchingRunner));
    registry.insert("rbposd".into(), Box::new(RbposdRunner));
    registry.insert("rilpqec".into(), Box::new(RilpqecRunner));
    registry
}

pub fn expand_runner_points(
    params: &BTreeMap<String, Value>,
) -> Result<Vec<BenchCasePoint>, String> {
    let distances = require_array(params, "distance")?;
    let rounds = require_array(params, "rounds")?;
    let ps = require_array(params, "p")?;
    let max_shots = require_u64(params, "max_shots")?;
    let max_errors = require_u64(params, "max_errors")?;
    let batch_size = require_usize(params, "batch_size")?;
    if distances.is_empty() {
        return Err("distance must not be empty".into());
    }
    if rounds.is_empty() {
        return Err("rounds must not be empty".into());
    }
    if ps.is_empty() {
        return Err("p must not be empty".into());
    }
    if batch_size == 0 {
        return Err("batch_size must be positive".into());
    }

    let mut points = Vec::new();
    for distance in distances {
        for round in rounds {
            for p in ps {
                let distance = value_as_usize(distance, "distance entry")?;
                let rounds = value_as_usize(round, "round entry")?;
                if distance < 2 {
                    return Err("distance entry must be >= 2".into());
                }
                if rounds < 1 {
                    return Err("round entry must be >= 1".into());
                }
                points.push(BenchCasePoint {
                    distance,
                    rounds,
                    p: value_as_f64(p, "p entry")?,
                    max_shots,
                    max_errors,
                    batch_size,
                });
            }
        }
    }
    Ok(points)
}

fn require_array<'a>(
    params: &'a BTreeMap<String, Value>,
    key: &str,
) -> Result<&'a Vec<Value>, String> {
    require_param(params, key)?
        .as_array()
        .ok_or_else(|| format!("{key} must be an array"))
}

fn require_u64(params: &BTreeMap<String, Value>, key: &str) -> Result<u64, String> {
    let value = require_param(params, key)?
        .as_integer()
        .ok_or_else(|| format!("{key} must be an integer"))?;
    u64::try_from(value).map_err(|_| format!("{key} must be non-negative"))
}

fn require_usize(params: &BTreeMap<String, Value>, key: &str) -> Result<usize, String> {
    let value = require_param(params, key)?
        .as_integer()
        .ok_or_else(|| format!("{key} must be an integer"))?;
    usize::try_from(value).map_err(|_| format!("{key} must be non-negative"))
}

fn require_param<'a>(params: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a Value, String> {
    params
        .get(key)
        .ok_or_else(|| format!("missing runner param: {key}"))
}

fn value_as_usize(value: &Value, label: &str) -> Result<usize, String> {
    let value = value
        .as_integer()
        .ok_or_else(|| format!("{label} must be an integer"))?;
    usize::try_from(value).map_err(|_| format!("{label} must be non-negative"))
}

fn value_as_f64(value: &Value, label: &str) -> Result<f64, String> {
    if let Some(value) = value.as_float() {
        return Ok(value);
    }
    if let Some(value) = value.as_integer() {
        return Ok(value as f64);
    }
    Err(format!("{label} must be numeric"))
}
