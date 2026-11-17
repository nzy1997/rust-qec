use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::instrument::FaultSelection;
use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler, RandomSource};
use rmagna::probability::Probability;

fn p(n: i64, d: i64) -> Probability {
    // Public capture must normalize even a caller's unreduced raw ratio.
    Probability::from_rational(BigRational::new_raw(n.into(), d.into())).unwrap()
}

fn assert_canonical(actual: &BigRational, expected: &BigRational) {
    assert_eq!(actual.numer(), expected.numer());
    assert_eq!(actual.denom(), expected.denom());
}

#[test]
fn heterogeneous_products_match_full_direct_enumeration_including_forced_sites() {
    let rows = vec![
        vec![p(6, 10), p(3, 10), p(1, 10)],
        vec![p(0, 1), p(5, 10), p(5, 10)],
        vec![p(6, 15), p(0, 1), p(9, 15)],
        vec![p(22, 77), p(55, 77)],
        vec![p(1, 1), p(0, 1)],
        vec![p(17, 101), p(84, 101)],
        vec![p(5, 8), p(3, 8)],
    ];
    let model = NoiseModel::new(rows.clone(), &NoiseOptions::default()).unwrap();
    let patterns = rows.iter().map(Vec::len).product();
    let mut mass = BigRational::from_integer(0.into());
    for mut code in 0..patterns {
        let mut faults = Vec::new();
        let mut expected = BigRational::from_integer(1.into());
        for (site, row) in rows.iter().enumerate() {
            let choice = code % row.len();
            code /= row.len();
            expected *= row[choice].value();
            if choice != 0 {
                faults.push(FaultSelection {
                    site,
                    alternative: choice - 1,
                });
            }
        }
        assert_canonical(&model.probability(&faults).unwrap(), &expected);
        mass += expected;
    }
    assert_eq!(patterns, 432);
    assert_eq!(mass, BigRational::from_integer(1.into()));
}

#[test]
fn thousands_of_sites_keep_exact_coprime_numerators_and_denominators() {
    const N: usize = 4096;
    let model = NoiseModel::new(
        vec![vec![p(999, 1000), p(1, 1000)]; N],
        &NoiseOptions::default(),
    )
    .unwrap();
    let denominator = BigInt::from(1000).pow(N as u32);
    for count in [0, 1, 2, 17, N - 1, N] {
        let faults = (0..count)
            .map(|site| FaultSelection {
                site,
                alternative: 0,
            })
            .collect::<Vec<_>>();
        // Each labelled pattern has this closed form, with no binomial
        // coefficient. Powers of 999 and 1000 are coprime by construction.
        let actual = model.probability(&faults).unwrap();
        assert_eq!(actual.numer(), &BigInt::from(999).pow((N - count) as u32));
        assert_eq!(actual.denom(), &denominator);
    }
}

struct Words {
    values: Vec<u64>,
    used: usize,
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let word = self.values[self.used];
        self.used += 1;
        word
    }
}

#[test]
fn exact_products_preserve_every_categorical_boundary_and_entropy_request() {
    let physical_rows = vec![
        vec![p(2, 3), p(1, 6), p(1, 6)],
        vec![p(0, 1), p(1, 7), p(6, 7)],
        vec![p(9, 10), p(1, 10)],
    ];
    let proposal_rows = vec![
        vec![p(1, 2), p(1, 4), p(1, 4)],
        vec![p(1, 8), p(3, 8), p(1, 2)],
        vec![p(1, 2), p(1, 2)],
    ];
    let make = |rows: &[Vec<Probability>]| {
        NoiseModel::new(rows.to_vec(), &NoiseOptions::default()).unwrap()
    };
    let sampler = NoiseSampler::importance(
        make(&physical_rows),
        make(&proposal_rows),
        NoiseOptions::default(),
    )
    .unwrap();
    for a in 0..4 {
        for b in 0..8 {
            for c in 0..2 {
                let choices: [usize; 3] = [
                    if a < 2 { 0 } else { a - 1 },
                    if b < 1 {
                        0
                    } else if b < 4 {
                        1
                    } else {
                        2
                    },
                    c,
                ];
                let mut words = Words {
                    values: vec![a as u64, b as u64, c as u64],
                    used: 0,
                };
                let draw = sampler.draw(&mut words).unwrap();
                let expected_faults = choices
                    .iter()
                    .enumerate()
                    .filter_map(|(site, &choice)| {
                        (choice != 0).then_some(FaultSelection {
                            site,
                            alternative: choice.saturating_sub(1),
                        })
                    })
                    .collect::<Vec<_>>();
                assert_eq!(draw.faults, expected_faults);
                let direct = |rows: &[Vec<Probability>]| {
                    choices.iter().enumerate().fold(
                        BigRational::from_integer(1.into()),
                        |mass, (site, &choice)| mass * rows[site][choice].value(),
                    )
                };
                let physical = direct(&physical_rows);
                let proposal = direct(&proposal_rows);
                assert_canonical(&draw.physical_probability, &physical);
                assert_canonical(&draw.proposal_probability, &proposal);
                assert_canonical(&draw.likelihood_ratio, &(&physical / &proposal));
                assert_eq!(draw.entropy_words, 3);
                assert_eq!(words.used, 3);
            }
        }
    }
}
