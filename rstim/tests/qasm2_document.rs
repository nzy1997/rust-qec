#![cfg(feature = "qasm")]

use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::CompiledNearCliffordExecutor;
use rstim::qasm::{
    Angle, Backend, ErrorCode, Gate, OperationKind, QasmCircuit, QasmLimits, from_qasm,
};
use serde_json::{Value, json};

fn fixture() -> QasmCircuit {
    from_qasm(
        r#"OPENQASM 2.0;
include "qelib1.inc";
qreg q[3]; qreg idle[2];
creg unwritten[2]; creg c[1]; creg out[2];
gate twist(a) t { rz(a) t; }
h q[0]; measure q[0] -> c[0];
if(c==0) y q[1]; measure q[1] -> out[1];
reset q[0]; measure q[0] -> c[0];
if(c==0) x q[2]; measure q[2] -> out[0];
twist(pi/7) q[1]; rx(0.12345678901234567) q[1];
barrier q[0],q[2];
"#,
    )
    .unwrap()
}
fn simple() -> Value {
    serde_json::from_str(
        &from_qasm("OPENQASM 2.0; qreg q[2]; creg c[1]; U(pi/7,0,0) q[0];")
            .unwrap()
            .to_json(&QasmLimits::default())
            .unwrap(),
    )
    .unwrap()
}
fn load(value: &Value) -> Result<QasmCircuit, rstim::qasm::QasmError> {
    QasmCircuit::from_json(&value.to_string(), &QasmLimits::default())
}
fn invalid(value: Value) {
    assert_eq!(
        load(&value).unwrap_err().code,
        ErrorCode::InvalidDocument,
        "{value}"
    );
}

#[test]
fn semantic_document_roundtrip_preserves_layout_exact_angles_origins_and_execution() {
    let limits = QasmLimits::default();
    let original = fixture();
    let text = original.to_json(&limits).unwrap();
    let wire: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(wire["schema"], "rstim.qasm2-program.v1");
    assert_eq!(wire.as_object().unwrap().len(), 4);
    for derived in [
        "instructions",
        "measurement_events",
        "classical_outputs",
        "backend",
        "target",
    ] {
        assert!(wire.get(derived).is_none());
    }
    let loaded = QasmCircuit::from_json(&text, &limits).unwrap();
    assert_eq!(original, loaded);
    assert_eq!(text, loaded.to_json(&limits).unwrap());
    assert!(
        loaded
            .operations()
            .iter()
            .any(|op| op.origin.as_ref().is_some_and(|o| !o.expansion.is_empty()))
    );
    assert!(loaded.operations().iter().any(|op| matches!(
        &op.kind,
        OperationKind::Gate {
            gate: Gate::Rz(Angle::Rational {
                numerator: 1,
                denominator: 7,
                pi: true
            }),
            ..
        }
    )));
    let before = original.lower(Backend::NearClifford, &limits).unwrap();
    let after = loaded.lower(Backend::NearClifford, &limits).unwrap();
    assert_eq!(before.num_qubits, 5);
    assert_eq!(before.instructions, after.instructions);
    assert_eq!(before.instruction_origins, after.instruction_origins);
    assert_eq!(before.measurement_events, after.measurement_events);
    assert_eq!(before.classical_outputs, after.classical_outputs);
    for mask in 0..16 {
        let raw: Vec<_> = (0..4).map(|i| mask & (1 << i) != 0).collect();
        let expected = vec![false, false, raw[2], raw[3], raw[1]];
        assert_eq!(before.project_classical(&raw).unwrap(), expected);
        assert_eq!(after.project_classical(&raw).unwrap(), expected);
    }
    let executor =
        CompiledNearCliffordExecutor::compile_with_limit(after.instructions.clone(), 16).unwrap();
    for shot in executor
        .sample(32, &mut StdRng::seed_from_u64(333))
        .unwrap()
    {
        let output = after.project_classical(&shot.measurements).unwrap();
        assert_eq!(&output[..3], &[false, false, false]);
        assert!(output[3]);
        assert_eq!(output[4], shot.measurements[1]);
    }
}

#[test]
fn empty_and_clifford_documents_remain_usable_without_backend_state() {
    for source in [
        "OPENQASM 2.0;",
        "OPENQASM 2.0; qreg q[1]; creg c[2]; measure q[0] -> c[1];",
    ] {
        let circuit = from_qasm(source).unwrap();
        let limits = QasmLimits::default();
        let loaded = QasmCircuit::from_json(&circuit.to_json(&limits).unwrap(), &limits).unwrap();
        assert_eq!(loaded, circuit);
        loaded.lower(Backend::Stabilizer, &limits).unwrap();
    }
}

