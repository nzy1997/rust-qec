//! Fixed-coordinate non-Clifford polynomial gauge and its exact Clifford rest.
use super::*;
#[derive(Clone, Copy, Debug)]
pub struct PhaseCoreLimits {
    pub max_dimension: usize,
    pub max_table_values: usize,
    /// Logical monomial contributions and dense subset-table visits.
    pub max_work: u128,
}
impl Default for PhaseCoreLimits {
    fn default() -> Self {
        Self {
            max_dimension: 20,
            max_table_values: 1 << 20,
            max_work: 64 << 20,
        }
    }
}
#[derive(Clone, Debug)]
pub struct CanonicalPhaseCore {
    dimension: usize,
    table: Vec<u8>,
    global: u8,
    clifford: QuadraticForm,
    work: u128,
}
fn check(option: &'static str, required: u128, limit: u128) -> Result<(), PhaseError> {
    if required > limit {
        Err(PhaseError::CoreLimit {
            option,
            limit,
            required,
        })
    } else {
        Ok(())
    }
}
impl CanonicalPhaseCore {
    /// Retain linear coefficients mod 2, quadratic coefficients mod 4 and
    /// cubic coefficients mod 8, in the original coordinate basis. All higher
    /// monomials of a parity polynomial vanish mod 8. No quotient is inferred.
    pub fn new(phase: &ParityPhase, limits: &PhaseCoreLimits) -> Result<Self, PhaseError> {
        let dimension = phase.num_variables();
        check(
            "phase_core.max_dimension",
            dimension as u128,
            limits.max_dimension as u128,
        )?;
        let count = u32::try_from(dimension)
            .ok()
            .and_then(|n| 1_u128.checked_shl(n))
            .unwrap_or(u128::MAX);
        check(
            "phase_core.max_table_values",
            count,
            limits.max_table_values.min(isize::MAX as usize) as u128,
        )?;
        // A byte vector must fit isize, even when the configured usize quota
        // is larger. Reject intrinsically unrepresentable capacities here.
        let count = usize::try_from(count).expect("table count bounded by usize option");
        let mut contributions = 0_u128;
        for (axis, coefficient) in phase.terms() {
            let n = axis.ones().count() as u128;
            let pairs = n * n.saturating_sub(1) / 2;
            let triples = if coefficient & 1 != 0 {
                n * n.saturating_sub(1) * n.saturating_sub(2) / 6
            } else {
                0
            };
            contributions =
                contributions
                    .checked_add(n + pairs + triples)
                    .ok_or(PhaseError::CoreLimit {
                        option: "phase_core.max_work",
                        limit: limits.max_work,
                        required: u128::MAX,
                    })?;
        }
        let work = contributions
            .checked_add((dimension as u128 + 1) * count as u128)
            .ok_or(PhaseError::CoreLimit {
                option: "phase_core.max_work",
                limit: limits.max_work,
                required: u128::MAX,
            })?;
        check("phase_core.max_work", work, limits.max_work)?;
        let mut polynomial = vec![0_u8; count];
        for (axis, coefficient) in phase.terms() {
            let support = axis.ones().collect::<Vec<_>>();
            for (j, &a) in support.iter().enumerate() {
                polynomial[1 << a] = (polynomial[1 << a] + coefficient) & 7;
                for (k, &b) in support.iter().enumerate().skip(j + 1) {
                    let index = (1 << a) | (1 << b);
                    polynomial[index] = (polynomial[index] + 8 - 2 * (coefficient & 3)) & 7;
                    if coefficient & 1 != 0 {
                        for &c in support.iter().skip(k + 1) {
                            polynomial[index | (1 << c)] ^= 4;
                        }
                    }
                }
            }
        }
        let linear = (0..dimension).map(|j| polynomial[1 << j] / 2).collect();
        let upper = (0..dimension)
            .map(|a| {
                BitVector::from_indices(
                    dimension,
                    (a + 1..dimension).filter(|&b| polynomial[(1 << a) | (1 << b)] & 4 != 0),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let clifford = QuadraticForm::new(0, linear, upper)?;
        for (index, coefficient) in polynomial.iter_mut().enumerate() {
            match index.count_ones() {
                1 => *coefficient &= 1,
                2 => *coefficient &= 3,
                _ => {}
            }
        }
        // Subset zeta transform evaluates the multilinear monomials at every
        // Boolean word. Its order does not select an affine-fiber convention.
        for bit in 0..dimension {
            for word in 0..count {
                if word & (1 << bit) != 0 {
                    polynomial[word] = (polynomial[word] + polynomial[word ^ (1 << bit)]) & 7;
                }
            }
        }
        Ok(Self {
            dimension,
            table: polynomial,
            global: phase.constant(),
            clifford,
            work,
        })
    }
    pub fn dimension(&self) -> usize {
        self.dimension
    }
    pub fn phase_table(&self) -> &[u8] {
        &self.table
    }
    /// Original phase = table[x] + global + 2*quadratic(x), modulo eight.
    pub fn clifford_correction(&self) -> (u8, &QuadraticForm) {
        (self.global, &self.clifford)
    }
    pub fn work_units(&self) -> u128 {
        self.work
    }
}
