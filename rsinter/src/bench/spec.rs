use std::collections::BTreeMap;

use serde::Deserialize;

pub const DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR: f64 = 9.0;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct BenchmarkSpec {
    pub name: String,
    pub version: u64,
    pub mode: BenchmarkMode,
    #[serde(rename = "runner")]
    pub runners: Vec<RunnerSpec>,
    pub plot: PlotSpec,
}

impl BenchmarkSpec {
    pub fn validate(&self) -> Result<(), String> {
        if self.runners.is_empty() {
            return Err("benchmark spec must declare at least one runner".into());
        }
        if self.plot.panels.is_empty() {
            return Err("benchmark spec must declare at least one plot panel".into());
        }
        if !self.plot.confidence_interval_likelihood_factor.is_finite()
            || self.plot.confidence_interval_likelihood_factor < 1.0
        {
            return Err(
                "plot confidence_interval_likelihood_factor must be finite and >= 1.0".into(),
            );
        }
        for runner in &self.runners {
            if runner.name.trim().is_empty() {
                return Err("runner name must not be empty".into());
            }
            if runner.impl_key.trim().is_empty() {
                return Err(format!("runner {} must declare impl_key", runner.name));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RunnerSpec {
    pub name: String,
    pub language: String,
    pub impl_key: String,
    pub params: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PlotSpec {
    pub title: String,
    #[serde(default = "default_confidence_interval_likelihood_factor")]
    pub confidence_interval_likelihood_factor: f64,
    #[serde(default)]
    pub logical_rate_unit: LogicalRateUnit,
    pub x: AxisSpec,
    pub series: SeriesSpec,
    #[serde(default, rename = "panel")]
    pub panels: Vec<PanelSpec>,
}

fn default_confidence_interval_likelihood_factor() -> f64 {
    DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogicalRateUnit {
    PerShot,
    PerRound,
    PerObservable,
    PerRoundPerObservable,
}

impl Default for LogicalRateUnit {
    fn default() -> Self {
        Self::PerShot
    }
}

impl LogicalRateUnit {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PerShot => "per_shot",
            Self::PerRound => "per_round",
            Self::PerObservable => "per_observable",
            Self::PerRoundPerObservable => "per_round_per_observable",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AxisSpec {
    pub field: String,
    pub scale: String,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SeriesSpec {
    #[serde(default)]
    pub group_by: Vec<String>,
    pub label_template: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PanelSpec {
    pub metric: String,
    pub scale: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkMode {
    Independent,
}

impl BenchmarkMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Independent => "independent",
        }
    }
}
