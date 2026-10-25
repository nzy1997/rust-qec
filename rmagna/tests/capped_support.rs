use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::clifford::CliffordGate;
use rmagna::fibers::FiberOptions;
use rmagna::gf2::BitVector;
use rmagna::instrument::{CompileOptions, InstrumentError, compile_affine};
use rmagna::noise::RandomSource;
use rmagna::pauli::PauliWord;
use rmagna::records::{RecordQuery, SamplingError, SamplingOptions, propose_affine_reported};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{
    CompilerStrategy, PauliInsertion, UnitaryCircuit, UnitaryError, UnitaryGate,
};
use std::collections::BTreeMap;

fn bits(width: usize, word: usize) -> BitVector {
    BitVector::from_indices(
        width,
        (0..width.min(usize::BITS as usize)).filter(|&j| word >> j & 1 != 0),
    )
    .unwrap()
}
fn record(j: usize) -> BitExpr {
    BitExpr::atom(BitAtom::Record(j))
}
struct Word(u64, usize);
impl RandomSource for Word {
    fn next_u64(&mut self) -> u64 {
        self.1 += 1;
        self.0
    }
}
fn walsh(word: usize) -> i64 {
    (0usize..16)
        .map(|x| {
            let b = |j: usize| (x >> j) & 1usize;
            let cubic = b(0) * b(1) * b(2) + b(0) * b(1) * b(3) + b(0) * b(2) * b(3);
            if (cubic + (x & word).count_ones() as usize) & 1 == 0 {
                1
            } else {
                -1
            }
        })
        .sum()
}
#[test]
fn diagonal_parent_terms_match_literal_walsh_probabilities_under_pauli_updates() {
    let a = ScalarArithmetic::default();
    let gates = (0..4)
        .map(|q| UnitaryGate::Clifford(CliffordGate::H(q)))
        .chain([(0, 1, 2), (0, 1, 3), (0, 2, 3)].map(|(a, b, c)| UnitaryGate::Ccz { a, b, c }))
        .chain((0..4).map(|q| UnitaryGate::Clifford(CliffordGate::H(q))));
    let circuit = UnitaryCircuit::new(4, gates).unwrap();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for input in [0, 3, 9, 15] {
            let state = circuit
                .compile_with_strategy(&bits(4, input), &a, strategy)
                .unwrap();
            let independent = circuit
                .compile_with_strategy(&bits(4, input), &a, strategy)
                .unwrap();
            for masks in [vec![1, 2, 4, 8], vec![5, 10, 5], vec![]] {
                let rows = masks.iter().map(|&m| bits(4, m)).collect::<Vec<_>>();
                let plan = state
                    .prepare_probability_support(&rows, 256, FiberOptions::default(), &a)
                    .unwrap();
                let count = plan.fixed_phase_resource().num_terms();
                assert!(count > 0 && count <= 256);
                assert!(matches!(
                    plan.evaluate(&state, count - 1, &a),
                    Err(UnitaryError::Phase(_))
                ));
                assert!(matches!(
                    plan.evaluate(&independent, 256, &a),
                    Err(UnitaryError::IncompatibleProgram)
                ));
                for x in 0..4 {
                    for z in 0..4 {
                        let changed = state
                            .with_insertions(
                                &[PauliInsertion {
                                    before_gate: 7,
                                    word: PauliWord::new(bits(4, x), bits(4, z), 0).unwrap(),
                                }],
                                &a,
                            )
                            .unwrap();
                        let terms = plan.evaluate(&changed, 256, &a).unwrap();
                        let prepared = terms
                            .iter()
                            .map(|k| k.prepare().unwrap())
                            .collect::<Vec<_>>();
                        let mut total = ExactScalar::zero();
                        for output in 0..1usize << masks.len() {
                            // Literal H^4 (-1)^cubic H^4 Fourier sum. A Pauli
                            // before the final H layer flips output bits by z;
                            // its x part changes only output amplitude signs.
                            let numerator: i64 = (0usize..16)
                                .filter(|&physical| {
                                    masks.iter().enumerate().all(|(j, m)| {
                                        ((physical & m).count_ones() as usize & 1)
                                            == (output >> j & 1)
                                    })
                                })
                                .map(|physical| walsh(physical ^ input ^ z).pow(2))
                                .sum();
                            let expected = a
                                .scalar([numerator.into(), 0.into(), 0.into(), 0.into()], -8)
                                .unwrap();
                            let actual = prepared.iter().fold(ExactScalar::zero(), |sum, k| {
                                a.add(&sum, &k.amplitude(&bits(masks.len(), output), &a).unwrap())
                                    .unwrap()
                            });
                            assert_eq!(
                                actual, expected,
                                "{strategy:?},input={input},x={x},z={z},masks={masks:?},output={output}"
                            );
                            total = a.add(&total, &actual).unwrap();
                        }
                        assert_eq!(total, ExactScalar::one());
                        assert_eq!(count, plan.fixed_phase_resource().num_terms());
                    }
                }
            }
            assert!(
                state
                    .prepare_probability_support(&[bits(5, 1)], 256, FiberOptions::default(), &a)
                    .is_err()
            );
            assert!(
                state
                    .prepare_probability_support(&[], 0, FiberOptions::default(), &a)
                    .is_err()
            );
        }
    }
}

