// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//! Original CircuitFileTests.test_native_rationals_and_replaced_views from
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_circuit_file.py,
//! plus owned versioned-expression and edited-provenance regressions.
use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::circuit::{
    BitAtom, BitExpr, CircuitView, CircuitViewError, ControlAnalysis, Gate, Operation,
    ParseOptions, RegisterVersion, parse_circuit,
};
use rmagna::instrument::{CompileOptions, compile_affine};
use rmagna::scalar::ScalarArithmetic;
use rmagna::unitary::CompilerStrategy;
use std::collections::BTreeMap;

const RATIONAL_LITERALS: [&str; 5] = [
    "1e-200",
    ".125",
    "+0.1250",
    "1_000/100_000",
    "123456789012345678901234567890/123456789012345678901234567891",
];
const TEXT: &str = "M 0\nCONTROL[NOT] rec[-1] -> ctrl[0]";

fn assert_unparsed(view: &CircuitView) {
    assert!(!view.has_parsed_form());
    assert_eq!(
        view.parsed_circuit().unwrap_err(),
        CircuitViewError::NoParsedForm
    );
    assert_eq!(view.format_version(), Err(CircuitViewError::NoParsedForm));
    assert_eq!(view.to_text(), Err(CircuitViewError::NoParsedForm));
    let message = view.parsed_circuit().unwrap_err().to_string();
    assert!(message.contains("no parsed form"));
    assert!(message.contains("parse the intended source text again"));
}

#[test]
fn native_rationals_and_replaced_views() {
    let expected = [
        BigRational::new(1.into(), BigInt::from(10).pow(200)),
        BigRational::new(1.into(), 8.into()),
        BigRational::new(1.into(), 8.into()),
        BigRational::new(1.into(), 100.into()),
        BigRational::new(
            "123456789012345678901234567890".parse().unwrap(),
            "123456789012345678901234567891".parse().unwrap(),
        ),
    ];
    for (literal, rational) in RATIONAL_LITERALS.iter().zip(expected) {
        let circuit =
            parse_circuit(&format!("X_ERROR({literal}) 0"), &ParseOptions::default()).unwrap();
        let Operation::Noise { probabilities, .. } = &circuit.operations()[0].operation else {
            panic!("expected original single X noise declaration")
        };
        assert_eq!(probabilities.len(), 1);
        assert_eq!(probabilities[0].value(), &rational, "literal {literal}");
    }
    let circuit = parse_circuit(TEXT, &ParseOptions::default()).unwrap();
    let view = circuit.view();
    assert!(view.has_parsed_form());
    assert_eq!(view.format_version().unwrap(), "magna-0.1");
    assert_eq!(view.to_text().unwrap(), TEXT);
    assert_eq!(view.operations().len(), 2);
    let target = RegisterVersion::new(0, 0);
    let original = BitExpr::atom(BitAtom::Record(0)).inverted();
    assert_eq!(view.analysis().expression(target), Some(&original));
    let rewritten = view
        .clone()
        .with_operations(view.operations()[..1].to_vec());
    assert_unparsed(&rewritten);
    assert_eq!(rewritten.operations().len(), 1);
    assert_eq!(rewritten.source_snapshot(), TEXT);
    let analysis = view
        .analysis()
        .clone()
        .with_expressions(BTreeMap::from([(target, BitExpr::default())]));
    assert_eq!(analysis.expression(target), Some(&BitExpr::default()));
    assert_eq!(view.analysis().expression(target), Some(&original));
    let edited = view.with_analysis(analysis);
    assert_eq!(
        edited.analysis().expression(target),
        Some(&BitExpr::default())
    );
    assert_unparsed(&edited);
    assert_eq!(edited.source_snapshot(), TEXT);

    let arithmetic = ScalarArithmetic::default();
    for compiler_strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let parsed = circuit.view();
        let compiled = compile_affine(
            parsed.parsed_circuit().unwrap(),
            &CompileOptions {
                compiler_strategy,
                ..Default::default()
            },
            &arithmetic,
        )
        .unwrap();
        assert_eq!(compiled.noise_sites().len(), 1);
    }
}

