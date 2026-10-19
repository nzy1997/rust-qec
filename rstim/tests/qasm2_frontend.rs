#![cfg(feature = "qasm")]
use rstim::qasm::{
    self, Angle, ErrorCode, Gate, IncludeResolver, OperationKind, Pauli, QasmError, QasmLimits,
    RejectIncludes, ResolvedSource,
};

fn source(body: &str) -> String {
    format!("OPENQASM 2.0;\ninclude \"qelib1.inc\";\n{body}")
}
fn fail(body: &str, code: ErrorCode) {
    let e = qasm::from_qasm(&source(body)).unwrap_err();
    assert_eq!(e.code, code, "{e}");
    assert!(e.origin.is_some(), "missing source span: {e}");
}
#[test]
fn register_layout_and_in_place_operations_are_preserved() {
    let p=qasm::from_qasm(&source("qreg a[2]; qreg b[1]; creg c[2]; creg d[1]; h a; measure a -> c; reset b; barrier a,b; if(d==0) x a;" )).unwrap();
    assert_eq!(p.num_qubits(), 3);
    assert_eq!(p.num_classical_bits(), 3);
    assert_eq!(p.quantum_registers()[1].offset, 2);
    assert_eq!(p.operations().len(), 8);
    assert!(matches!(
        p.operations()[2].kind,
        OperationKind::Measure { qubit: 0, bit: 0 }
    ));
    assert!(matches!(
        p.operations()[4].kind,
        OperationKind::Reset { qubit: 2 }
    ));
    assert!(matches!(
        p.operations()[6].kind,
        OperationKind::Conditional {
            bit: 2,
            value: false,
            gate: Pauli::X,
            ..
        }
    ));
}
#[test]
fn user_gate_named_x_keeps_its_resolved_body() {
    let p = qasm::from_qasm("OPENQASM 2.0; gate x a { U(0,0,0) a; } qreg q[1]; x q;").unwrap();
    assert!(matches!(
        p.operations()[0].kind,
        OperationKind::Gate {
            gate: Gate::U { .. },
            ..
        }
    ));
    assert_eq!(
        p.operations()[0].origin.as_ref().unwrap().expansion.len(),
        1
    );
}
#[test]
fn parameter_binding_and_scientific_notation_keep_precision() {
    let p=qasm::from_qasm(&source("gate custom(a) q { rx(a/2) q; } qreg q[1]; custom(3*pi) q; rz(1.5707963267948966) q; rx(1.2345678901234567e-3) q;")).unwrap();
    let OperationKind::Gate {
        gate: Gate::Rx(a), ..
    } = &p.operations()[0].kind
    else {
        panic!()
    };
    assert_eq!(a.clifford_quarters(), Some(3));
    let OperationKind::Gate {
        gate: Gate::Rz(a), ..
    } = &p.operations()[1].kind
    else {
        panic!()
    };
    assert_eq!(a.clifford_quarters(), None);
    let OperationKind::Gate {
        gate: Gate::Rx(a), ..
    } = &p.operations()[2].kind
    else {
        panic!()
    };
    assert_eq!(a.value(), 1.2345678901234567e-3);
}
#[test]
fn expressions_have_mathematical_precedence_and_checked_domains() {
    let p = qasm::from_qasm(&source(
        "qreg q[1]; rx(-2^2) q; ry(2^3^2) q; rz(sin(pi/2)+sqrt(4)+ln(exp(1))) q;",
    ))
    .unwrap();
    let values: Vec<f64> = p
        .operations()
        .iter()
        .map(|o| match &o.kind {
            OperationKind::Gate {
                gate: Gate::Rx(a) | Gate::Ry(a) | Gate::Rz(a),
                ..
            } => a.value(),
            _ => panic!(),
        })
        .collect();
    assert_eq!(values[0], -4.0);
    assert_eq!(values[1], 512.0);
    assert!((values[2] - 4.0).abs() < 1e-14);
    for expression in [
        "1/0",
        "sqrt(-1)",
        "ln(0)",
        "exp(1000)",
        "tan(pi/2)",
        "1e309",
    ] {
        fail(&format!("qreg q[1]; rx({expression}) q;"), ErrorCode::Type);
    }
}
#[test]
fn malformed_or_unsupported_programs_fail_explicitly() {
    for body in [
        "qreg q[0];",
        "qreg q[1]; qreg q[1];",
        "qreg q[1]; x q[1];",
        "qreg q[1]; cx q,q;",
        "qreg a[2]; qreg b[3]; cx a,b;",
        "qreg q[1]; creg c[2]; measure q -> c;",
        "creg c[1]; x c;",
        "qreg q[1]; missing q;",
        "qreg q[1]; rx() q;",
        "gate a(t) q { U(missing,0,0) q; }",
        "gate a q { absent q; }",
        "gate a q { CX q,q; }",
        "gate a q { barrier nope; }",
        "gate a(t,t) q { U(t,0,0) q; }",
        "gate a(t) q { U(t/(1-1),0,0) q; }",
        "gate a q { U(1/0,0,0) q; }",
        "gate a q { a q; }",
        "gate a q { b q; } gate b q { U(0,0,0) q; }",
        "x q; qreg q[1];",
        "qreg q[1]; creg c[1]; if(c==2) x q;",
    ] {
        fail(body, ErrorCode::Type);
    }
    for body in [
        "qreg q[1]; creg c[2]; if(c==1) x q;",
        "qreg q[1]; creg c[1]; if(c==1) h q;",
        "qreg q[1]; creg c[1]; if(c==0) reset q;",
        "opaque blackbox q; qreg q[1]; blackbox q;",
    ] {
        fail(body, ErrorCode::Unsupported);
    }
    for text in [
        "qreg q[1];",
        "OPENQASM 3.0;",
        "OPENQASM 2.0; qreg Q[1];",
        "OPENQASM 2.0; qreg q[1]; U(+1,0,0) q;",
        "OPENQASM 2.0; qreg q[1]; U(1e+,0,0) q;",
        "OPENQASM 2.0; é",
        "OPENQASM 2.0; λ",
    ] {
        assert_eq!(
            qasm::from_qasm(text).unwrap_err().code,
            ErrorCode::Parse,
            "{text}"
        );
    }
    qasm::from_qasm("OPENQASM 2.0; opaque unused(theta) q;").unwrap();
}
#[derive(Default)]
struct Includes;
impl IncludeResolver for Includes {
    fn resolve(&self, n: &str, _: &str) -> Result<ResolvedSource, QasmError> {
        let text = match n {
            "good.inc" => "gate custom(theta) q { U(theta,0,0) q; }",
            "bad.inc" => "gate custom q { missing q; }",
            "cycle.inc" => "include \"cycle.inc\";",
            _ => return Err(QasmError::new(ErrorCode::Io, "missing include")),
        };
        Ok(ResolvedSource {
            name: n.to_owned(),
            text: text.to_owned(),
        })
    }
}
#[test]
fn includes_resolve_explicitly_keep_locations_and_detect_cycles() {
    let limits = QasmLimits::default();
    let p = qasm::parse_and_check(
        "OPENQASM 2.0; include \"good.inc\"; qreg q[1]; custom(pi/2) q;",
        "root.qasm",
        &Includes,
        &limits,
    )
    .unwrap();
    let span = p.operations()[0].origin.as_ref().unwrap();
    assert_eq!(span.source, "good.inc");
    assert!(!span.expansion.is_empty());
    let e = qasm::parse_and_check(
        "OPENQASM 2.0; include \"bad.inc\";",
        "root.qasm",
        &Includes,
        &limits,
    )
    .unwrap_err();
    assert_eq!(e.origin.unwrap().source, "bad.inc");
    assert_eq!(
        qasm::parse_and_check(
            "OPENQASM 2.0; include \"cycle.inc\";",
            "root.qasm",
            &Includes,
            &limits
        )
        .unwrap_err()
        .code,
        ErrorCode::Type
    );
    assert_eq!(
        qasm::parse_and_check(
            "OPENQASM 2.0; include \"absent.inc\";",
            "root.qasm",
            &RejectIncludes,
            &limits
        )
        .unwrap_err()
        .code,
        ErrorCode::Io
    );
}
#[test]
fn parser_budgets_bound_source_tokens_depth_and_expansion() {
    let body = source("qreg q[2]; h q;");
    for limits in [
        QasmLimits {
            max_source_bytes: body.len() - 1,
            ..Default::default()
        },
        QasmLimits {
            max_tokens: 10,
            ..Default::default()
        },
        QasmLimits {
            max_include_files: 0,
            ..Default::default()
        },
        QasmLimits {
            max_include_depth: 0,
            ..Default::default()
        },
        QasmLimits {
            max_operations: 1,
            ..Default::default()
        },
        QasmLimits {
            max_qubits: 1,
            ..Default::default()
        },
    ] {
        assert_eq!(
            qasm::parse_and_check(&body, "test", &RejectIncludes, &limits)
                .unwrap_err()
                .code,
            ErrorCode::Limit
        );
    }
    let limits = QasmLimits {
        max_expr_depth: 8,
        ..Default::default()
    };
    let chain = std::iter::repeat_n("1", 10000)
        .collect::<Vec<_>>()
        .join("+");
    let e = qasm::parse_and_check(
        &format!("OPENQASM 2.0; qreg q[1]; U({chain},0,0) q;"),
        "test",
        &RejectIncludes,
        &limits,
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::Limit);
    let e = qasm::parse_and_check(
        "OPENQASM 2.0; qreg q[1]; U((((((((((1)))))))))),0,0) q;",
        "test",
        &RejectIncludes,
        &limits,
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::Limit);
    let limits = QasmLimits {
        max_classical_bits: 1,
        ..Default::default()
    };
    assert_eq!(
        qasm::parse_and_check("OPENQASM 2.0; creg c[2];", "test", &RejectIncludes, &limits)
            .unwrap_err()
            .code,
        ErrorCode::Limit
    );
}
#[test]
fn exact_angles_validate_serialize_and_never_snap_floats() {
    assert_eq!(Angle::pi_ratio(-3, 2).unwrap().clifford_quarters(), Some(1));
    assert_eq!(
        Angle::radians(std::f64::consts::FRAC_PI_2)
            .unwrap()
            .clifford_quarters(),
        None
    );
    assert!(Angle::pi_ratio(1, 0).is_err());
    assert!(Angle::radians(f64::INFINITY).is_err());
    let a = Angle::pi_ratio(7, 3).unwrap();
    let encoded = serde_json::to_string(&a).unwrap();
    let b: Angle = serde_json::from_str(&encoded).unwrap();
    assert_eq!(a, b);
    b.validate().unwrap();
}
#[test]
fn crlf_comments_and_include_strings_are_lexed_without_rewriting() {
    let p = qasm::from_qasm("OPENQASM 2.0;\r\n// café λ\r\nqreg q[1];\r\nU(0,0,0) q;").unwrap();
    let span = p.operations()[0].origin.as_ref().unwrap();
    assert_eq!(span.line, 4);
    assert_eq!(span.column, 1);
}

