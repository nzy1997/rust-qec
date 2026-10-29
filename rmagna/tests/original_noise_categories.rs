use rmagna::circuit::{Basis, BitAtom, ParseOptions, parse_circuit};
use rmagna::noise::{
    OriginalNoiseAction as Action, OriginalNoiseError, OriginalNoiseOptions,
    original_noise_categories,
};
use rmagna::probability::Probability;

fn p(text: &str) -> Probability {
    text.parse().unwrap()
}

#[test]
fn explicit_readout_endpoints_preserve_internal_site_gaps() {
    let circuit = parse_circuit(
        "M 0\nM(0) 0\nMR 1\nMR(1) 1\nMPP X0*Y1\nMPP(0) Z0 X1",
        &ParseOptions::default(),
    )
    .unwrap();
    let table = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(
        table.iter().map(|c| c.internal_site).collect::<Vec<_>>(),
        vec![1, 3, 5, 6]
    );
    for (category, record, probability) in [
        (&table[0], 1, "0"),
        (&table[1], 3, "1"),
        (&table[2], 5, "0"),
        (&table[3], 6, "0"),
    ] {
        assert_eq!(category.channels.len(), 1);
        assert_eq!(category.channels[0].action, Action::ReportFlip { record });
        assert_eq!(category.channels[0].probability, p(probability));
        assert_eq!(category.identity_probability, p(probability).complement());
        assert!(circuit.readout_noise_declared(category.source_operations[0].operation));
    }
    assert!(!circuit.readout_noise_declared(0));
    assert!(!circuit.readout_noise_declared(7));
    assert!(!circuit.readout_noise_declared(usize::MAX));
}

#[test]
fn pauli_order_and_location_actions_are_physical_after_reset() {
    let args = (1..=15)
        .map(|i| format!("{i}/200"))
        .collect::<Vec<_>>()
        .join(",");
    let text = format!("R 2\nDEPOLARIZE1(3/5) 2 129\nPAULI_CHANNEL_2({args}) 2 129");
    let circuit = parse_circuit(&text, &ParseOptions::default()).unwrap();
    let table = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(table.len(), 3);
    for (category, wire) in table[..2].iter().zip([2, 129]) {
        assert_eq!(category.identity_probability, p("2/5"));
        for (channel, basis) in category.channels.iter().zip([Basis::X, Basis::Y, Basis::Z]) {
            assert_eq!(channel.probability, p("1/5"));
            assert_eq!(
                channel.action,
                Action::Pauli {
                    factors: vec![(wire, basis)]
                }
            );
        }
    }
    assert_ne!(table[0].source_operations, table[1].source_operations);
    assert_eq!(table[2].identity_probability, p("2/5"));
    let bases = [None, Some(Basis::X), Some(Basis::Y), Some(Basis::Z)];
    for (index, channel) in (1..16).zip(&table[2].channels) {
        let mut factors = Vec::new();
        if let Some(basis) = bases[index / 4] {
            factors.push((2, basis));
        }
        if let Some(basis) = bases[index % 4] {
            factors.push((129, basis));
        }
        assert_eq!(channel.action, Action::Pauli { factors });
        assert_eq!(channel.probability, p(&format!("{index}/200")));
    }
}

#[test]
fn correlated_survival_keeps_distinct_anchors_and_endpoint_channels() {
    let circuit = parse_circuit(
        "CORRELATED_ERROR(1/3) X0\nELSE_CORRELATED_ERROR(1/2) Y1\nELSE_CORRELATED_ERROR(1) Z2\nELSE_CORRELATED_ERROR(1) X3",
        &ParseOptions::default(),
    ).unwrap();
    let table = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(table.len(), 1);
    let category = &table[0];
    assert_eq!(category.internal_site, 0);
    assert_eq!(category.identity_probability, p("0"));
    assert_eq!(
        category
            .channels
            .iter()
            .map(|c| c.probability.clone())
            .collect::<Vec<_>>(),
        vec![p("1/3"), p("1/3"), p("1/3"), p("0")]
    );
    for ((channel, anchor), (wire, basis)) in category
        .channels
        .iter()
        .zip(&category.source_operations)
        .zip([(0, Basis::X), (1, Basis::Y), (2, Basis::Z), (3, Basis::X)])
    {
        assert_eq!(channel.source, *anchor);
        assert_eq!(anchor.operation, wire);
        assert_eq!(
            channel.action,
            Action::Pauli {
                factors: vec![(wire, basis)]
            }
        );
    }
    assert_eq!(
        original_noise_categories(
            &circuit,
            &OriginalNoiseOptions {
                max_categories: 1,
                max_channels: 3,
            }
        ),
        Err(OriginalNoiseError::ResourceLimit {
            option: "max_channels",
            limit: 3,
            required: 4
        })
    );
}

