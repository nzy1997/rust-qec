use std::collections::BTreeSet;
use std::fmt;

use qec_code::css::CssCode;

use crate::codegen::NoiseParams;
use crate::ir::{StimInstr, StimTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBasis {
    X,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssSchedule {
    Sequential,
    Greedy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssCheckMatrices {
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
    pub num_data_qubits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssObservableSource {
    Explicit(Vec<Vec<usize>>),
    CanonicalFallback,
    ExplicitOrCanonical(Vec<Vec<usize>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssMemoryConfig {
    pub checks: CssCheckMatrices,
    pub rounds: usize,
    pub noise: NoiseParams,
    pub basis: MemoryBasis,
    pub schedule: CssSchedule,
    pub observables: CssObservableSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssCodegenError {
    InvalidRounds,
    InvalidWidth,
    DuplicateIndex {
        matrix: &'static str,
        row: usize,
        col: usize,
    },
    OutOfRangeIndex {
        matrix: &'static str,
        row: usize,
        col: usize,
        width: usize,
    },
    InvalidCss(String),
    MissingObservables,
    InvalidObservable {
        row: usize,
        col: usize,
        width: usize,
    },
    MixedCanonicalLogical {
        index: usize,
        basis: MemoryBasis,
    },
}

impl fmt::Display for CssCodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRounds => write!(f, "rounds must be >= 1"),
            Self::InvalidWidth => write!(f, "CSS matrices must have at least one data qubit"),
            Self::DuplicateIndex { matrix, row, col } => {
                write!(f, "{matrix} row {row} repeats column {col}")
            }
            Self::OutOfRangeIndex {
                matrix,
                row,
                col,
                width,
            } => write!(
                f,
                "{matrix} row {row} contains out-of-range column {col} for width {width}"
            ),
            Self::InvalidCss(message) => write!(f, "{message}"),
            Self::MissingObservables => {
                write!(f, "canonical logical fallback produced no observables")
            }
            Self::InvalidObservable { row, col, width } => write!(
                f,
                "observable {row} references data qubit {col}, but width is {width}"
            ),
            Self::MixedCanonicalLogical { index, basis } => {
                write!(
                    f,
                    "canonical logical {index} is not pure in memory-{basis:?} basis"
                )
            }
        }
    }
}

impl std::error::Error for CssCodegenError {}

pub fn css_memory(config: CssMemoryConfig) -> Result<Vec<StimInstr>, CssCodegenError> {
    if config.rounds == 0 {
        return Err(CssCodegenError::InvalidRounds);
    }
    validate_supports("hx", &config.checks.hx, config.checks.num_data_qubits)?;
    validate_supports("hz", &config.checks.hz, config.checks.num_data_qubits)?;
    let hx_dense = supports_to_dense(&config.checks.hx, config.checks.num_data_qubits);
    let hz_dense = supports_to_dense(&config.checks.hz, config.checks.num_data_qubits);
    CssCode::from_hx_hz(hx_dense, hz_dense)
        .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?;
    Ok(Vec::new())
}

fn validate_supports(
    matrix: &'static str,
    rows: &[Vec<usize>],
    width: usize,
) -> Result<(), CssCodegenError> {
    if width == 0 {
        return Err(CssCodegenError::InvalidWidth);
    }
    for (row_index, row) in rows.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for &col in row {
            if col >= width {
                return Err(CssCodegenError::OutOfRangeIndex {
                    matrix,
                    row: row_index,
                    col,
                    width,
                });
            }
            if !seen.insert(col) {
                return Err(CssCodegenError::DuplicateIndex {
                    matrix,
                    row: row_index,
                    col,
                });
            }
        }
    }
    Ok(())
}

fn supports_to_dense(rows: &[Vec<usize>], width: usize) -> Vec<Vec<u8>> {
    rows.iter()
        .map(|row| {
            let mut dense = vec![0; width];
            for &col in row {
                dense[col] = 1;
            }
            dense
        })
        .collect()
}

#[allow(dead_code)]
fn op(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}
