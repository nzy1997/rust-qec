use std::collections::BTreeMap;

use serde::Deserialize;

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
    pub x: AxisSpec,
    pub series: SeriesSpec,
    #[serde(default, rename = "panel")]
    pub panels: Vec<PanelSpec>,
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