#[test]
fn repeat_addresses_and_external_guards_are_captured_without_callbacks() {
    let circuit = parse_circuit(
        "CONTROL[opaque] -> ctrl[0]\nREPEAT 2 {\n M(0) 129\n X_ERROR(1/7) ctrl[0] 129\n}",
        &ParseOptions {
            source: Some("owned-input.magna".into()),
            controllers: ["opaque".into()].into_iter().collect(),
            ..ParseOptions::default()
        },
    )
    .unwrap();
    let table = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(table.len(), 4);
    for (iteration, pair) in table.chunks(2).enumerate() {
        for category in pair {
            let source = &category.source_operations[0];
            assert_eq!(source.location.source.as_deref(), Some("owned-input.magna"));
            assert_eq!(source.location.repeat, vec![iteration as u64]);
        }
        let guard = pair[1].guard.as_ref().unwrap();
        assert!(!guard.constant);
        assert_eq!(
            guard.terms.iter().cloned().collect::<Vec<_>>(),
            vec![BitAtom::Decision { call: 0, bit: 0 }]
        );
        assert_eq!(
            pair[0].channels[0].action,
            Action::ReportFlip { record: iteration }
        );
    }
}

#[test]
fn metadata_budgets_count_channels_and_omit_only_implicit_readout() {
    let implicit = parse_circuit("M 0\nMR 1\nMPP X0 Z1", &ParseOptions::default()).unwrap();
    assert!(
        original_noise_categories(
            &implicit,
            &OriginalNoiseOptions {
                max_categories: 0,
                max_channels: 0,
            }
        )
        .unwrap()
        .is_empty()
    );
    let circuit = parse_circuit("DEPOLARIZE2(0) 0 1", &ParseOptions::default()).unwrap();
    assert_eq!(
        original_noise_categories(
            &circuit,
            &OriginalNoiseOptions {
                max_categories: 0,
                max_channels: 15,
            }
        ),
        Err(OriginalNoiseError::ResourceLimit {
            option: "max_categories",
            limit: 0,
            required: 1
        })
    );
    assert_eq!(
        original_noise_categories(
            &circuit,
            &OriginalNoiseOptions {
                max_categories: 1,
                max_channels: 14,
            }
        ),
        Err(OriginalNoiseError::ResourceLimit {
            option: "max_channels",
            limit: 14,
            required: 15
        })
    );
    let table = original_noise_categories(
        &circuit,
        &OriginalNoiseOptions {
            max_categories: 1,
            max_channels: 15,
        },
    )
    .unwrap();
    assert_eq!(table[0].channels.len(), 15);
    assert!(table[0].channels.iter().all(|c| c.probability == p("0")));
    assert_eq!(table[0].identity_probability, p("1"));
}

#[test]
fn empty_readout_arguments_match_pinned_native_rejections() {
    for (name, targets) in [
        ("M", "0"),
        ("MR", "0"),
        ("MPP", "X0"),
        ("MX", "0"),
        ("MY", "0"),
        ("MZ", "0"),
        ("MRX", "0"),
        ("MRY", "0"),
        ("MRZ", "0"),
        ("MXX", "0 1"),
        ("MYY", "0 1"),
        ("MZZ", "0 1"),
    ] {
        for empty in ["", " "] {
            let error = parse_circuit(
                &format!("{name}({empty}) {targets}"),
                &ParseOptions::default(),
            )
            .unwrap_err();
            assert!(
                error
                    .message
                    .contains("probability argument cannot be empty")
            );
            assert_eq!(error.location.line, 1);
        }
        assert!(parse_circuit(&format!("{name} {targets}"), &ParseOptions::default()).is_ok());
        assert!(parse_circuit(&format!("{name}(0) {targets}"), &ParseOptions::default()).is_ok());
    }
}

