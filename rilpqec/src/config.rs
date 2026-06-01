#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Auto,
    Highs,
    Gurobi,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackendConfig {
    pub kind: BackendKind,
    pub time_limit_seconds: Option<f64>,
    pub mip_gap: Option<f64>,
    pub threads: Option<u32>,
    pub verbose: bool,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            kind: BackendKind::Auto,
            time_limit_seconds: None,
            mip_gap: None,
            threads: None,
            verbose: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IlpDecoderConfig {
    pub backend: BackendConfig,
}
