#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpVariant {
    MinimumSum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    Parallel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdVariant {
    Osd0,
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
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            max_bp_iterations: 30,
            early_stop: true,
            bp_variant: BpVariant::MinimumSum,
            schedule: Schedule::Parallel,
            osd_variant: OsdVariant::Osd0,
        }
    }
}
