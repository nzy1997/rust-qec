use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    CompileOptions, FaultSelection, InstrumentError, compile_affine, compile_unitary,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

fn circuit(text: &str) -> rmagna::circuit::Circuit {
    parse_circuit(text, &ParseOptions::default()).unwrap()
}
fn target(text: &str, arithmetic: &ScalarArithmetic) -> rmagna::unitary::CompiledUnitary {
    compile_unitary(&circuit(text), &CompileOptions::default(), arithmetic).unwrap()
}

#[test]
fn measurement_feedback_and_selection_preserve_exact_conditional_masses() {
    let arithmetic = ScalarArithmetic::default();
    for text in [
        "H 0\nM 0\nCX rec[-1] 0",
        "H 0\nM 0\nCONTROL[NOT] rec[-1] -> ctrl[0]\nCX !ctrl[0] 0",
    ] {
        let protocol =
            compile_affine(&circuit(text), &CompileOptions::default(), &arithmetic).unwrap();
        let zero = target("I 0", &arithmetic);
        let masses = protocol.masses(&[0], &zero, &[], 1, &arithmetic).unwrap();
        assert_eq!(masses.accepted.value, ExactScalar::one());
        assert_eq!(masses.target_success.value, ExactScalar::one());
        assert_eq!(masses.error, ExactScalar::zero());
    }
    let protocol = compile_affine(
        &circuit("H 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]"),
        &CompileOptions::default(),
        &arithmetic,
    )
    .unwrap();
    let masses = protocol
        .masses(&[0], &target("I 0", &arithmetic), &[], 1, &arithmetic)
        .unwrap();
    assert_eq!(masses.accepted.value, arithmetic.omega(0, -2).unwrap());
    assert_eq!(masses.target_success.value, masses.accepted.value);
    let parity = BitExpr::atom(BitAtom::Record(0));
    for bit in 0..2 {
        let outcome = BitVector::from_indices(1, (bit != 0).then_some(0)).unwrap();
        assert_eq!(
            protocol
                .reported_probability(std::slice::from_ref(&parity), &outcome, &[], 1, &arithmetic)
                .unwrap()
                .value,
            arithmetic.omega(0, -2).unwrap()
        );
    }
}

#[test]
fn report_errors_affect_classical_readers_and_leave_true_measurement_projection_intact() {
    let arithmetic = ScalarArithmetic::default();
    let protocol = compile_affine(
        &circuit("H 0\nM(1/3) 0\nCX rec[-1] 0"),
        &CompileOptions::default(),
        &arithmetic,
    )
    .unwrap();
    assert_eq!(protocol.noise_sites().len(), 1);
    let flip = [FaultSelection {
        site: 0,
        alternative: 0,
    }];
    assert_eq!(
        protocol.fault_probability(&flip).unwrap().to_string(),
        "1/3"
    );
    assert_eq!(protocol.fault_probability(&[]).unwrap().to_string(), "2/3");
    let zero = target("I 0", &arithmetic);
    assert_eq!(
        protocol
            .masses(&[0], &zero, &[], 1, &arithmetic)
            .unwrap()
            .error,
        ExactScalar::zero()
    );
    assert_eq!(
        protocol
            .masses(&[0], &zero, &flip, 1, &arithmetic)
            .unwrap()
            .error,
        ExactScalar::one()
    );
    let rejected = compile_affine(
        &circuit("M(1/3) 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]"),
        &CompileOptions::default(),
        &arithmetic,
    )
    .unwrap();
    assert_eq!(
        rejected.acceptance(&flip, 1, &arithmetic).unwrap().value,
        ExactScalar::zero()
    );
    assert_eq!(
        rejected.acceptance(&[], 1, &arithmetic).unwrap().value,
        ExactScalar::one()
    );
}

#[test]
fn resets_and_mr_trace_old_entanglement_and_prepare_the_declared_basis() {
    let arithmetic = ScalarArithmetic::default();
    let protocol = compile_affine(
        &circuit("H 0\nCX 0 1\nR 0"),
        &CompileOptions::default(),
        &arithmetic,
    )
    .unwrap();
    let masses = protocol
        .masses(&[0, 1], &target("I 0 1", &arithmetic), &[], 1, &arithmetic)
        .unwrap();
    assert_eq!(masses.accepted.value, ExactScalar::one());
    assert_eq!(
        masses.target_success.value,
        arithmetic.omega(0, -2).unwrap()
    );
    for (reset, ideal) in [
        ("RX", "H 0"),
        ("RY", "H 0\nS 0"),
        ("MRX", "H 0"),
        ("MRY", "H 0\nS 0"),
    ] {
        let protocol = compile_affine(
            &circuit(&format!("H 0\n{reset} 0")),
            &CompileOptions::default(),
            &arithmetic,
        )
        .unwrap();
        assert_eq!(
            protocol
                .masses(&[0], &target(ideal, &arithmetic), &[], 1, &arithmetic)
                .unwrap()
                .error,
            ExactScalar::zero()
        );
    }
    let inverted = compile_affine(
        &circuit("X 0\nMR !0\nDETECTOR rec[-1]\nPOSTSELECT det[0]"),
        &CompileOptions::default(),
        &arithmetic,
    )
    .unwrap();
    assert_eq!(
        inverted
            .masses(&[0], &target("I 0", &arithmetic), &[], 1, &arithmetic)
            .unwrap()
            .target_success
            .value,
        ExactScalar::one()
    );
}

