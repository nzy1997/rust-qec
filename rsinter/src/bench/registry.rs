use std::collections::BTreeMap;

use toml::Value;

use crate::bench::result::BenchmarkResultRow;
use crate::bench::runners::rbposd::RbposdRunner;
use crate::bench::runners::rilpqec::RilpqecRunner;
use crate::bench::runners::rmatching::RmatchingRunner;

pub struct BenchCasePoint {
    pub input_type: String,
    pub code_id: Option<String>,
    pub distance: Option<usize>,
    pub rounds: usize,
    pub p: f64,
    pub basis: Option<String>,
    pub schedule: Option<String>,
    pub hx_path: Option<String>,
    pub hz_path: Option<String>,
    pub observables_path: Option<String>,
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
    let input_type =
        optional_string(params, "input_type")?.unwrap_or_else(|| "surface_rotated_memory_x".into());
    let rounds = require_array(params, "rounds")?;
    let ps = require_array(params, "p")?;
    let max_shots = require_u64(params, "max_shots")?;
    let max_errors = require_u64(params, "max_errors")?;
    let batch_size = require_usize(params, "batch_size")?;
    if rounds.is_empty() {
        return Err("rounds must not be empty".into());
    }
    if ps.is_empty() {
        return Err("p must not be empty".into());
    }
    if batch_size == 0 {
        return Err("batch_size must be positive".into());
    }

    match input_type.as_str() {
        "surface_rotated_memory_x" => {
            expand_surface_points(params, rounds, ps, max_shots, max_errors, batch_size)
        }
        "css" => expand_css_points(params, rounds, ps, max_shots, max_errors, batch_size),
        other => Err(format!("unknown input_type: {other}")),
    }
}

fn expand_surface_points(
    params: &BTreeMap<String, Value>,
    rounds: &[Value],
    ps: &[Value],
    max_shots: u64,
    max_errors: u64,
    batch_size: usize,
) -> Result<Vec<BenchCasePoint>, String> {
    let distances = require_array(params, "distance")?;
    if distances.is_empty() {
        return Err("distance must not be empty".into());
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
                    input_type: "surface_rotated_memory_x".into(),
                    code_id: None,
                    distance: Some(distance),
                    rounds,
                    p: value_as_f64(p, "p entry")?,
                    basis: None,
                    schedule: None,
                    hx_path: None,
                    hz_path: None,
                    observables_path: None,
                    max_shots,
                    max_errors,
                    batch_size,
                });
            }
        }
    }
    Ok(points)
}

fn expand_css_points(
    params: &BTreeMap<String, Value>,
    rounds: &[Value],
    ps: &[Value],
    max_shots: u64,
    max_errors: u64,
    batch_size: usize,
) -> Result<Vec<BenchCasePoint>, String> {
    let basis = require_string(params, "basis")?;
    let schedule = optional_string(params, "schedule")?.unwrap_or_else(|| "greedy".to_string());
    let hx_path = require_string(params, "hx")?;
    let hz_path = require_string(params, "hz")?;
    let observables_path = optional_string(params, "observables")?;
    let code_id = optional_string(params, "code_id")?;

    let mut points = Vec::new();
    for round in rounds {
        for p in ps {
            let rounds = value_as_usize(round, "round entry")?;
            if rounds < 1 {
                return Err("round entry must be >= 1".into());
            }
            points.push(BenchCasePoint {
                input_type: "css".into(),
                code_id: code_id.clone(),
                distance: None,
                rounds,
                p: value_as_f64(p, "p entry")?,
                basis: Some(basis.clone()),
                schedule: Some(schedule.clone()),
                hx_path: Some(hx_path.clone()),
                hz_path: Some(hz_path.clone()),
                observables_path: observables_path.clone(),
                max_shots,
                max_errors,
                batch_size,
            });
        }
    }
    Ok(points)
}

fn optional_string(params: &BTreeMap<String, Value>, key: &str) -> Result<Option<String>, String> {
    match params.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(str::to_string)
            .map(Some)
            .ok_or_else(|| format!("{key} must be a string")),
    }
}

fn require_string(params: &BTreeMap<String, Value>, key: &str) -> Result<String, String> {
    require_param(params, key)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{key} must be a string"))
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
