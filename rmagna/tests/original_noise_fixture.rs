use rmagna::circuit::{Basis, ParseOptions, SourceLocation, parse_circuit};
use rmagna::instrument::{CompileOptions, compile_affine};
use rmagna::noise::{OriginalNoiseAction, OriginalNoiseOptions, original_noise_categories};
use rmagna::probability::Probability;
use rmagna::runtime::{CausalProgram, CausalStrategy, ControllerRegistry, RuntimeOptions};
use rmagna::scalar::ScalarArithmetic;
use rmagna::unitary::CompilerStrategy;
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn rational(value: &Value) -> Probability {
    format!(
        "{}/{}",
        value[0].as_i64().unwrap(),
        value[1].as_i64().unwrap()
    )
    .parse()
    .unwrap()
}
fn source_matches(actual: &SourceLocation, expected: &Value) {
    assert_eq!(actual.source.as_deref(), expected["filename"].as_str());
    assert_eq!(actual.line as u64, expected["line"].as_u64().unwrap());
    assert_eq!(actual.column as u64, expected["column"].as_u64().unwrap());
    let repeats = expected["repeat"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actual.repeat, repeats);
}

#[test]
fn pinned_native_categories_match_exact_actions_sources_and_internal_sites() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/original_noise_categories.json")).unwrap();
    assert_eq!(
        fixture["source_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(fixture["source_files"], 475);
    assert_eq!(fixture["callback_calls"], 0);
    assert_eq!(
        fixture["native_sha256"],
        "c602a7270cc64c02123c5403147c000c8bf431160a3286fc0b2dca52b27545da"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 29);
    let arithmetic = ScalarArithmetic::default();
    for case in cases {
        let callback_calls = Arc::new(AtomicUsize::new(0));
        let count = callback_calls.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("opaque", "metadata-v1", move |_: &[bool]| {
                count.fetch_add(1, Ordering::SeqCst);
                Err("metadata must not invoke a callback".to_owned())
            })
            .unwrap();
        let parsed = parse_circuit(
            case["text"].as_str().unwrap(),
            &ParseOptions {
                source: Some("metadata-input.magna".to_owned()),
                controllers: registry.names(),
                ..Default::default()
            },
        );
        if case["rejected"].as_bool() == Some(true) {
            assert!(parsed.is_err(), "{}", case["name"]);
            continue;
        }
        let circuit = parsed.unwrap();
        let table = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
        assert_eq!(
            table.len() as u64,
            case["categories"].as_u64().unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(
            circuit.num_qubits() as u64,
            case["circuit"]["num_qubits"].as_u64().unwrap()
        );
        assert_eq!(
            circuit.num_records() as u64,
            case["circuit"]["num_records"].as_u64().unwrap()
        );
        let instructions = case["circuit"]["instructions"].as_array().unwrap();
        let mut channels = 0;
        for (category_index, category) in table.iter().enumerate() {
            let probabilities = case["probabilities"][category_index].as_array().unwrap();
            let actions = case["actions"][category_index].as_array().unwrap();
            assert_eq!(category.channels.len(), probabilities.len());
            assert_eq!(category.channels.len(), actions.len());
            let total = Probability::checked_sum(probabilities.iter().map(rational)).unwrap();
            assert_eq!(category.identity_probability, total.complement());
            for ((channel, expected_probability), action) in
                category.channels.iter().zip(probabilities).zip(actions)
            {
                channels += 1;
                assert_eq!(channel.probability, rational(expected_probability));
                let native_source = match &channel.action {
                    OriginalNoiseAction::ReportFlip { record } => {
                        assert_eq!(action[0], "report");
                        assert_eq!(action[1], format!("m{record}"));
                        instructions
                            .iter()
                            .find(|inst| {
                                inst["records"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|v| v.as_u64() == Some(*record as u64))
                            })
                            .unwrap()
                    }
                    OriginalNoiseAction::Pauli { factors } => {
                        let gates = action[1].as_array().unwrap();
                        let expected = gates
                            .iter()
                            .map(|gate| {
                                let basis = match gate["kind"].as_str().unwrap() {
                                    "X" => Basis::X,
                                    "Y" => Basis::Y,
                                    "Z" => Basis::Z,
                                    _ => panic!("non-Pauli native action"),
                                };
                                (gate["qubits"][0].as_u64().unwrap() as usize, basis)
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(*factors, expected);
                        instructions
                            .iter()
                            .find(|inst| inst["op_id"] == action[0])
                            .unwrap()
                    }
                };
                // Native operation strings are translated by captured source
                // and record metadata, never equated to expanded Rust IDs.
                source_matches(&channel.source.location, &native_source["source"]);
                let original = &circuit.operations()[channel.source.operation];
                assert_eq!(original.id, channel.source.operation);
                assert_eq!(original.location, channel.source.location);
            }
        }
        assert_eq!(channels as u64, case["action_count"].as_u64().unwrap());
        assert_eq!(
            table.iter().filter(|c| c.guard.is_some()).count() as u64,
            case["guarded"].as_u64().unwrap()
        );
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let compile = CompileOptions {
                compiler_strategy: compiler,
                ..Default::default()
            };
            if case["guarded"].as_u64() == Some(0) {
                let compiled = compile_affine(&circuit, &compile, &arithmetic).unwrap();
                for category in &table {
                    let site = &compiled.noise_sites()[category.internal_site];
                    assert_eq!(
                        site.source_operations(),
                        category
                            .source_operations
                            .iter()
                            .map(|a| a.operation)
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(site.identity_probability(), &category.identity_probability);
                    assert_eq!(
                        site.alternatives()
                            .iter()
                            .map(|c| c.probability().clone())
                            .collect::<Vec<_>>(),
                        category
                            .channels
                            .iter()
                            .map(|c| c.probability.clone())
                            .collect::<Vec<_>>()
                    );
                }
            }
            for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
                let program = CausalProgram::new(
                    &circuit,
                    &registry,
                    RuntimeOptions {
                        strategy,
                        compile,
                        ..Default::default()
                    },
                    &arithmetic,
                )
                .unwrap();
                for category in &table {
                    for anchor in &category.source_operations {
                        assert_eq!(
                            program.potential_site(anchor.operation),
                            Some(category.internal_site)
                        );
                    }
                }
            }
        }
        assert_eq!(callback_calls.load(Ordering::SeqCst), 0);
    }
}
