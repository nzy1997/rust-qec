// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//! Complete original weighted-noise nonuniform and unit-scale/endpoint contracts.
//! from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_weighted_noise.py.
//! Canonical XYZ channels retain zero X/Y masses. Typed source addresses and
//! SplitMix64 replace native string anchors and Python Random trajectories.
use num_rational::BigRational;
use rmagna::circuit::{Basis, ParseOptions, SourceLocation, parse_circuit};
use rmagna::instrument::{CompileOptions, FaultSelection, compile_affine};
use rmagna::noise::{
    NoiseError, NoiseModel, NoiseOptions, NoiseSampler, NoiseSampling, OriginalNoiseAction,
    OriginalNoiseAnchor, OriginalNoiseOptions, SplitMix64, original_noise_categories,
};
use rmagna::probability::Probability;
use rmagna::scalar::ScalarArithmetic;
use rmagna::unitary::CompilerStrategy;
use std::collections::BTreeSet;

const TEXT: &str = "PAULI_CHANNEL_1(0,0,1/3) 4";

fn p(text: &str) -> Probability {
    text.parse().unwrap()
}

// Native physical_rate applies to every original site, including declared endpoints.
// These explicit parsed ensembles retain the zero/one/variable source roles without
// introducing a physical_rate keyword or claiming Python Random stream identity.
const EFFECTIVE_ENSEMBLES: [(&str, [&str; 3]); 4] = [
    (
        r"X_ERROR(0) 0
Z_ERROR(1) 1
M(1/5) 0",
        ["0", "1", "1/5"],
    ),
    (
        r"X_ERROR(0) 0
Z_ERROR(0) 1
M(0) 0",
        ["0", "0", "0"],
    ),
    (
        r"X_ERROR(1/7) 0
Z_ERROR(1/7) 1
M(1/7) 0",
        ["1/7", "1/7", "1/7"],
    ),
    (
        r"X_ERROR(1) 0
Z_ERROR(1) 1
M(1) 0",
        ["1", "1", "1"],
    ),
];

