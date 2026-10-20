use rmagna::clifford::CliffordGate as Gate;
use rmagna::gf2::BitVector;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

pub fn bits(width: usize, value: usize) -> BitVector {
    BitVector::from_indices(
        width,
        (0..width.min(usize::BITS as usize)).filter(|&j| value >> j & 1 != 0),
    )
    .unwrap()
}

// Literal matrices act on the full state. This oracle does not use affine
// substitution, quadratic phase construction, or hidden-path elimination.
pub fn dense_gate(gate: Gate, values: &mut Vec<ExactScalar>, arithmetic: &ScalarArithmetic) {
    let n = values.len();
    let mut output = vec![ExactScalar::zero(); n];
    for (x, value) in values.iter().enumerate() {
        let (target, phase) = match gate {
            Gate::X(q) => (x ^ (1 << q), 0),
            Gate::Z(q) => (x, 4 * ((x >> q) & 1) as i64),
            Gate::S { qubit, turns } => (x, 2 * i64::from(turns) * ((x >> qubit) & 1) as i64),
            Gate::Cx { control, target } => (x ^ (((x >> control) & 1) << target), 0),
            Gate::Cz { a, b } => (x, 4 * ((x >> a) & (x >> b) & 1) as i64),
            Gate::Swap { a, b } => {
                let different = ((x >> a) ^ (x >> b)) & 1;
                (x ^ (different << a) ^ (different << b), 0)
            }
            Gate::H(q) => {
                let low = x & !(1 << q);
                let high = low | (1 << q);
                let half = arithmetic
                    .multiply(value, &arithmetic.omega(0, -1).unwrap())
                    .unwrap();
                output[low] = arithmetic.add(&output[low], &half).unwrap();
                let high_value = if x >> q & 1 == 0 {
                    half
                } else {
                    half.negated()
                };
                output[high] = arithmetic.add(&output[high], &high_value).unwrap();
                continue;
            }
        };
        output[target] = arithmetic
            .multiply(value, &arithmetic.omega(phase, 0).unwrap())
            .unwrap();
    }
    *values = output;
}

pub fn next(state: &mut u64) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state as usize
}