#[test]
fn pauli_products_and_correlated_noise_keep_exact_probabilities_and_y_phase() {
    let arithmetic = ScalarArithmetic::default();
    let protocol=compile_affine(&circuit("RY 0 1\nMPP Y0 * Y1\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nE(1/10) X0 Y1\nELSE_CORRELATED_ERROR(2/9) Z1"),&CompileOptions::default(),&arithmetic).unwrap();
    assert_eq!(protocol.noise_sites().len(), 2);
    let site = &protocol.noise_sites()[1];
    assert_eq!(site.alternatives()[0].probability().to_string(), "1/10");
    assert_eq!(site.alternatives()[1].probability().to_string(), "1/5");
    assert_eq!(site.identity_probability().to_string(), "7/10");
    assert_eq!(site.alternatives()[0].word().phase(), 2);
    assert_eq!(site.source_operations().len(), 2);
    assert_eq!(
        protocol.acceptance(&[], 1, &arithmetic).unwrap().value,
        ExactScalar::one()
    );
    assert!(matches!(
        protocol.fault_probability(&[FaultSelection {
            site: 9,
            alternative: 0
        }]),
        Err(InstrumentError::InvalidFault { .. })
    ));
    assert!(matches!(
        protocol.fault_probability(&[
            FaultSelection {
                site: 1,
                alternative: 0
            },
            FaultSelection {
                site: 1,
                alternative: 1
            }
        ]),
        Err(InstrumentError::RepeatedFault { .. })
    ));
}

#[test]
fn affine_clifford_feedback_and_nonclifford_interference_are_exact() {
    let arithmetic = ScalarArithmetic::default();
    let protocol = compile_affine(
        &circuit("H 0\nM 0\nH 1\nT 1\nCS rec[-1] 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]"),
        &CompileOptions::default(),
        &arithmetic,
    )
    .unwrap();
    let masses = protocol
        .masses(&[1], &target("H 0\nT 0", &arithmetic), &[], 1, &arithmetic)
        .unwrap();
    assert_eq!(masses.accepted.value, arithmetic.omega(0, -2).unwrap());
    assert_eq!(masses.error, ExactScalar::zero());
    let ch = compile_affine(
        &circuit("H 0\nM 0\nCH rec[-1] 1"),
        &CompileOptions::default(),
        &arithmetic,
    )
    .unwrap();
    let masses = ch
        .masses(&[1], &target("I 0", &arithmetic), &[], 256, &arithmetic)
        .unwrap();
    assert_eq!(
        masses.target_success.value,
        arithmetic
            .scalar([3.into(), 0.into(), 0.into(), 0.into()], -2)
            .unwrap()
    );
    assert_eq!(masses.error, arithmetic.omega(0, -4).unwrap());
}

#[test]
fn guarded_and_external_operations_require_history_and_resource_budgets_fail_explicitly() {
    let arithmetic = ScalarArithmetic::default();
    let guarded = circuit("CONTROL[XOR](1) -> ctrl[0]\nX_ERROR(.1) ctrl[0] 0");
    assert!(matches!(
        compile_affine(&guarded, &CompileOptions::default(), &arithmetic),
        Err(InstrumentError::NeedsHistory { .. })
    ));
    let options = ParseOptions {
        controllers: std::collections::BTreeSet::from(["decoder".to_owned()]),
        ..Default::default()
    };
    let external = parse_circuit("CONTROL[decoder] -> ctrl[0]", &options).unwrap();
    assert!(matches!(
        compile_affine(&external, &CompileOptions::default(), &arithmetic),
        Err(InstrumentError::NeedsHistory { .. })
    ));
    for options in [
        CompileOptions {
            max_coherent_qubits: 1,
            ..Default::default()
        },
        CompileOptions {
            max_gates: 0,
            ..Default::default()
        },
        CompileOptions {
            max_noise_sites: 0,
            ..Default::default()
        },
    ] {
        assert!(compile_affine(&circuit("H 0\nM 0"), &options, &arithmetic).is_err());
    }
    assert!(compile_unitary(&circuit("M 0"), &CompileOptions::default(), &arithmetic).is_err());
}

