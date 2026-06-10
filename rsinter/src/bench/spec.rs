use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct BenchmarkSpec {
    pub name: String,
    pub version: u64,
    pub mode: BenchmarkMode,
    #[serde(default, rename = "runner")]
    pub runners: Vec<RunnerSpec>,
    pub plot: PlotSpec,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RunnerSpec {
    pub name: String,
    pub language: String,
    pub impl_key: String,
    #[serde(default)]
    pub params: toml::Table,
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
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Independent => "independent",
        }
    }
}
