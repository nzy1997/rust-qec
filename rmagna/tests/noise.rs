use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::instrument::{CompileOptions, FaultSelection, compile_affine};
use rmagna::noise::{NoiseError, NoiseModel, NoiseOptions, NoiseSampler, RandomSource, SplitMix64};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

struct Words {
    values: Vec<u64>,
    at: usize,
}
impl Words {
    fn new(values: Vec<u64>) -> Self {
        Self { values, at: 0 }
    }
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let value = self.values[self.at];
        self.at += 1;
        value
    }
}
fn model(rows: &[&[&str]]) -> NoiseModel {
    NoiseModel::new(
        rows.iter()
            .map(|row| row.iter().map(|p| p.parse().unwrap()).collect())
            .collect(),
        &NoiseOptions::default(),
    )
    .unwrap()
}
#[test]
fn categorical_intervals_match_exact_rational_mass_and_zero_choices_are_impossible() {
    let p = model(&[&["1/8", "0", "3/8", "1/2"]]);
    let sampler = NoiseSampler::physical(p.clone(), NoiseOptions::default());
    let mut counts = [0; 4];
    for word in 0..8 {
        let mut rng = Words::new(vec![word]);
        let draw = sampler.draw(&mut rng).unwrap();
        let choice = draw.faults.first().map_or(0, |f| f.alternative + 1);
        counts[choice] += 1;
        assert_eq!(
            draw.physical_probability,
            p.probability(&draw.faults).unwrap()
        );
        assert_eq!(draw.proposal_probability, draw.physical_probability);
        assert_eq!(draw.likelihood_ratio, BigRational::from_integer(1.into()));
        assert_eq!(draw.entropy_words, 1);
    }
    assert_eq!(counts, [1, 0, 3, 4]);
    let sampler = NoiseSampler::physical(model(&[&["1/3", "2/3"]]), NoiseOptions::default());
    let mut rejected = Words::new(vec![3, 2]);
    let draw = sampler.draw(&mut rejected).unwrap();
    assert_eq!(
        draw.faults,
        [FaultSelection {
            site: 0,
            alternative: 0
        }]
    );
    assert_eq!(draw.entropy_words, 2);
}
#[test]
fn physical_instrument_sampling_and_importance_weights_match_complete_enumeration() {
    let a = ScalarArithmetic::default();
    let circuit = parse_circuit(
        "X_ERROR(1/8) 0\nY_ERROR(1/4) 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
        &ParseOptions::default(),
    )
    .unwrap();
    let protocol = compile_affine(&circuit, &CompileOptions::default(), &a).unwrap();
    let physical = NoiseModel::physical(&protocol, &NoiseOptions::default()).unwrap();
    let proposal = model(&[&["1/2", "1/2"], &["1/2", "1/2"], &["1", "0"]]);
    let sampler =
        NoiseSampler::importance(physical.clone(), proposal, NoiseOptions::default()).unwrap();
    let mut weighted_acceptance = BigRational::from_integer(0.into());
    let mut total_weight = BigRational::from_integer(0.into());
    for mask in 0..4 {
        let mut rng = Words::new(vec![mask & 1, (mask >> 1) & 1]);
        let draw = sampler.draw(&mut rng).unwrap();
        assert_eq!(
            draw.physical_probability,
            protocol.fault_probability(&draw.faults).unwrap()
        );
        assert_eq!(
            draw.physical_probability,
            physical.probability(&draw.faults).unwrap()
        );
        assert_eq!(
            draw.proposal_probability,
            BigRational::new(1.into(), 4.into())
        );
        let mass = protocol.acceptance(&draw.faults, 1, &a).unwrap().value;
        assert!(mass == ExactScalar::one() || mass == ExactScalar::zero());
        total_weight += &draw.proposal_probability * &draw.likelihood_ratio;
        if mass == ExactScalar::one() {
            weighted_acceptance += &draw.proposal_probability * &draw.likelihood_ratio;
        }
    }
    assert_eq!(total_weight, BigRational::from_integer(1.into()));
    assert_eq!(weighted_acceptance, BigRational::new(11.into(), 16.into()));
}
#[test]
fn tiny_probabilities_remain_reachable_and_failure_budgets_are_explicit() {
    let tiny: rmagna::probability::Probability = "1e-400".parse().unwrap();
    let p = NoiseModel::new(
        vec![vec![tiny.clone(), tiny.complement()]],
        &NoiseOptions::default(),
    )
    .unwrap();
    let sampler = NoiseSampler::physical(p, NoiseOptions::default());
    let mut zeros = Words::new(vec![0; 21]);
    let draw = sampler.draw(&mut zeros).unwrap();
    assert!(draw.faults.is_empty());
    assert_eq!(draw.physical_probability, tiny.value().clone());
    assert_eq!(draw.entropy_words, 21);
    assert!(matches!(
        NoiseModel::new(
            vec![vec![tiny.clone(), tiny.complement()]],
            &NoiseOptions {
                max_denominator_bits: 100,
                ..Default::default()
            }
        ),
        Err(NoiseError::DenominatorLimit { .. })
    ));
    let sampler = NoiseSampler::physical(
        model(&[&["1/3", "2/3"]]),
        NoiseOptions {
            max_entropy_words: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        sampler.draw(&mut Words::new(vec![3])),
        Err(NoiseError::EntropyLimit { limit: 1 })
    );
    let sampler = NoiseSampler::physical(
        model(&[&["0", "1"]]),
        NoiseOptions {
            max_entropy_words: 0,
            ..Default::default()
        },
    );
    assert_eq!(
        sampler.draw(&mut Words::new(vec![])).unwrap().faults,
        [FaultSelection {
            site: 0,
            alternative: 0
        }]
    );
}
#[test]
fn proposal_support_shapes_and_prescribed_faults_are_validated() {
    assert!(matches!(
        NoiseModel::new(vec![vec![]], &NoiseOptions::default()),
        Err(NoiseError::InvalidCategory { site: 0 })
    ));
    assert!(matches!(
        NoiseModel::new(vec![vec!["1/3".parse().unwrap()]], &NoiseOptions::default()),
        Err(NoiseError::InvalidCategory { site: 0 })
    ));
    let physical = model(&[&["1/2", "1/2"]]);
    assert!(matches!(
        NoiseSampler::importance(physical.clone(), model(&[]), NoiseOptions::default()),
        Err(NoiseError::ProposalShape)
    ));
    assert!(matches!(
        NoiseSampler::importance(
            physical.clone(),
            model(&[&["1/2", "1/4", "1/4"]]),
            NoiseOptions::default()
        ),
        Err(NoiseError::ProposalShape)
    ));
    assert!(matches!(
        NoiseSampler::importance(
            physical.clone(),
            model(&[&["0", "1"]]),
            NoiseOptions::default()
        ),
        Err(NoiseError::MissingSupport { site: 0, choice: 0 })
    ));
    assert!(matches!(
        physical.probability(&[FaultSelection {
            site: 0,
            alternative: usize::MAX
        }]),
        Err(NoiseError::InvalidFault { .. })
    ));
    assert!(matches!(
        physical.probability(&[FaultSelection {
            site: 1,
            alternative: 0
        }]),
        Err(NoiseError::InvalidFault { .. })
    ));
    assert!(matches!(
        physical.probability(
            &[FaultSelection {
                site: 0,
                alternative: 0
            }; 2]
        ),
        Err(NoiseError::RepeatedFault { site: 0 })
    ));
    let sampler =
        NoiseSampler::importance(model(&[&["1", "0"]]), physical, NoiseOptions::default()).unwrap();
    assert_eq!(
        sampler
            .draw(&mut Words::new(vec![1]))
            .unwrap()
            .likelihood_ratio,
        BigRational::from_integer(0.into())
    );
}
#[test]
fn seeded_streams_are_stable_resumable_and_independent_of_draw_chunking() {
    let mut rng = SplitMix64::new(0);
    assert_eq!(rng.next_u64(), 0xe220a8397b1dcdaf);
    assert_eq!(rng.next_u64(), 0x6e789e6aa1b965f4);
    let sampler = NoiseSampler::physical(
        model(&[&["1/3", "1/6", "1/2"], &["7/8", "1/8"]]),
        NoiseOptions::default(),
    );
    let mut uninterrupted = SplitMix64::new(20261024);
    let expected = (0..100)
        .map(|_| sampler.draw(&mut uninterrupted).unwrap())
        .collect::<Vec<_>>();
    let mut chunked = SplitMix64::new(20261024);
    let mut actual = (0..37)
        .map(|_| sampler.draw(&mut chunked).unwrap())
        .collect::<Vec<_>>();
    chunked = SplitMix64::new(chunked.state());
    actual.extend((37..100).map(|_| sampler.draw(&mut chunked).unwrap()));
    assert_eq!(actual, expected);
    assert_eq!(chunked.state(), uninterrupted.state());
    let sampler = NoiseSampler::physical(model(&[]), NoiseOptions::default());
    let empty = sampler.draw(&mut Words::new(vec![])).unwrap();
    assert!(empty.faults.is_empty());
    assert_eq!(
        empty.physical_probability,
        BigRational::from_integer(1.into())
    );
}