#[test]
fn prescribed_faults_match_native_instrument_masses_including_bh2() {
    use rmagna::circuit::Operation;
    let arithmetic = ScalarArithmetic::default();
    let data: serde_json::Value =
        serde_json::from_str(include_str!("data/instrument-reference.json")).unwrap();
    assert_eq!(data["schema"], "rmagna.instrument-reference.v1");
    assert_eq!(
        data["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    let vectors = data["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 28);
    for insertion_preparation in [
        rmagna::instrument::InsertionPreparation::Direct,
        rmagna::instrument::InsertionPreparation::Prepared(
            rmagna::unitary::InsertionPreparationOptions::default(),
        ),
    ] {
        for compiler_strategy in [
            rmagna::unitary::CompilerStrategy::ParityCopies,
            rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
        ] {
            for probability_preparation in [
                rmagna::instrument::ProbabilityPreparation::Cold,
                rmagna::instrument::ProbabilityPreparation::Trace,
                rmagna::instrument::ProbabilityPreparation::FixedFibers(
                    rmagna::fibers::FiberOptions::default(),
                ),
            ] {
                for density_preparation in [
                    rmagna::instrument::QueryPreparation::Cold,
                    rmagna::instrument::QueryPreparation::Trace,
                    rmagna::instrument::QueryPreparation::FixedFibers(
                        rmagna::fibers::FiberOptions::default(),
                    ),
                ] {
                    let options = CompileOptions {
                        insertion_preparation,
                        density_preparation,
                        probability_preparation,
                        compiler_strategy,
                        ..Default::default()
                    };
                    let mut cache = std::collections::BTreeMap::new();
                    for (index, vector) in vectors.iter().enumerate() {
                        let name = vector["case"].as_str().unwrap();
                        let (parsed, protocol, ideal) = cache.entry(name).or_insert_with(|| {
                            let (source, ideal) = if name == "bh2-T" {
                                (
                                    include_str!("data/magna-bh2/physical.magna"),
                                    include_str!("data/magna-bh2/ideal.magna"),
                                )
                            } else {
                                (
                                    vector["source"].as_str().unwrap(),
                                    vector["ideal"].as_str().unwrap(),
                                )
                            };
                            let parsed = circuit(source);
                            let protocol = compile_affine(&parsed, &options, &arithmetic).unwrap();
                            (
                                parsed,
                                protocol,
                                compile_unitary(&circuit(ideal), &options, &arithmetic).unwrap(),
                            )
                        });
                        let parsed = &*parsed;
                        let mut faults = Vec::new();
                        for fault in vector["faults"].as_array().unwrap() {
                            let line = fault["line"].as_u64().unwrap() as usize;
                            let candidates = protocol
                                .noise_sites()
                                .iter()
                                .enumerate()
                                .flat_map(|(site, noise)| {
                                    noise.source_operations().iter().enumerate().filter_map(
                                        move |(alternative, &op)| {
                                            (parsed.operations()[op].location.line == line
                                                && matches!(
                                                    parsed.operations()[op].operation,
                                                    Operation::Noise { .. }
                                                ))
                                            .then_some(FaultSelection { site, alternative })
                                        },
                                    )
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(
                                candidates.len(),
                                1,
                                "unambiguous source noise at line {line}"
                            );
                            faults.push(candidates[0]);
                        }
                        for record in vector["report_flips"].as_array().unwrap() {
                            let record = record.as_u64().unwrap() as usize;
                            let candidates = protocol
                                .noise_sites()
                                .iter()
                                .enumerate()
                                .filter_map(|(site, noise)| {
                                    noise
                                        .source_operations()
                                        .iter()
                                        .any(|&op| {
                                            matches!(parsed.operations()[op].operation,
                    Operation::Measure { record: r, .. } if r == record)
                                        })
                                        .then_some(FaultSelection {
                                            site,
                                            alternative: 0,
                                        })
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(candidates.len(), 1);
                            faults.push(candidates[0]);
                        }
                        let outputs = vector["outputs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_u64().unwrap() as usize)
                            .collect::<Vec<_>>();
                        let masses = protocol
                            .masses(
                                &outputs,
                                ideal,
                                &faults,
                                if name == "bh2-T"
                                    && !matches!(
                                        probability_preparation,
                                        rmagna::instrument::ProbabilityPreparation::FixedFibers(_)
                                    )
                                    && !matches!(
                                        density_preparation,
                                        rmagna::instrument::QueryPreparation::FixedFibers(_)
                                    )
                                {
                                    1
                                } else {
                                    256
                                },
                                &arithmetic,
                            )
                            .unwrap();
                        for (actual, key) in [
                            (&masses.accepted.value, "accepted"),
                            (&masses.target_success.value, "target_success"),
                            (&masses.error, "error"),
                        ] {
                            let expected = &vector[key];
                            let coefficients = std::array::from_fn(|i| {
                                expected["coefficients"][i]
                                    .as_str()
                                    .unwrap()
                                    .parse()
                                    .unwrap()
                            });
                            assert_eq!(
                                actual,
                                &arithmetic
                                    .scalar(coefficients, expected["power"].as_i64().unwrap())
                                    .unwrap(),
                                "{name} vector {index} {key}"
                            );
                        }
                        if name == "bh2-T" && faults.is_empty() {
                            if !matches!(
                                probability_preparation,
                                rmagna::instrument::ProbabilityPreparation::FixedFibers(_)
                            ) {
                                assert_eq!(masses.accepted.terms, 1);
                            }
                            if !matches!(
                                density_preparation,
                                rmagna::instrument::QueryPreparation::FixedFibers(_)
                            ) {
                                assert_eq!(masses.target_success.terms, 1);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_frontend_gate_matches_its_literal_matrix_including_global_phase() {
    use rmagna::circuit::Gate;
    let a = ScalarArithmetic::default();
    let z = ExactScalar::zero();
    let one = ExactScalar::one();
    let i = a.omega(2, 0).unwrap();
    let h = a.omega(0, -1).unwrap();
    let ih = a.omega(2, -1).unwrap();
    let plus = a
        .scalar([1.into(), 0.into(), 1.into(), 0.into()], -1)
        .unwrap();
    let minus = plus.conjugate();
    let matrices = [
        (
            Gate::I,
            [[one.clone(), z.clone()], [z.clone(), one.clone()]],
        ),
        (
            Gate::X,
            [[z.clone(), one.clone()], [one.clone(), z.clone()]],
        ),
        (Gate::Y, [[z.clone(), i.negated()], [i.clone(), z.clone()]]),
        (
            Gate::Z,
            [[one.clone(), z.clone()], [z.clone(), one.negated()]],
        ),
        (Gate::H, [[h.clone(), h.clone()], [h.clone(), h.negated()]]),
        (Gate::S, [[one.clone(), z.clone()], [z.clone(), i.clone()]]),
        (
            Gate::Sdag,
            [[one.clone(), z.clone()], [z.clone(), i.negated()]],
        ),
        (
            Gate::T,
            [
                [one.clone(), z.clone()],
                [z.clone(), a.omega(1, 0).unwrap()],
            ],
        ),
        (
            Gate::Tdag,
            [
                [one.clone(), z.clone()],
                [z.clone(), a.omega(7, 0).unwrap()],
            ],
        ),
        (
            Gate::SqrtX,
            [[plus.clone(), minus.clone()], [minus.clone(), plus.clone()]],
        ),
        (
            Gate::SqrtXdag,
            [[minus.clone(), plus.clone()], [plus.clone(), minus.clone()]],
        ),
        (
            Gate::SqrtY,
            [[plus.clone(), plus.negated()], [plus.clone(), plus.clone()]],
        ),
        (
            Gate::SqrtYdag,
            [
                [minus.clone(), minus.clone()],
                [minus.negated(), minus.clone()],
            ],
        ),
        (
            Gate::Hxy,
            [
                [z.clone(), a.omega(7, 0).unwrap()],
                [a.omega(1, 0).unwrap(), z.clone()],
            ],
        ),
        (
            Gate::Hyz,
            [[h.clone(), ih.negated()], [ih.clone(), h.negated()]],
        ),
        (
            Gate::Cxyz,
            [
                [minus.clone(), plus.negated()],
                [minus.clone(), plus.clone()],
            ],
        ),
        (
            Gate::Czyx,
            [
                [plus.clone(), plus.clone()],
                [minus.negated(), minus.clone()],
            ],
        ),
    ];
    let mut cases = Vec::new();
    for (gate, matrix) in &matrices {
        cases.push((
            *gate,
            matrix.iter().map(|row| row.to_vec()).collect::<Vec<_>>(),
        ));
    }
    for (controlled, single) in [
        (Gate::Cx, Gate::X),
        (Gate::Cy, Gate::Y),
        (Gate::Cz, Gate::Z),
        (Gate::Cs, Gate::S),
        (Gate::CsDag, Gate::Sdag),
        (Gate::Ch, Gate::H),
    ] {
        let matrix = &matrices.iter().find(|(gate, _)| *gate == single).unwrap().1;
        let mut full = vec![vec![z.clone(); 4]; 4];
        full[0][0] = one.clone();
        full[2][2] = one.clone();
        for row in 0..2 {
            for col in 0..2 {
                full[1 + 2 * row][1 + 2 * col] = matrix[row][col].clone();
            }
        }
        cases.push((controlled, full));
    }
    for gate in [
        Gate::Swap,
        Gate::Iswap,
        Gate::IswapDag,
        Gate::Ccz,
        Gate::Ccx,
    ] {
        let n = 1 << gate.arity();
        let mut full = vec![vec![z.clone(); n]; n];
        let columns = (0..n)
            .map(|col| match gate {
                Gate::Swap | Gate::Iswap | Gate::IswapDag => {
                    let different = (col & 1) != ((col >> 1) & 1);
                    (
                        if different { col ^ 3 } else { col },
                        if different && gate != Gate::Swap {
                            if gate == Gate::Iswap {
                                i.clone()
                            } else {
                                i.negated()
                            }
                        } else {
                            one.clone()
                        },
                    )
                }
                Gate::Ccz => (col, if col == 7 { one.negated() } else { one.clone() }),
                Gate::Ccx => (if col & 3 == 3 { col ^ 4 } else { col }, one.clone()),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        for (col, (row, value)) in columns.into_iter().enumerate() {
            full[row][col] = value;
        }
        cases.push((gate, full));
    }
    assert_eq!(cases.len(), 28);
    for (gate, matrix) in cases {
        let width = gate.arity();
        for input in 0..1 << width {
            let mut source = String::new();
            for q in 0..width {
                if input >> q & 1 != 0 {
                    source.push_str(&format!("X {q}\n"));
                }
            }
            source.push_str(gate.name());
            for q in 0..width {
                source.push_str(&format!(" {q}"));
            }
            let compiled = target(&source, &a);
            for (output, row) in matrix.iter().enumerate() {
                let bits =
                    BitVector::from_indices(width, (0..width).filter(|&q| output >> q & 1 != 0))
                        .unwrap();
                assert_eq!(
                    compiled.amplitude(&bits, 256, &a).unwrap().value,
                    row[input],
                    "{} input {input} output {output}",
                    gate.name()
                );
            }
        }
    }
}

#[test]
fn categorical_channels_keep_stim_order_and_exact_mutually_exclusive_weights() {
    let arithmetic = ScalarArithmetic::default();
    for (name, width, count) in [
        ("DEPOLARIZE1(3/5)", 1, 3),
        ("DEPOLARIZE2(3/5)", 2, 15),
        ("PAULI_CHANNEL_1(1/10,2/10,3/10)", 1, 3),
        (
            "PAULI_CHANNEL_2(1/1000,2/1000,3/1000,4/1000,5/1000,6/1000,7/1000,8/1000,9/1000,10/1000,11/1000,12/1000,13/1000,14/1000,15/1000)",
            2,
            15,
        ),
    ] {
        let endpoints = if width == 1 { "0" } else { "0 1" };
        let protocol = compile_affine(
            &circuit(&format!("{name} {endpoints}")),
            &CompileOptions::default(),
            &arithmetic,
        )
        .unwrap();
        assert_eq!(protocol.noise_sites().len(), 1);
        let site = &protocol.noise_sites()[0];
        assert_eq!(site.alternatives().len(), count);
        let mut total = protocol.fault_probability(&[]).unwrap();
        for (j, alternative) in site.alternatives().iter().enumerate() {
            let index = j + 1;
            let symbols = if width == 1 {
                vec![index]
            } else {
                vec![index / 4, index % 4]
            };
            for (q, symbol) in symbols.iter().enumerate() {
                assert_eq!(
                    alternative.word().x().get(q).unwrap(),
                    matches!(symbol, 1 | 2)
                );
                assert_eq!(
                    alternative.word().z().get(q).unwrap(),
                    matches!(symbol, 2 | 3)
                );
            }
            assert_eq!(
                alternative.word().phase(),
                (2 * symbols.iter().filter(|&&s| s == 2).count()) as u8
            );
            let expected = if name.starts_with("DEPOLARIZE") {
                format!("3/{}", 5 * count)
            } else if width == 1 {
                format!("{index}/10")
            } else {
                format!("{index}/1000")
            };
            assert_eq!(alternative.probability(), &expected.parse().unwrap());
            let fault = [FaultSelection {
                site: 0,
                alternative: j,
            }];
            total += protocol.fault_probability(&fault).unwrap();
            assert_eq!(
                protocol.acceptance(&fault, 1, &arithmetic).unwrap().value,
                ExactScalar::one()
            );
        }
        assert_eq!(total, num_rational::BigRational::from_integer(1.into()));
    }
}

#[test]
fn prepared_instrument_probabilities_reuse_fault_and_outcome_families() {
    use rmagna::instrument::ProbabilityPreparation as P;
    let a = ScalarArithmetic::default();
    let source = circuit(
        "H 0\nT 0\nZ_ERROR(1/7) 0\nH 0\nM(1/5) !0\nCX rec[-1] 1\nM 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
    );
    let cold = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
    let r0 = BitExpr::atom(BitAtom::Record(0));
    let r1 = BitExpr::atom(BitAtom::Record(1));
    for compiler in [
        rmagna::unitary::CompilerStrategy::ParityCopies,
        rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
    ] {
        for preparation in [
            P::Cold,
            P::Trace,
            P::FixedFibers(rmagna::fibers::FiberOptions::default()),
        ] {
            let options = CompileOptions {
                compiler_strategy: compiler,
                probability_preparation: preparation,
                max_prepared_probabilities: 8,
                ..Default::default()
            };
            let protocol = compile_affine(&source, &options, &a).unwrap();
            for fault_mask in 0..4 {
                let faults = (0..2)
                    .filter(|&site| fault_mask & (1 << site) != 0)
                    .map(|site| FaultSelection {
                        site,
                        alternative: 0,
                    })
                    .collect::<Vec<_>>();
                for outcome in 0..4 {
                    let bits =
                        BitVector::from_indices(2, (0..2).filter(|&j| outcome & (1 << j) != 0))
                            .unwrap();
                    let rows = [r0.clone(), r1.clone()];
                    assert_eq!(
                        protocol
                            .reported_probability(&rows, &bits, &faults, 256, &a)
                            .unwrap()
                            .value,
                        cold.reported_probability(&rows, &bits, &faults, 256, &a)
                            .unwrap()
                            .value
                    );
                }
                for outcome in 0..2 {
                    let bits = BitVector::from_indices(1, (outcome != 0).then_some(0)).unwrap();
                    assert_eq!(
                        protocol
                            .accepted_probability(
                                std::slice::from_ref(&r0),
                                &bits,
                                &faults,
                                256,
                                &a
                            )
                            .unwrap()
                            .value,
                        cold.accepted_probability(
                            std::slice::from_ref(&r0),
                            &bits,
                            &faults,
                            256,
                            &a
                        )
                        .unwrap()
                        .value
                    );
                    let mut inverted = r0.clone();
                    inverted.constant = true;
                    assert_eq!(
                        protocol
                            .reported_probability(&[inverted], &bits, &faults, 256, &a)
                            .unwrap()
                            .value,
                        cold.reported_probability(
                            std::slice::from_ref(&r0),
                            &BitVector::from_indices(1, (outcome == 0).then_some(0)).unwrap(),
                            &faults,
                            256,
                            &a
                        )
                        .unwrap()
                        .value
                    );
                }
                assert_eq!(
                    protocol.acceptance(&faults, 256, &a).unwrap().value,
                    cold.acceptance(&faults, 256, &a).unwrap().value
                );
                let stats = protocol.prepared_probability_statistics().unwrap();
                assert_eq!(
                    stats.cached_queries,
                    if matches!(preparation, P::Cold) { 0 } else { 4 }
                );
                assert_eq!(
                    stats.fixed_resources,
                    if matches!(preparation, P::FixedFibers(_)) {
                        4
                    } else {
                        0
                    }
                );
            }
            // Clones retain one bounded cache, including concurrent first use.
            let clone = protocol.clone();
            assert_eq!(
                clone.prepared_probability_statistics().unwrap(),
                protocol.prepared_probability_statistics().unwrap()
            );
        }
    }
}

#[test]
fn probability_cache_limits_and_concurrent_preparation_remain_explicit() {
    use rmagna::instrument::ProbabilityPreparation as P;
    let a = ScalarArithmetic::default();
    let source = circuit("H 0\nT 0\nH 0\nM 0");
    let row = BitExpr::atom(BitAtom::Record(0));
    for preparation in [
        P::Trace,
        P::FixedFibers(rmagna::fibers::FiberOptions::default()),
    ] {
        let options = CompileOptions {
            probability_preparation: preparation,
            max_prepared_probabilities: 1,
            ..Default::default()
        };
        let protocol = compile_affine(&source, &options, &a).unwrap();
        let cold = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
        std::thread::scope(|scope| {
            for bit in 0..8 {
                let protocol = &protocol;
                let row = &row;
                let cold = &cold;
                let a = &a;
                scope.spawn(move || {
                    let outcome = BitVector::from_indices(1, (bit & 1 != 0).then_some(0)).unwrap();
                    assert_eq!(
                        protocol
                            .reported_probability(std::slice::from_ref(row), &outcome, &[], 256, a)
                            .unwrap()
                            .value,
                        cold.reported_probability(std::slice::from_ref(row), &outcome, &[], 256, a)
                            .unwrap()
                            .value
                    );
                });
            }
        });
        assert_eq!(
            protocol
                .prepared_probability_statistics()
                .unwrap()
                .cached_queries,
            1
        );
        assert!(matches!(
            protocol.acceptance(&[], 256, &a),
            Err(InstrumentError::ResourceLimit {
                option: "max_prepared_probabilities",
                limit: 1,
                required: 2
            })
        ));
        let zero = CompileOptions {
            max_prepared_probabilities: 0,
            ..options
        };
        assert!(matches!(
            compile_affine(&source, &zero, &a),
            Err(InstrumentError::ResourceLimit {
                option: "max_prepared_probabilities",
                ..
            })
        ));
        let empty = compile_affine(&source, &options, &a).unwrap();
        assert!(
            empty
                .reported_probability(
                    std::slice::from_ref(&row),
                    &BitVector::zeros(1).unwrap(),
                    &[],
                    0,
                    &a
                )
                .is_err()
        );
        if matches!(preparation, P::FixedFibers(_)) {
            assert_eq!(
                empty
                    .prepared_probability_statistics()
                    .unwrap()
                    .cached_queries,
                0
            );
        }
        // A failed preparation remains retryable with a sufficient term budget.
        let outcome = BitVector::zeros(1).unwrap();
        assert_eq!(
            empty
                .reported_probability(std::slice::from_ref(&row), &outcome, &[], 256, &a)
                .unwrap()
                .value,
            cold.reported_probability(std::slice::from_ref(&row), &outcome, &[], 256, &a)
                .unwrap()
                .value
        );
    }
}

#[test]
fn target_density_cache_distinguishes_programs_and_reuses_outcomes_and_faults() {
    use rmagna::instrument::QueryPreparation as Q;
    use rmagna::unitary::DensityBoundary;
    let a = ScalarArithmetic::default();
    let source = circuit(
        "H 0\nT 0\nZ_ERROR(1/7) 0\nH 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nCX rec[-1] 1",
    );
    let cold = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
    let ideal = target("H 0\nT 0", &a);
    let other = target("H 0\nT 0", &a);
    let row = BitVector::from_indices(1, [0]).unwrap();
    for mode in [
        Q::Trace,
        Q::FixedFibers(rmagna::fibers::FiberOptions::default()),
    ] {
        let options = CompileOptions {
            density_preparation: mode,
            max_prepared_densities: 2,
            ..Default::default()
        };
        let protocol = compile_affine(&source, &options, &a).unwrap();
        for faulted in [false, true] {
            let faults = if faulted {
                vec![FaultSelection {
                    site: 0,
                    alternative: 0,
                }]
            } else {
                vec![]
            };
            for t in [&ideal, &other] {
                for bit in 0..2 {
                    let outcomes = BitVector::from_indices(1, (bit != 0).then_some(0)).unwrap();
                    let boundary = DensityBoundary {
                        outputs: &[0],
                        event_rows: std::slice::from_ref(&row),
                        outcomes: &outcomes,
                    };
                    assert_eq!(
                        protocol
                            .target_density_weight(&[1], t, boundary, &faults, 256, &a)
                            .unwrap()
                            .value,
                        cold.target_density_weight(&[1], t, boundary, &faults, 256, &a)
                            .unwrap()
                            .value
                    );
                }
            }
            assert_eq!(
                protocol
                    .prepared_density_statistics()
                    .unwrap()
                    .cached_queries,
                2
            );
            assert_eq!(
                protocol
                    .prepared_density_statistics()
                    .unwrap()
                    .fixed_resources,
                if matches!(mode, Q::FixedFibers(_)) {
                    2
                } else {
                    0
                }
            );
        }
        let changed = ideal
            .with_insertions(
                &[rmagna::unitary::PauliInsertion {
                    before_gate: 0,
                    word: rmagna::pauli::PauliWord::new(
                        BitVector::from_indices(1, [0]).unwrap(),
                        BitVector::zeros(1).unwrap(),
                        3,
                    )
                    .unwrap(),
                }],
                &a,
            )
            .unwrap();
        let zero = BitVector::zeros(1).unwrap();
        assert_eq!(
            protocol
                .target_density_weight(
                    &[1],
                    &changed,
                    DensityBoundary {
                        outputs: &[0],
                        event_rows: std::slice::from_ref(&row),
                        outcomes: &zero
                    },
                    &[],
                    256,
                    &a
                )
                .unwrap()
                .value,
            cold.target_density_weight(
                &[1],
                &changed,
                DensityBoundary {
                    outputs: &[0],
                    event_rows: std::slice::from_ref(&row),
                    outcomes: &zero
                },
                &[],
                256,
                &a
            )
            .unwrap()
            .value
        );
        assert_eq!(
            protocol
                .prepared_density_statistics()
                .unwrap()
                .cached_queries,
            2
        );
        let empty = BitVector::zeros(0).unwrap();
        assert!(matches!(
            protocol.target_density_weight(
                &[1],
                &ideal,
                DensityBoundary {
                    outputs: &[0],
                    event_rows: &[],
                    outcomes: &empty
                },
                &[],
                256,
                &a
            ),
            Err(InstrumentError::ResourceLimit {
                option: "max_prepared_densities",
                limit: 2,
                required: 3
            })
        ));
        let invalid = CompileOptions {
            max_prepared_densities: 0,
            ..options
        };
        assert!(matches!(
            compile_affine(&source, &invalid, &a),
            Err(InstrumentError::ResourceLimit {
                option: "max_prepared_densities",
                ..
            })
        ));
        let retry = compile_affine(&source, &options, &a).unwrap();
        assert!(
            retry
                .target_density_weight(
                    &[1],
                    &ideal,
                    DensityBoundary {
                        outputs: &[0],
                        event_rows: &[],
                        outcomes: &empty
                    },
                    &[],
                    0,
                    &a
                )
                .is_err()
        );
        if matches!(mode, Q::FixedFibers(_)) {
            assert_eq!(
                retry.prepared_density_statistics().unwrap().cached_queries,
                0
            );
        }
        let value = retry
            .target_density_weight(
                &[1],
                &ideal,
                DensityBoundary {
                    outputs: &[0],
                    event_rows: &[],
                    outcomes: &empty,
                },
                &[],
                256,
                &a,
            )
            .unwrap()
            .value;
        assert_eq!(
            value,
            cold.target_density_weight(
                &[1],
                &ideal,
                DensityBoundary {
                    outputs: &[0],
                    event_rows: &[],
                    outcomes: &empty
                },
                &[],
                256,
                &a
            )
            .unwrap()
            .value
        );
    }
}

#[test]
fn probability_preparation_compatibility_name_preserves_variant_imports() {
    use rmagna::instrument::ProbabilityPreparation::{Cold, FixedFibers, Trace};

    let arithmetic = ScalarArithmetic::default();
    let source = circuit("H 0\nM 0");
    let parity = BitExpr::atom(BitAtom::Record(0));
    let zero = BitVector::zeros(1).unwrap();
    for mode in [
        Cold,
        Trace,
        FixedFibers(rmagna::fibers::FiberOptions::default()),
    ] {
        let options = CompileOptions {
            probability_preparation: mode,
            ..Default::default()
        };
        let protocol = compile_affine(&source, &options, &arithmetic).unwrap();
        for _ in 0..2 {
            assert_eq!(
                protocol
                    .reported_probability(
                        std::slice::from_ref(&parity),
                        &zero,
                        &[],
                        256,
                        &arithmetic
                    )
                    .unwrap()
                    .value,
                arithmetic.omega(0, -2).unwrap()
            );
        }
        let statistics = protocol.prepared_probability_statistics().unwrap();
        assert_eq!(
            statistics.cached_queries,
            usize::from(!matches!(mode, Cold))
        );
        assert_eq!(
            statistics.fixed_resources,
            usize::from(matches!(mode, FixedFibers(_)))
        );
    }
}

#[test]
fn target_density_cache_keys_both_ordered_output_layouts_and_reuses_each_plan() {
    use rmagna::instrument::{PreparedQueryStatistics, QueryPreparation};
    use rmagna::unitary::DensityBoundary;

    let arithmetic = ScalarArithmetic::default();
    // |1,+,0> has squared overlap 1/4 with |+,1,0>, and unit self-overlap.
    let source = circuit("X 0\nH 1\nI 2");
    let ideal = target("X 0\nH 1\nI 2", &arithmetic);
    let layouts = [[0, 1, 2], [1, 0, 2]];
    let empty = BitVector::zeros(0).unwrap();
    for strategy in [
        rmagna::unitary::CompilerStrategy::ParityCopies,
        rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
    ] {
        let cold_options = CompileOptions {
            compiler_strategy: strategy,
            ..Default::default()
        };
        let cold = compile_affine(&source, &cold_options, &arithmetic).unwrap();
        for mode in [
            QueryPreparation::Trace,
            QueryPreparation::FixedFibers(rmagna::fibers::FiberOptions::default()),
        ] {
            let options = CompileOptions {
                density_preparation: mode,
                max_prepared_densities: 4,
                ..cold_options
            };
            let protocol = compile_affine(&source, &options, &arithmetic).unwrap();
            // Four keys on the SAME instrument and target, then repeat in reverse.
            for (query, (left, right)) in [
                (0, 0),
                (0, 1),
                (1, 0),
                (1, 1),
                (1, 1),
                (1, 0),
                (0, 1),
                (0, 0),
            ]
            .into_iter()
            .enumerate()
            {
                let boundary = DensityBoundary {
                    outputs: &layouts[right],
                    event_rows: &[],
                    outcomes: &empty,
                };
                let expected = if left == right {
                    ExactScalar::one()
                } else {
                    arithmetic.omega(0, -4).unwrap()
                };
                assert_eq!(
                    protocol
                        .target_density_weight(
                            &layouts[left],
                            &ideal,
                            boundary,
                            &[],
                            256,
                            &arithmetic
                        )
                        .unwrap()
                        .value,
                    expected
                );
                assert_eq!(
                    cold.target_density_weight(
                        &layouts[left],
                        &ideal,
                        boundary,
                        &[],
                        256,
                        &arithmetic
                    )
                    .unwrap()
                    .value,
                    expected
                );
                let cached_queries = (query + 1).min(4);
                assert_eq!(
                    protocol.prepared_density_statistics().unwrap(),
                    PreparedQueryStatistics {
                        cached_queries,
                        catalogue_resources: 0,
                        fixed_resources: if matches!(mode, QueryPreparation::FixedFibers(_)) {
                            cached_queries
                        } else {
                            0
                        },
                    }
                );
            }
            assert_eq!(
                cold.prepared_density_statistics().unwrap(),
                PreparedQueryStatistics {
                    cached_queries: 0,
                    fixed_resources: 0,
                    catalogue_resources: 0,
                }
            );
        }
    }
}
