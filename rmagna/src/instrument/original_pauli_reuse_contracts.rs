//! Three complete PauliFeedbackReuseTests methods at itpyi/magna
//! 56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_pauli_feedback_reuse.py.
use super::original_feedback_support::*;
use super::*;
use crate::noise::SplitMix64;
use crate::runtime::{CausalProgram, ControllerRegistry, RuntimeOptions};
use std::sync::Mutex;

const SOURCE: &str = "H 0 1 2\nT 2\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[echo] det[-2] det[-1] -> ctrl[1] ctrl[2]\nCONTROL[XOR] !ctrl[0] rec[-1] -> ctrl[3]\nCX ctrl[3] 2\nY_ERROR(1/3) 2\nT 2\nCY ctrl[1] 2\nH 2\nCZ 2 ctrl[2]\nM(1/4) 2\nR 2\nCX ctrl[0] 2\nH 2\nT 2";
fn literal(
    word: usize,
    config: (crate::unitary::CompilerStrategy, QueryPreparation),
) -> CompiledInstrument {
    let mut text = String::from("H 0 1 2\nT 2\nM 0 1\n");
    text.push_str(if word & 1 != 0 {
        "CX rec[-1] 2\n"
    } else {
        "CX !rec[-1] 2\n"
    });
    text.push_str("Y_ERROR(1/3) 2\nT 2\n");
    if word & 2 != 0 {
        text.push_str("Y 2\n");
    }
    text.push_str("H 2\n");
    if word & 4 != 0 {
        text.push_str("Z 2\n");
    }
    text.push_str("M(1/4) 2\nR 2\n");
    if word & 1 != 0 {
        text.push_str("X 2\n");
    }
    text.push_str("H 2\nT 2");
    compile_affine(
        &parse(&text),
        &CompileOptions {
            compiler_strategy: config.0,
            moment_preparation: config.1,
            ..Default::default()
        },
        &ScalarArithmetic::default(),
    )
    .unwrap()
}
fn event(view: &CompiledInstrument, parities: &[&[usize]], bits: usize) -> CompiledInstrument {
    let conditions = parities
        .iter()
        .enumerate()
        .map(|(i, records)| {
            let mut p = BitExpr::default();
            for &r in *records {
                p.xor_assign(&BitExpr::atom(BitAtom::Record(r)));
            }
            (p, bits & (1 << i) != 0)
        })
        .collect::<Vec<_>>();
    view.with_reported_conditions(&conditions).unwrap()
}
fn value(
    view: &CompiledInstrument,
    parities: &[&[usize]],
    bits: usize,
    axes: usize,
    faults: &[FaultSelection],
) -> ExactScalar {
    event(view, parities, bits)
        .pauli_moment(
            &[2],
            &pauli(1, axes),
            faults,
            256,
            &ScalarArithmetic::default(),
        )
        .unwrap()
        .value
}
fn branch_bits(compiled: &CompiledInstrument, word: usize) -> CompiledInstrument {
    let actual = compiled
        .branch(
            &BTreeMap::from([
                ((0, 0), word & 1 != 0),
                ((1, 0), word & 2 != 0),
                ((1, 1), word & 4 != 0),
            ]),
            &(0..compiled.noise_sites().len()).collect(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(&actual.unitary, &compiled.unitary));
    assert!(Arc::ptr_eq(
        &actual.moment_queries,
        &compiled.moment_queries
    ));
    actual
}
#[test]
fn original_all_paulis_mixed_guards_faults_and_record_distributions_share_one_plan() {
    let a = ScalarArithmetic::default();
    let mut comparisons = 0;
    let mut reset_checks = 0;
    let full: &[&[usize]] = &[&[0], &[1], &[2]];
    let parity: &[&[usize]] = &[&[0, 1], &[2]];
    for config in configurations() {
        let compiled = template(&parse(SOURCE), config);
        assert_eq!(compiled.decision_wires.len(), 3);
        assert_eq!(compiled.noise_sites().len(), 4);
        for word in 0..8 {
            let actual = branch_bits(&compiled, word);
            let expected = literal(word, config);
            let mut final_state = Dense::zero(1);
            if word & 1 != 0 {
                final_state.x(0);
            }
            final_state.h(0);
            final_state.phase(0, 1);
            let sign = if word & 1 == 0 { 1 } else { -1 };
            let half_root = a.omega(0, -1).unwrap();
            let signed = if sign == 1 {
                half_root.clone()
            } else {
                half_root.negated()
            };
            assert_eq!(final_state.moment(&[0], 1), signed);
            assert_eq!(final_state.moment(&[0], 2), ExactScalar::zero());
            assert_eq!(final_state.moment(&[0], 3), signed);
            for noisy in [false, true] {
                let faults = if noisy {
                    vec![
                        FaultSelection {
                            site: 0,
                            alternative: 0,
                        },
                        FaultSelection {
                            site: 2,
                            alternative: 0,
                        },
                        FaultSelection {
                            site: 3,
                            alternative: 0,
                        },
                    ]
                } else {
                    vec![]
                };
                let mut mass = ExactScalar::zero();
                for bits in 0..8 {
                    let joint = value(&actual, full, bits, 0, &faults);
                    mass = a.add(&mass, &joint).unwrap();
                    for axes in 0..4 {
                        let result = value(&actual, full, bits, axes, &faults);
                        assert_eq!(result, value(&expected, full, bits, axes, &faults));
                        comparisons += 1;
                        if axes != 0 {
                            assert_eq!(
                                result,
                                a.multiply(&joint, &final_state.moment(&[0], axes)).unwrap()
                            );
                            reset_checks += 1;
                        }
                    }
                }
                assert_eq!(mass, ExactScalar::one());
                let mut parity_mass = ExactScalar::zero();
                for bits in 0..4 {
                    for axes in 0..4 {
                        let result = value(&actual, parity, bits, axes, &faults);
                        assert_eq!(result, value(&expected, parity, bits, axes, &faults));
                        if axes == 0 {
                            parity_mass = a.add(&parity_mass, &result).unwrap();
                        }
                        comparisons += 1;
                    }
                }
                assert_eq!(parity_mass, ExactScalar::one());
                let statistics = compiled.prepared_moment_statistics().unwrap();
                let _ = value(&actual, full, 0, 3, &faults);
                assert_eq!(statistics, compiled.prepared_moment_statistics().unwrap());
            }
        }
    }
    assert_eq!(comparisons, 3072);
    assert_eq!(reset_checks, 1536);
}

#[test]
fn original_unreached_mixed_feedback_preserves_earlier_causal_marginals() {
    let source = parse(
        "H 0 1\nT 1\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCONTROL[XOR] ctrl[0] rec[-1] -> ctrl[1]\nCY ctrl[1] 1\nH 1\nM 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[2]\nCX ctrl[2] 0",
    );
    let a = ScalarArithmetic::default();
    let mut comparisons = 0;
    for config in configurations() {
        let compiled = template(&source, config);
        for (decision, record) in [(None, 0), (Some(false), 1), (Some(true), 1)] {
            let decisions = decision
                .map(|b| BTreeMap::from([((0, 0), b)]))
                .unwrap_or_default();
            let actual = compiled
                .branch(&decisions, &(0..compiled.noise_sites().len()).collect())
                .unwrap();
            assert!(Arc::ptr_eq(&actual.unitary, &compiled.unitary));
            assert!(Arc::ptr_eq(
                &actual.moment_queries,
                &compiled.moment_queries
            ));
            let mut text = String::from("H 0 1\nT 1\nM 0\n");
            text.push_str(if decision.unwrap_or(false) {
                "CY !rec[-1] 1\n"
            } else {
                "CY rec[-1] 1\n"
            });
            text.push_str("H 1\nM 1");
            let expected = compile_affine(
                &parse(&text),
                &CompileOptions {
                    compiler_strategy: config.0,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
            for flip in [false, true] {
                let faults = if flip {
                    vec![FaultSelection {
                        site: 0,
                        alternative: 0,
                    }]
                } else {
                    vec![]
                };
                for bit in [false, true] {
                    let conditions = [(BitExpr::atom(BitAtom::Record(record)), bit)];
                    let left = actual
                        .with_reported_conditions(&conditions)
                        .unwrap()
                        .acceptance(&faults, 256, &a)
                        .unwrap()
                        .value;
                    let right = expected
                        .with_reported_conditions(&conditions)
                        .unwrap()
                        .acceptance(&faults, 256, &a)
                        .unwrap()
                        .value;
                    assert_eq!(left, right);
                    assert_eq!(left, a.omega(0, -2).unwrap());
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 48);
}

#[test]
fn original_decoder_values_are_not_cached_and_mixed_cliffords_reuse_template() {
    let source_text =
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[count] det[-1] -> ctrl[0]\nCX ctrl[0] 1\nH 1\nT 1";
    let a = ScalarArithmetic::default();
    let mut comparisons = 0;
    for config in configurations() {
        let calls = Arc::new(Mutex::new(Vec::<Vec<bool>>::new()));
        let observed = calls.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("count", "1", move |bits: &[bool]| {
                observed.lock().unwrap().push(bits.to_vec());
                Ok(bits.to_vec())
            })
            .unwrap();
        let p = crate::circuit::ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let source = crate::circuit::parse_circuit(source_text, &p).unwrap();
        let options = RuntimeOptions {
            compile: CompileOptions {
                compiler_strategy: config.0,
                moment_preparation: config.1,
                ..Default::default()
            },
            ..Default::default()
        };
        let runtime = CausalProgram::new(&source, &registry, options, &a).unwrap();
        assert_eq!(runtime.reference_compilations(), 1);
        for seed in 0..12 {
            let draw = runtime
                .resolve(&[], &mut SplitMix64::new(seed), &a)
                .unwrap();
            assert_eq!(draw.network_compilations(), 0);
            assert_eq!(draw.controller_calls().len(), 1);
        }
        let log = calls.lock().unwrap();
        assert_eq!(log.len(), 12);
        assert!(log.contains(&vec![false]));
        assert!(log.contains(&vec![true]));
        drop(log);
        assert_eq!(runtime.reference_compilations(), 1);
        let mixed = crate::circuit::parse_circuit(
            &format!("{source_text}\nCS ctrl[0] 1\nCH ctrl[0] 1"),
            &p,
        )
        .unwrap();
        let compiled = template(&mixed, config);
        for bit in [false, true] {
            let actual = branch(&compiled, Some(bit));
            let mut text = String::from("H 0\nM 0\n");
            if bit {
                text.push_str("X 1\n");
            }
            text.push_str("H 1\nT 1\n");
            if bit {
                text.push_str("S 1\nH 1\n");
            }
            let expected = compile_affine(&parse(&text), &options.compile, &a).unwrap();
            for axes in 1..4 {
                assert_eq!(
                    actual
                        .pauli_moment(&[1], &pauli(1, axes), &[], 256, &a)
                        .unwrap()
                        .value,
                    expected
                        .pauli_moment(&[1], &pauli(1, axes), &[], 256, &a)
                        .unwrap()
                        .value
                );
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 24);
}
