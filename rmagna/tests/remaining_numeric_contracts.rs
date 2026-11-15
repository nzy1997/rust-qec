// SPDX-License-Identifier: MPL-2.0
// Full license: fixtures/original_many_record_input.LICENSE.
// Explicit finite-word portions of pinned Magna tests/native/rational_random.cpp.
// Full source-method numeric/proposal witnesses are added in this target before
// any ledger promotion; the existing masked stream is a separate public policy.
use num_bigint::BigUint;
use rmagna::noise::{
    ModuloRangeError, ModuloWordRangeSource, RandRangeSource, RandomSource, WordRangeSource,
};
struct Words {
    values: Vec<u64>,
    position: usize,
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let word = self.values[self.position];
        self.position += 1;
        word
    }
}
#[test]
fn original_uniform_index_rejects_the_incomplete_bucket_and_bound_one_uses_no_entropy() {
    let mut words = Words {
        values: vec![0, 1],
        position: 0,
    };
    let mut source = ModuloWordRangeSource::new(&mut words, 2);
    assert_eq!(
        source.randrange(&15u32.into()).unwrap(),
        BigUint::from(1u32)
    );
    assert_eq!(source.entropy_words(), 2);
    assert_eq!(source.randrange(&1u32.into()).unwrap(), BigUint::from(0u32));
    assert_eq!(source.entropy_words(), 2);
    assert_eq!(
        source.randrange(&0u32.into()),
        Err(ModuloRangeError::EmptyRange)
    );
    assert_eq!(
        source.randrange(&(BigUint::from(1u32) << 64)),
        Err(ModuloRangeError::RangeTooWide)
    );
    assert_eq!(
        source.randrange(&2u32.into()),
        Err(ModuloRangeError::EntropyLimit { limit: 2 })
    );
    assert_eq!(source.entropy_words(), 2);
    assert_eq!(words.position, 2);
}
#[test]
fn modulo_and_masked_conventions_are_explicit_and_preserve_boundary_words() {
    for count in [2u64, 3, 7, 15, 16, 257, u64::MAX] {
        let threshold = count.wrapping_neg() % count;
        let mut candidates = vec![threshold];
        if threshold > 0 {
            candidates.insert(0, threshold - 1);
        }
        let mut words = Words {
            values: candidates.clone(),
            position: 0,
        };
        let mut source = ModuloWordRangeSource::new(&mut words, candidates.len());
        assert_eq!(
            source.randrange(&count.into()).unwrap(),
            BigUint::from(threshold % count)
        );
        assert_eq!(source.entropy_words(), candidates.len());
        assert_eq!(words.position, candidates.len());
        let mut maximum = Words {
            values: vec![u64::MAX],
            position: 0,
        };
        assert_eq!(
            ModuloWordRangeSource::new(&mut maximum, 1)
                .randrange(&count.into())
                .unwrap(),
            BigUint::from(u64::MAX % count)
        );
    }
    let mut words = Words {
        values: vec![0, 1],
        position: 0,
    };
    assert_eq!(
        WordRangeSource::new(&mut words, 2)
            .randrange(&15u32.into())
            .unwrap(),
        BigUint::from(0u32)
    );
    assert_eq!(words.position, 1); // Intentionally distinct from full-word modulo rejection.
    let mut exhausted = Words {
        values: vec![0],
        position: 0,
    };
    let mut source = ModuloWordRangeSource::new(&mut exhausted, 1);
    assert_eq!(
        source.randrange(&15u32.into()),
        Err(ModuloRangeError::EntropyLimit { limit: 1 })
    );
    assert_eq!(source.entropy_words(), 1);
}

