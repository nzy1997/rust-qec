mod highs;

use crate::config::{BackendKind, IlpDecoderConfig};
use crate::error::IlpDecodeError;
use crate::problem::LoweredDemProblem;

pub trait BatchBackend {
    fn solve(&mut self, syndrome: &[bool]) -> Result<Vec<bool>, IlpDecodeError>;
}

pub fn build_batch_backend(
    problem: &LoweredDemProblem,
    config: &IlpDecoderConfig,
) -> Result<Box<dyn BatchBackend>, IlpDecodeError> {
    match config.backend.kind {
        BackendKind::Highs => Ok(Box::new(highs::HighsBatchBackend::new(problem, config)?)),
        BackendKind::Auto | BackendKind::Gurobi => Err(IlpDecodeError::BackendUnavailable {
            requested: BackendKind::Gurobi,
        }),
    }
}
