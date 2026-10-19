#![cfg(feature = "qasm")]

use rstim::qasm::{
    Angle, Backend, ClassicalBinding, ErrorCode, FileResolver, Gate, IncludeResolver,
    LoweredQasmCircuit, OperationKind, QasmError, QasmLimits, RejectIncludes, SourceSpan,
    from_qasm, from_qasm_file, parse_and_check,
};
use std::f64::consts::PI;
use std::fs;

fn expression(text: &str) -> Result<Angle, QasmError> {
    let program = from_qasm(&format!("OPENQASM 2.0; qreg q[1]; U({text},0,0) q;"))?;
    let OperationKind::Gate {
        gate: Gate::U { theta, .. },
        ..
    } = &program.operations()[0].kind
    else {
        panic!("primitive U")
    };
    Ok(theta.clone())
}

#[test]
fn angle_text_preserves_exactness_units_and_finite_decimal_parameters() {
    for angle in [
        Angle::pi_ratio(-6, 4).unwrap(),
        Angle::pi_ratio(3, 1).unwrap(),
        Angle::pi_ratio(0, 7).unwrap(),
        Angle::Rational {
            numerator: 2,
            denominator: 3,
            pi: false,
        },
        Angle::Rational {
            numerator: -2,
            denominator: 1,
            pi: false,
        },
        Angle::radians(0.12345678901234566).unwrap(),
    ] {
        angle.validate().unwrap();
        let restored = expression(&angle.expression()).unwrap();
        assert!((restored.value() - angle.value()).abs() < 1e-15);
        assert_eq!(restored.clifford_quarters(), angle.clifford_quarters());
        assert!((angle.half_turns() * PI - angle.value()).abs() < 1e-15);
    }
    assert_eq!(Angle::pi_ratio(3, 2).unwrap().half_turns(), 1.5);
    assert!(
        Angle::Rational {
            numerator: 1,
            denominator: 0,
            pi: true
        }
        .validate()
        .is_err()
    );
    assert!(Angle::Radians { value: f64::NAN }.validate().is_err());
}

#[test]
fn expression_algebra_preserves_exact_pi_and_bounds_numeric_domains() {
    for (text, want) in [
        ("0+pi/2", PI / 2.),
        ("pi/2+0", PI / 2.),
        ("pi/pi", 1.),
        ("pi/(-2)", -PI / 2.),
        ("2^-3", 0.125),
        ("2^0", 1.),
        ("pi^1", PI),
        ("cos(0)", 1.),
        ("tan(0)", 0.),
        ("-1e-20", -1e-20),
        ("1e-20-2e-20", -1e-20),
        ("1e-20*2", 2e-20),
        ("1e-20/2", 0.5e-20),
        ("1e-20^2", 1e-40),
        ("pi*pi", PI * PI),
        (
            "123456789012345678901234567890123456789e-40",
            0.012345678901234568,
        ),
        ("1.25e2", 125.),
        ("0.125e-2", 0.00125),
    ] {
        let actual = expression(text).unwrap();
        assert!(
            (actual.value() - want).abs() <= 1e-14 * want.abs().max(1e-40),
            "{text}: {} != {want}",
            actual.value()
        );
    }
    assert_eq!(
        expression("pi/2+pi/2").unwrap().clifford_quarters(),
        Some(2)
    );
    for text in [
        "9223372036854775807+1",
        "2^32^2",
        "9223372036854775807^3",
        "(1/9223372036854775807)^32",
        "1/1e-400",
        "1e2147483648",
    ] {
        let error = expression(text).unwrap_err();
        assert!(
            matches!(error.code, ErrorCode::Type | ErrorCode::Limit),
            "{text}: {error}"
        );
    }
}

#[test]
fn file_import_uses_relative_then_explicit_search_paths_and_bundled_header() {
    let root = tempfile::tempdir().unwrap();
    let local = root.path().join("local");
    let includes = root.path().join("includes");
    fs::create_dir(&local).unwrap();
    fs::create_dir(&includes).unwrap();
    fs::write(local.join("custom.inc"), "gate custom a { U(0,0,0) a; }").unwrap();
    fs::write(
        includes.join("custom.inc"),
        "gate custom a { U(pi,0,pi) a; }",
    )
    .unwrap();
    fs::write(includes.join("other.inc"), "gate other a { U(pi,0,pi) a; }").unwrap();
    fs::write(local.join("qelib1.inc"), "this is deliberately invalid").unwrap();
    let path = local.join("input.qasm");
    fs::write(&path,"OPENQASM 2.0; include \"qelib1.inc\"; include \"custom.inc\"; include \"other.inc\"; qreg q[1]; custom q; other q; h q;").unwrap();
    let resolver = FileResolver {
        include_dirs: vec![includes.clone()],
        ..Default::default()
    };
    let program = from_qasm_file(&path, &resolver, &QasmLimits::default()).unwrap();
    assert_eq!(program.operations().len(), 3);
    let OperationKind::Gate {
        gate: Gate::U { theta, .. },
        ..
    } = &program.operations()[0].kind
    else {
        panic!()
    };
    assert_eq!(theta.value(), 0.);
    let resolved = resolver.resolve("other.inc", "").unwrap();
    assert_eq!(
        resolved.name,
        includes
            .join("other.inc")
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
    );
    assert!(resolved.text.contains("gate other"));
    assert_eq!(
        resolver
            .resolve("missing.inc", &path.display().to_string())
            .unwrap_err()
            .code,
        ErrorCode::Io
    );
}

