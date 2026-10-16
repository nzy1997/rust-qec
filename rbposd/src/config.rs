#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpVariant {
    MinimumSum,
    ProductSum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    Parallel,
    Serial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdVariant {
    Osd0,
    LegacyCombinationSweep,
    LdpcCombinationSweep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LsdMethod {
    LocalizedStatistics,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelModel {
    Bsc { error_rate: f64 },
    BitFlipProbabilities(Vec<f64>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderConfig {
    pub max_bp_iterations: usize,
    pub early_stop: bool,
    pub bp_variant: BpVariant,
    pub schedule: Schedule,
    pub osd_variant: OsdVariant,
    pub osd_order: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LsdConfig {
    pub method: LsdMethod,
    pub lsd_order: usize,
}

impl OsdVariant {
    pub fn from_method_name(method: &str) -> Result<Self, crate::error::DecodeError> {
        match method {
            "combination_sweep" | "legacy_combination_sweep" => Ok(Self::LegacyCombinationSweep),
            "ldpc_osd_cs" | "osd_cs" => Ok(Self::LdpcCombinationSweep),
            other => Err(crate::error::DecodeError::UnsupportedOsdMethod {
                method: other.to_string(),
            }),
        }
    }

    pub fn planner_name(self) -> &'static str {
        match self {
            Self::Osd0 => "osd0",
            Self::LegacyCombinationSweep => "legacy_combination_sweep",
            Self::LdpcCombinationSweep => "ldpc_osd_cs",
        }
    }
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            max_bp_iterations: 30,
            early_stop: true,
            bp_variant: BpVariant::MinimumSum,
            schedule: Schedule::Parallel,
            osd_variant: OsdVariant::Osd0,
            osd_order: 0,
        }
    }
}

impl Default for LsdConfig {
    fn default() -> Self {
        Self {
            method: LsdMethod::LocalizedStatistics,
            lsd_order: 0,
        }
    }
}
