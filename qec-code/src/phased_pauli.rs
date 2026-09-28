//! Exact Pauli phases, signed stabilizers, and caller-supplied logical bases.
//!
//! An operator is `i^phase X^x Z^z`, with qubits in the same order as [`Pauli`].
//! Thus `Y = i XZ`: support `(x, z) = (1, 1)` with phase [`Phase::PlusI`].
//! The support-only [`Pauli`] remains unchanged; adding or discarding a phase
//! always requires an explicit method call.

use crate::Pauli;
use crate::binary::try_binary_rank;
use thiserror::Error;

/// The scalar multiplying an ordered `X^x Z^z` Pauli product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Phase {
    PlusOne = 0,
    PlusI = 1,
    MinusOne = 2,
    MinusI = 3,
}

impl Phase {
    pub fn exponent(self) -> u8 {
        self as u8
    }

    /// Convert an exponent of `i`, reduced modulo four.
    pub fn from_exponent(exponent: u8) -> Self {
        match exponent % 4 {
            0 => Self::PlusOne,
            1 => Self::PlusI,
            2 => Self::MinusOne,
            _ => Self::MinusI,
        }
    }
}

/// A phase-aware Pauli operator. Equality includes the scalar phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhasedPauli {
    support: Pauli,
    phase: Phase,
}

impl PhasedPauli {
    /// Attach an explicit phase to a support-only Pauli.
    pub fn new(support: Pauli, phase: Phase) -> Self {
        Self { support, phase }
    }

    pub fn identity(n: usize) -> Self {
        Self::new(
            Pauli::from_xz_bits(vec![0; n], vec![0; n]).expect("zero bits are valid"),
            Phase::PlusOne,
        )
    }

    pub fn support(&self) -> &Pauli {
        &self.support
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn n(&self) -> usize {
        self.support.n()
    }

    /// Return the X/Z support, explicitly discarding the scalar phase.
    pub fn into_support(self) -> Pauli {
        self.support
    }

    /// Exact ordered product, including the sign from moving the right X past the left Z.
    pub fn multiply(&self, rhs: &Self) -> Result<Self, PhaseAlgebraError> {
        if self.n() != rhs.n() {
            return Err(PhaseAlgebraError::WidthMismatch {
                expected: self.n(),
                actual: rhs.n(),
            });
        }
        let crossing_parity = self
            .support
            .z_bits()
            .iter()
            .zip(rhs.support.x_bits())
            .fold(0u8, |parity, (&z, &x)| parity ^ (z & x));
        let phase = Phase::from_exponent(
            self.phase.exponent() + rhs.phase.exponent() + 2 * crossing_parity,
        );
        let x = self
            .support
            .x_bits()
            .iter()
            .zip(rhs.support.x_bits())
            .map(|(&left, &right)| left ^ right)
            .collect();
        let z = self
            .support
            .z_bits()
            .iter()
            .zip(rhs.support.z_bits())
            .map(|(&left, &right)| left ^ right)
            .collect();
        Ok(Self::new(
            Pauli::from_xz_bits(x, z).expect("XOR of valid bits is valid"),
            phase,
        ))
    }

    /// The adjoint is also the inverse, since every Pauli is unitary.
    pub fn adjoint(&self) -> Self {
        let xz_parity = self
            .support
            .x_bits()
            .iter()
            .zip(self.support.z_bits())
            .fold(0u8, |parity, (&x, &z)| parity ^ (x & z));
        let exponent = (4 - self.phase.exponent() + 2 * xz_parity) % 4;
        Self::new(self.support.clone(), Phase::from_exponent(exponent))
    }

    pub fn inverse(&self) -> Self {
        self.adjoint()
    }

    pub fn is_hermitian(&self) -> bool {
        self == &self.adjoint()
    }

    pub fn commutes_with(&self, other: &Self) -> Result<bool, PhaseAlgebraError> {
        if self.n() != other.n() {
            return Err(PhaseAlgebraError::WidthMismatch {
                expected: self.n(),
                actual: other.n(),
            });
        }
        Ok(self.support.commutes_with(&other.support))
    }
}

/// Validation and query errors for the phase-aware API.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PhaseAlgebraError {
    #[error("Pauli width mismatch: expected {expected}, got {actual}")]
    WidthMismatch { expected: usize, actual: usize },
    #[error("stabilizer generator {index} has width {actual}, expected {expected}")]
    GeneratorWidth {
        index: usize,
        expected: usize,
        actual: usize,
    },
    #[error("stabilizer generator {index} is not Hermitian")]
    NonHermitianGenerator { index: usize },
    #[error("stabilizer generators {left} and {right} do not commute")]
    NonCommutingGenerators { left: usize, right: usize },
    #[error("stabilizer generator {index} contradicts the preceding generators (-I)")]
    ContradictoryGenerator { index: usize },
    #[error("expected {expected} logical X/Z pairs, got {x_count} X and {z_count} Z operators")]
    LogicalCount {
        expected: usize,
        x_count: usize,
        z_count: usize,
    },
    #[error("logical {kind}[{index}] has width {actual}, expected {expected}")]
    LogicalWidth {
        kind: &'static str,
        index: usize,
        expected: usize,
        actual: usize,
    },
    #[error("logical {kind}[{index}] is not Hermitian")]
    NonHermitianLogical { kind: &'static str, index: usize },
    #[error("logical {kind}[{index}] does not commute with stabilizer generator {generator}")]
    NonCentralizingLogical {
        kind: &'static str,
        index: usize,
        generator: usize,
    },
    #[error(
        "logical {left_kind}[{left_index}] and {right_kind}[{right_index}] have the wrong commutation relation: expected anticommutation = {expected_anticommutation}"
    )]
    LogicalCommutation {
        left_kind: &'static str,
        left_index: usize,
        right_kind: &'static str,
        right_index: usize,
        expected_anticommutation: bool,
    },
    #[error(
        "logical operators are incomplete modulo stabilizers: rank {actual}, expected {expected}"
    )]
    IncompleteLogicalBasis { actual: usize, expected: usize },
    #[error("generator witness index {index} is out of range for {count} generators")]
    WitnessIndex { index: usize, count: usize },
}