#[test]
fn control_analysis_owns_all_register_versions_and_replacement_maps() {
    let source =
        "M 0\nCONTROL[NOT] rec[-1] -> ctrl[0]\nREPEAT 2 {\nCONTROL[NOT] ctrl[0] -> ctrl[0]\n}";
    let circuit = parse_circuit(source, &ParseOptions::default()).unwrap();
    let analysis = circuit.control_analysis();
    let record = BitExpr::atom(BitAtom::Record(0));
    assert_eq!(
        analysis.expressions().keys().copied().collect::<Vec<_>>(),
        (0..3)
            .map(|version| RegisterVersion::new(0, version))
            .collect::<Vec<_>>()
    );
    for (version, expression) in [
        record.clone().inverted(),
        record.clone(),
        record.clone().inverted(),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            analysis.expression(RegisterVersion::new(0, version)),
            Some(&expression)
        );
    }
    for unknown in [
        RegisterVersion::new(0, 3),
        RegisterVersion::new(1, 0),
        RegisterVersion::new(usize::MAX, usize::MAX),
    ] {
        assert_eq!(analysis.expression(unknown), None);
    }
    let target = RegisterVersion::new(0, 0);
    let mut supplied = BTreeMap::from([(target, BitExpr::default())]);
    let replacement = ControlAnalysis::from_expressions(supplied.clone());
    supplied.insert(target, record.clone());
    drop(supplied);
    assert_eq!(replacement.expression(target), Some(&BitExpr::default()));
    assert_eq!(replacement.expression(RegisterVersion::new(0, 1)), None);
    let edited = circuit.view().with_analysis(replacement);
    assert_unparsed(&edited);
    assert_eq!(circuit.control_analysis(), analysis);
    drop(circuit);
    assert_eq!(
        analysis.expression(target),
        Some(&record.clone().inverted())
    );
    assert_eq!(
        analysis.expression(RegisterVersion::new(0, 2)),
        Some(&record.inverted())
    );
    assert_eq!(
        edited.analysis().expression(target),
        Some(&BitExpr::default())
    );
    assert_eq!(edited.source_snapshot(), source);
}

#[test]
fn identical_operation_and_analysis_edits_never_restore_parsed_provenance() {
    let mut source = TEXT.to_owned();
    let circuit = parse_circuit(&source, &ParseOptions::default()).unwrap();
    let view = CircuitView::from(circuit);
    source.clear();
    assert_eq!(view.source_snapshot(), TEXT);
    assert_eq!(view.parsed_circuit().unwrap().to_text(), TEXT);
    let identical_operations = view.clone().with_operations(view.operations().to_vec());
    assert_eq!(identical_operations.operations(), view.operations());
    assert_unparsed(&identical_operations);
    let identical_analysis = view.clone().with_analysis(view.analysis().clone());
    assert_eq!(identical_analysis.analysis(), view.analysis());
    assert_unparsed(&identical_analysis);
    let restored = identical_operations
        .with_operations(view.operations().to_vec())
        .with_analysis(view.analysis().clone());
    assert_unparsed(&restored.clone());
    assert_eq!(restored.source_snapshot(), TEXT);
    let mut malformed = view.operations()[0].clone();
    malformed.id = usize::MAX;
    malformed.operation = Operation::Unitary {
        gate: Gate::Ccz,
        qubits: vec![usize::MAX],
        control: None,
    };
    let inspection_only = view.clone().with_operations(vec![malformed]);
    assert_eq!(inspection_only.operations()[0].id, usize::MAX);
    assert_unparsed(&inspection_only);
    assert_eq!(inspection_only.source_snapshot(), TEXT);
    assert!(view.has_parsed_form());
    assert_eq!(view.to_text().unwrap(), TEXT);
}
