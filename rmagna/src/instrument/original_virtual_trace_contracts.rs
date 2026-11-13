//! Complete VirtualCliffordFeedbackTests::test_virtual_trace_future_completion_reports_faults_and_reset_match_literal,
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_virtual_clifford_feedback.py.
use super::original_feedback_support::*;
use super::*;

const SOURCE: &str = "H 0 1 2\nT 2\nM(1/3) 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCONTROL[XOR] ctrl[0] rec[-1] -> ctrl[1]\nCS ctrl[1] 2\nY_ERROR(1/3) 2\nH 2\nM 2\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[2]\nCH ctrl[2] 2\nR 2\nH 2\nCS_DAG ctrl[0] 2";

fn literal(
    first: bool,
    second: bool,
    config: (crate::unitary::CompilerStrategy, QueryPreparation),
) -> CompiledInstrument {
    // Independent text substitution: no control-expression evaluator, template
    // lowering or compiler metadata is consulted to build the literal circuit.
    let mut text = String::from("H 0 1 2\nT 2\nM(1/3) 0 1\n");
    text.push_str(if first {
        "CS !rec[-1] 2\n"
    } else {
        "CS rec[-1] 2\n"
    });
    text.push_str("Y_ERROR(1/3) 2\nH 2\nM 2\n");
    if second {
        text.push_str("H 2\n");
    }
    text.push_str("R 2\nH 2\n");
    if first {
        text.push_str("S_DAG 2\n");
    }
    let options = CompileOptions {
        compiler_strategy: config.0,
        moment_preparation: config.1,
        ..Default::default()
    };
    compile_affine(&parse(&text), &options, &ScalarArithmetic::default()).unwrap()
}
fn event(view: &CompiledInstrument, bits: usize, count: usize) -> CompiledInstrument {
    view.with_reported_conditions(
        &(0..count)
            .map(|r| (BitExpr::atom(BitAtom::Record(r)), bits & (1 << r) != 0))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn moment(
    view: &CompiledInstrument,
    bits: usize,
    count: usize,
    axes: usize,
    faults: &[FaultSelection],
) -> ExactScalar {
    event(view, bits, count)
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
fn view(
    template: &CompiledInstrument,
    decisions: BTreeMap<(usize, usize), bool>,
) -> CompiledInstrument {
    let branch = template
        .branch(&decisions, &(0..template.noise_sites().len()).collect())
        .unwrap();
    assert!(Arc::ptr_eq(&branch.unitary, &template.unitary));
    assert!(Arc::ptr_eq(
        &branch.moment_queries,
        &template.moment_queries
    ));
    for q in branch.decision_wires.values() {
        assert!(!branch.live_wires.iter().flatten().any(|p| p == q));
    }
    branch
}
#[test]
fn original_virtual_trace_faults_reset_and_future_completion_match_literal() {
    let a = ScalarArithmetic::default();
    let mut full_checks = 0;
    let mut prefix_checks = 0;
    for config in configurations() {
        let compiled = template(&parse(SOURCE), config);
        assert_eq!(compiled.num_records(), 3);
        assert_eq!(compiled.noise_sites().len(), 4);
        assert_eq!(compiled.decision_wires.len(), 2);
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
            for word in 0..4 {
                let actual = view(
                    &compiled,
                    BTreeMap::from([((0, 0), word & 1 != 0), ((1, 0), word & 2 != 0)]),
                );
                let expected = literal(word & 1 != 0, word & 2 != 0, config);
                let mut mass = ExactScalar::zero();
                for bits in 0..8 {
                    for axes in 0..4 {
                        let value = moment(&actual, bits, 3, axes, &faults);
                        assert_eq!(value, moment(&expected, bits, 3, axes, &faults));
                        if axes == 0 {
                            mass = a.add(&mass, &value).unwrap();
                        }
                        full_checks += 1;
                    }
                }
                assert_eq!(mass, ExactScalar::one());
            }
            for (decisions, count, first) in [
                (BTreeMap::new(), 2, false),
                (BTreeMap::from([((0, 0), true)]), 3, true),
            ] {
                let actual = view(&compiled, decisions);
                let expected = literal(first, false, config);
                for bits in 0..(1 << count) {
                    let value = moment(&actual, bits, count, 0, &faults);
                    assert_eq!(value, moment(&expected, bits, count, 0, &faults));
                    if count == 2 {
                        assert_eq!(value, a.omega(0, -4).unwrap());
                    }
                    prefix_checks += 1;
                }
            }
        }
    }
    assert_eq!(full_checks, 1024);
    assert_eq!(prefix_checks, 96);
}