fn record_plan(width: usize) -> rmagna::instrument::OriginalAssignmentPlan {
    use rmagna::{
        clifford::CliffordCircuit, gf2::BitVector, instrument::*, scalar::ScalarArithmetic,
        unitary::CompilerStrategy,
    };
    let a = ScalarArithmetic::default();
    let source = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(BitVector::zeros(1).unwrap()).unwrap(),
        (0..width)
            .map(|j| OriginalOperation {
                id: format!("m{j}"),
                gate: OriginalGate::Measure {
                    factors: vec![(0, OriginalPauli::Z)],
                    inverted: false,
                    record: format!("r{j}"),
                },
            })
            .collect(),
        vec![],
        Default::default(),
        &a,
    )
    .unwrap();
    OriginalAssignmentPlan::from_owned(source, CompilerStrategy::ParityCopies, a)
}
#[test]
fn independent_record_proposals_own_probabilities_and_all_eight_exact_point_masses() {
    use rmagna::instrument::*;
    use std::{collections::BTreeMap, convert::Infallible};
    let source = record_plan(3);
    let mut probabilities = BTreeMap::from([
        ("r0".into(), "2/7".parse().unwrap()),
        ("r2".into(), "3/11".parse().unwrap()),
    ]);
    let proposal =
        OriginalIndependentRecordProposal::new(&source, &probabilities, Default::default())
            .unwrap();
    probabilities.insert("r0".into(), "1/3".parse().unwrap());
    let numerators = [40, 16, 40, 16, 15, 6, 15, 6];
    let mut sum = num_rational::BigRational::from_integer(0.into());
    for (word, numerator) in numerators.into_iter().enumerate() {
        let mut requests = Vec::new();
        let mut random = |bound: &BigUint| -> Result<BigUint, Infallible> {
            let j = requests.len();
            requests.push(bound.clone());
            Ok(if word & (1 << j) != 0 {
                0u32.into()
            } else {
                bound - BigUint::from(1u32)
            })
        };
        let result = proposal
            .draw(&mut random, OriginalSharedFaults::empty())
            .unwrap();
        assert_eq!(result.range_requests, 3);
        assert_eq!(requests, vec![7u32.into(), 2u32.into(), 11u32.into()]);
        for j in 0..3 {
            assert_eq!(
                result.assignment.word().get(j).unwrap(),
                word & (1 << j) != 0
            );
        }
        assert_eq!(
            result.proposal_probability,
            num_rational::BigRational::new(numerator.into(), 154.into())
        );
        sum += &result.proposal_probability;
        let expected = if word == 0 {
            rmagna::scalar::ExactScalar::one()
        } else {
            rmagna::scalar::ExactScalar::zero()
        };
        assert_eq!(source.probability(result.assignment).unwrap(), expected);
    }
    assert_eq!(sum, num_rational::BigRational::from_integer(1.into()));
    assert_eq!(source.input_key_generations(), 0);
}
#[test]
fn affine_real_record_proposals_own_equations_and_sample_the_checked_nullspace() {
    use rmagna::{gf2::BitVector, instrument::*};
    use std::convert::Infallible;
    let source = record_plan(3);
    let mut rows = vec![BitVector::from_indices(3, [0, 1]).unwrap()];
    let mut certificate = "finite test".to_owned();
    let proposal = OriginalAffineRecordProposal::from_integer_equations(
        &source,
        &rows,
        &[1],
        &certificate,
        Default::default(),
    )
    .unwrap();
    rows[0].set(0, false).unwrap();
    certificate.clear();
    assert_eq!(proposal.coverage_certificate(), "finite test");
    assert_eq!(proposal.family().dimension(), 2);
    for (coordinate, point) in [1usize, 2, 5, 6].into_iter().enumerate() {
        let mut calls = 0;
        let mut random = |bound: &BigUint| -> Result<BigUint, Infallible> {
            assert_eq!(*bound, 4u32.into());
            calls += 1;
            Ok(coordinate.into())
        };
        let result = proposal
            .draw(&mut random, OriginalSharedFaults::empty())
            .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(result.range_requests, 1);
        for j in 0..3 {
            assert_eq!(
                result.assignment.word().get(j).unwrap(),
                point & (1 << j) != 0
            );
        }
        assert_eq!(
            result.proposal_probability,
            num_rational::BigRational::new(1.into(), 4.into())
        );
        // The class checks algebra/caller evidence,not the physical coverage
        // promise: this repeated-MZ source has zero mass on the declared family.
        assert!(source.probability(result.assignment).unwrap().is_zero());
    }
    let identities = (0..3)
        .map(|j| BitVector::from_indices(3, [j]).unwrap())
        .collect::<Vec<_>>();
    let deterministic = OriginalAffineRecordProposal::from_integer_equations(
        &source,
        &identities,
        &[0, 0, 0],
        "deterministic",
        OriginalProposalOptions {
            max_range_calls: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let mut never = |_: &BigUint| -> Result<BigUint, Infallible> {
        panic!("deterministic family must not consume entropy")
    };
    let result = deterministic
        .draw(&mut never, OriginalSharedFaults::empty())
        .unwrap();
    assert_eq!(result.range_requests, 0);
    assert_eq!(
        result.proposal_probability,
        num_rational::BigRational::from_integer(1.into())
    );
    assert_eq!(
        source.probability(result.assignment).unwrap(),
        rmagna::scalar::ExactScalar::one()
    );
}
#[test]
fn record_proposal_errors_refuse_invalid_support_quotas_and_range_results() {
    use rmagna::{gf2::BitVector, instrument::*};
    use std::{collections::BTreeMap, convert::Infallible};
    let source = record_plan(3);
    let rows = vec![BitVector::from_indices(3, [0, 1]).unwrap()];
    for rhs in [2, -1] {
        assert!(
            OriginalAffineRecordProposal::from_integer_equations(
                &source,
                &rows,
                &[rhs],
                "finite",
                Default::default()
            )
            .is_err()
        );
    }
    assert!(
        OriginalAffineRecordProposal::from_integer_equations(
            &source,
            &rows,
            &[1],
            "",
            Default::default()
        )
        .is_err()
    );
    assert!(
        OriginalAffineRecordProposal::from_integer_equations(
            &source,
            &[BitVector::zeros(3).unwrap()],
            &[1],
            "finite",
            Default::default()
        )
        .is_err()
    );
    for text in ["0", "1"] {
        assert!(
            OriginalIndependentRecordProposal::new(
                &source,
                &BTreeMap::from([("r0".into(), text.parse().unwrap())]),
                Default::default()
            )
            .is_err()
        );
    }
    assert!(
        OriginalIndependentRecordProposal::new(
            &source,
            &BTreeMap::from([("foreign".into(), "1/2".parse().unwrap())]),
            Default::default()
        )
        .is_err()
    );
    assert!(
        OriginalIndependentRecordProposal::new(
            &source,
            &BTreeMap::new(),
            OriginalProposalOptions {
                max_probability_bits: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    for options in [
        OriginalProposalOptions {
            max_geometry_words: 0,
            ..Default::default()
        },
        OriginalProposalOptions {
            max_elimination_work: 0,
            ..Default::default()
        },
        OriginalProposalOptions {
            max_certificate_bytes: 0,
            ..Default::default()
        },
    ] {
        assert!(
            OriginalAffineRecordProposal::from_integer_equations(
                &source,
                &rows,
                &[1],
                "finite",
                options
            )
            .is_err()
        );
    }
    let proposal = OriginalIndependentRecordProposal::new(
        &source,
        &BTreeMap::new(),
        OriginalProposalOptions {
            max_range_calls: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let mut never =
        |_: &BigUint| -> Result<BigUint, Infallible> { panic!("quota must reject before RNG") };
    assert!(matches!(
        proposal.draw(&mut never, OriginalSharedFaults::empty()),
        Err(OriginalProposalDrawError::Original(_))
    ));
    let proposal =
        OriginalIndependentRecordProposal::new(&source, &BTreeMap::new(), Default::default())
            .unwrap();
    let mut bad = |bound: &BigUint| -> Result<BigUint, Infallible> { Ok(bound.clone()) };
    assert!(matches!(
        proposal.draw(&mut bad, OriginalSharedFaults::empty()),
        Err(OriginalProposalDrawError::OutOfRange)
    ));
    let mut calls = 0;
    let mut failed = |_: &BigUint| -> Result<BigUint, &'static str> {
        calls += 1;
        Err("range failure")
    };
    assert!(matches!(
        proposal.draw(&mut failed, OriginalSharedFaults::empty()),
        Err(OriginalProposalDrawError::Source("range failure"))
    ));
    assert_eq!(calls, 1);
}