/// Original generator indices, in the order supplied to [`SignedStabilizerGroup::new`].
/// Each index occurs at most once and indices are sorted ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratorProductWitness {
    generator_indices: Vec<usize>,
}

impl GeneratorProductWitness {
    pub fn generator_indices(&self) -> &[usize] {
        &self.generator_indices
    }
}

#[derive(Debug, Clone)]
struct BasisRow {
    pivot: usize,
    operator: PhasedPauli,
    witness: Vec<usize>,
}

/// A validated abelian stabilizer group, including generator signs.
///
/// The original generators, including consistent redundant ones, retain their
/// input order. A reduced basis is stored for repeated membership queries.
#[derive(Debug, Clone)]
pub struct SignedStabilizerGroup {
    n: usize,
    generators: Vec<PhasedPauli>,
    basis: Vec<BasisRow>,
}

impl SignedStabilizerGroup {
    pub fn new(n: usize, generators: Vec<PhasedPauli>) -> Result<Self, PhaseAlgebraError> {
        let mut basis: Vec<BasisRow> = Vec::new();
        for (index, generator) in generators.iter().enumerate() {
            if generator.n() != n {
                return Err(PhaseAlgebraError::GeneratorWidth {
                    index,
                    expected: n,
                    actual: generator.n(),
                });
            }
            if !generator.is_hermitian() {
                return Err(PhaseAlgebraError::NonHermitianGenerator { index });
            }
            for (left, preceding) in generators[..index].iter().enumerate() {
                if !generator.commutes_with(preceding)? {
                    return Err(PhaseAlgebraError::NonCommutingGenerators { left, right: index });
                }
            }

            let mut residual = generator.clone();
            let mut witness = vec![index];
            for row in &basis {
                if bit_at(&residual, row.pivot) != 0 {
                    residual = residual.multiply(&row.operator)?;
                    witness = symmetric_difference(&witness, &row.witness);
                }
            }
            if let Some(pivot) = first_set_bit(&residual) {
                let position = basis.partition_point(|row| row.pivot < pivot);
                basis.insert(
                    position,
                    BasisRow {
                        pivot,
                        operator: residual,
                        witness,
                    },
                );
            } else if residual.phase() != Phase::PlusOne {
                return Err(PhaseAlgebraError::ContradictoryGenerator { index });
            }
        }
        Ok(Self {
            n,
            generators,
            basis,
        })
    }

    pub fn n(&self) -> usize {
        self.n
    }

    pub fn rank(&self) -> usize {
        self.basis.len()
    }

    pub fn num_logical_qubits(&self) -> usize {
        self.n - self.rank()
    }

    pub fn generators(&self) -> &[PhasedPauli] {
        &self.generators
    }

    /// Return a recomputable witness for exact signed membership.
    pub fn contains_with_witness(
        &self,
        target: &PhasedPauli,
    ) -> Result<Option<GeneratorProductWitness>, PhaseAlgebraError> {
        if target.n() != self.n {
            return Err(PhaseAlgebraError::WidthMismatch {
                expected: self.n,
                actual: target.n(),
            });
        }
        let mut residual = target.clone();
        let mut witness = Vec::new();
        for row in &self.basis {
            if bit_at(&residual, row.pivot) != 0 {
                residual = residual.multiply(&row.operator)?;
                witness = symmetric_difference(&witness, &row.witness);
            }
        }
        if first_set_bit(&residual).is_none() && residual.phase() == Phase::PlusOne {
            Ok(Some(GeneratorProductWitness {
                generator_indices: witness,
            }))
        } else {
            Ok(None)
        }
    }