#[test]
fn file_import_reports_missing_invalid_utf8_and_bounded_sources() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("input.qasm");
    let resolver = FileResolver::default();
    let limits = QasmLimits::default();
    assert_eq!(
        from_qasm_file(&path, &resolver, &limits).unwrap_err().code,
        ErrorCode::Io
    );
    fs::write(&path, [0xff, 0xfe]).unwrap();
    assert_eq!(
        from_qasm_file(&path, &resolver, &limits).unwrap_err().code,
        ErrorCode::Io
    );
    assert_eq!(
        from_qasm_file(root.path(), &resolver, &limits)
            .unwrap_err()
            .code,
        ErrorCode::Io
    );
    fs::write(&path, "OPENQASM 2.0;").unwrap();
    let small = QasmLimits {
        max_source_bytes: 4,
        ..Default::default()
    };
    assert_eq!(
        from_qasm_file(&path, &resolver, &small).unwrap_err().code,
        ErrorCode::Limit
    );
    fs::write(
        root.path().join("large.inc"),
        "gate custom a { U(0,0,0) a; }",
    )
    .unwrap();
    fs::write(&path, "OPENQASM 2.0; include \"large.inc\";").unwrap();
    let resolver = FileResolver {
        max_file_bytes: 8,
        ..Default::default()
    };
    assert_eq!(
        from_qasm_file(&path, &resolver, &limits).unwrap_err().code,
        ErrorCode::Limit
    );
    let cumulative = QasmLimits {
        max_source_bytes: fs::metadata(&path).unwrap().len() as usize + 1,
        ..Default::default()
    };
    assert_eq!(
        from_qasm_file(&path, &FileResolver::default(), &cumulative)
            .unwrap_err()
            .code,
        ErrorCode::Limit
    );
}