#[test]
fn category_inspection_preserves_affine_and_causal_masses_and_entropy() {
    use rmagna::circuit::BitExpr;
    use rmagna::gf2::BitVector;
    use rmagna::instrument::{CompileOptions, compile_affine};
    use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler, SplitMix64};
    use rmagna::runtime::{CausalProgram, CausalStrategy, ControllerRegistry, RuntimeOptions};
    use rmagna::scalar::ScalarArithmetic;
    use rmagna::unitary::CompilerStrategy;

    let arithmetic = ScalarArithmetic::default();
    let text = "H 0\nT 0\nH 0\nM 0\nDEPOLARIZE1(1/5) 1";
    let implicit = parse_circuit(text, &ParseOptions::default()).unwrap();
    let explicit = parse_circuit(&text.replace("M 0", "M(0) 0"), &ParseOptions::default()).unwrap();
    assert_eq!(
        original_noise_categories(&implicit, &OriginalNoiseOptions::default())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        original_noise_categories(&explicit, &OriginalNoiseOptions::default())
            .unwrap()
            .len(),
        2
    );
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let compile = CompileOptions {
            compiler_strategy: compiler,
            ..Default::default()
        };
        let a = compile_affine(&implicit, &compile, &arithmetic).unwrap();
        let b = compile_affine(&explicit, &compile, &arithmetic).unwrap();
        // Existing ordinary measurement sites retain their original selections.
        assert_eq!(a.noise_sites().len(), 2);
        assert_eq!(b.noise_sites().len(), 2);
        // The old zero-probability readout location remains an admissible
        // counterfactual fault selection, although omitted from original metadata.
        let flip = [rmagna::instrument::FaultSelection {
            site: 0,
            alternative: 0,
        }];
        let rows = [BitExpr::atom(BitAtom::Record(0))];
        let zero = BitVector::zeros(1).unwrap();
        let mut one = BitVector::zeros(1).unwrap();
        one.set(0, true).unwrap();
        let physical_zero = a
            .reported_probability(&rows, &zero, &[], 1024, &arithmetic)
            .unwrap();
        let physical_one = a
            .reported_probability(&rows, &one, &[], 1024, &arithmetic)
            .unwrap();
        let flipped_zero = a
            .reported_probability(&rows, &zero, &flip, 1024, &arithmetic)
            .unwrap();
        let flipped_one = a
            .reported_probability(&rows, &one, &flip, 1024, &arithmetic)
            .unwrap();
        assert_ne!(physical_zero.value, physical_one.value);
        assert_eq!(flipped_zero.value, physical_one.value);
        assert_eq!(flipped_one.value, physical_zero.value);
        assert_eq!(
            NoiseModel::physical(&a, &NoiseOptions::default())
                .unwrap()
                .probability(&flip)
                .unwrap()
                .to_string(),
            "0"
        );
        let samplers = [&a, &b].map(|compiled| {
            NoiseSampler::physical(
                NoiseModel::physical(compiled, &NoiseOptions::default()).unwrap(),
                NoiseOptions::default(),
            )
        });
        for seed in 0..16 {
            let mut left = SplitMix64::new(seed);
            let mut right = left;
            let draw = samplers[0].draw(&mut left).unwrap();
            assert_eq!(draw, samplers[1].draw(&mut right).unwrap());
            assert_eq!(left.state(), right.state());
            let _ = original_noise_categories(&implicit, &OriginalNoiseOptions::default()).unwrap();
            let _ = original_noise_categories(&explicit, &OriginalNoiseOptions::default()).unwrap();
            for outcome in [false, true] {
                let mut rhs = BitVector::zeros(1).unwrap();
                rhs.set(0, outcome).unwrap();
                let rows = [BitExpr::atom(BitAtom::Record(0))];
                let x = a
                    .reported_probability(&rows, &rhs, &draw.faults, 1024, &arithmetic)
                    .unwrap();
                let y = b
                    .reported_probability(&rows, &rhs, &draw.faults, 1024, &arithmetic)
                    .unwrap();
                assert!(!x.value.is_zero());
                assert_eq!(x.value, y.value);
            }
        }
    }

    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "v1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    let text = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nX_ERROR(1/3) ctrl[0] 1\nH 1\nT 1\nH 1\nM 1";
    let parse = |text: &str| {
        parse_circuit(
            text,
            &ParseOptions {
                controllers: registry.names(),
                ..Default::default()
            },
        )
        .unwrap()
    };
    let a = parse(text);
    let b = parse(&text.replace("M 0", "M(0) 0").replace("M 1", "M(0) 1"));
    assert_eq!(
        original_noise_categories(&a, &OriginalNoiseOptions::default())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        original_noise_categories(&b, &OriginalNoiseOptions::default())
            .unwrap()
            .len(),
        3
    );
    let requested = [
        BitExpr::atom(BitAtom::Record(0)),
        BitExpr::atom(BitAtom::Record(1)),
    ];
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            let options = RuntimeOptions {
                strategy,
                compile: CompileOptions {
                    compiler_strategy: compiler,
                    ..Default::default()
                },
                ..Default::default()
            };
            let x = CausalProgram::new(&a, &registry, options, &arithmetic).unwrap();
            let y = CausalProgram::new(&b, &registry, options, &arithmetic).unwrap();
            for seed in 0..16 {
                let mut left = SplitMix64::new(seed);
                let mut right = left;
                let _ = original_noise_categories(&a, &OriginalNoiseOptions::default()).unwrap();
                let _ = original_noise_categories(&b, &OriginalNoiseOptions::default()).unwrap();
                let u = x.resolve(&requested, &mut left, &arithmetic).unwrap();
                let v = y.resolve(&requested, &mut right, &arithmetic).unwrap();
                assert_eq!(u.history_mass(), v.history_mass());
                assert_eq!(u.history_proposal_mass(), v.history_proposal_mass());
                assert_eq!(u.noise_probability(), v.noise_probability());
                assert_eq!(u.noise_likelihood_ratio(), v.noise_likelihood_ratio());
                assert_eq!(u.controller_calls(), v.controller_calls());
                assert_eq!(u.requested_values(), v.requested_values());
                assert_eq!(u.faults(), v.faults());
                assert_eq!(u.entropy_words(), v.entropy_words());
                assert_eq!(u.history_queries(), v.history_queries());
                assert_eq!(u.network_compilations(), v.network_compilations());
                assert_eq!(left.state(), right.state());
            }
        }
    }
}
