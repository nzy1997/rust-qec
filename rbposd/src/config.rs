/// Belief-propagation check-node update rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpVariant {
    /// Minimum-sum approximation.
    MinimumSum,
    /// Product-sum update using the tanh rule.
    ProductSum,
}

/// Order in which belief-propagation messages are updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    /// Update all check messages before updating variable messages.
    Parallel,
    /// Update affected variable messages immediately after each check.
    Serial,
}

/// Ordered-statistics candidate planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdVariant {
    /// Solve the order-zero reduced system.
    Osd0,
    /// Search orders up to `osd_order` over at most 16 free columns.
    LegacyCombinationSweep,
    /// Search all single free columns and pairs in the `osd_order` frontier.
    LdpcCombinationSweep,
}

/// Localized-statistics decoding method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LsdMethod {
    /// Deterministic localized-statistics post-processing.
    LocalizedStatistics,
}

/// Independent bit-flip channel probabilities used to initialize BP.
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelModel {
    /// One bit-flip probability shared by all bits.
    Bsc { error_rate: f64 },
    /// One bit-flip probability per matrix bit.
    BitFlipProbabilities(Vec<f64>),
}

/// Configuration for [`crate::BpOsdDecoder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderConfig {
    /// Maximum number of BP iterations; zero skips BP iterations.
    pub max_bp_iterations: usize,
    /// Stop BP as soon as its correction satisfies the syndrome.
    pub early_stop: bool,
    /// Check-node update rule.
    pub bp_variant: BpVariant,
    /// Message update schedule.
    pub schedule: Schedule,
    /// OSD candidate planner.
    pub osd_variant: OsdVariant,
    /// Candidate-search order or frontier size, depending on `osd_variant`.
    pub osd_order: usize,
}

/// Configuration for [`crate::BpLsdDecoder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LsdConfig {
    /// Localized-statistics algorithm.
    pub method: LsdMethod,
    /// LSD order; only 0 and 1 are supported.
    pub lsd_order: usize,
}

impl OsdVariant {
    /// Parse a supported external OSD method name.
    pub fn from_method_name(method: &str) -> Result<Self, crate::error::DecodeError> {
        match method {
            "combination_sweep" | "legacy_combination_sweep" => Ok(Self::LegacyCombinationSweep),
            "ldpc_osd_cs" | "osd_cs" => Ok(Self::LdpcCombinationSweep),
            other => Err(crate::error::DecodeError::UnsupportedOsdMethod {
                method: other.to_string(),
            }),
        }
    }

    /// Return the stable diagnostic name for this planner.
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
