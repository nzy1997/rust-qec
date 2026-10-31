// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// Original budget cases are independently translated from Magna, revision
// 56847478c840395735146ef1e1ccdc3d8aedc10f, under MPL-2.0.
// Additional cases exercise exact Rust parser admission boundaries.
use num_bigint::BigUint;
use rmagna::circuit::{ParseError, ParseOptions, ParseResource, ParseResourceLimit, parse_circuit};
use std::error::Error;

fn stop(error: &ParseError, option: ParseResource, limit: usize, needed: BigUint) {
    let payload = error
        .resource_limit
        .as_deref()
        .expect("typed resource stop");
    assert_eq!(payload.option, option);
    assert_eq!(payload.limit, limit);
    assert_eq!(payload.needed, needed);
    assert!(error.message.contains(option.option_name()));
    assert_eq!(
        error.source().unwrap().downcast_ref::<ParseResourceLimit>(),
        Some(payload)
    );
}

#[test]
fn original_expansion_budget_reports_exact_instruction_and_qubit_requirements() {
    let options = ParseOptions {
        max_instructions: 10,
        ..Default::default()
    };
    let error = parse_circuit("REPEAT 20 {\nH 0\n}", &options).unwrap_err();
    stop(&error, ParseResource::Instructions, 10, 20_u8.into());
    let error = parse_circuit("REPEAT 4 {\nREPEAT 4 {\nH 0\n}\n}", &options).unwrap_err();
    stop(&error, ParseResource::Instructions, 10, 16_u8.into());
    assert!(error.to_string().contains("max_instructions = 10"));
    let options = ParseOptions {
        source: Some("sparse.magna".into()),
        ..Default::default()
    };
    let error = parse_circuit("H 0\nM 70000\n", &options).unwrap_err();
    stop(&error, ParseResource::Qubits, 65536, 70001_u32.into());
    assert_eq!(
        (
            error.location.source.as_deref(),
            error.location.line,
            error.location.column
        ),
        (Some("sparse.magna"), 2, 1)
    );
    assert!(error.message.contains("qubit index 70000"));
    let options = ParseOptions {
        max_qubits: 70001,
        ..Default::default()
    };
    assert_eq!(
        parse_circuit("M 70000\n", &options).unwrap().num_qubits(),
        70001
    );
    let circuit = parse_circuit(
        "REPEAT 999999999999999999 {\nREPEAT 999999999999999999 {\n}\n}",
        &ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(circuit.num_instructions(), 0);
    assert!(circuit.operations().is_empty());
}

#[test]
fn multiplication_and_addition_overflow_stop_before_target_parsing() {
    let options = ParseOptions {
        max_instructions: usize::MAX,
        max_qubits: 0,
        ..Default::default()
    };
    for (text, needed) in [
        (
            format!("REPEAT {} {{\nH 0\nH 0\n}}", usize::MAX),
            BigUint::from(usize::MAX) * 2_u8,
        ),
        (
            format!("REPEAT {} {{\nH 0\n}}\nH 0", usize::MAX),
            BigUint::from(usize::MAX) + 1_u8,
        ),
    ] {
        let error = parse_circuit(&text, &options).unwrap_err();
        stop(&error, ParseResource::Instructions, usize::MAX, needed);
    }
}

#[test]
fn nested_preflight_keeps_the_complete_count_beyond_u128() {
    let options = ParseOptions {
        max_instructions: usize::MAX,
        max_qubits: 0,
        ..Default::default()
    };
    let text = format!(
        "REPEAT {0} {{\nREPEAT {0} {{\nREPEAT {0} {{\nH 0\n}}\n}}\n}}",
        u64::MAX
    );
    let error = parse_circuit(&text, &options).unwrap_err();
    stop(
        &error,
        ParseResource::Instructions,
        usize::MAX,
        BigUint::from(u64::MAX).pow(3),
    );
    let options = ParseOptions {
        max_instructions: 10,
        max_qubits: 0,
        ..Default::default()
    };
    let error = parse_circuit("REPEAT 2 {\nREPEAT 20 {\nH 0\n}\n}\nH 0", &options).unwrap_err();
    stop(&error, ParseResource::Instructions, 10, 41_u8.into());
    assert_eq!(error.location.line, 1);
}

#[test]
fn huge_empty_and_zero_repeats_admit_zero_quotas_without_expansion() {
    let options = ParseOptions {
        max_instructions: 0,
        max_operations: 0,
        max_qubits: 0,
        ..Default::default()
    };
    for text in [
        "",
        "REPEAT 18446744073709551615 {\nREPEAT 18446744073709551615 {\n}\n}",
        "REPEAT 0 {\nREPEAT 18446744073709551615 {\nUNKNOWN 999999\n}\n}",
        "REPEAT 18446744073709551615 {\nREPEAT 0 {\nH 999999\n}\n}",
    ] {
        let circuit = parse_circuit(text, &options).unwrap();
        assert_eq!(circuit.num_instructions(), 0);
        assert_eq!(circuit.num_qubits(), 0);
        assert!(circuit.operations().is_empty());
    }
}

#[test]
fn operation_limits_report_first_required_prefix_and_repeat_location() {
    let options = ParseOptions {
        max_operations: 2,
        ..Default::default()
    };
    let error = parse_circuit("H 0 1 2", &options).unwrap_err();
    stop(&error, ParseResource::Operations, 2, 3_u8.into());
    let options = ParseOptions {
        max_operations: 1,
        ..Default::default()
    };
    let error = parse_circuit("REPEAT 2 {\nH 0\n}", &options).unwrap_err();
    stop(&error, ParseResource::Operations, 1, 2_u8.into());
    assert_eq!(error.location.line, 2);
    assert_eq!(error.location.repeat, [1]);
    let options = ParseOptions {
        max_operations: 0,
        ..Default::default()
    };
    stop(
        &parse_circuit("H 0", &options).unwrap_err(),
        ParseResource::Operations,
        0,
        1_u8.into(),
    );
}

#[test]
fn qubit_limits_keep_exact_width_at_the_platform_boundary() {
    let options = ParseOptions {
        max_qubits: usize::MAX,
        ..Default::default()
    };
    let error = parse_circuit(&format!("H {}", usize::MAX), &options).unwrap_err();
    stop(
        &error,
        ParseResource::Qubits,
        usize::MAX,
        BigUint::from(usize::MAX) + 1_u8,
    );
    assert_eq!(
        parse_circuit(&format!("H {}", usize::MAX - 1), &options)
            .unwrap()
            .num_qubits(),
        usize::MAX
    );
    let options = ParseOptions {
        max_qubits: 0,
        ..Default::default()
    };
    stop(
        &parse_circuit("H 0", &options).unwrap_err(),
        ParseResource::Qubits,
        0,
        1_u8.into(),
    );
}

#[test]
fn source_bytes_and_nesting_limits_have_owned_payloads() {
    let source = String::from("# é\nH 0\n");
    let options = ParseOptions {
        source: Some("utf8.magna".into()),
        max_source_bytes: source.len() - 1,
        ..Default::default()
    };
    let error = parse_circuit(&source, &options).unwrap_err().clone();
    stop(&error, ParseResource::SourceBytes, 8, 9_u8.into());
    drop(source);
    drop(options);
    assert_eq!(error.location.source.as_deref(), Some("utf8.magna"));
    assert_eq!((error.location.line, error.location.column), (1, 1));
    assert!(
        parse_circuit(
            "# é\nH 0\n",
            &ParseOptions {
                max_source_bytes: 9,
                ..Default::default()
            }
        )
        .is_ok()
    );
    let options = ParseOptions {
        max_repeat_depth: 1,
        ..Default::default()
    };
    let error = parse_circuit("REPEAT 1 {\nREPEAT 1 {\nH 0\n}\n}", &options).unwrap_err();
    stop(&error, ParseResource::RepeatDepth, 1, 2_u8.into());
    assert_eq!(error.location.line, 2);
    assert_eq!(ParseResource::SourceBytes.option_name(), "max_source_bytes");
    assert_eq!(ParseResource::RepeatDepth.option_name(), "max_repeat_depth");
    assert_eq!(
        ParseResource::Instructions.option_name(),
        "max_instructions"
    );
    assert_eq!(ParseResource::Operations.option_name(), "max_operations");
    assert_eq!(ParseResource::Qubits.option_name(), "max_qubits");
}

#[test]
fn malformed_input_remains_a_syntax_error() {
    for source in [
        "}",
        "{",
        "REPEAT 1 2 {\n}",
        "REPEAT -1 {\n}",
        "REPEAT 18446744073709551616 {\n}",
        "REPEAT 1",
        "REPEAT 1 {\nH 0",
        "H -1",
        "H word",
        "X_ERROR(bad) 0",
    ] {
        let error = parse_circuit(source, &ParseOptions::default()).unwrap_err();
        assert!(error.resource_limit.is_none(), "{source}: {error}");
        assert!(error.source().is_none());
        assert!(!error.message.is_empty());
    }
}