#[test]
fn schema_versions_and_malformed_json_are_rejected() {
    for text in [
        "",
        "null",
        "[]",
        "{}",
        "{",
        "{} trailing",
        "{\"schema\":\"rstim.qasm2-program.v1\",\"schema\":\"rstim.qasm2-program.v1\"}",
    ] {
        assert_eq!(
            QasmCircuit::from_json(text, &QasmLimits::default())
                .unwrap_err()
                .code,
            ErrorCode::InvalidDocument
        );
    }
    for schema in ["rstim.qasm2-program.v2", "rstim.qasm2-program.v0", "other"] {
        let mut v = simple();
        v["schema"] = json!(schema);
        invalid(v);
    }
}

#[test]
fn unknown_fields_are_rejected_at_every_schema_envelope() {
    for path in [
        vec![],
        vec!["quantum_registers", "0"],
        vec!["classical_registers", "0"],
        vec!["operations", "0"],
        vec!["operations", "0", "kind"],
        vec!["operations", "0", "kind", "gate"],
        vec!["operations", "0", "kind", "gate", "parameters"],
        vec!["operations", "0", "kind", "gate", "parameters", "theta"],
        vec!["operations", "0", "origin"],
    ] {
        let mut v = simple();
        let mut target = &mut v;
        for item in path {
            target = if let Ok(i) = item.parse::<usize>() {
                &mut target[i]
            } else {
                &mut target[item]
            };
        }
        target["unexpected"] = json!(true);
        invalid(v);
    }
    let mut v = simple();
    v["operations"][0]["kind"]["gate"] = json!({"kind":"x","unexpected":true});
    invalid(v);
    let mut v = simple();
    v["operations"][0]["kind"]["gate"] = json!({"kind":"x","parameters":{"bogus":true}});
    invalid(v);
}