#[test]
fn clifford_parent_geometry_changes_keep_exact_probabilities_and_global_phases() {
    let a = ScalarArithmetic::default();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for width in [2, 65] {
            let free = width - 1;
            let circuit = UnitaryCircuit::new(
                width,
                [
                    UnitaryGate::Clifford(CliffordGate::H(0)),
                    UnitaryGate::T { qubit: 0, turns: 1 },
                    UnitaryGate::T { qubit: 0, turns: 1 },
                    UnitaryGate::Clifford(CliffordGate::H(0)),
                    UnitaryGate::Clifford(CliffordGate::H(free)),
                ],
            )
            .unwrap();
            for input in 0..2 {
                let state = circuit
                    .compile_with_strategy(&bits(width, input), &a, strategy)
                    .unwrap();
                let rows = [
                    BitVector::from_indices(width, [free]).unwrap(),
                    BitVector::from_indices(width, [0]).unwrap(),
                    BitVector::from_indices(width, [0, free]).unwrap(),
                    BitVector::from_indices(width, [0]).unwrap(),
                ];
                let plan = state
                    .prepare_probability_support(&rows, 1, FiberOptions::default(), &a)
                    .unwrap();
                assert_eq!(
                    (
                        plan.fixed_phase_resource().quotient_rank(),
                        plan.fixed_phase_resource().num_terms()
                    ),
                    (0, 1)
                );
                for x in 0..2 {
                    for z in 0..2 {
                        for global in 0..8 {
                            let changed = state
                                .with_insertions(
                                    &[PauliInsertion {
                                        before_gate: 2,
                                        word: PauliWord::new(
                                            bits(width, x),
                                            bits(width, z),
                                            global,
                                        )
                                        .unwrap(),
                                    }],
                                    &a,
                                )
                                .unwrap();
                            let terms = plan.evaluate(&changed, 1, &a).unwrap();
                            assert_eq!(terms.len(), 1);
                            assert_eq!(
                                rmagna::gf2::AffineSupport::new(
                                    terms[0].offset().clone(),
                                    terms[0].columns().to_vec()
                                )
                                .unwrap()
                                .dimension(),
                                if x == 1 { 1 } else { 2 }
                            );
                            let prepared = terms
                                .iter()
                                .map(|k| k.prepare().unwrap())
                                .collect::<Vec<_>>();
                            for word in 0..16 {
                                let freebit = word & 1;
                                let bit = word >> 1 & 1;
                                let valid =
                                    (word >> 2 & 1) == (freebit ^ bit) && (word >> 3 & 1) == bit;
                                // H T T H has fair outcomes. Inserting X (or XZ)
                                // between the Ts gives Z (or ZX), up to global phase,
                                // hence a deterministic input^z. The other wire is fair.
                                let expected = if !valid || x == 1 && bit != (input ^ z) {
                                    ExactScalar::zero()
                                } else {
                                    a.omega(0, if x == 1 { -2 } else { -4 }).unwrap()
                                };
                                let actual = prepared.iter().fold(ExactScalar::zero(), |sum, k| {
                                    a.add(&sum, &k.amplitude(&bits(4, word), &a).unwrap())
                                        .unwrap()
                                });
                                assert_eq!(
                                    actual, expected,
                                    "{strategy:?} width={width},input={input},x={x},z={z},global={global},word={word}"
                                );
                            }
                        }
                    }
                }
                assert!(plan.evaluate(&state, 0, &a).is_err());
            }
        }
    }
}

