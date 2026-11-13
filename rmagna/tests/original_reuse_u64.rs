// SPDX-License-Identifier: MPL-2.0
// Complete FileReuseTests.test_uint64_denominator_boundary_and_reproducibility,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_file_reuse.py.
use num_bigint::BigInt;
use num_rational::BigRational as R;
use num_traits::One;
use rmagna::instrument::FaultSelection;
use rmagna::noise::{NoiseError, NoiseModel, NoiseOptions, NoiseSampler, RandomSource, SplitMix64};
use rmagna::probability::Probability;
const D: u64 = u64::MAX;
const IDENTITY: [u64; 3] = [9223372036854775801, 18446744073709551614, 1];
fn alternatives() -> [R; 3] {
    [
        R::new((D / 2 + 7).into(), D.into()),
        R::new(1.into(), D.into()),
        R::new((D - 1).into(), D.into()),
    ]
}
fn model(options: &NoiseOptions) -> Result<NoiseModel, NoiseError> {
    NoiseModel::new(
        alternatives()
            .into_iter()
            .map(|p| {
                vec![
                    Probability::from_rational(R::one() - &p).unwrap(),
                    Probability::from_rational(p).unwrap(),
                ]
            })
            .collect(),
        options,
    )
}
fn sampler() -> NoiseSampler {
    NoiseSampler::physical(
        model(&NoiseOptions::default()).unwrap(),
        NoiseOptions::default(),
    )
}
fn choices(faults: &[FaultSelection]) -> [i8; 3] {
    let mut choices = [-1; 3];
    for f in faults {
        assert_eq!(f.alternative, 0);
        choices[f.site] = 0;
    }
    choices
}
#[test]
fn original_uint64_denominator_boundary_and_reproducibility() {
    assert_eq!(
        alternatives()[0],
        R::new(
            BigInt::from(9223372036854775814_u64),
            BigInt::from(18446744073709551615_u64)
        )
    );
    let reused = sampler();
    let mut count = 0;
    let mut draws = 0;
    for seed in 0..128 {
        let mut left = SplitMix64::new(seed);
        let mut right = SplitMix64::new(seed);
        let actual = reused.draw(&mut left).unwrap();
        let fresh = sampler().draw(&mut right).unwrap();
        assert_eq!(actual, fresh);
        assert_eq!(left, right);
        let selected = choices(&actual.faults);
        assert_eq!(&selected[1..], &[-1, 0]);
        count += usize::from(selected[0] == 0);
        draws += 1;
        assert_eq!(actual.physical_probability, actual.proposal_probability);
        assert_eq!(actual.likelihood_ratio, R::one());
    }
    assert_eq!(draws, 128);
    assert!(
        40 < count && count < 88,
        "first alternative frequency {count}/128"
    );
}
struct Words {
    values: Vec<u64>,
    used: usize,
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let word = *self
            .values
            .get(self.used)
            .expect("unexpected extra entropy");
        self.used += 1;
        word
    }
}
#[test]
fn literal_uint64_cdf_boundaries_rejection_and_denominator_caps() {
    for (probability, identity) in alternatives().iter().zip(IDENTITY) {
        assert_eq!(R::one() - probability, R::new(identity.into(), D.into()));
    }
    let sampler = sampler();
    for (values, expected) in [
        (vec![IDENTITY[0] - 1, IDENTITY[1] - 1, 0], [-1, -1, -1]),
        (vec![IDENTITY[0], IDENTITY[1], 1], [0, 0, 0]),
    ] {
        let mut words = Words { values, used: 0 };
        let draw = sampler.draw(&mut words).unwrap();
        assert_eq!(choices(&draw.faults), expected);
        assert_eq!(words.used, 3);
        assert_eq!(draw.entropy_words, 3);
        assert_eq!(draw.physical_probability, draw.proposal_probability);
        assert_eq!(draw.likelihood_ratio, R::one());
    }
    let mut words = Words {
        values: vec![D, IDENTITY[0], IDENTITY[1], 1],
        used: 0,
    };
    let draw = sampler.draw(&mut words).unwrap();
    assert_eq!(choices(&draw.faults), [0, 0, 0]);
    assert_eq!(words.used, 4);
    assert_eq!(draw.entropy_words, 4);
    let exact = NoiseOptions {
        max_denominator_bits: 64,
        ..Default::default()
    };
    assert!(model(&exact).is_ok());
    let strict = NoiseOptions {
        max_denominator_bits: 63,
        ..exact
    };
    assert!(matches!(
        model(&strict),
        Err(NoiseError::DenominatorLimit {
            required: 64,
            limit: 63
        })
    ));
    let strict_sampler = NoiseSampler::physical(model(&exact).unwrap(), strict);
    let mut no_words = Words {
        values: vec![],
        used: 0,
    };
    assert!(matches!(
        strict_sampler.draw(&mut no_words),
        Err(NoiseError::DenominatorLimit {
            required: 64,
            limit: 63
        })
    ));
    assert_eq!(no_words.used, 0);
}
