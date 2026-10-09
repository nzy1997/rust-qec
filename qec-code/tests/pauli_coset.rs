use qec_code::Pauli;
use qec_code::pauli_coset::{CosetSearchError, PauliCosetSearch};
use qec_code::phased_pauli::{Phase, PhaseAlgebraError, PhasedPauli, SignedStabilizerGroup};

fn operator(n: usize, x: u64, z: u64, phase: u8) -> PhasedPauli {
    PhasedPauli::new(
        Pauli::from_xz_bits(
            (0..n).map(|q| ((x >> q) & 1) as u8).collect(),
            (0..n).map(|q| ((z >> q) & 1) as u8).collect(),
        )
        .unwrap(),
        Phase::from_exponent(phase),
    )
}

#[test]
fn signed_minima_match_independent_literal_bell_cosets() {
    let group =
        SignedStabilizerGroup::new(2, vec![operator(2, 3, 0, 2), operator(2, 0, 3, 0)]).unwrap();
    let cache = PauliCosetSearch::new(&group, 4).unwrap();
    assert_eq!(cache.product_count(), 4);
    // Literal signed group, not emitted with the implementation's multiply.
    let stabilizers = [(0u64, 0u64, 0u8), (3, 0, 2), (0, 3, 0), (3, 3, 2)];
    for x in 0u64..4 {
        for z in 0u64..4 {
            for phase in 0..4 {
                let mut best = None;
                for &(sx, sz, se) in &stabilizers {
                    let weight = ((x ^ sx) | (z ^ sz)).count_ones();
                    let e = (phase + se + 2 * ((z & sx).count_ones() as u8 % 2)) % 4;
                    if best.as_ref().is_none_or(|(w, _)| weight < *w) {
                        best = Some((weight, operator(2, x ^ sx, z ^ sz, e)));
                    }
                }
                let input = operator(2, x, z, phase);
                let result = cache.minimum_weight(&input).unwrap();
                assert_eq!(result, best.unwrap().1);
                let difference = input.inverse().multiply(&result).unwrap();
                let witness = group.contains_with_witness(&difference).unwrap().unwrap();
                assert_eq!(
                    group
                        .product_from_indices(witness.generator_indices())
                        .unwrap(),
                    difference
                );
            }
        }
    }
}

#[test]
fn packed_supports_preserve_high_coordinates_and_negative_identity() {
    let n = 65;
    let mut z = vec![0; n];
    z[64] = 1;
    let target = PhasedPauli::new(
        Pauli::from_xz_bits(vec![0; n], z.clone()).unwrap(),
        Phase::PlusOne,
    );
    let negative = PhasedPauli::new(target.support().clone(), Phase::MinusOne);
    let group = SignedStabilizerGroup::new(n, vec![negative]).unwrap();
    let cache = PauliCosetSearch::new(&group, 2).unwrap();
    assert_eq!(
        cache.minimum_weight(&target).unwrap(),
        PhasedPauli::new(PhasedPauli::identity(n).into_support(), Phase::MinusOne)
    );
    z[0] = 1;
    let target = PhasedPauli::new(Pauli::from_xz_bits(vec![0; n], z).unwrap(), Phase::PlusOne);
    let result = cache.minimum_weight(&target).unwrap();
    assert_eq!(result.support().weight(), 1);
    assert_eq!(result.support().z_bits()[0], 1);
    assert_eq!(result.support().z_bits()[64], 0);
    assert_eq!(result.phase(), Phase::MinusOne);
}

#[test]
fn ties_redundant_generators_and_empty_group_have_declared_behavior() {
    let g = operator(2, 0, 3, 0);
    let group = SignedStabilizerGroup::new(2, vec![g.clone(), g]).unwrap();
    let cache = PauliCosetSearch::new(&group, 4).unwrap();
    assert_eq!(cache.product_count(), 4);
    let target = operator(2, 0, 1, 2);
    assert_eq!(cache.minimum_weight(&target).unwrap(), target);
    let empty = SignedStabilizerGroup::new(2, vec![]).unwrap();
    let cache = PauliCosetSearch::new(&empty, 1).unwrap();
    assert_eq!(cache.product_count(), 1);
    assert_eq!(cache.minimum_weight(&target).unwrap(), target);
}

#[test]
fn allocation_and_query_limits_fail_before_exhaustive_work() {
    let g = operator(1, 0, 1, 0);
    let too_many = SignedStabilizerGroup::new(1, vec![g.clone(); 13]).unwrap();
    assert_eq!(
        PauliCosetSearch::new(&too_many, usize::MAX).unwrap_err(),
        CosetSearchError::GeneratorLimit {
            actual: 13,
            limit: 12
        }
    );
    let too_wide = SignedStabilizerGroup::new(4097, vec![]).unwrap();
    assert_eq!(
        PauliCosetSearch::new(&too_wide, usize::MAX).unwrap_err(),
        CosetSearchError::WidthLimit {
            actual: 4097,
            limit: 4096
        }
    );
    let group = SignedStabilizerGroup::new(1, vec![g]).unwrap();
    assert_eq!(
        PauliCosetSearch::new(&group, 1).unwrap_err(),
        CosetSearchError::ProductLimit {
            required: 2,
            limit: 1
        }
    );
    let cache = PauliCosetSearch::new(&group, 2).unwrap();
    assert_eq!(
        cache.minimum_weight(&PhasedPauli::identity(2)),
        Err(CosetSearchError::Algebra(
            PhaseAlgebraError::WidthMismatch {
                expected: 1,
                actual: 2
            }
        ))
    );
}