#[test]
fn capped_envelopes_keep_genuine_nonaffine_zeros_and_condition_before_the_hull() {
    let a = ScalarArithmetic::default();
    let source = "H 0 1 2 3\nCCZ 0 1 2\nCCZ 0 1 3\nCCZ 0 2 3\nH 0 1 2 3\nM 0 1 2 3";
    let circuit = parse_circuit(source, &ParseOptions::default()).unwrap();
    let instrument = compile_affine(&circuit, &CompileOptions::default(), &a).unwrap();
    let parities = (0..4).map(record).collect::<Vec<_>>();
    let mut observed = BTreeMap::new();
    for word in 0..16 {
        let draw = propose_affine_reported(
            &instrument,
            &RecordQuery {
                given: &[],
                parities: &parities,
            },
            &[],
            &mut Word(word, 0),
            &SamplingOptions::default(),
            &a,
        )
        .unwrap();
        assert_eq!(draw.proposal_mass, a.omega(0, -8).unwrap());
        let sampled = draw
            .values
            .iter()
            .enumerate()
            .fold(0usize, |x, (j, &b)| x | usize::from(b) << j);
        observed.insert(sampled, walsh(sampled).pow(2));
    }
    assert_eq!(observed.len(), 16);
    assert_eq!(observed.values().filter(|&&v| v == 0).count(), 6);
    assert_eq!(observed.values().sum::<i64>(), 256);
    for condition in [false, true] {
        let given = [(record(1), condition)];
        let mut found = BTreeMap::new();
        for word in 0..16 {
            let draw = propose_affine_reported(
                &instrument,
                &RecordQuery {
                    given: &given,
                    parities: &parities,
                },
                &[],
                &mut Word(word, 0),
                &SamplingOptions::default(),
                &a,
            )
            .unwrap();
            assert_eq!(draw.values[1], condition);
            let sampled = draw
                .values
                .iter()
                .enumerate()
                .fold(0usize, |x, (j, &b)| x | usize::from(b) << j);
            found.insert(sampled, draw.proposal_mass);
        }
        assert!(
            (0..16)
                .filter(|&w| walsh(w) != 0 && (w & 2 != 0) == condition)
                .all(|w| found.contains_key(&w))
        );
        let expected = if condition { 4 } else { 8 };
        assert_eq!(
            found.len(),
            expected,
            "condition={condition},found={found:?}"
        );
        assert!(
            found
                .values()
                .all(|p| *p == a.omega(0, if condition { -4 } else { -6 }).unwrap())
        );
    }
}

#[test]
fn captured_transport_limits_and_wide_support_fail_before_entropy_or_enumeration() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit(
        "H 0\nT 0\nT_DAG 0\nH 0\nM 0\nH 1\nM 1",
        &ParseOptions::default(),
    )
    .unwrap();
    let instrument = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
    assert_eq!(
        instrument
            .reported_support(&[record(0)], &[], &a)
            .unwrap()
            .dimension(),
        1
    );
    let captured = instrument.with_support_transport(&[record(0)]).unwrap();
    let mut entropy = Word(1, 0);
    assert!(
        propose_affine_reported(
            &captured,
            &RecordQuery {
                given: &[],
                parities: &[record(0)]
            },
            &[],
            &mut entropy,
            &SamplingOptions {
                max_terms: 0,
                ..Default::default()
            },
            &a
        )
        .is_err()
    );
    assert_eq!(entropy.1, 0);
    assert!(captured.prepared_support_statistics().unwrap().is_none());
    let draw = propose_affine_reported(
        &captured,
        &RecordQuery {
            given: &[],
            parities: &[record(0)],
        },
        &[],
        &mut entropy,
        &SamplingOptions {
            max_terms: 1,
            max_entropy_words: 0,
        },
        &a,
    )
    .unwrap();
    assert_eq!(draw.values, [false]);
    assert_eq!(draw.proposal_mass, ExactScalar::one());
    assert_eq!(entropy.1, 0);
    let statistics = captured.prepared_support_statistics().unwrap().unwrap();
    assert_eq!(
        (
            statistics.labels,
            statistics.quotient_rank,
            statistics.terms
        ),
        (1, 0, 1)
    );
    assert!(matches!(
        propose_affine_reported(
            &captured,
            &RecordQuery {
                given: &[],
                parities: &[record(1)]
            },
            &[],
            &mut entropy,
            &SamplingOptions::default(),
            &a
        ),
        Err(SamplingError::Instrument(
            InstrumentError::UncapturedSupportQuery
        ))
    ));
    assert_eq!(entropy.1, 0);
    let both = instrument
        .with_support_transport(&[record(0), record(1)])
        .unwrap();
    let mut combined = record(0);
    combined.xor_assign(&record(1));
    let draw = propose_affine_reported(
        &both,
        &RecordQuery {
            given: &[],
            parities: &[combined],
        },
        &[],
        &mut Word(1, 0),
        &SamplingOptions::default(),
        &a,
    )
    .unwrap();
    assert_eq!(draw.values, [true]);
    assert_eq!(draw.proposal_mass, a.omega(0, -2).unwrap());
    let before = both.prepared_support_statistics().unwrap();
    let mut inverted = record(1);
    inverted.constant = true;
    let draw = propose_affine_reported(
        &both,
        &RecordQuery {
            given: &[],
            parities: &[inverted],
        },
        &[],
        &mut Word(1, 0),
        &SamplingOptions::default(),
        &a,
    )
    .unwrap();
    assert_eq!(draw.values, [false]);
    assert_eq!(both.prepared_support_statistics().unwrap(), before);
    let source = format!(
        "H {}\nT 0\nMX 0\nMX 0\nM {}",
        (0..81).map(|q| q.to_string()).collect::<Vec<_>>().join(" "),
        (1..81).map(|q| q.to_string()).collect::<Vec<_>>().join(" ")
    );
    let circuit = parse_circuit(&source, &ParseOptions::default()).unwrap();
    let wide = compile_affine(&circuit, &CompileOptions::default(), &a).unwrap();
    let parities = (0..82).map(record).collect::<Vec<_>>();
    let mut entropy = Word(u64::MAX, 0);
    let draw = propose_affine_reported(
        &wide,
        &RecordQuery {
            given: &[],
            parities: &parities,
        },
        &[],
        &mut entropy,
        &SamplingOptions::default(),
        &a,
    )
    .unwrap();
    assert_eq!(draw.proposal_mass, a.omega(0, -162).unwrap());
    assert_eq!(draw.values[0], draw.values[1]);
    assert_eq!(entropy.1, 2);
    assert_eq!(draw.entropy_words, 2);
}