#[test]
fn cumulative_include_tokens_and_empty_gate_expansion_have_budgets() {
    struct SmallIncludes;
    impl IncludeResolver for SmallIncludes {
        fn resolve(&self, name: &str, _: &str) -> Result<ResolvedSource, QasmError> {
            Ok(ResolvedSource {
                name: name.to_owned(),
                text: "U(0,0,0) q;".to_owned(),
            })
        }
    }
    let limits = QasmLimits {
        max_tokens: 30,
        ..Default::default()
    };
    let e = qasm::parse_and_check(
        "OPENQASM 2.0; qreg q[1]; include \"a.inc\"; include \"b.inc\";",
        "root",
        &SmallIncludes,
        &limits,
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::Limit);
    let limits = QasmLimits {
        max_operations: 1,
        max_gate_depth: 8,
        ..Default::default()
    };
    let e=qasm::parse_and_check("OPENQASM 2.0; gate empty q {} gate duplicate q {empty q; empty q;} gate four q {duplicate q; duplicate q;} gate eight q {four q; four q;} qreg q[1]; eight q;", "root", &RejectIncludes, &limits).unwrap_err();
    assert_eq!(e.code, ErrorCode::Limit);
}

#[test]
fn primitive_u_feedback_is_classified_by_exact_semantics() {
    let program=qasm::from_qasm("OPENQASM 2.0; qreg h[1]; creg c[1]; if(c==1) U(pi,0,pi) h; if(c==0) U(pi,pi/2,pi/2) h; if(c==1) U(0,0,pi) h; if(c==1) U(-pi,2*pi,pi) h;").unwrap();
    let expected = [Pauli::X, Pauli::Y, Pauli::Z, Pauli::X];
    for (operation, expected) in program.operations().iter().zip(expected) {
        let OperationKind::Conditional { gate, .. } = operation.kind else {
            panic!("expected conditional Pauli")
        };
        assert_eq!(gate, expected);
    }
    for parameters in [
        "pi/2,0,pi",
        "pi,0,3.141592653589793",
        "0,0,3.141592653589793",
        "pi,0,pi+1e-16",
        "pi,pi/2,pi/2+1e-16",
    ] {
        let text = format!("OPENQASM 2.0; qreg h[1]; creg c[1]; if(c==1) U({parameters}) h;");
        assert_eq!(
            qasm::from_qasm(&text).unwrap_err().code,
            ErrorCode::Unsupported
        );
    }
}
