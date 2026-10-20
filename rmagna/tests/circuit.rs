use rmagna::circuit::{BitAtom, BitExpr, Gate, NoiseKind, Operation, ParseOptions, parse_circuit};
use rmagna::probability::Probability;
use std::collections::BTreeSet;

fn parse(text: &str) -> rmagna::circuit::Circuit {
    parse_circuit(text, &ParseOptions::default()).unwrap()
}
fn records(indices: impl IntoIterator<Item = usize>, constant: bool) -> BitExpr {
    BitExpr {
        constant,
        terms: indices.into_iter().map(BitAtom::Record).collect(),
    }
}

#[test]
fn repeat_expansion_snapshots_register_versions_and_reported_record_parities() {
    let source = "# sites\nR 0 1\nM 0\nCONTROL[XOR] rec[-1] -> ctrl[7]\nREPEAT 2 {\n MX(1/10) 1\n CONTROL[XOR](1) ctrl[7] rec[-1] -> ctrl[7]\n CZ ctrl[7] 0\n}\n";
    let circuit = parse(source);
    assert_eq!(circuit.to_text(), source);
    assert_eq!(circuit.format_version(), "magna-0.1");
    assert_eq!(
        (
            circuit.num_qubits(),
            circuit.num_records(),
            circuit.num_instructions()
        ),
        (2, 3, 9)
    );
    let writes = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Control { outputs, .. } => Some((op, outputs)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        writes
            .iter()
            .map(|(_, out)| out[0].version)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(writes[1].0.location.repeat, vec![0]);
    assert_eq!(writes[2].0.location.repeat, vec![1]);
    assert_eq!(writes[1].0.location.line, 7);
    assert_eq!(writes[2].1[0].expression, records([0, 1, 2], false));
    assert_eq!(
        circuit
            .operations()
            .iter()
            .map(|o| o.id)
            .collect::<BTreeSet<_>>()
            .len(),
        circuit.operations().len()
    );
    assert!(!circuit.needs_history_sampling());
}

#[test]
fn controller_inputs_are_causal_atomic_and_not_evaluated_by_the_parser() {
    let source = "M 0 1 2\nCONTROL[XOR] rec[-3] rec[-2] -> ctrl[0]\nDETECTOR rec[-1]\nCONTROL[decoder] ctrl[0] !det[-1] -> ctrl[0] ctrl[1]\nCONTROL[XOR] ctrl[0] ctrl[1] ctrl[0] -> ctrl[2]\nCH ctrl[2] 0\nPOSTSELECT !ctrl[1]\nCONTROL[NOT] ctrl[1] -> ctrl[1]\nCONTROL[XOR] -> ctrl[1]\nOBSERVABLE_INCLUDE(0) ctrl[1]\n";
    let options = ParseOptions {
        controllers: BTreeSet::from(["decoder".to_owned()]),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &options).unwrap();
    assert_eq!(circuit.external_calls(), 1);
    assert!(circuit.needs_history_sampling());
    let call = circuit
        .operations()
        .iter()
        .find_map(|op| match &op.operation {
            Operation::Control {
                call: Some(_),
                inputs,
                outputs,
                ..
            } => Some((inputs, outputs)),
            _ => None,
        })
        .unwrap();
    assert_eq!(call.0, &vec![records([0, 1], false), records([2], true)]);
    assert_eq!(call.1[0].version, 1);
    let decision = BitExpr::atom(BitAtom::Decision { call: 0, bit: 1 });
    let controlled = circuit
        .operations()
        .iter()
        .find_map(|op| match &op.operation {
            Operation::Unitary { gate, control, .. } => Some((gate, control)),
            _ => None,
        })
        .unwrap();
    assert_eq!(controlled.0, &Gate::H);
    assert_eq!(controlled.1, &Some(decision.clone()));
    let selected = circuit
        .operations()
        .iter()
        .find_map(|op| match &op.operation {
            Operation::Postselect { expression } => Some(expression),
            _ => None,
        })
        .unwrap();
    assert_eq!(selected, &decision.clone().inverted());
    assert_eq!(circuit.observables()[&0], BitExpr::default());
    assert_eq!(circuit.detectors(), &[records([2], false)]);
    for bad in [
        "CONTROL[decoder] rec[-1] -> ctrl[0]",
        "CONTROL[decoder] ctrl[0] -> ctrl[0]",
        "CONTROL[decoder] det[0] -> ctrl[0]",
    ] {
        assert!(parse_circuit(bad, &options).is_err());
    }
    assert!(parse_circuit("CONTROL[missing] -> ctrl[0]", &options).is_err());
}

#[test]
fn measurement_products_inversion_and_report_flips_preserve_distinct_records() {
    let circuit = parse(
        "MXX(.01) 0 1\nMPP(1/30) !X0 * X1 Z2\nMRX 0 1\nMY !2\nDETECTOR rec[-1] rec[-4]\nOBSERVABLE_INCLUDE(4.0) rec[-2]\n",
    );
    assert_eq!(circuit.num_records(), 6);
    let measurements = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Measure {
                product,
                reset,
                report_flip,
                record,
            } => Some((product, *reset, report_flip, *record)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        measurements.iter().map(|m| m.3).collect::<Vec<_>>(),
        (0..6).collect::<Vec<_>>()
    );
    assert_eq!(measurements[0].2, &"1/100".parse::<Probability>().unwrap());
    assert_eq!(measurements[1].2, &"1/30".parse::<Probability>().unwrap());
    assert!(measurements[1].0.inverted);
    assert!(measurements[5].0.inverted);
    assert!(measurements[3].1 && measurements[4].1);
    assert!(!measurements[5].1);
    assert_eq!(circuit.detectors(), &[records([5, 2], false)]);
    assert_eq!(circuit.observables()[&4], records([4], false));
    let raw = parse("X 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT !det[0]");
    assert_eq!(raw.detectors(), &[records([0], false)]);
    let selection = raw.operations().last().unwrap();
    assert!(
        matches!(&selection.operation,Operation::Postselect {expression} if expression==&records([0],true))
    );
}

#[test]
fn guarded_correlated_channels_retain_conditional_probabilities_and_site_order() {
    let circuit = parse(
        "M 0\nCONTROL[XOR] rec[-1] -> ctrl[0]\nE(1/10) ctrl[0] X0 Y1\nELSE_CORRELATED_ERROR(2/9) ctrl[0] Z1\nX_ERROR(1_000/100_000) 0\nPAULI_CHANNEL_1(.1,.2,.3) 0 1\n",
    );
    assert!(circuit.needs_history_sampling());
    let noise = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Noise {
                kind,
                probabilities,
                guard,
                ..
            } => Some((kind, probabilities, guard)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(noise.len(), 5);
    assert_eq!(
        noise[0].0,
        &NoiseKind::Correlated {
            chain: 0,
            alternative: 0
        }
    );
    assert_eq!(
        noise[1].0,
        &NoiseKind::Correlated {
            chain: 0,
            alternative: 1
        }
    );
    assert_eq!(noise[0].2, &Some(records([0], false)));
    assert!(noise[2].2.is_none());
    assert_eq!(noise[0].1[0].to_string(), "1/10");
    assert_eq!(noise[1].1[0].to_string(), "2/9");
    assert_eq!(noise[2].1[0].to_string(), "1/100");
    assert!(
        parse_circuit(
            "M 0\nE(.1) rec[-1] X0\nELSE_CORRELATED_ERROR(.1) X0",
            &ParseOptions::default()
        )
        .is_err()
    );
    assert!(
        parse_circuit(
            "E(.1) X0\nTICK\nELSE_CORRELATED_ERROR(.1) X0",
            &ParseOptions::default()
        )
        .is_err()
    );
}

#[test]
fn aliases_conditional_gates_annotations_and_constant_controls_are_explicit() {
    let circuit = parse(
        "RZ 0\nH_XZ 0\nCNOT 0 1\nZCY 1 2\nZCZ 1 2\nMZ 0\nCZ 1 rec[-1]\nCONTROL[XOR](1/1) -> ctrl[0]\nCCX !ctrl[0] 1 2\nCCZ rec[-1] 2 3\nQUBIT_COORDS(-1/2,2.5) 0\nSHIFT_COORDS(0,1)\nTICK\n",
    );
    let gates = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Unitary {
                gate,
                qubits,
                control,
            } => Some((gate, qubits, control)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(gates[0].0, &Gate::H);
    assert_eq!(gates[1].0, &Gate::Cx);
    assert_eq!(gates[2].0, &Gate::Cy);
    assert_eq!(gates[3].0, &Gate::Cz);
    assert_eq!(gates[4], (&Gate::Z, &vec![1], &Some(records([0], false))));
    assert_eq!(
        gates[5],
        (&Gate::Cx, &vec![1, 2], &Some(BitExpr::default()))
    );
    assert_eq!(gates[6].0, &Gate::Cz);
    assert!(
        matches!(&circuit.operations()[circuit.operations().len()-3].operation,Operation::Annotation {coordinates,..} if coordinates==&vec![-0.5,2.5])
    );
}

#[test]
fn syntax_and_resource_rejections_report_source_and_repeat_location() {
    for text in [
        "CX rec[-1] 0",
        "CONTROL[XOR] ctrl[0] -> ctrl[0]",
        "CONTROL[XOR] -> ctrl[0] ctrl[1]",
        "X_ERROR(.1)",
        "PAULI_CHANNEL_1(.6,.3,.2) 0",
        "DEPOLARIZE2(.1) 0",
        "M(1.1) 0",
        "S(.1) 0",
        "CX 0 0",
        "MPP X0*Y0",
        "MPP 0",
        "X_ERROR(.1) X0",
        "ELSE_CORRELATED_ERROR(.1) X0",
        "IF ctrl[0]",
        "REPEAT 2 { H 0",
        "REPEAT 2\nH 0",
        "}",
        "T !0",
        "CONTROL[NOT](1) -> ctrl[0]",
        "POSTSELECT rec[-1]",
        "QUBIT_COORDS(NaN) 0",
    ] {
        let options = ParseOptions {
            source: Some("bad.magna".to_owned()),
            ..Default::default()
        };
        let error = parse_circuit(text, &options).expect_err(text);
        assert!(error.to_string().starts_with("bad.magna:"), "{error}");
    }
    let options = ParseOptions {
        max_instructions: 10,
        ..Default::default()
    };
    assert!(
        parse_circuit("REPEAT 20 { H 0 }", &options)
            .unwrap_err()
            .message
            .contains("max_instructions")
    );
    assert!(parse_circuit("REPEAT 4 { REPEAT 4 { H 0 } }", &options).is_err());
    assert_eq!(
        parse("REPEAT 999999999999999999 { REPEAT 999999999999999999 {} }")
            .operations()
            .len(),
        0
    );
    assert_eq!(
        parse("REPEAT 0 { REPEAT 999999999999999999 { H 0 } }")
            .operations()
            .len(),
        0
    );
    assert!(parse_circuit("M 70000", &ParseOptions::default()).is_err());
    assert_eq!(
        parse_circuit(
            "M 70000",
            &ParseOptions {
                max_qubits: 70001,
                ..Default::default()
            }
        )
        .unwrap()
        .num_qubits(),
        70001
    );
    assert!(
        parse_circuit(
            "H 0 1",
            &ParseOptions {
                max_operations: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        parse_circuit(
            "REPEAT 1 { H 0 }",
            &ParseOptions {
                max_repeat_depth: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        parse_circuit(
            "H 0",
            &ParseOptions {
                max_source_bytes: 2,
                ..Default::default()
            }
        )
        .is_err()
    );
    let error = parse_circuit(
        "M 0\nREPEAT 2 {\nCONTROL[NOT] rec[-1] -> ctrl[0]\nM 0\nCX ctrl[9] 0\n}",
        &ParseOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.location.line, 5);
    assert_eq!(error.location.repeat, vec![0]);
}