#[test]
fn complete_small_proposals_match_pinned_native_capped_support_witnesses() {
    use rmagna::instrument::FaultSelection;
    use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
    use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
    use serde_json::{Value, json};
    struct Words([u64; 2], usize);
    impl RandomSource for Words {
        fn next_u64(&mut self) -> u64 {
            let x = self.0[self.1];
            self.1 += 1;
            x
        }
    }
    let a = ScalarArithmetic::default();
    let scalar = |v: &Value| {
        a.scalar(
            std::array::from_fn(|j| v["coefficients"][j].as_str().unwrap().parse().unwrap()),
            v["power"].as_i64().unwrap(),
        )
        .unwrap()
    };
    let data: Value =
        serde_json::from_str(include_str!("data/capped-support-reference.json")).unwrap();
    assert_eq!(
        data["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    for case in data["vectors"].as_array().unwrap() {
        let source = parse_circuit(
            case["source"].as_str().unwrap(),
            &ParseOptions {
                controllers: registry.names(),
                ..Default::default()
            },
        )
        .unwrap();
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let modules = parse_task(
            &json!({"sample_records":case["sample_records"]}).to_string(),
            std::path::Path::new("."),
            &TaskOptions::default(),
        )
        .unwrap()
        .load_modules(&source, &parse)
        .unwrap();
        let faults = case["report_flips"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| FaultSelection {
                site: r.as_u64().unwrap() as usize,
                alternative: 0,
            })
            .collect::<Vec<_>>();
        let expected = case["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["records"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|b| b.as_u64().unwrap() != 0)
                        .collect::<Vec<_>>(),
                    r,
                )
            })
            .collect::<BTreeMap<_, _>>();
        for strategy in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = strategy;
                options.runtime.strategy = causal;
                options.runtime.history_sampler = HistorySampler::AffineSupport;
                let evaluator = TaskEvaluator::new(&source, &modules, &registry, options).unwrap();
                let mut found = BTreeMap::new();
                for first in 0..16 {
                    for second in 0..8 {
                        let row = evaluator
                            .sample_prescribed(&faults, &mut Words([first, second], 0))
                            .unwrap();
                        let expected = expected.get(&row.sampled_records).unwrap_or_else(|| {
                            panic!(
                                "unexpected proposal {:?}:{} {strategy:?} {causal:?}",
                                row.sampled_records, case["case"]
                            )
                        });
                        assert_eq!(
                            row.history_proposal_mass,
                            Some(scalar(&expected["proposal_probability"])),
                            "{} {strategy:?} {causal:?} {:?}",
                            case["case"],
                            row.sampled_records
                        );
                        assert_eq!(row.history_mass, scalar(&expected["history_mass"]));
                        assert_eq!(row.raw_acceptance, scalar(&expected["acceptance"]));
                        found.insert(row.sampled_records, ());
                    }
                }
                assert_eq!(
                    found.len(),
                    expected.len(),
                    "{} {strategy:?} {causal:?}",
                    case["case"]
                );
            }
        }
    }
}
