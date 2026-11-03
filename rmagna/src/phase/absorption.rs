//! Symbolic contraction of paths invisible to physical and odd-phase labels.
use super::*;

/// Admission bounds for raw Clifford-radical absorption. These do not constrain
/// a subsequent resource selection, which the caller requests separately.
#[derive(Clone, Copy, Debug)]
pub struct RadicalAbsorptionOptions {
    pub max_physical_bits: usize,
    pub max_path_bits: usize,
    /// Maximum number of actual hidden-path contractions. Fixed-point analysis
    /// is allowed with zero stages and is still charged to the work bound.
    pub max_stages: usize,
    /// Sum of conservative polynomial charges, one per analyzed tensor.
    /// This is a declared work model, not measured instruction counts.
    pub max_work_units: u128,
    /// Conservative peak logical words for copies, packed vectors, quadratic
    /// matrices, elimination provenance, phase terms and the stage history.
    /// Excludes scalar limbs, allocator overhead and caller-owned storage.
    pub max_geometry_words: u128,
}
impl Default for RadicalAbsorptionOptions {
    fn default() -> Self {
        Self {
            max_physical_bits: 65_536,
            max_path_bits: 4096,
            max_stages: 4096,
            max_work_units: 1 << 40,
            max_geometry_words: 16 << 20,
        }
    }
}

/// One executed Clifford contraction; no resource decomposition is performed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadicalAbsorptionStage {
    pub input_paths: usize,
    pub signature_rank: usize,
    pub joint_boundary_rank: usize,
    pub hidden_paths: usize,
    pub output_paths: usize,
    pub zero: bool,
}

/// Counts calls made by absorption, separately from its conservative charges.
/// Internal row operations inside the called algebra routines are not counted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadicalAbsorptionDiagnostics {
    pub quotient_analyses: usize,
    pub joint_factorizations: usize,
    pub clifford_contractions: usize,
    pub declared_work_units: u128,
    pub declared_peak_geometry_words: u128,
    pub stages: Vec<RadicalAbsorptionStage>,
}

/// An owned exact raw tensor and immutable access to its operation diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliffordRadicalAbsorption {
    tensor: PhasePathTensor,
    diagnostics: RadicalAbsorptionDiagnostics,
}
impl CliffordRadicalAbsorption {
    pub fn tensor(&self) -> &PhasePathTensor {
        &self.tensor
    }
    pub fn into_tensor(self) -> PhasePathTensor {
        self.tensor
    }
    pub fn diagnostics(&self) -> &RadicalAbsorptionDiagnostics {
        &self.diagnostics
    }
}

fn cap(name: &'static str, required: u128, limit: u128) -> Result<(), PathTensorError> {
    if required > limit {
        return Err(PhaseError::ResourceLimit {
            option: name,
            required,
            limit,
        }
        .into());
    }
    Ok(())
}
fn add(x: u128, y: u128) -> Result<u128, PathTensorError> {
    x.checked_add(y).ok_or(PathTensorError::DimensionOverflow)
}
fn mul(x: u128, y: u128) -> Result<u128, PathTensorError> {
    x.checked_mul(y).ok_or(PathTensorError::DimensionOverflow)
}

// Admit before cloning or calling either factorization. The joint boundary has
// at most m+n rows. Include full row-provenance storage for that factorization
// and for the at-most-t odd axes, as well as the n-by-n quadratic scratch.
fn charges(m: usize, n: usize, t: usize, history: usize) -> Result<(u128, u128), PathTensorError> {
    let j = m.checked_add(n).ok_or(PathTensorError::DimensionOverflow)? as u128;
    let n = n as u128;
    let t = t as u128;
    let size = add(add(add(j, n)?, t)?, 1)?;
    let packed = add(add(n.div_ceil(64), j.div_ceil(64))?, t.div_ceil(64))?;
    let geometry = add(
        add(
            mul(mul(64, size)?, add(packed, 8)?)?,
            mul(16, mul(add(n, 1)?, add(n, 1)?)?)?,
        )?,
        mul(8, history as u128)?,
    )?;
    let work = mul(
        mul(mul(64, mul(add(n, 1)?, add(n, 1)?)?)?, size)?,
        add(packed, 1)?,
    )?;
    Ok((geometry, work))
}

fn zero_tensor(input: &PhasePathTensor) -> Result<PhasePathTensor, PathTensorError> {
    PhasePathTensor::new(
        AffineKet::new(
            input.boundary().offset().clone(),
            Vec::new(),
            QuadraticForm::new(0, Vec::new(), Vec::new())?,
            ExactScalar::zero(),
        )?,
        ParityPhase::new(0, 0, [])?,
    )
}

