//! Complete three controlled-Clifford contracts, itpyi/magna
//! 56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_virtual_clifford_feedback.py.
use super::original_feedback_support::*;
use super::*;
use crate::circuit::Operation;

#[test]
fn original_two_qubit_clifford_controls_match_dense_states_and_reuse_template() {
    let mut checks = 0;
    for config in configurations() {
        for gate in ["CCZ", "CCX"] {
            for mode in ["direct", "xor", "linear"] {
                let mut text = String::from("H 0 1 2 3\nT 2 3\nS 3\nM 0 1\n");
                if mode != "linear" {
                    text.push_str("DETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\n");
                }
                if mode == "xor" {
                    text.push_str("CONTROL[XOR] !ctrl[0] rec[-1] -> ctrl[1]\n");
                }
                let control = match mode {
                    "direct" => "ctrl[0]",
                    "xor" => "ctrl[1]",
                    _ => "!rec[-1]",
                };
                text.push_str(&format!("{gate} {control} 2 3"));
                let source = parse(&text);
                let compiled = template(&source, config);
                assert_eq!(compiled.decision_wires.len(), usize::from(mode != "linear"));
                for decision in [false, true] {
                    let view = branch(&compiled, (mode != "linear").then_some(decision));
                    for record in 0..4 {
                        let r1 = record & 2 != 0;
                        let active = match mode {
                            "direct" => decision,
                            "xor" => !r1 ^ decision,
                            _ => !r1,
                        };
                        let mut expected = Dense::zero(4);
                        for q in 0..4 {
                            expected.h(q);
                        }
                        expected.phase(2, 1);
                        expected.phase(3, 3);
                        expected.project(0, record & 1 != 0);
                        expected.project(1, r1);
                        if active {
                            if gate == "CCZ" {
                                expected.cz(2, 3);
                            } else {
                                expected.cx(2, 3);
                            }
                        }
                        let event = conditioned(&view, record);
                        assert_eq!(
                            event
                                .acceptance(&[], 256, &ScalarArithmetic::default())
                                .unwrap()
                                .value,
                            ScalarArithmetic::default().omega(0, -4).unwrap()
                        );
                        for axes in 0..16 {
                            check(&event, &[2, 3], axes, expected.moment(&[2, 3], axes));
                            checks += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checks, 3072);
}

#[test]
fn original_direct_and_xor_cliffords_keep_one_template_and_match_dense_states() {
    let mut checks = 0;
    for config in configurations() {
        for gate in ["CS", "CS_DAG", "CH"] {
            for mixed in [false, true] {
                let mut text = String::from(
                    "H 0 1 2\nT 2\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\n",
                );
                if mixed {
                    text.push_str("CONTROL[XOR] !ctrl[0] rec[-1] -> ctrl[1]\n");
                }
                text.push_str(&format!("{gate} ctrl[{}] 2\nT 2", usize::from(mixed)));
                let compiled = template(&parse(&text), config);
                assert_eq!(compiled.decision_wires.len(), 1);
                for decision in [false, true] {
                    let view = branch(&compiled, Some(decision));
                    for record in 0..4 {
                        let active = decision ^ (mixed && record & 2 == 0);
                        let mut expected = Dense::zero(3);
                        for q in 0..3 {
                            expected.h(q);
                        }
                        expected.phase(2, 1);
                        expected.project(0, record & 1 != 0);
                        expected.project(1, record & 2 != 0);
                        if active {
                            match gate {
                                "CS" => expected.phase(2, 2),
                                "CS_DAG" => expected.phase(2, -2),
                                _ => expected.h(2),
                            }
                        }
                        expected.phase(2, 1);
                        let event = conditioned(&view, record);
                        assert_eq!(
                            event
                                .acceptance(&[], 256, &ScalarArithmetic::default())
                                .unwrap()
                                .value,
                            ScalarArithmetic::default().omega(0, -4).unwrap()
                        );
                        for axes in 0..4 {
                            check(&event, &[2], axes, expected.moment(&[2], axes));
                            checks += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checks, 768);
}

#[test]
fn original_composed_control_simplification_and_versions_determine_route() {
    let source = parse(
        "H 0 1 2\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCONTROL[XOR] !ctrl[0] rec[-1] -> ctrl[1]\nCONTROL[XOR] ctrl[1] -> ctrl[2]\nCONTROL[XOR] ctrl[2] ctrl[2] -> ctrl[3]\nCONTROL[XOR] ctrl[2] ctrl[2] rec[-1] -> ctrl[4]\nCCZ ctrl[2] 1 2\nCCX ctrl[3] 1 2\nCCZ ctrl[4] 1 2\nCONTROL[XOR] ctrl[0] -> ctrl[0]\nCH ctrl[0] 2\nCONTROL[XOR] rec[-1] -> ctrl[0]\nCS ctrl[0] 2",
    );
    let decision = BitExpr::atom(BitAtom::Decision { call: 0, bit: 0 });
    let record = BitExpr::atom(BitAtom::Record(1));
    let mut composite = decision.clone();
    composite.xor_assign(&record);
    composite = composite.inverted();
    let writes = source
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Control {
                call: None,
                outputs,
                ..
            } => Some(outputs[0].clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        writes
            .iter()
            .map(|w| w.expression.clone())
            .collect::<Vec<_>>(),
        [
            composite.clone(),
            composite,
            BitExpr::default(),
            record.clone(),
            decision,
            record
        ]
    );
    assert_eq!(
        writes
            .iter()
            .map(|w| (w.register, w.version))
            .collect::<Vec<_>>(),
        [(1, 0), (2, 0), (3, 0), (4, 0), (0, 1), (0, 2)]
    );
    let mut checks = 0;
    let a = ScalarArithmetic::default();
    for config in configurations() {
        let compiled = template(&source, config);
        assert_eq!(
            compiled.decision_wires.len(),
            1,
            "Rust stores one external decision; affine composites use reusable work"
        );
        for decision in [false, true] {
            let view = branch(&compiled, Some(decision));
            let mut totals = vec![ExactScalar::zero(); 16];
            for record in 0..4 {
                let r1 = record & 2 != 0;
                let mut expected = Dense::zero(3);
                for q in 0..3 {
                    expected.h(q);
                }
                expected.project(0, record & 1 != 0);
                expected.project(1, r1);
                if decision ^ !r1 {
                    expected.cz(1, 2);
                }
                // Duplicate XOR cancels the controlled-X. The affine CZ reads
                // the earlier record, and overwritten ctrl0 cannot change it.
                if r1 {
                    expected.cz(1, 2);
                }
                if decision {
                    expected.h(2);
                }
                if r1 {
                    expected.phase(2, 2);
                }
                for (axes, total) in totals.iter_mut().enumerate() {
                    *total = a.add(total, &expected.moment(&[1, 2], axes)).unwrap();
                }
            }
            for (axes, total) in totals.into_iter().enumerate() {
                check(&view, &[1, 2], axes, total);
                checks += 1;
            }
        }
    }
    assert_eq!(checks, 128);
}
