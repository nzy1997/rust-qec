//! Normalized independent stabilizer inputs for the unitary compiler.
use super::*;
use crate::gf2::{LinearFactorization, transpose_columns};

impl UnitaryCircuit {
    /// Compile a normalized independent affine quadratic stabilizer input with
    /// the parity-copy lowering. The caller's input is retained by value in the
    /// compiled boundary; it is never contracted to repair invalid input.
    pub fn compile_stabilizer(
        &self,
        input: &AffineKet,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompiledUnitary, UnitaryError> {
        self.compile_stabilizer_with_strategy(input, arithmetic, CompilerStrategy::ParityCopies)
    }

    /// Select either global lowering for a normalized stabilizer input.
    pub fn compile_stabilizer_with_strategy(
        &self,
        input: &AffineKet,
        arithmetic: &ScalarArithmetic,
        strategy: CompilerStrategy,
    ) -> Result<CompiledUnitary, UnitaryError> {
        self.compile_stabilizer_with_path_limit(input, arithmetic, strategy, usize::MAX)
    }

    /// Validate width, nonzero coefficient, independent columns and exact unit
    /// norm before lowering. Raw paths include the initial support rank and all
    /// lowered Hadamards, including observation copies, before contraction.
    /// Arbitrary independent column order and all quadratic/global phases are
    /// preserved. Arithmetic failures propagate without changing the input.
    pub fn compile_stabilizer_with_path_limit(
        &self,
        input: &AffineKet,
        arithmetic: &ScalarArithmetic,
        strategy: CompilerStrategy,
        max_path_bits: usize,
    ) -> Result<CompiledUnitary, UnitaryError> {
        validate_stabilizer_input(input, self.width, arithmetic)?;
        match strategy {
            CompilerStrategy::ParityCopies => self.compile_parity(input, arithmetic, max_path_bits),
            CompilerStrategy::SharedCommutingLayer => {
                shared::compile(self, input, arithmetic, max_path_bits)
            }
        }
    }
}

/// Check the physical input before any coherent ancillas or circuit lowering.
pub(crate) fn validate_stabilizer_input(
    input: &AffineKet,
    width: usize,
    arithmetic: &ScalarArithmetic,
) -> Result<(), UnitaryError> {
    if input.width() != width {
        return Err(UnitaryError::DimensionMismatch);
    }
    if input.coefficient().is_zero() {
        return Err(UnitaryError::ZeroStabilizerInput);
    }
    let rank = input.columns().len();
    let factor =
        LinearFactorization::new(rank, &transpose_columns(input.width(), input.columns())?)?;
    if factor.rank() != rank {
        return Err(UnitaryError::DependentStabilizerInput);
    }
    // Independence gives one equal-magnitude amplitude per support point.
    // Scale the exact coefficient norm by 2^rank without enumerating labels
    // or allocating a coefficient containing that power of two.
    let coefficient_norm = arithmetic.norm_squared(input.coefficient())?;
    let support_power = i64::try_from(rank).map_err(|_| UnitaryError::DimensionOverflow)?;
    let power = coefficient_norm
        .power()
        .checked_add(support_power)
        .ok_or(ScalarError::ExponentOverflow)?;
    let norm = arithmetic.scalar(coefficient_norm.coefficients().clone(), power)?;
    if norm != ExactScalar::one() {
        return Err(UnitaryError::UnnormalizedStabilizerInput);
    }
    Ok(())
}

/// Append fresh zero ancillas without changing path coordinates or phases.
pub(crate) fn with_zero_ancillas(
    input: &AffineKet,
    width: usize,
) -> Result<AffineKet, UnitaryError> {
    if width < input.width() {
        return Err(UnitaryError::DimensionMismatch);
    }
    Ok(AffineKet::new(
        BitVector::from_indices(width, input.offset().ones())?,
        input
            .columns()
            .iter()
            .map(|c| BitVector::from_indices(width, c.ones()))
            .collect::<Result<Vec<_>, _>>()?,
        input.phase().clone(),
        input.coefficient().clone(),
    )?)
}
