#[cfg(feature = "gurobi")]
mod gurobi;
mod highs;

use std::fmt::Debug;

use crate::config::{BackendKind, BinaryIlpConfig};
use crate::error::BinaryIlpError;
use crate::model::{BinaryIlpModel, ModelSolution};

pub trait BinaryBackend: Debug {
    fn solve(&mut self) -> Result<ModelSolution, BinaryIlpError>;
    fn set_rhs(&mut self, row: usize, rhs: f64) -> Result<(), BinaryIlpError>;
}

pub fn build_binary_backend(
    model: &BinaryIlpModel,
    config: &BinaryIlpConfig,
) -> Result<Box<dyn BinaryBackend>, BinaryIlpError> {
    model.validate()?;
    match config.backend.kind {
        BackendKind::Highs => Ok(Box::new(highs::HighsBinaryBackend::new(model, config)?)),
        BackendKind::Auto => build_auto_backend(model, config),
        BackendKind::Gurobi => build_gurobi_backend(model, config),
    }
}

fn build_auto_backend(
    model: &BinaryIlpModel,
    config: &BinaryIlpConfig,
) -> Result<Box<dyn BinaryBackend>, BinaryIlpError> {
    #[cfg(feature = "gurobi")]
    if let Ok(backend) = gurobi::GurobiBinaryBackend::new(model, config) {
        return Ok(Box::new(backend));
    }

    Ok(Box::new(highs::HighsBinaryBackend::new(model, config)?))
}

fn build_gurobi_backend(
    model: &BinaryIlpModel,
    config: &BinaryIlpConfig,
) -> Result<Box<dyn BinaryBackend>, BinaryIlpError> {
    #[cfg(feature = "gurobi")]
    {
        return Ok(Box::new(gurobi::GurobiBinaryBackend::new(model, config)?));
    }

    #[cfg(not(feature = "gurobi"))]
    {
        let _ = (model, config);
        Err(BinaryIlpError::BackendUnavailable {
            requested: BackendKind::Gurobi,
        })
    }
}