#[test]
fn invalid_register_layouts_cannot_bypass_frontend_checks() {
    for (field, value) in [
        ("size", json!(0)),
        ("size", json!(u64::MAX)),
        ("offset", json!(1)),
        ("offset", json!(u64::MAX)),
        ("name", json!("Q")),
        ("name", json!("if")),
    ] {
        let mut v = simple();
        v["quantum_registers"][0][field] = value;
        let error = load(&v).unwrap_err();
        assert!(matches!(
            error.code,
            ErrorCode::InvalidDocument | ErrorCode::Limit
        ));
    }
    let mut v = simple();
    v["classical_registers"][0]["name"] = json!("q");
    invalid(v);
    let mut v = simple();
    v["quantum_registers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"other","size":1,"offset":99}));
    invalid(v);
}

#[test]
fn wrong_arity_aliases_and_out_of_range_targets_are_rejected() {
    for targets in [json!([]), json!([0, 1]), json!([2]), json!([u32::MAX])] {
        let mut v = simple();
        v["operations"][0]["kind"]["targets"] = targets;
        invalid(v);
    }
    for kind in [
        json!({"kind":"gate","gate":{"kind":"c_x"},"targets":[0,0]}),
        json!({"kind":"gate","gate":{"kind":"h"},"targets":[0,1]}),
        json!({"kind":"measure","qubit":0,"bit":1}),
        json!({"kind":"measure","qubit":2,"bit":0}),
        json!({"kind":"reset","qubit":2}),
        json!({"kind":"barrier","qubits":[2]}),
        json!({"kind":"barrier","qubits":[0,0]}),
        json!({"kind":"conditional","bit":0,"value":true,"gate":"X","targets":[]}),
        json!({"kind":"conditional","bit":0,"value":true,"gate":"X","targets":[0,0]}),
        json!({"kind":"conditional","bit":1,"value":true,"gate":"X","targets":[0]}),
    ] {
        let mut v = simple();
        v["operations"][0]["kind"] = kind;
        invalid(v);
    }
    let mut v = simple();
    v["classical_registers"][0]["size"] = json!(2);
    v["operations"][0]["kind"] =
        json!({"kind":"conditional","bit":0,"value":true,"gate":"X","targets":[0]});
    invalid(v);
}

#[test]
fn invalid_numeric_angles_are_rejected_without_panics_or_snapping() {
    for angle in [
        json!({"kind":"rational","numerator":1,"denominator":0,"pi":true}),
        json!({"kind":"rational","numerator":0,"denominator":0,"pi":false}),
        json!({"kind":"radians","value":null}),
        json!({"kind":"radians","value":"NaN"}),
        json!({"kind":"radians","value":"Infinity"}),
    ] {
        let mut v = simple();
        v["operations"][0]["kind"]["gate"]["parameters"]["theta"] = angle;
        invalid(v);
    }
    let mut v = simple();
    v["operations"][0]["kind"]["gate"]["parameters"]["theta"] =
        json!({"kind":"radians","value":std::f64::consts::FRAC_PI_2});
    let circuit = load(&v).unwrap();
    assert_eq!(
        circuit
            .lower(Backend::Stabilizer, &QasmLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::BackendUnsupported
    );
    let text = v.to_string().replace("1.5707963267948966", "1e999");
    assert_eq!(
        QasmCircuit::from_json(&text, &QasmLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidDocument
    );
}

#[test]
fn optional_provenance_is_checked_and_can_be_absent() {
    for origin in [
        json!({"source":"","line":1,"column":1,"offset":0,"length":1,"expansion":[]}),
        json!({"source":"input","line":0,"column":1,"offset":0,"length":1,"expansion":[]}),
        json!({"source":"input","line":1,"column":0,"offset":0,"length":1,"expansion":[]}),
        json!({"source":"input","line":1,"column":1,"offset":u64::MAX,"length":1,"expansion":[]}),
        json!({"source":"input","line":1,"column":1,"offset":0,"length":1,"expansion":[""]}),
    ] {
        let mut v = simple();
        v["operations"][0]["origin"] = origin;
        invalid(v);
    }
    for omit in [true, false] {
        let mut v = simple();
        if omit {
            v["operations"][0].as_object_mut().unwrap().remove("origin");
        } else {
            v["operations"][0]["origin"] = Value::Null;
        }
        assert!(load(&v).unwrap().operations()[0].origin.is_none());
    }
}

#[test]
fn byte_operation_register_and_provenance_budgets_are_enforced() {
    let circuit = fixture();
    let text = circuit.to_json(&QasmLimits::default()).unwrap();
    let exact = QasmLimits {
        max_output_bytes: text.len(),
        ..QasmLimits::default()
    };
    assert_eq!(circuit.to_json(&exact).unwrap().len(), text.len());
    assert_eq!(QasmCircuit::from_json(&text, &exact).unwrap(), circuit);
    for bytes in [0, 1, text.len() - 1] {
        let limits = QasmLimits {
            max_output_bytes: bytes,
            ..QasmLimits::default()
        };
        assert_eq!(circuit.to_json(&limits).unwrap_err().code, ErrorCode::Limit);
        assert_eq!(
            QasmCircuit::from_json(&text, &limits).unwrap_err().code,
            ErrorCode::Limit
        );
    }
    for limits in [
        QasmLimits {
            max_operations: 0,
            ..QasmLimits::default()
        },
        QasmLimits {
            max_qubits: 1,
            ..QasmLimits::default()
        },
        QasmLimits {
            max_classical_bits: 1,
            ..QasmLimits::default()
        },
        QasmLimits {
            max_gate_depth: 0,
            max_include_depth: 0,
            ..QasmLimits::default()
        },
    ] {
        assert_eq!(circuit.to_json(&limits).unwrap_err().code, ErrorCode::Limit);
        assert_eq!(
            QasmCircuit::from_json(&text, &limits).unwrap_err().code,
            ErrorCode::Limit
        );
    }
    // Original QASM byte budgets do not constrain a semantic document's bytes.
    let limits = QasmLimits {
        max_source_bytes: 0,
        ..QasmLimits::default()
    };
    assert_eq!(QasmCircuit::from_json(&text, &limits).unwrap(), circuit);
}

#[test]
fn duplicate_fields_and_deep_unknown_values_fail_boundedly() {
    let text = simple().to_string();
    let duplicate = text.replacen(
        "\"schema\":",
        "\"schema\":\"rstim.qasm2-program.v1\",\"schema\":",
        1,
    );
    assert_eq!(
        QasmCircuit::from_json(&duplicate, &QasmLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidDocument
    );
    let deep = format!("{{\"unknown\":{}0{}}}", "[".repeat(1000), "]".repeat(1000));
    assert_eq!(
        QasmCircuit::from_json(&deep, &QasmLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidDocument
    );
}

#[test]
fn every_gate_variant_roundtrips_through_the_strict_wire_adapter() {
    let angle = Angle::pi_ratio(1, 7).unwrap();
    for gate in [
        Gate::I,
        Gate::H,
        Gate::X,
        Gate::Y,
        Gate::Z,
        Gate::S,
        Gate::SDag,
        Gate::SqrtX,
        Gate::SqrtXDag,
        Gate::SqrtY,
        Gate::SqrtYDag,
        Gate::T,
        Gate::TDag,
        Gate::CX,
        Gate::CY,
        Gate::CZ,
        Gate::SWAP,
        Gate::Rx(angle.clone()),
        Gate::Ry(angle.clone()),
        Gate::Rz(angle.clone()),
        Gate::U {
            theta: angle.clone(),
            phi: Angle::radians(-0.37).unwrap(),
            lambda: Angle::pi_ratio(-2, 3).unwrap(),
        },
    ] {
        let arity = if matches!(gate, Gate::CX | Gate::CY | Gate::CZ | Gate::SWAP) {
            2
        } else {
            1
        };
        let mut value = simple();
        value["operations"][0]["kind"]["gate"] = serde_json::to_value(&gate).unwrap();
        value["operations"][0]["kind"]["targets"] = json!((0..arity).collect::<Vec<_>>());
        let circuit = load(&value).unwrap();
        assert!(
            matches!(&circuit.operations()[0].kind,OperationKind::Gate {gate:loaded,..} if *loaded==gate)
        );
        let text = circuit.to_json(&QasmLimits::default()).unwrap();
        assert_eq!(
            circuit,
            QasmCircuit::from_json(&text, &QasmLimits::default()).unwrap()
        );
    }
}

#[test]
fn evaluated_fp64_angles_preserve_all_bits_across_document_roundtrips() {
    let limits = QasmLimits::default();
    // Values returned by transcendental functions are FP64, not symbolic ratios.
    // Deterministic inputs exercise significands and several exponent scales.
    for i in 1..257 {
        let source = format!(
            "OPENQASM 2.0; include \"qelib1.inc\"; qreg q[1]; rx(sin({}/37)) q[0]; ry(exp(-{}/13)) q[0];",
            i, i
        );
        let circuit = from_qasm(&source).unwrap();
        let loaded = QasmCircuit::from_json(&circuit.to_json(&limits).unwrap(), &limits).unwrap();
        for (before, after) in circuit.operations().iter().zip(loaded.operations()) {
            let angle = |op: &rstim::qasm::Operation| match &op.kind {
                OperationKind::Gate {
                    gate: Gate::Rx(a) | Gate::Ry(a),
                    ..
                } => a.value().to_bits(),
                _ => panic!("expected a rotation"),
            };
            assert_eq!(
                angle(before),
                angle(after),
                "FP64 angle changed for source {source}"
            );
        }
    }
}

#[test]
fn document_export_preserves_registers_overwrites_and_feedback() {
    let limits = QasmLimits::default();
    let original = fixture();
    let loaded = QasmCircuit::from_json(&original.to_json(&limits).unwrap(), &limits).unwrap();
    let options = rstim::qasm::ExportOptions::default();
    let before = rstim::qasm::to_qasm(&original, &options, &limits).unwrap();
    let after = rstim::qasm::to_qasm(&loaded, &options, &limits).unwrap();
    assert_eq!(before.text, after.text);
    let reparsed = from_qasm(&after.text).unwrap();
    assert_eq!(reparsed.quantum_registers(), original.quantum_registers());
    assert_eq!(
        reparsed.classical_registers(),
        original.classical_registers()
    );
    let a = original.lower(Backend::NearClifford, &limits).unwrap();
    let b = reparsed.lower(Backend::NearClifford, &limits).unwrap();
    assert_eq!(a.measurement_events, b.measurement_events);
    assert_eq!(a.classical_outputs, b.classical_outputs);
    for mask in 0..16 {
        let raw: Vec<_> = (0..4).map(|i| mask & (1 << i) != 0).collect();
        assert_eq!(
            a.project_classical(&raw).unwrap(),
            b.project_classical(&raw).unwrap()
        );
    }
}

#[test]
fn combined_include_and_gate_provenance_uses_both_configured_depth_budgets() {
    struct Resolver;
    impl rstim::qasm::IncludeResolver for Resolver {
        fn resolve(
            &self,
            name: &str,
            _from: &str,
        ) -> Result<rstim::qasm::ResolvedSource, rstim::qasm::QasmError> {
            let text = match name {
                "a.inc" => "include \"b.inc\";",
                "b.inc" => "gate twist a { U(pi/7,0,0) a; }",
                _ => {
                    return Err(rstim::qasm::QasmError::new(
                        ErrorCode::Io,
                        "unknown fixture include",
                    ));
                }
            };
            Ok(rstim::qasm::ResolvedSource {
                name: name.to_owned(),
                text: text.to_owned(),
            })
        }
    }
    let limits = QasmLimits {
        max_include_depth: 2,
        max_gate_depth: 2,
        ..QasmLimits::default()
    };
    let circuit = rstim::qasm::parse_and_check(
        "OPENQASM 2.0; include \"a.inc\"; qreg q[1]; twist q[0];",
        "root.qasm",
        &Resolver,
        &limits,
    )
    .unwrap();
    assert_eq!(
        circuit.operations()[0]
            .origin
            .as_ref()
            .unwrap()
            .expansion
            .len(),
        3
    );
    let text = circuit.to_json(&limits).unwrap();
    assert_eq!(QasmCircuit::from_json(&text, &limits).unwrap(), circuit);
    let tight = QasmLimits {
        max_include_depth: 1,
        max_gate_depth: 1,
        ..QasmLimits::default()
    };
    assert_eq!(
        QasmCircuit::from_json(&text, &tight).unwrap_err().code,
        ErrorCode::Limit
    );
}
