// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//! Complete ten static CircuitFileTests methods adapted from
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_circuit_file.py.
//! Python instruction groups lower to individual Rust operations; resolved
//! BitExpr values replace Python SSA Target references. FileTask inspection and
//! its separate rmagna-provenance-v1 encoding witness serializability and stable
//! identity, without promising Python dataclass or fingerprint-byte identity.
//! Diagnostics preserve rejection reasons and locations, not Python wording.
use rmagna::circuit::{
    Basis, BitAtom, BitExpr, Circuit, Gate, NoiseKind, Operation, ParseOptions, parse_circuit,
};
use rmagna::file_api::{FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::probability::Probability;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{TaskOptions, TaskRunOptions, parse_task};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

fn parse(text: &str) -> Circuit {
    parse_circuit(text, &ParseOptions::default()).unwrap()
}
fn record(constant: bool) -> BitExpr {
    BitExpr {
        constant,
        terms: [BitAtom::Record(0)].into(),
    }
}
fn writes(circuit: &Circuit) -> Vec<&rmagna::circuit::RegisterWrite> {
    circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Control { outputs, .. } => Some(outputs),
            _ => None,
        })
        .flatten()
        .collect()
}
fn file_task(text: &str) -> FileTask {
    let circuit = parse(text);
    let modules = parse_task("{}", Path::new("."), &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &ParseOptions::default())
        .unwrap();
    FileTask::from_loaded(
        circuit,
        modules,
        &ControllerRegistry::default(),
        NoiseSampling::Physical,
        FileTaskOptions::default(),
        BTreeMap::new(),
    )
    .unwrap()
}

#[test]
fn xor_cancellation_constants_and_inversion() {
    let circuit = parse(
        "M 0\nCONTROL[XOR](1) rec[-1] rec[-1] -> ctrl[0]\nCONTROL[XOR] !ctrl[0] -> ctrl[1]\nCX ctrl[1] 0\n",
    );
    // Static affine route: no external decisions or history-dependent noise.
    assert!(!circuit.needs_history_sampling());
    assert_eq!(circuit.external_calls(), 0);
    assert_eq!(circuit.num_records(), 1);
    let out = writes(&circuit);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].expression, BitExpr::default().inverted());
    assert_eq!(out[1].expression, BitExpr::default());
    assert!(matches!(
        &circuit.operations().last().unwrap().operation,
        Operation::Unitary { gate: Gate::X, qubits, control: Some(expr) }
        if qubits == &[0] && expr == &BitExpr::default()
    ));
}

#[test]
fn measurement_record_counts_products_and_exact_probability() {
    let circuit = parse(
        "R 0 1 2\nMXX(0.01) 0 1\nMPP(1/30) !X0*X1 Z2\nMRX 0 1\nMY !2\nDETECTOR rec[-1] rec[-4]\nOBSERVABLE_INCLUDE(4) rec[-2]\n",
    );
    assert_eq!(circuit.num_records(), 6);
    let measured = circuit
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
        measured.iter().map(|x| x.3).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );
    assert_eq!(measured[0].2, &"1/100".parse::<Probability>().unwrap());
    assert_eq!(measured[1].2, &"1/30".parse::<Probability>().unwrap());
    assert_eq!(measured[2].2, measured[1].2);
    assert_eq!(measured[1].0.factors, [(0, Basis::X), (1, Basis::X)]);
    assert!(measured[1].0.inverted);
    assert_eq!(measured[2].0.factors, [(2, Basis::Z)]);
    assert!(measured[3].1 && measured[4].1);
    assert!(measured[5].0.inverted);
    assert_eq!(
        circuit.detectors(),
        &[BitExpr {
            constant: false,
            terms: [BitAtom::Record(5), BitAtom::Record(2)].into(),
        }]
    );
    assert_eq!(circuit.observables()[&4], BitExpr::atom(BitAtom::Record(4)));
}

