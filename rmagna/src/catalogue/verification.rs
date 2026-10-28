//! Complete proofs from supplied indicators, with admission before dense scratch.
use super::*;

/// The complete exact reconstruction used to certify immutable input terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificateVerificationMethod {
    Pointwise,
    InverseWalsh,
}
impl CertificateVerificationMethod {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pointwise => "pointwise",
            Self::InverseWalsh => "inverse_walsh",
        }
    }
}

/// Admitted logical proof work, excluding allocator overhead and integer bit cost.
/// For N assignments, T indicators, C constraints and P normalized parity rows,
/// pointwise work is N*(1+T+C+P), preserving the historical bound. Character-only
/// inverse work is N*(dimension+2)+4*T+P: dense initialization/comparisons,
/// dimension*N butterfly operations, coefficient validation/root/multiply/add,
/// and one packed-axis XOR per parity row. Table dimensions fit a platform word.
/// Ties select pointwise. Neither count describes native memory layout or time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertificateVerification {
    pub method: CertificateVerificationMethod,
    pub work: u128,
}

pub(super) fn characters(terms: &[CliffordIndicator]) -> bool {
    terms
        .iter()
        .all(|term| term.constraints.is_empty() && term.phase.terms().iter().all(|(_, c)| *c == 4))
}

pub(super) fn plan(
    dimension: usize,
    count: usize,
    terms: u128,
    constraints: u128,
    parities: u128,
    character_only: bool,
) -> Result<CertificateVerification, CatalogueError> {
    let overflow = || {
        error(
            "catalogue.max_verification_work",
            "verification work exceeds u128",
        )
    };
    let pointwise = terms
        .checked_add(1)
        .and_then(|v| v.checked_add(constraints))
        .and_then(|v| v.checked_add(parities))
        .and_then(|v| v.checked_mul(count as u128));
    if character_only {
        let inverse = (count as u128)
            .checked_mul(dimension as u128 + 2)
            .and_then(|v| terms.checked_mul(4).and_then(|t| v.checked_add(t)))
            .and_then(|v| v.checked_add(parities));
        if let Some(inverse) = inverse.filter(|inverse| pointwise.is_none_or(|p| *inverse < p)) {
            return Ok(CertificateVerification {
                method: CertificateVerificationMethod::InverseWalsh,
                work: inverse,
            });
        }
    }
    Ok(CertificateVerification {
        method: CertificateVerificationMethod::Pointwise,
        work: pointwise.ok_or_else(overflow)?,
    })
}

fn compare(
    input: &PhaseDecompositionInput,
    word: usize,
    actual: &ExactScalar,
    arithmetic: &ScalarArithmetic,
    prefix: &str,
) -> Result<(), CatalogueError> {
    let expected = arithmetic
        .omega(i64::from(input.phase_table[word]), 0)
        .map_err(|e| error(prefix, e))?;
    if *actual != expected {
        return Err(error(
            format!("{prefix}.assignment.{word}"),
            format!(
                "decomposition does not reconstruct omega^{}; actual {actual:?}",
                input.phase_table[word]
            ),
        ));
    }
    Ok(())
}

pub(super) fn verify(
    input: &PhaseDecompositionInput,
    proof: CertificateVerification,
    arithmetic: &ScalarArithmetic,
    prefix: &str,
) -> Result<(), CatalogueError> {
    match proof.method {
        CertificateVerificationMethod::Pointwise => pointwise(input, arithmetic, prefix),
        CertificateVerificationMethod::InverseWalsh => inverse(input, arithmetic, prefix),
    }
}

fn inverse(
    input: &PhaseDecompositionInput,
    arithmetic: &ScalarArithmetic,
    prefix: &str,
) -> Result<(), CatalogueError> {
    let count = input.phase_table.len();
    vector_capacity::<ExactScalar>("catalogue.inverse_walsh_capacity", count as u128)?;
    // This buffer is independent of any forward transform. Only the actual
    // supplied terms populate it, including duplicate masks, zeros and cancels.
    let mut values = vec![ExactScalar::zero(); count];
    for (j, term) in input.terms.iter().enumerate() {
        let field = format!("{prefix}.terms.{j}.character");
        let mask = term.phase.terms().iter().fold(0, |mask, (axis, _)| {
            mask ^ axis.words().first().copied().unwrap_or(0) as usize
        });
        let root = arithmetic
            .omega(i64::from(term.phase.constant()), 0)
            .map_err(|e| error(&field, e))?;
        let value = arithmetic
            .multiply(&term.coefficient, &root)
            .map_err(|e| error(&field, e))?;
        values[mask] = arithmetic
            .add(&values[mask], &value)
            .map_err(|e| error(&field, e))?;
    }
    let mut half = 1;
    while half < count {
        for block in values.chunks_exact_mut(2 * half) {
            for j in 0..half {
                let left = block[j].clone();
                let right = block[j + half].clone();
                block[j] = arithmetic
                    .add(&left, &right)
                    .map_err(|e| error(format!("{prefix}.inverse_walsh"), e))?;
                block[j + half] = arithmetic
                    .subtract(&left, &right)
                    .map_err(|e| error(format!("{prefix}.inverse_walsh"), e))?;
            }
        }
        half *= 2;
    }
    for (word, value) in values.iter().enumerate() {
        compare(input, word, value, arithmetic, prefix)?;
    }
    Ok(())
}

fn pointwise(
    input: &PhaseDecompositionInput,
    arithmetic: &ScalarArithmetic,
    prefix: &str,
) -> Result<(), CatalogueError> {
    for word in 0..input.phase_table.len() {
        let x = BitVector::from_indices(
            input.dimension,
            (0..input.dimension).filter(|j| word >> j & 1 != 0),
        )
        .map_err(|e| error(prefix, e))?;
        let mut actual = ExactScalar::zero();
        for (j, term) in input.terms.iter().enumerate() {
            let field = format!("{prefix}.terms.{j}.assignment.{word}");
            let present = term
                .constraints
                .iter()
                .try_fold(true, |present, (row, bit)| {
                    row.dot(&x).map(|v| present && (v == *bit))
                })
                .map_err(|e| error(&field, e))?;
            if present {
                let phase = term.phase.evaluate(&x).map_err(|e| error(&field, e))?;
                let value = arithmetic
                    .multiply(
                        &term.coefficient,
                        &arithmetic
                            .omega(i64::from(phase), 0)
                            .map_err(|e| error(&field, e))?,
                    )
                    .map_err(|e| error(&field, e))?;
                actual = arithmetic
                    .add(&actual, &value)
                    .map_err(|e| error(&field, e))?;
            }
        }
        compare(input, word, &actual, arithmetic, prefix)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_proof_overflow_does_not_reject_a_representable_cheaper_proof() {
        let pointwise = plan(1, 2, u128::MAX / 3, 0, 0, true).unwrap();
        assert_eq!(pointwise.method, CertificateVerificationMethod::Pointwise);
        let inverse = plan(1, 2, 1, 0, u128::MAX / 2, true).unwrap();
        assert_eq!(inverse.method, CertificateVerificationMethod::InverseWalsh);
        assert!(plan(1, 2, u128::MAX, 0, 0, true).is_err());
        assert!(plan(1, 2, 1, 0, u128::MAX / 2, false).is_err());
    }
}
