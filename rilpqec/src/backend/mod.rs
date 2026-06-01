#[cfg(feature = "gurobi")]
mod gurobi;
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
        BackendKind::Auto => build_auto_backend(problem, config),
        BackendKind::Gurobi => build_gurobi_backend(problem, config),
    }
}

fn build_auto_backend(
    problem: &LoweredDemProblem,
    config: &IlpDecoderConfig,
) -> Result<Box<dyn BatchBackend>, IlpDecodeError> {
    #[cfg(feature = "gurobi")]
    if let Ok(backend) = gurobi::GurobiBatchBackend::new(problem, config) {
        return Ok(Box::new(backend));
    }

    Ok(Box::new(highs::HighsBatchBackend::new(problem, config)?))
}

fn build_gurobi_backend(
    problem: &LoweredDemProblem,
    config: &IlpDecoderConfig,
) -> Result<Box<dyn BatchBackend>, IlpDecodeError> {
    #[cfg(feature = "gurobi")]
    {
        return Ok(Box::new(gurobi::GurobiBatchBackend::new(problem, config)?));
    }

    #[cfg(not(feature = "gurobi"))]
    {
        let _ = (problem, config);
        Err(IlpDecodeError::BackendUnavailable {
            requested: BackendKind::Gurobi,
        })
    }
}
