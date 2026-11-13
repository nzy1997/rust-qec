// SPDX-License-Identifier: MPL-2.0
// Complete PauliFeedbackReuseTests::test_physical_and_recovery_callbacks_match_literal_sampling,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_pauli_feedback_reuse.py.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::FaultSelection;
use rmagna::noise::SplitMix64;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, TaskEvaluator, TaskOptions, TaskSampleDiagnosticOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

const PHYSICAL: &str = "H 0 1 2\nT 2\nM(1/3) 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCY ctrl[0] 2\nX_ERROR(1/3) 2\nH 2\nM 2\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[1]\nCZ ctrl[1] 2\nH 2\nT 2\nDETECTOR rec[-3]\nPOSTSELECT det[3]";
const RECOVERY: &str = "H 1\nM 1\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCONTROL[XOR] ctrl[0] rec[-1] -> ctrl[1]\nCY ctrl[1] 0\nCZ ctrl[0] 0\nT 0";
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-pauli-terminal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("recovery.magna"), RECOVERY).unwrap();
        std::fs::write(path.join("target.magna"), "H 0\nT 0").unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scalar(value: &Value) -> ExactScalar {
    let c = value["coefficients"].as_array().unwrap();
    assert_eq!(c.len(), 4);
    ScalarArithmetic::default()
        .scalar(
            std::array::from_fn(|i| c[i].as_i64().unwrap().into()),
            value["power"].as_i64().unwrap(),
        )
        .unwrap()
}
fn bits(value: &Value) -> Vec<bool> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| match v.as_u64().unwrap() {
            0 => false,
            1 => true,
            _ => panic!("fixture bit must be binary"),
        })
        .collect()
}
fn normalized(value: &ExactScalar, mass: &ExactScalar) -> ExactField {
    let f = FieldArithmetic::default();
    f.divide(
        &f.from_scalar(value).unwrap(),
        &f.from_scalar(mass).unwrap(),
    )
    .unwrap()
}
#[test]
fn original_physical_and_recovery_callback_masses_match_native_literal_vectors() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/original_pauli_terminal.json")).unwrap();
    assert_eq!(
        fixture["provenance"]["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 24);
    let files = Files::new();
    let mut comparisons = 0;
    let mut configurations = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let log = Arc::new(Mutex::new(Vec::<Vec<bool>>::new()));
                let mut registry = ControllerRegistry::default();
                for name in ["and", "echo"] {
                    let observed = log.clone();
                    registry
                        .register(name, "1", move |input: &[bool]| {
                            observed.lock().unwrap().push(input.to_vec());
                            Ok(if name == "and" {
                                assert_eq!(input.len(), 2);
                                vec![input[0] & input[1]]
                            } else {
                                input.to_vec()
                            })
                        })
                        .unwrap();
                }
                let parse = ParseOptions {
                    controllers: registry.names(),
                    ..Default::default()
                };
                let source = parse_circuit(PHYSICAL, &parse).unwrap();
                let task=parse_task(r#"{"outputs":[2],"sample_records":[0,1,2],"observables":[[1,0],[0,1],[1,1]],"recovery":"recovery.magna","ideal_state":"target.magna"}"#,&files.0,&TaskOptions::default()).unwrap();
                let modules = task.load_modules(&source, &parse).unwrap();
                let mut options = EvaluationOptions::default();
                options.runtime.strategy = causal;
                options.runtime.history_sampler = sampler;
                options.runtime.compile.compiler_strategy = compiler;
                let evaluator = TaskEvaluator::new(&source, &modules, &registry, options).unwrap();
                for (seed, row) in cases.iter().enumerate() {
                    assert_eq!(row["seed"].as_u64().unwrap(), seed as u64);
                    let mut faults = vec![];
                    let mut probability = "1".parse::<BigRational>().unwrap();
                    let flips = row["report_flips"].as_array().unwrap();
                    for (record, name) in ["m0", "m1"].iter().enumerate() {
                        let selected = flips.iter().any(|v| v == *name);
                        if selected {
                            faults.push(FaultSelection {
                                site: record,
                                alternative: 0,
                            });
                        }
                        probability *= if selected { "1/3" } else { "2/3" }
                            .parse::<BigRational>()
                            .unwrap();
                    }
                    assert!(flips.iter().all(|v| v == "m0" || v == "m1"));
                    let gates = row["noise_gates"].as_object().unwrap();
                    let fault = !gates.is_empty();
                    if fault {
                        assert_eq!(gates.len(), 1);
                        assert_eq!(gates["op7"][0]["kind"], "X");
                        assert_eq!(gates["op7"][0]["qubits"], serde_json::json!([2]));
                        faults.push(FaultSelection {
                            site: 2,
                            alternative: 0,
                        });
                    }
                    probability *= if fault { "1/3" } else { "2/3" }
                        .parse::<BigRational>()
                        .unwrap();
                    let inputs = row["callback_inputs"].as_array().unwrap();
                    assert_eq!(inputs.len(), 3);
                    let mut groups = inputs.iter().map(bits).collect::<Vec<_>>();
                    let sampled = bits(&row["draw"]["sampled_records"]);
                    groups.push(sampled.clone());
                    let start = log.lock().unwrap().len();
                    let draw = evaluator
                        .evaluate_prescribed_history(&faults, &groups)
                        .unwrap();
                    assert_eq!(&log.lock().unwrap()[start..], &groups[..3]);
                    let exact = &row["draw"]["exact"];
                    let expected = [
                        scalar(&exact["history_mass"]),
                        scalar(&exact["acceptance"]),
                        scalar(&exact["error"]),
                        scalar(&exact["moments"][0]),
                        scalar(&exact["moments"][1]),
                        scalar(&exact["moments"][2]),
                    ];
                    let literal = row["literal_raw"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(scalar)
                        .collect::<Vec<_>>();
                    assert_eq!(expected.as_slice(), literal);
                    let actual = [
                        draw.history_mass.clone(),
                        draw.raw_acceptance.clone(),
                        draw.raw_error.clone(),
                        draw.raw_moments[0].clone(),
                        draw.raw_moments[1].clone(),
                        draw.raw_moments[2].clone(),
                    ];
                    for (left, right) in actual.iter().zip(&expected) {
                        assert_eq!(left, right, "raw physical masses seed={seed}");
                        comparisons += 1;
                    }
                    assert_eq!(
                        draw.raw_target_success,
                        Some(scalar(&exact["target_success"]))
                    );
                    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                    assert_eq!(
                        draw.accepted,
                        normalized(&draw.raw_acceptance, &draw.history_mass)
                    );
                    assert_eq!(draw.error, normalized(&draw.raw_error, &draw.history_mass));
                    assert_eq!(
                        draw.target_success,
                        Some(normalized(
                            draw.raw_target_success.as_ref().unwrap(),
                            &draw.history_mass
                        ))
                    );
                    assert_eq!(
                        draw.moments,
                        draw.raw_moments
                            .iter()
                            .map(|v| normalized(v, &draw.history_mass))
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(draw.noise_probability, probability);
                    assert_eq!(
                        draw.noise_likelihood_ratio,
                        "1".parse::<BigRational>().unwrap()
                    );
                    assert_eq!(draw.history_proposal_mass, None);
                    assert_eq!(draw.history_weight, ExactField::one());
                    assert_eq!(draw.sampled_records, sampled);
                    assert_eq!(draw.controller_calls.len(), 3);
                    for (i, name) in ["and", "echo", "echo"].iter().enumerate() {
                        let call = &draw.controller_calls[i];
                        assert_eq!(call.call, i);
                        assert_eq!(call.name, *name);
                        assert_eq!(call.inputs, groups[i]);
                        assert_eq!(
                            call.outputs,
                            if i == 0 {
                                vec![groups[0][0] & groups[0][1]]
                            } else {
                                groups[i].clone()
                            }
                        );
                    }
                    if causal == CausalStrategy::FixedTemplate {
                        assert_eq!(draw.network_compilations, 0);
                    }
                }
                for seed in 0..8 {
                    let start = log.lock().unwrap().len();
                    let (draw, diagnostics) = evaluator
                        .sample_with_diagnostics(
                            &mut SplitMix64::new(seed),
                            &TaskSampleDiagnosticOptions::default(),
                        )
                        .unwrap();
                    assert_eq!(diagnostics.protocol_controller_calls, 2);
                    assert_eq!(diagnostics.recovery_controller_calls, 1);
                    assert_eq!(diagnostics.learned_record_rank, 4);
                    assert_eq!(draw.controller_calls.len(), 3);
                    assert_eq!(log.lock().unwrap().len() - start, 3);
                    for call in &draw.controller_calls[1..] {
                        assert_eq!(call.inputs, call.outputs);
                    }
                    if causal == CausalStrategy::FixedTemplate {
                        assert_eq!(draw.network_compilations, 0);
                    }
                }
                configurations += 1;
            }
        }
    }
    assert_eq!(configurations, 8);
    assert_eq!(comparisons, 1152);
}
