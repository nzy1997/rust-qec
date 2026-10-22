use num_rational::BigRational as R;
use num_traits::{One, Zero};
use rmagna::enumeration::{EnumerationError, EnumerationOptions, enumerate_noise};
use rmagna::noise::{NoiseModel, NoiseOptions};
use rmagna::probability::Probability;
use std::collections::BTreeMap;

fn rows(input: &[&[&str]]) -> Vec<Vec<Probability>> {
    input
        .iter()
        .map(|row| row.iter().map(|p| p.parse().unwrap()).collect())
        .collect()
}
type Pattern = Vec<(usize, usize)>;
// Independent full Cartesian product, including zero assignments before filtering.
fn cartesian(rows: &[Vec<Probability>], cutoff: usize) -> BTreeMap<Pattern, R> {
    let count = rows.iter().map(Vec::len).product::<usize>();
    let mut result = BTreeMap::new();
    for code in 0..count {
        let mut digits = code;
        let mut probability = R::one();
        let mut faults = Vec::new();
        for (site, row) in rows.iter().enumerate() {
            let choice = digits % row.len();
            digits /= row.len();
            probability *= row[choice].value();
            if choice > 0 {
                faults.push((site, choice - 1));
            }
        }
        if faults.len() <= cutoff && !probability.is_zero() {
            assert!(result.insert(faults, probability).is_none());
        }
    }
    result
}
#[test]
fn every_small_weight_sector_matches_a_full_cartesian_probability_oracle() {
    for probabilities in [
        rows(&[]),
        rows(&[&["1", "0"], &["1", "0", "0"]]),
        rows(&[&["0", "1"]]),
        rows(&[&["0", "1/3", "2/3"], &["0", "1/4", "3/4"]]),
        rows(&[
            &["1/2", "1/3", "1/6"],
            &["0", "2/5", "3/5"],
            &["1", "0", "0"],
            &["1/7", "0", "6/7"],
        ]),
        rows(&[
            &["1/2", "1/2"],
            &["1/3", "1/6", "1/2"],
            &["1/4", "1/4", "1/4", "1/4"],
        ]),
    ] {
        let model = NoiseModel::new(probabilities.clone(), &NoiseOptions::default()).unwrap();
        for cutoff in 0..=probabilities.len() + 2 {
            let options = EnumerationOptions {
                max_weight: cutoff,
                ..Default::default()
            };
            let actual = enumerate_noise(&model, &options).unwrap();
            let expected = cartesian(&probabilities, cutoff);
            let mut by_weight = vec![R::zero(); cutoff.min(probabilities.len()) + 1];
            for (faults, p) in &expected {
                by_weight[faults.len()] += p;
            }
            let observed = actual
                .patterns()
                .iter()
                .map(|p| {
                    (
                        p.faults
                            .iter()
                            .map(|f| (f.site, f.alternative))
                            .collect::<Vec<_>>(),
                        p.probability.value().clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(observed.len(), actual.patterns().len());
            assert_eq!(observed, expected);
            assert_eq!(
                actual
                    .by_weight()
                    .iter()
                    .map(|p| p.value().clone())
                    .collect::<Vec<_>>(),
                by_weight
            );
            let covered = by_weight.iter().sum::<R>();
            assert_eq!(actual.covered_probability().value(), &covered);
            assert_eq!(actual.omitted_probability().value(), &(R::one() - covered));
            assert!(
                actual
                    .patterns()
                    .windows(2)
                    .all(|p| p[0].weight() <= p[1].weight())
            );
            assert!(
                actual
                    .patterns()
                    .iter()
                    .all(|p| p.probability.value() > &R::zero())
            );
            assert_eq!(
                actual.patterns(),
                enumerate_noise(&model, &options).unwrap().patterns()
            );
        }
    }
}
#[test]
fn mandatory_choices_and_sparse_patterns_preserve_large_site_indices_and_closed_form_mass() {
    let mut probabilities = vec![vec!["1/2".parse().unwrap(), "1/2".parse().unwrap()]; 131];
    probabilities[64] = vec![
        "0".parse().unwrap(),
        "1/3".parse().unwrap(),
        "2/3".parse().unwrap(),
    ];
    let model = NoiseModel::new(probabilities, &NoiseOptions::default()).unwrap();
    let impossible = enumerate_noise(
        &model,
        &EnumerationOptions {
            max_weight: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(impossible.patterns().is_empty());
    assert_eq!(impossible.covered_probability().value(), &R::zero());
    assert_eq!(impossible.omitted_probability().value(), &R::one());
    let result = enumerate_noise(
        &model,
        &EnumerationOptions {
            max_weight: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.patterns().len(), 262);
    let baseline = R::new(1.into(), num_bigint::BigInt::one() << 130);
    assert_eq!(result.by_weight()[1].value(), &baseline);
    assert_eq!(
        result.by_weight()[2].value(),
        &(&baseline * R::from_integer(130.into()))
    );
    assert_eq!(
        result.covered_probability().value(),
        &(&baseline * R::from_integer(131.into()))
    );
    for p in result.patterns() {
        let forced = p.faults.iter().find(|f| f.site == 64).unwrap();
        let forced_mass = if forced.alternative == 0 {
            R::new(1.into(), 3.into())
        } else {
            R::new(2.into(), 3.into())
        };
        assert_eq!(p.probability.value(), &(&baseline * forced_mass));
    }
    for site in [0, 63, 65, 128, 130] {
        assert!(
            result
                .patterns()
                .iter()
                .any(|p| p.faults.iter().any(|f| f.site == site))
        );
    }
}
#[test]
fn pattern_storage_and_counting_caps_have_exact_acceptance_boundaries() {
    let probabilities = rows(&[
        &["1/2", "1/4", "1/4"],
        &["1/3", "1/3", "1/3"],
        &["0", "1/2", "1/2"],
    ]);
    let expected = cartesian(&probabilities, 2);
    let entries = expected.keys().map(Vec::len).sum();
    let model = NoiseModel::new(probabilities, &NoiseOptions::default()).unwrap();
    let exact = EnumerationOptions {
        max_weight: 2,
        max_patterns: expected.len(),
        max_fault_entries: entries,
        max_sites: 3,
        max_site_weight_pairs: 9,
        ..Default::default()
    };
    assert_eq!(
        enumerate_noise(&model, &exact).unwrap().patterns().len(),
        expected.len()
    );
    for (option, limited) in [
        (
            "max_patterns",
            EnumerationOptions {
                max_patterns: exact.max_patterns - 1,
                ..exact
            },
        ),
        (
            "max_fault_entries",
            EnumerationOptions {
                max_fault_entries: exact.max_fault_entries - 1,
                ..exact
            },
        ),
        (
            "max_sites",
            EnumerationOptions {
                max_sites: 2,
                ..exact
            },
        ),
        (
            "max_site_weight_pairs",
            EnumerationOptions {
                max_site_weight_pairs: 8,
                ..exact
            },
        ),
    ] {
        assert!(
            matches!(enumerate_noise(&model,&limited),Err(EnumerationError::ResourceLimit {option:actual,..}) if actual==option)
        );
    }
    assert!(matches!(
        enumerate_noise(
            &model,
            &EnumerationOptions {
                max_patterns: 0,
                ..exact
            }
        ),
        Err(EnumerationError::InvalidPatternLimit)
    ));
    let empty = NoiseModel::new(vec![], &NoiseOptions::default()).unwrap();
    let result = enumerate_noise(
        &empty,
        &EnumerationOptions {
            max_patterns: usize::MAX,
            max_weight: usize::MAX,
            max_fault_entries: 0,
            max_sites: 0,
            max_site_weight_pairs: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.patterns().len(), 1);
    assert!(result.patterns()[0].faults.is_empty());
    assert_eq!(result.covered_probability().value(), &R::one());
    assert_eq!(result.omitted_probability().value(), &R::zero());
}
#[test]
fn probability_bit_budgets_preserve_tiny_nonzero_sectors_and_fail_explicitly() {
    let tiny: Probability = "1e-400".parse().unwrap();
    let model = NoiseModel::new(
        vec![vec![tiny.complement(), tiny.clone()]],
        &NoiseOptions::default(),
    )
    .unwrap();
    let result = enumerate_noise(&model, &EnumerationOptions::default()).unwrap();
    assert_eq!(result.by_weight()[1], tiny);
    assert_eq!(result.covered_probability().value(), &R::one());
    assert!(matches!(
        enumerate_noise(
            &model,
            &EnumerationOptions {
                max_probability_bits: 100,
                ..Default::default()
            }
        ),
        Err(EnumerationError::ResourceLimit {
            option: "max_probability_bits",
            ..
        })
    ));
    let simple = NoiseModel::new(rows(&[&["2/3", "1/3"]]), &NoiseOptions::default()).unwrap();
    let boundary = EnumerationOptions {
        max_weight: 0,
        max_probability_bits: 2,
        ..Default::default()
    };
    assert_eq!(
        enumerate_noise(&simple, &boundary)
            .unwrap()
            .covered_probability()
            .value(),
        &R::new(2.into(), 3.into())
    );
    assert!(matches!(
        enumerate_noise(
            &simple,
            &EnumerationOptions {
                max_probability_bits: 1,
                ..boundary
            }
        ),
        Err(EnumerationError::ResourceLimit {
            option: "max_probability_bits",
            required: 2,
            ..
        })
    ));
}
#[test]
fn correlated_multi_pauli_errors_and_report_flips_each_count_as_one_category() {
    use rmagna::circuit::{ParseOptions, parse_circuit};
    use rmagna::instrument::{CompileOptions, compile_affine};
    use rmagna::scalar::ScalarArithmetic;
    let circuit = parse_circuit(
        "CORRELATED_ERROR(1/4) X0 Z1\nELSE_CORRELATED_ERROR(1/2) Y0\nM(1/5) 0",
        &ParseOptions::default(),
    )
    .unwrap();
    let instrument = compile_affine(
        &circuit,
        &CompileOptions::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap();
    assert!(
        instrument.noise_sites()[0].alternatives()[0]
            .word()
            .x()
            .get(0)
            .unwrap()
    );
    assert!(
        instrument.noise_sites()[0].alternatives()[0]
            .word()
            .z()
            .get(1)
            .unwrap()
    );
    let model = NoiseModel::physical(&instrument, &NoiseOptions::default()).unwrap();
    let result = enumerate_noise(&model, &EnumerationOptions::default()).unwrap();
    assert_eq!(result.patterns().len(), 4);
    assert!(result.patterns().iter().all(|p| p.weight() <= 1));
    assert_eq!(
        result.covered_probability().value(),
        &R::new(7.into(), 8.into())
    );
    assert_eq!(
        result.omitted_probability().value(),
        &R::new(1.into(), 8.into())
    );
}

#[test]
fn mandatory_tail_can_remove_transient_counts_above_the_final_pattern_cap() {
    let model = NoiseModel::new(
        rows(&[&["1/2", "1/4", "1/4"], &["0", "1"]]),
        &NoiseOptions::default(),
    )
    .unwrap();
    let result = enumerate_noise(
        &model,
        &EnumerationOptions {
            max_weight: 1,
            max_patterns: 1,
            max_fault_entries: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.patterns().len(), 1);
    assert_eq!(
        result.patterns()[0].faults,
        [rmagna::instrument::FaultSelection {
            site: 1,
            alternative: 0
        }]
    );
    assert_eq!(
        result.covered_probability().value(),
        &R::new(1.into(), 2.into())
    );
}