impl PhasePathTensor {
    /// Sum only directions in the joint kernel of the physical label map and
    /// the actual third-derivative signature quotient. Restrict the remaining
    /// parity function after contraction, then repeat if paths shrink.
    ///
    /// Arbitrary complex coefficients and physical label order are preserved.
    /// No phase resource, table or stabilizer expansion is constructed here.
    /// This produces a safe fixed point, not an optimal stabilizer-rank claim.
    /// Errors return no partial result and leave the borrowed input unchanged.
    pub fn absorb_clifford_radical(
        &self,
        options: RadicalAbsorptionOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CliffordRadicalAbsorption, PathTensorError> {
        let m = self.boundary().width();
        let initial = self.phase().num_variables();
        cap(
            "radical_absorption.max_physical_bits",
            m as u128,
            options.max_physical_bits as u128,
        )?;
        cap(
            "radical_absorption.max_path_bits",
            initial as u128,
            options.max_path_bits as u128,
        )?;
        let (geometry, work) = charges(m, initial, self.phase().terms().len(), initial)?;
        cap(
            "radical_absorption.max_geometry_words",
            geometry,
            options.max_geometry_words,
        )?;
        if !self.boundary().coefficient().is_zero() {
            cap(
                "radical_absorption.max_work_units",
                work,
                options.max_work_units,
            )?;
        }
        arithmetic.multiply(self.boundary().coefficient(), &ExactScalar::one())?;
        let mut diagnostics = RadicalAbsorptionDiagnostics {
            declared_peak_geometry_words: geometry,
            ..Default::default()
        };
        if self.boundary().coefficient().is_zero() {
            return Ok(CliffordRadicalAbsorption {
                tensor: zero_tensor(self)?,
                diagnostics,
            });
        }
        let mut tensor = self.clone();
        loop {
            let boundary = tensor.boundary();
            let phase = tensor.phase();
            let n = phase.num_variables();
            let (geometry, work) = charges(m, n, phase.terms().len(), initial)?;
            cap(
                "radical_absorption.max_geometry_words",
                geometry,
                options.max_geometry_words,
            )?;
            let total = add(diagnostics.declared_work_units, work)?;
            cap(
                "radical_absorption.max_work_units",
                total,
                options.max_work_units,
            )?;
            diagnostics.declared_work_units = total;
            diagnostics.declared_peak_geometry_words =
                diagnostics.declared_peak_geometry_words.max(geometry);
            let quotient = phase.quotient()?;
            diagnostics.quotient_analyses += 1;
            let rank = quotient.rank();
            if rank == n {
                break;
            }
            let joint_width = m
                .checked_add(rank)
                .ok_or(PathTensorError::DimensionOverflow)?;
            let columns = quotient
                .basis()
                .iter()
                .enumerate()
                .map(|(j, vector)| {
                    let mut physical = BitVector::zeros(m)?;
                    for k in vector.ones() {
                        physical.xor_assign(&boundary.columns()[k])?;
                    }
                    BitVector::from_indices(
                        joint_width,
                        physical.ones().chain((j < rank).then_some(m + j)),
                    )
                })
                .collect::<Result<Vec<_>, Gf2Error>>()?;
            let factor = LinearFactorization::new(n, &transpose_columns(joint_width, &columns)?)?;
            diagnostics.joint_factorizations += 1;
            let joint_rank = factor.rank();
            if joint_rank == n {
                break;
            }
            cap(
                "radical_absorption.max_stages",
                diagnostics.stages.len() as u128 + 1,
                options.max_stages as u128,
            )?;
            let zero = BitVector::zeros(n)?;
            let resource = phase.restrict(&zero, quotient.complement())?;
            let lifted = ParityPhase::new(
                n,
                resource.constant(),
                resource
                    .terms()
                    .iter()
                    .map(|(axis, c)| Ok((BitVector::from_indices(n, axis.ones())?, *c)))
                    .collect::<Result<Vec<_>, Gf2Error>>()?,
            )?;
            let (global, correction) = phase
                .restrict(&zero, quotient.basis())?
                .subtract(&lifted)?
                .clifford()?;
            let quadratic = add_quadratic(
                &boundary.phase().restrict(&zero, quotient.basis())?,
                &correction,
            )?;
            let joint = AffineKet::new(
                BitVector::from_indices(joint_width, boundary.offset().ones())?,
                columns,
                quadratic,
                arithmetic.multiply(
                    boundary.coefficient(),
                    &arithmetic.omega(i64::from(global), 0)?,
                )?,
            )?;
            let contracted = joint.contract_with_basis(
                factor.pivot_columns(),
                &factor.nullspace()?,
                arithmetic,
            )?;
            diagnostics.clifford_contractions += 1;
            let output_paths = contracted.as_ref().map_or(0, |ket| ket.columns().len());
            diagnostics.stages.push(RadicalAbsorptionStage {
                input_paths: n,
                signature_rank: rank,
                joint_boundary_rank: joint_rank,
                hidden_paths: n - joint_rank,
                output_paths,
                zero: contracted.is_none(),
            });
            let Some(ket) = contracted else {
                tensor = zero_tensor(&tensor)?;
                break;
            };
            // Every contraction has a nonempty joint kernel. Elimination must
            // strictly shrink its path set, so the loop cannot repeat forever.
            if output_paths >= n {
                return Err(PhaseError::DimensionMismatch.into());
            }
            let tail = |v: &BitVector| {
                BitVector::from_indices(rank, v.ones().filter(|&k| k >= m).map(|k| k - m))
            };
            let offset = tail(ket.offset())?;
            let columns = ket
                .columns()
                .iter()
                .map(tail)
                .collect::<Result<Vec<_>, _>>()?;
            tensor = PhasePathTensor::new(
                AffineKet::new(
                    ket.offset().low(m)?,
                    ket.columns()
                        .iter()
                        .map(|v| v.low(m))
                        .collect::<Result<Vec<_>, _>>()?,
                    ket.phase().clone(),
                    ket.coefficient().clone(),
                )?,
                resource.restrict(&offset, &columns)?,
            )?;
        }
        Ok(CliffordRadicalAbsorption {
            tensor,
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn impossible_shape_charge_overflow_is_typed_without_allocation() {
        assert_eq!(
            charges(usize::MAX, 1, 0, 0),
            Err(PathTensorError::DimensionOverflow)
        );
        assert_eq!(
            charges(0, usize::MAX, usize::MAX, 0),
            Err(PathTensorError::DimensionOverflow)
        );
    }
}
