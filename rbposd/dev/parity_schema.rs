use std::fs;
use std::path::{Path, PathBuf};

use rbposd::{
    BpOsdDecoder, BpVariant, ChannelModel, DecodeError, DecodeResult, DecoderConfig, OsdVariant,
    ParityCheckMatrix, Schedule, Syndrome,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuccessDiagnostics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub converged: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bp_iterations: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_osd: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_syndrome_weight: Option<usize>,
}

impl SuccessDiagnostics {
    fn matches_actual(&self, actual: &Self) -> bool {
        self.converged
            .map_or(true, |value| actual.converged == Some(value))
            && self
                .bp_iterations
                .map_or(true, |value| actual.bp_iterations == Some(value))
            && self
                .used_osd
                .map_or(true, |value| actual.used_osd == Some(value))
            && self
                .residual_syndrome_weight
                .map_or(true, |value| actual.residual_syndrome_weight == Some(value))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ParityOutcome {
    Success {
        correction: Vec<bool>,
        #[serde(default)]
        diagnostics: SuccessDiagnostics,
    },
    Error {
        error: String,
    },
}

impl ParityOutcome {
    pub fn from_decode_result(result: DecodeResult) -> Self {
        Self::Success {
            correction: result.correction.as_slice().to_vec(),
            diagnostics: SuccessDiagnostics {
                converged: Some(result.converged),
                bp_iterations: Some(result.bp_iterations),
                used_osd: Some(result.used_osd),
                residual_syndrome_weight: Some(result.residual_syndrome_weight),
            },
        }
    }

    pub fn from_decode_error(error: DecodeError) -> Self {
        Self::Error {
            error: error_code(&error).to_string(),
        }
    }

    pub fn matches_actual(&self, actual: &Self) -> bool {
        match (self, actual) {
            (
                Self::Success {
                    correction: expected_correction,
                    diagnostics: expected_diagnostics,
                },
                Self::Success {
                    correction: actual_correction,
                    diagnostics: actual_diagnostics,
                },
            ) => {
                expected_correction == actual_correction
                    && expected_diagnostics.matches_actual(actual_diagnostics)
            }
            (Self::Error { error: expected }, Self::Error { error: actual }) => expected == actual,
            _ => false,
        }
    }
}

fn error_code(error: &DecodeError) -> &'static str {
    match error {
        DecodeError::EmptyMatrix => "EmptyMatrix",
        DecodeError::InvalidProbability => "InvalidProbability",
        DecodeError::InvalidColumnIndex { .. } => "InvalidColumnIndex",
        DecodeError::InvalidRowIndex { .. } => "InvalidRowIndex",
        DecodeError::DimensionMismatch { .. } => "DimensionMismatch",
        DecodeError::SingularSystem => "SingularSystem",
        DecodeError::BpDidNotConverge => "BpDidNotConverge",
        DecodeError::NoOsdSolution => "NoOsdSolution",
        DecodeError::UnsupportedLsdOrder { .. } => "UnsupportedLsdOrder",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixSpec {
    pub num_checks: usize,
    pub num_bits: usize,
    pub rows: Vec<Vec<usize>>,
}

impl MatrixSpec {
    fn build(&self) -> Result<ParityCheckMatrix, DecodeError> {
        ParityCheckMatrix::from_sparse_rows(self.num_checks, self.num_bits, self.rows.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChannelSpec {
    Bsc { error_rate: f64 },
    BitFlipProbabilities { probabilities: Vec<f64> },
}

impl ChannelSpec {
    fn build(&self) -> ChannelModel {
        match self {
            Self::Bsc { error_rate } => ChannelModel::Bsc {
                error_rate: *error_rate,
            },
            Self::BitFlipProbabilities { probabilities } => {
                ChannelModel::BitFlipProbabilities(probabilities.clone())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BpVariantSpec {
    MinimumSum,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleSpec {
    Parallel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsdVariantSpec {
    Osd0,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConfigSpec {
    pub max_bp_iterations: usize,
    pub early_stop: bool,
    pub bp_variant: BpVariantSpec,
    pub schedule: ScheduleSpec,
    pub osd_variant: OsdVariantSpec,
}

impl ConfigSpec {
    fn build(&self) -> DecoderConfig {
        DecoderConfig {
            max_bp_iterations: self.max_bp_iterations,
            early_stop: self.early_stop,
            bp_variant: match self.bp_variant {
                BpVariantSpec::MinimumSum => BpVariant::MinimumSum,
            },
            schedule: match self.schedule {
                ScheduleSpec::Parallel => Schedule::Parallel,
            },
            osd_variant: match self.osd_variant {
                OsdVariantSpec::Osd0 => OsdVariant::Osd0,
            },
            osd_order: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParityCase {
    pub name: String,
    pub matrix: MatrixSpec,
    pub channel: ChannelSpec,
    pub syndrome: Vec<bool>,
    pub config: ConfigSpec,
    #[serde(default)]
    pub expected: Option<ParityOutcome>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ParityCase {
    pub fn build_decoder(&self) -> Result<BpOsdDecoder, DecodeError> {
        BpOsdDecoder::new(
            self.matrix.build()?,
            self.channel.build(),
            self.config.build(),
        )
    }

    pub fn syndrome(&self) -> Syndrome {
        Syndrome::from(self.syndrome.clone())
    }
}

pub fn load_case(path: &Path) -> ParityCase {
    serde_json::from_str(&fs::read_to_string(path).unwrap())
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

pub fn load_cases(dir: &Path) -> Vec<ParityCase> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect();
    paths.sort();
    paths.into_iter().map(|path| load_case(&path)).collect()
}