#[test]
fn unit_scale_and_effective_deterministic_endpoints() {
    let arithmetic = ScalarArithmetic::default();
    let options = NoiseOptions::default();
    let excluded = BTreeSet::new();
    for compiler_strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for (ensemble, (text, rates)) in EFFECTIVE_ENSEMBLES.iter().enumerate() {
            let circuit = parse_circuit(
                text,
                &ParseOptions {
                    source: Some("weighted-endpoints.magna".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!((circuit.num_qubits(), circuit.num_records()), (2, 1));
            let categories =
                original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
            assert_eq!(categories.len(), 3);
            let actions = [
                OriginalNoiseAction::Pauli {
                    factors: vec![(0, Basis::X)],
                },
                OriginalNoiseAction::Pauli {
                    factors: vec![(1, Basis::Z)],
                },
                OriginalNoiseAction::ReportFlip { record: 0 },
            ];
            // Native source IDs zero/one/variable map respectively to these
            // owned operation/source addresses; m0 maps to record0.
            for (site, ((category, rate), action)) in
                categories.iter().zip(rates).zip(&actions).enumerate()
            {
                let anchor = OriginalNoiseAnchor {
                    operation: site,
                    location: SourceLocation {
                        source: Some("weighted-endpoints.magna".into()),
                        line: site + 1,
                        column: 1,
                        repeat: vec![],
                    },
                };
                assert_eq!(category.internal_site, site);
                assert_eq!(category.guard, None);
                assert_eq!(category.identity_probability, p(rate).complement());
                assert_eq!(
                    category.source_operations.as_slice(),
                    std::slice::from_ref(&anchor)
                );
                assert_eq!(category.channels.len(), 1);
                assert_eq!(category.channels[0].source, anchor);
                assert_eq!(category.channels[0].probability, p(rate));
                assert_eq!(category.channels[0].action, *action);
            }
            let compiled = compile_affine(
                &circuit,
                &CompileOptions {
                    compiler_strategy,
                    ..Default::default()
                },
                &arithmetic,
            )
            .unwrap();
            assert_eq!(compiled.noise_sites().len(), 3);
            let model = NoiseModel::physical(&compiled, &options).unwrap();
            for (site, rate) in rates.iter().enumerate() {
                assert_eq!(compiled.noise_sites()[site].source_operations(), &[site]);
                assert_eq!(compiled.noise_sites()[site].alternatives().len(), 1);
                assert_eq!(
                    model.probabilities(site).unwrap(),
                    [p(rate).complement(), p(rate)]
                );
            }
            let physical = NoiseSampler::physical(model.clone(), options);
            let unit = NoiseSampler::configured(
                model.clone(),
                &NoiseSampling::Importance {
                    proposal_scale: q(1, 1),
                },
                &excluded,
                options,
            )
            .unwrap();
            let summary = unit.scaled_proposal_summary().unwrap();
            assert_eq!(summary.proposal_scale, q(1, 1));
            assert_eq!(
                (
                    summary.categories,
                    summary.scaled_categories,
                    summary.unchanged_categories,
                    summary.excluded_categories,
                    summary.endpoint_categories
                ),
                (
                    3,
                    0,
                    3,
                    0,
                    match ensemble {
                        0 => 2,
                        2 => 0,
                        _ => 3,
                    }
                )
            );
            let mut left = SplitMix64::new(26);
            let mut right = SplitMix64::new(26);
            for _ in 0..40 {
                let draw = physical.draw(&mut left).unwrap();
                assert_eq!(draw, unit.draw(&mut right).unwrap());
                assert_eq!(left.state(), right.state());
                assert_eq!(draw.likelihood_ratio, q(1, 1));
                assert_eq!(draw.physical_probability, draw.proposal_probability);
                for fault in &draw.faults {
                    assert_eq!(fault.alternative, 0);
                    assert!(fault.site < 3);
                    assert_ne!(rates[fault.site], "0");
                    assert_eq!(
                        categories[fault.site].channels[0].action,
                        actions[fault.site]
                    );
                }
                for (site, rate) in rates.iter().enumerate() {
                    let selected = draw.faults.contains(&FaultSelection {
                        site,
                        alternative: 0,
                    });
                    if *rate == "0" {
                        assert!(!selected);
                    }
                    if *rate == "1" {
                        assert!(selected);
                    }
                }
                if ensemble == 1 || ensemble == 3 {
                    assert_eq!(draw.physical_probability, q(1, 1));
                    assert_eq!(draw.entropy_words, 0);
                    assert_eq!(left.state(), 26);
                }
            }
            if ensemble == 1 || ensemble == 3 {
                let endpoint = NoiseSampler::configured(
                    model,
                    &NoiseSampling::Importance {
                        proposal_scale: q(100, 1),
                    },
                    &excluded,
                    options,
                )
                .unwrap();
                let summary = endpoint.scaled_proposal_summary().unwrap();
                assert_eq!(summary.proposal_scale, q(100, 1));
                assert_eq!(
                    (summary.scaled_categories, summary.endpoint_categories),
                    (0, 3)
                );
                let mut left = SplitMix64::new(17);
                let mut right = SplitMix64::new(17);
                let draw = physical.draw(&mut left).unwrap();
                assert_eq!(draw, endpoint.draw(&mut right).unwrap());
                assert_eq!((left.state(), right.state()), (17, 17));
                assert_eq!(draw.entropy_words, 0);
                assert_eq!(draw.likelihood_ratio, q(1, 1));
                let expected = if ensemble == 1 {
                    vec![]
                } else {
                    (0..3)
                        .map(|site| FaultSelection {
                            site,
                            alternative: 0,
                        })
                        .collect()
                };
                assert_eq!(draw.faults, expected);
            } else {
                // Large scales do not clip a genuinely stochastic category.
                assert!(matches!(
                    model.scaled_proposal(&q(100, 1), &excluded, &options),
                    Err(NoiseError::ScaledProbability { site }) if site == if ensemble == 0 { 2 } else { 0 }
                ));
            }
        }
    }
}
fn q(numerator: i64, denominator: i64) -> BigRational {
    BigRational::new(numerator.into(), denominator.into())
}

#[test]
fn deterministic_nonuniform_and_importance() {
    let circuit = parse_circuit(
        TEXT,
        &ParseOptions {
            source: Some("original".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((circuit.num_qubits(), circuit.num_records()), (5, 0));
    let categories = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(categories.len(), 1);
    let category = &categories[0];
    let original = OriginalNoiseAnchor {
        operation: 0,
        location: SourceLocation {
            source: Some("original".into()),
            line: 1,
            column: 1,
            repeat: vec![],
        },
    };
    assert_eq!(category.internal_site, 0);
    assert_eq!(
        category.source_operations.as_slice(),
        std::slice::from_ref(&original)
    );
    assert_eq!(category.guard, None);
    assert_eq!(category.identity_probability, p("2/3"));
    assert_eq!(category.channels.len(), 3);
    for ((channel, basis), mass) in category
        .channels
        .iter()
        .zip([Basis::X, Basis::Y, Basis::Z])
        .zip(["0", "0", "1/3"])
    {
        assert_eq!(channel.source, original);
        assert_eq!(channel.probability, p(mass));
        assert_eq!(
            channel.action,
            OriginalNoiseAction::Pauli {
                factors: vec![(4, basis)]
            }
        );
    }
    // Native X/Z conditional weights (0,1) become XYZ masses (0,0,1/3).
    // There are no measurement sites or report-flip actions in this model.
    assert!(
        category
            .channels
            .iter()
            .all(|channel| matches!(channel.action, OriginalNoiseAction::Pauli { .. }))
    );
    let arithmetic = ScalarArithmetic::default();
    for compiler_strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let compiled = compile_affine(
            &circuit,
            &CompileOptions {
                compiler_strategy,
                ..Default::default()
            },
            &arithmetic,
        )
        .unwrap();
        assert_eq!(compiled.noise_sites().len(), 1);
        let site = &compiled.noise_sites()[0];
        assert_eq!(site.source_operations(), &[0]);
        assert_eq!(site.identity_probability(), &p("2/3"));
        assert_eq!(
            site.alternatives()
                .iter()
                .map(|alt| alt.probability().clone())
                .collect::<Vec<_>>(),
            [p("0"), p("0"), p("1/3")]
        );
        let word = site.alternatives()[2].word();
        assert_eq!((word.width(), word.phase()), (5, 0));
        for qubit in 0..5 {
            assert!(!word.x().get(qubit).unwrap());
            assert_eq!(word.z().get(qubit).unwrap(), qubit == 4);
        }

        let options = NoiseOptions::default();
        let physical = NoiseModel::physical(&compiled, &options).unwrap();
        assert_eq!(physical.num_sites(), 1);
        assert_eq!(
            physical.probabilities(0).unwrap(),
            [p("2/3"), p("0"), p("0"), p("1/3")]
        );
        let proposal = physical
            .scaled_proposal(&q(2, 1), &BTreeSet::new(), &options)
            .unwrap();
        assert_eq!(
            proposal.model().probabilities(0).unwrap(),
            [p("1/3"), p("0"), p("0"), p("2/3")]
        );
        // Zero channels remain addressable but have zero physical and proposal mass.
        for alternative in [0, 1] {
            let zero = [FaultSelection {
                site: 0,
                alternative,
            }];
            assert_eq!(physical.probability(&zero).unwrap(), q(0, 1));
            assert_eq!(proposal.model().probability(&zero).unwrap(), q(0, 1));
        }
        let sampler = NoiseSampler::configured(
            physical.clone(),
            &NoiseSampling::Importance {
                proposal_scale: q(2, 1),
            },
            &BTreeSet::new(),
            options,
        )
        .unwrap();
        assert_eq!(sampler.scaled_proposal_summary(), Some(proposal.summary()));
        let mut rng = SplitMix64::new(14);
        let mut seen = BTreeSet::new();
        for _ in 0..40 {
            let draw = sampler.draw(&mut rng).unwrap();
            let fault = !draw.faults.is_empty();
            let (mass, proposed, ratio) = if fault {
                assert_eq!(
                    draw.faults,
                    [FaultSelection {
                        site: 0,
                        alternative: 2
                    }]
                );
                let chosen = &category.channels[draw.faults[0].alternative];
                assert_eq!(chosen.source, original);
                assert_eq!(
                    chosen.action,
                    OriginalNoiseAction::Pauli {
                        factors: vec![(4, Basis::Z)]
                    }
                );
                (q(1, 3), q(2, 3), q(1, 2))
            } else {
                assert!(draw.faults.is_empty());
                (q(2, 3), q(1, 3), q(2, 1))
            };
            assert_eq!(draw.physical_probability, mass);
            assert_eq!(draw.proposal_probability, proposed);
            assert_eq!(draw.likelihood_ratio, ratio);
            assert_eq!(physical.probability(&draw.faults).unwrap(), mass);
            assert_eq!(
                proposal.model().probability(&draw.faults).unwrap(),
                proposed
            );
            assert_eq!(&draw.likelihood_ratio * &draw.proposal_probability, mass);
            seen.insert(fault);
        }
        assert_eq!(seen, BTreeSet::from([false, true]));
    }
}
