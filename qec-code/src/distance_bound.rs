use crate::Pauli;
use crate::binary::try_in_row_span;
use crate::code::StabilizerCode;
use crate::distance::LogicalClass;
use crate::error::{QecError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistanceBoundMethod {
    RandomizedUpperBound,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BoundType {
    Upper,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceBoundStatus {
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomizedUpperBoundOptions {
    pub iterations: usize,
    pub restarts: usize,
    pub seed: u64,
    pub target_weight: Option<usize>,
}

impl RandomizedUpperBoundOptions {
    pub fn validate(&self) -> Result<()> {
        if self.iterations == 0 {
            return Err(QecError::InvalidDistanceBoundOption {
                option: "iterations",
                reason: "must be greater than zero".to_owned(),
            });
        }
        if self.restarts == 0 {
            return Err(QecError::InvalidDistanceBoundOption {
                option: "restarts",
                reason: "must be greater than zero".to_owned(),
            });
        }
        if self.target_weight == Some(0) {
            return Err(QecError::InvalidDistanceBoundOption {
                option: "target_weight",
                reason: "must be greater than zero when provided".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundWitness {
    pub x: Vec<u8>,
    pub z: Vec<u8>,
    pub weight: usize,
}

impl DistanceBoundWitness {
    pub fn from_pauli(pauli: &Pauli) -> Self {
        Self {
            x: pauli.x_bits().to_vec(),
            z: pauli.z_bits().to_vec(),
            weight: pauli.weight(),
        }
    }

    pub fn to_pauli(&self) -> Result<Pauli> {
        Pauli::from_xz_bits(self.x.clone(), self.z.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundProvenance {
    pub tool: String,
    pub tool_version: String,
    pub method_revision: u32,
}

impl DistanceBoundProvenance {
    pub fn current() -> Self {
        Self {
            tool: "qec-code".to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            method_revision: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceBoundResult {
    pub status: DistanceBoundStatus,
    pub method: DistanceBoundMethod,
    pub bound_type: BoundType,
    pub upper_bound: usize,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    pub options: RandomizedUpperBoundOptions,
    pub provenance: DistanceBoundProvenance,
}

impl DistanceBoundResult {
    pub fn completed(
        upper_bound: usize,
        logical_class: LogicalClass,
        witness: DistanceBoundWitness,
        options: RandomizedUpperBoundOptions,
    ) -> Self {
        Self {
            status: DistanceBoundStatus::Completed,
            method: DistanceBoundMethod::RandomizedUpperBound,
            bound_type: BoundType::Upper,
            upper_bound,
            logical_class,
            witness,
            options,
            provenance: DistanceBoundProvenance::current(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoundValidationContext<'a> {
    pub code: &'a StabilizerCode,
    pub known_exact_distance: Option<usize>,
}

pub fn validate_randomized_upper_bound_result(
    result: &DistanceBoundResult,
    context: BoundValidationContext<'_>,
) -> Result<()> {
    if result.method != DistanceBoundMethod::RandomizedUpperBound {
        return Err(QecError::DistanceBoundValidationFailed(
            "distance bound method must be randomized-upper-bound".to_owned(),
        ));
    }
    if result.bound_type != BoundType::Upper {
        return Err(QecError::DistanceBoundValidationFailed(
            "randomized-upper-bound results must use bound_type upper".to_owned(),
        ));
    }
    if result.upper_bound == 0 {
        return Err(QecError::DistanceBoundValidationFailed(
            "completed upper_bound must be positive".to_owned(),
        ));
    }
    if result.upper_bound != result.witness.weight {
        return Err(QecError::DistanceBoundValidationFailed(
            "upper_bound must equal witness weight".to_owned(),
        ));
    }

    let witness = result.witness.to_pauli()?;
    validate_witness_against_code(context.code, &witness)?;

    if let Some(known_exact_distance) = context.known_exact_distance {
        if result.upper_bound < known_exact_distance {
            return Err(QecError::DistanceBoundValidationFailed(format!(
                "upper_bound {} is below known exact distance {}",
                result.upper_bound, known_exact_distance
            )));
        }
    }

    Ok(())
}

fn validate_witness_against_code(code: &StabilizerCode, witness: &Pauli) -> Result<()> {
    if witness.weight() == 0 {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness must be non-identity".to_owned(),
        ));
    }
    for stabilizer in code.stabilizers() {
        if !witness.try_commutes_with(stabilizer)? {
            return Err(QecError::DistanceBoundValidationFailed(
                "witness does not commute with stabilizers".to_owned(),
            ));
        }
    }
    if try_in_row_span(&code.stabilizer_rows(), &witness.to_symplectic_row())? {
        return Err(QecError::DistanceBoundValidationFailed(
            "witness lies in stabilizer span".to_owned(),
        ));
    }
    Ok(())
}
