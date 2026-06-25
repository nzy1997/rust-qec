#[cfg(feature = "gurobi")]
mod gurobi;
mod highs;

use crate::config::{BackendKind, BinaryIlpConfig};
use crate::error::BinaryIlpError;
use crate::model::{BinaryIlpModel, ModelSolution};

pub trait BinaryBackend {
    fn kind(&self) -> BackendKind;
    fn solve(&mut self) -> Result<ModelSolution, BinaryIlpError>;
    fn set_rhs(&mut self, row: usize, rhs: f64) -> Result<(), BinaryIlpError>;
}

impl std::fmt::Debug for dyn BinaryBackend + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BinaryBackend(..)")
    }
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