    /// Multiply original generators in the given order; useful to verify a witness.
    pub fn product_from_indices(
        &self,
        indices: &[usize],
    ) -> Result<PhasedPauli, PhaseAlgebraError> {
        let mut product = PhasedPauli::identity(self.n);
        for &index in indices {
            let generator = self
                .generators
                .get(index)
                .ok_or(PhaseAlgebraError::WitnessIndex {
                    index,
                    count: self.generators.len(),
                })?;
            product = product.multiply(generator)?;
        }
        Ok(product)
    }

    /// Validate and retain the exact ordered logical representatives and signs.
    pub fn validate_logical_basis(
        &self,
        logical_x: Vec<PhasedPauli>,
        logical_z: Vec<PhasedPauli>,
    ) -> Result<ValidatedLogicalBasis, PhaseAlgebraError> {
        let k = self.num_logical_qubits();
        if logical_x.len() != k || logical_z.len() != k {
            return Err(PhaseAlgebraError::LogicalCount {
                expected: k,
                x_count: logical_x.len(),
                z_count: logical_z.len(),
            });
        }
        for (kind, operators) in [("X", &logical_x), ("Z", &logical_z)] {
            for (index, operator) in operators.iter().enumerate() {
                if operator.n() != self.n {
                    return Err(PhaseAlgebraError::LogicalWidth {
                        kind,
                        index,
                        expected: self.n,
                        actual: operator.n(),
                    });
                }
                if !operator.is_hermitian() {
                    return Err(PhaseAlgebraError::NonHermitianLogical { kind, index });
                }
                for (generator, stabilizer) in self.generators.iter().enumerate() {
                    if !operator.commutes_with(stabilizer)? {
                        return Err(PhaseAlgebraError::NonCentralizingLogical {
                            kind,
                            index,
                            generator,
                        });
                    }
                }
            }
        }

        for i in 0..k {
            for j in (i + 1)..k {
                check_commutation("X", i, &logical_x[i], "X", j, &logical_x[j], false)?;
                check_commutation("Z", i, &logical_z[i], "Z", j, &logical_z[j], false)?;
            }
            for j in 0..k {
                check_commutation("X", i, &logical_x[i], "Z", j, &logical_z[j], i == j)?;
            }
        }

        let rows: Vec<Vec<u8>> = self
            .generators
            .iter()
            .chain(&logical_x)
            .chain(&logical_z)
            .map(|operator| operator.support().to_symplectic_row())
            .collect();
        let actual = try_binary_rank(&rows).expect("validated Pauli rows have equal widths");
        let expected = self.rank() + 2 * k;
        if actual != expected {
            return Err(PhaseAlgebraError::IncompleteLogicalBasis { actual, expected });
        }

        Ok(ValidatedLogicalBasis {
            logical_x,
            logical_z,
        })
    }
}

/// A logical basis validated against a signed stabilizer group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLogicalBasis {
    logical_x: Vec<PhasedPauli>,
    logical_z: Vec<PhasedPauli>,
}

impl ValidatedLogicalBasis {
    pub fn k(&self) -> usize {
        self.logical_x.len()
    }

    pub fn logical_x(&self) -> &[PhasedPauli] {
        &self.logical_x
    }

    pub fn logical_z(&self) -> &[PhasedPauli] {
        &self.logical_z
    }
}

fn check_commutation(
    left_kind: &'static str,
    left_index: usize,
    left: &PhasedPauli,
    right_kind: &'static str,
    right_index: usize,
    right: &PhasedPauli,
    expected_anticommutation: bool,
) -> Result<(), PhaseAlgebraError> {
    if left.commutes_with(right)? == expected_anticommutation {
        return Err(PhaseAlgebraError::LogicalCommutation {
            left_kind,
            left_index,
            right_kind,
            right_index,
            expected_anticommutation,
        });
    }
    Ok(())
}

fn bit_at(operator: &PhasedPauli, column: usize) -> u8 {
    let n = operator.n();
    if column < n {
        operator.support().x_bits()[column]
    } else {
        operator.support().z_bits()[column - n]
    }
}

fn first_set_bit(operator: &PhasedPauli) -> Option<usize> {
    operator
        .support()
        .x_bits()
        .iter()
        .chain(operator.support().z_bits())
        .position(|&bit| bit != 0)
}

fn symmetric_difference(left: &[usize], right: &[usize]) -> Vec<usize> {
    let mut result = Vec::with_capacity(left.len() + right.len());
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => {
                result.push(left[i]);
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                result.push(right[j]);
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    result.extend_from_slice(&left[i..]);
    result.extend_from_slice(&right[j..]);
    result
}