#[test]
fn builtin_not_is_affine_versioned_and_requires_no_binding() {
    let text = "M 0\nDETECTOR rec[-1]\nCONTROL[NOT] det[0] -> ctrl[0]\nREPEAT 2 {\nCONTROL[NOT] ctrl[0] -> ctrl[0]\n}\nCONTROL[NOT] !rec[-1] -> ctrl[1]\n";
    let circuit = parse(text);
    assert_eq!(circuit.format_version(), "magna-0.1");
    assert!(!circuit.needs_history_sampling());
    assert_eq!(circuit.external_calls(), 0);
    let out = writes(&circuit);
    assert_eq!(
        out.iter().map(|w| w.version).collect::<Vec<_>>(),
        [0, 1, 2, 0]
    );
    assert_eq!(
        out.iter().map(|w| w.register).collect::<Vec<_>>(),
        [0, 0, 0, 1]
    );
    for (write, bit) in out.iter().zip([true, false, true, false]) {
        assert_eq!(write.expression, record(bit));
        assert!(write.expression.is_affine());
    }
    let first = file_task(text);
    let second = file_task(text);
    let run = TaskRunOptions::default();
    let fingerprints = first.fingerprints(&run).unwrap().to_json();
    assert_eq!(fingerprints["encoding"], "rmagna-provenance-v1");
    assert_eq!(fingerprints, second.fingerprints(&run).unwrap().to_json());
    let descriptor = first.inspect(&run).unwrap();
    assert_eq!(descriptor["circuit"]["format"], "magna-0.1");
    assert_eq!(descriptor["circuit"]["records"], 1);
    assert_eq!(descriptor["circuit"]["external_calls"], 0);
    assert_eq!(descriptor["circuit"]["history"], false);
    let encoded = serde_json::to_string(&descriptor).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&encoded).unwrap(), descriptor);
    let mut registry = ControllerRegistry::default();
    let error = registry
        .register("NOT", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap_err();
    assert!(
        matches!(error, rmagna::runtime::RuntimeError::InvalidBinding { ref name } if name == "NOT")
    );
    assert!(registry.names().is_empty());
}

#[test]
fn builtin_not_arity_and_future_inputs_are_rejected() {
    for text in [
        "CONTROL[NOT] -> ctrl[0]",
        "M 0\nCONTROL[NOT] rec[-1] rec[-1] -> ctrl[0]",
        "M 0\nCONTROL[NOT] rec[-1] -> ctrl[0] ctrl[1]",
        "M 0\nCONTROL[NOT](1) rec[-1] -> ctrl[0]",
        "CONTROL[NOT] ctrl[0] -> ctrl[0]",
    ] {
        parse_circuit(text, &ParseOptions::default()).expect_err(text);
    }
}