#[test]
fn diagnostics_have_stable_codes_and_preserve_the_innermost_origin() {
    for (code, want) in [
        (ErrorCode::Parse, "qasm_parse_error"),
        (ErrorCode::Type, "qasm_type_error"),
        (ErrorCode::Unsupported, "qasm_unsupported_construct"),
        (ErrorCode::BackendUnsupported, "qasm_backend_unsupported"),
        (ErrorCode::Unrepresentable, "qasm_unrepresentable_export"),
        (ErrorCode::MissingLayout, "qasm_missing_layout"),
        (ErrorCode::Limit, "qasm_limit_exceeded"),
        (ErrorCode::Io, "qasm_io_error"),
        (ErrorCode::InvalidDocument, "qasm_invalid_document"),
    ] {
        assert_eq!(code.as_str(), want);
        assert_eq!(
            QasmError::new(code, "detail").to_string(),
            format!("{want}: detail")
        );
    }
    let origin = SourceSpan {
        source: "included.inc".into(),
        line: 7,
        column: 3,
        offset: 42,
        length: 4,
        expansion: vec!["custom".into()],
    };
    let error = QasmError::new(ErrorCode::Type, "bad operand")
        .at(origin.clone())
        .at(SourceSpan {
            source: "outer.qasm".into(),
            ..origin.clone()
        });
    assert_eq!(error.origin, Some(origin));
    assert_eq!(
        error.to_string(),
        "included.inc:7:3: qasm_type_error: bad operand"
    );
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn corrupted_public_output_bindings_return_errors_instead_of_panicking() {
    let mut plan = LoweredQasmCircuit {
        instructions: vec![],
        num_qubits: 1,
        measurement_events: vec![0],
        classical_outputs: vec![ClassicalBinding::Measurement(1)],
        instruction_origins: vec![],
        target: Backend::Stabilizer,
    };
    assert_eq!(
        plan.project_classical(&[true]).unwrap_err().code,
        ErrorCode::InvalidDocument
    );
    plan.classical_outputs = vec![ClassicalBinding::Measurement(0)];
    plan.measurement_events = vec![1];
    assert_eq!(
        plan.project_classical(&[true]).unwrap_err().code,
        ErrorCode::InvalidDocument
    );
}

#[test]
fn malformed_grammar_and_static_gate_bodies_fail_with_source_locations() {
    for (body, code) in [
        ("include \"unterminated;", ErrorCode::Parse),
        ("include \"bad\\name\";", ErrorCode::Parse),
        ("include \"line\nbreak\";", ErrorCode::Parse),
        ("include custom;", ErrorCode::Parse),
        ("qreg q[1]", ErrorCode::Parse),
        ("qreg pi[1];", ErrorCode::Parse),
        ("qreg q[1.5];", ErrorCode::Parse),
        ("qreg q[18446744073709551616];", ErrorCode::Limit),
        ("gate custom a { measure a -> a; }", ErrorCode::Type),
        ("gate custom a { reset a; }", ErrorCode::Type),
        ("gate custom a { if(a==0) U(0,0,0) a; }", ErrorCode::Type),
        ("qreg q[1]; creg c[1]; measure q c;", ErrorCode::Parse),
        (
            "qreg q[1]; creg c[1]; if(c 0) U(0,0,0) q;",
            ErrorCode::Parse,
        ),
        (
            "qreg q[1]; creg c[1]; if(c==0) barrier q;",
            ErrorCode::Parse,
        ),
        (
            "qreg q[1]; creg c[1]; if(c==0) if(c==1) U(0,0,0) q;",
            ErrorCode::Parse,
        ),
        ("gate custom a { U(0,0,0) a;", ErrorCode::Parse),
        ("gate custom a {} gate custom a {}", ErrorCode::Type),
        ("gate custom a { U(0,0,0) a[0]; }", ErrorCode::Type),
        ("qreg q[1]; if(missing==0) U(pi,0,pi) q;", ErrorCode::Type),
        ("qreg q[2]; barrier q,q[0];", ErrorCode::Type),
    ] {
        let error = from_qasm(&format!("OPENQASM 2.0; {body}")).unwrap_err();
        assert_eq!(error.code, code, "{body}: {error}");
        assert!(error.origin.is_some(), "{body}");
    }
    from_qasm("OPENQASM 2.0; gate custom() a { barrier a; U(0,0,0) a; } qreg q[1]; custom() q;")
        .unwrap();
    let limits = QasmLimits {
        max_gate_depth: 1,
        ..Default::default()
    };
    let error = parse_and_check(
        "OPENQASM 2.0; gate zed q { U(0,0,0) q; } gate alpha q { zed q; }",
        "depth.qasm",
        &RejectIncludes,
        &limits,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Limit);
}

#[test]
fn analysis_reports_resource_failures_instead_of_misclassifying_them_as_capabilities() {
    let program = from_qasm("OPENQASM 2.0; qreg q[1]; U(pi/2,pi/2,pi/2) q;").unwrap();
    let error = program
        .analyze(&QasmLimits {
            max_operations: 1,
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Limit);
}

#[test]
fn declared_width_is_checked_without_allocating_expanded_operand_vectors() {
    let limits = QasmLimits {
        max_qubits: 100_000_000,
        max_classical_bits: 100_000_000,
        max_operations: 8,
        ..Default::default()
    };
    for statement in [
        "h q;",
        "U(0,0,0) q;",
        "measure q -> c;",
        "barrier q;",
        "reset q;",
        "measure q[0] -> c;",
    ] {
        let source = format!(
            "OPENQASM 2.0; include \"qelib1.inc\"; qreg q[100000000]; creg c[100000000]; {statement}"
        );
        let error = parse_and_check(&source, "wide.qasm", &RejectIncludes, &limits).unwrap_err();
        assert_eq!(error.code, ErrorCode::Limit, "{statement}: {error}");
        assert!(error.message.contains("operand"));
    }
    let scalar_operations = "U(0,0,0) q[0];".repeat(9);
    let error = parse_and_check(
        &format!("OPENQASM 2.0; qreg q[100000000]; {scalar_operations}"),
        "scalar.qasm",
        &RejectIncludes,
        &limits,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Limit);
    let idle = parse_and_check(
        "OPENQASM 2.0; qreg q[100000000]; creg c[100000000];",
        "idle.qasm",
        &RejectIncludes,
        &limits,
    )
    .unwrap();
    assert_eq!(idle.num_qubits(), 100_000_000);
    assert_eq!(idle.num_classical_bits(), 100_000_000);
    assert!(idle.operations().is_empty());
}

#[test]
fn wide_aliased_gate_arguments_are_rejected_without_width_times_arity_allocation() {
    let formals = (0..4000)
        .map(|index| format!("a{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let actuals = std::iter::repeat_n("q", 4000).collect::<Vec<_>>().join(",");
    let source = format!("OPENQASM 2.0; gate empty {formals} {{}} qreg q[4096]; empty {actuals};");
    let error = from_qasm(&source).unwrap_err();
    assert_eq!(error.code, ErrorCode::Type);
    assert!(error.message.contains("aliased"));
    assert!(error.origin.is_some());
}

#[test]
fn empty_gate_broadcasts_charge_operand_visits_before_entering_lanes() {
    let source = "OPENQASM 2.0; gate empty a,b,c,d {} qreg q[4]; qreg spare[3]; empty q,spare[0],spare[1],spare[2];";
    let limits = QasmLimits {
        max_operations: 16,
        max_gate_depth: 1,
        ..Default::default()
    };
    let checked = parse_and_check(source, "broadcast.qasm", &RejectIncludes, &limits).unwrap();
    assert!(checked.operations().is_empty());
    let insufficient = QasmLimits {
        max_operations: 15,
        ..limits
    };
    let error =
        parse_and_check(source, "broadcast.qasm", &RejectIncludes, &insufficient).unwrap_err();
    assert_eq!(error.code, ErrorCode::Limit);
    assert!(error.message.contains("expansion work"));
    assert_eq!(error.origin.unwrap().source, "broadcast.qasm");
}