#[test]
fn correlated_chain_and_conditional_channels() {
    let circuit = parse(
        "M 0\nCONTROL[XOR] rec[-1] -> ctrl[0]\nE(0.1) ctrl[0] X0 Y1\nELSE_CORRELATED_ERROR(2/9) ctrl[0] Z1\nX_ERROR(0.001) 0\n",
    );
    let noise = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Noise {
                kind,
                probabilities,
                product,
                guard,
                ..
            } => Some((kind, probabilities, product, guard)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(noise.len(), 3);
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
    // A version-0 ctrl[0] guard resolves to the sole earlier reported record.
    assert_eq!(writes(&circuit)[0].version, 0);
    assert_eq!(noise[0].3, &Some(record(false)));
    assert_eq!(noise[1].3, noise[0].3);
    assert_eq!(
        noise[0].2.as_ref().unwrap().factors,
        [(0, Basis::X), (1, Basis::Y)]
    );
    assert!(!noise[0].2.as_ref().unwrap().inverted);
    assert!(noise[2].3.is_none());
    assert_eq!(noise[2].1[0], "1/1000".parse::<Probability>().unwrap());
    let first = &noise[0].1[0];
    let second = &noise[1].1[0];
    assert_eq!(first, &"1/10".parse::<Probability>().unwrap());
    assert_eq!(second, &"2/9".parse::<Probability>().unwrap());
    assert_eq!(
        first.value() + first.complement().value() * second.value(),
        "3/10".parse::<Probability>().unwrap().value().clone()
    );
}

#[test]
fn s_and_t_are_distinct_and_aliases_are_canonical() {
    let circuit = parse(
        "S 0\nT 0\nS_DAG 0\nT_DAG 0\nCNOT 0 1\nCCX 0 1 2\nCCZ 0 1 2\nCS_DAG 0 1\nCH 1 2\nMZ 0\n",
    );
    let names = circuit
        .operations()
        .iter()
        .map(|op| match &op.operation {
            Operation::Unitary { gate, .. } => gate.name(),
            Operation::Measure { .. } => "M",
            op => panic!("unexpected original operation: {op:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "S", "T", "S_DAG", "T_DAG", "CX", "CCX", "CCZ", "CS_DAG", "CH", "M"
        ]
    );
}

#[test]
fn stim_subset_matches_record_count() {
    // Captured with installed Stim, also checked against the unchanged native
    // method; no Stim installation or optional skip in this portable Rust test.
    let reference: Value =
        serde_json::from_str(include_str!("fixtures/original_static_stim.json")).unwrap();
    assert_eq!(reference["stim_version"], "1.15.0");
    let circuit = parse(reference["source"].as_str().unwrap());
    assert_eq!(reference["num_measurements"], 6);
    assert_eq!(reference["num_qubits"], 3);
    assert_eq!(
        circuit.num_records() as u64,
        reference["num_measurements"].as_u64().unwrap()
    );
    assert_eq!(
        circuit.num_qubits() as u64,
        reference["num_qubits"].as_u64().unwrap()
    );
}

#[test]
fn two_qubit_classical_controls_are_versioned_and_validate_targets() {
    let text =
        "M 0\nCONTROL[XOR] rec[-1] -> ctrl[0]\nCCZ rec[-1] 1 2 !ctrl[0] 2 3\nCCX ctrl[0] 3 1\n";
    let circuit = parse(text);
    assert_eq!(circuit.format_version(), "magna-0.1");
    assert_eq!(circuit.to_text(), text);
    assert_eq!(circuit.num_qubits(), 4);
    assert_eq!(parse("CCZ 0 1 2\nCCX 0 1 2").format_version(), "magna-0.1");
    assert_eq!(writes(&circuit)[0].version, 0);
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
    assert_eq!(
        gates,
        [
            (&Gate::Cz, &vec![1, 2], &Some(record(false))),
            (&Gate::Cz, &vec![2, 3], &Some(record(true))),
            (&Gate::Cx, &vec![3, 1], &Some(record(false))),
        ]
    );
    for name in ["CCZ", "CCX"] {
        for targets in [
            "rec[-1] 1 1",
            "rec[-1] rec[-1] 1",
            "0 rec[-1] 1",
            "rec[-1] !1 2",
        ] {
            let source = format!("M 0\n{name} {targets}");
            parse_circuit(&source, &ParseOptions::default()).expect_err(&source);
        }
    }
}

#[test]
fn source_located_rejections() {
    // Each original reason has an explicit independent Rust diagnostic mapping.
    for (text, reason) in [
        ("CX rec[-1] 0", "earlier measurement"),
        ("CONTROL[XOR] ctrl[0] -> ctrl[0]", "uninitialized"),
        ("CONTROL[XOR] -> ctrl[0] ctrl[1]", "exactly one"),
        ("CONTROL[missing] -> ctrl[0]", "missing controller"),
        ("X_ERROR(0.1)", "requires targets"),
        ("PAULI_CHANNEL_1(0.6,0.3,0.2) 0", "sum"),
        ("DEPOLARIZE2(0.1) 0", "groups of 2"),
        ("M(1.1) 0", "between zero and one"),
        ("S(0.1) 0", "does not take arguments"),
        ("CX 0 0", "repeats a qubit"),
        ("MPP X0*Y0", "repeats a qubit"),
        ("MPP 0", "Pauli targets"),
        ("X_ERROR(0.1) X0", "quantum target"),
        ("ELSE_CORRELATED_ERROR(0.1) X0", "immediately follow"),
        ("IF ctrl[0]", "unsupported instruction"),
        ("REPEAT 2 {\nH 0", "unclosed"),
        ("REPEAT 2\nH 0", "requires"),
        ("}", "unmatched"),
        ("T !0", "quantum target"),
    ] {
        let error = parse_circuit(
            text,
            &ParseOptions {
                source: Some("bad.magna".into()),
                ..Default::default()
            },
        )
        .expect_err(text);
        assert_eq!(error.location.source.as_deref(), Some("bad.magna"));
        assert!(error.location.line >= 1);
        assert!(error.location.column >= 1);
        assert!(
            error.to_string().starts_with("bad.magna:"),
            "{text}: {error}"
        );
        assert!(
            error.message.contains(reason),
            "{text}: expected {reason:?}, got {error}"
        );
    }
    let error = parse_circuit(
        "M 0\nE(0.1) rec[-1] X0\nELSE_CORRELATED_ERROR(0.1) X0",
        &ParseOptions::default(),
    )
    .unwrap_err();
    assert!(error.message.contains("same control guard"), "{error}");
}

#[test]
fn cz_allows_classical_endpoint_in_either_position() {
    let circuit = parse("M 0\nCZ 1 rec[-1]\n");
    assert_eq!(circuit.num_records(), 1);
    assert!(matches!(
        &circuit.operations().last().unwrap().operation,
        Operation::Unitary { gate: Gate::Z, qubits, control: Some(expr) }
        if qubits == &[1] && expr == &record(false)
    ));
}
