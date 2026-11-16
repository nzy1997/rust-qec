//! Full upstream incremental-history circuit, with a declared Rust stream.
//! Python MT19937 digests and private max_plans are not Rust ABI promises.
//! Native max_plans=1 is covered by the explicit scalar-plan eviction witness;
//! this replay independently exercises a real cumulative query-work budget.
use super::*;
use rmagna::{
    costs::{ExecutionLimits, ExecutionMeter},
    noise::{RandomSource, SplitMix64},
    runtime::{CausalStrategy, ControllerRegistry, HistorySampler},
    task::{EvaluationOptions, TaskEvaluation, TaskEvaluator, TaskOptions, parse_task},
    unitary::CompilerStrategy,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
enum Event {
    Word(u64),
    Callback(Vec<bool>),
}
struct LoggedRandom {
    random: SplitMix64,
    events: Arc<Mutex<Vec<Event>>>,
}
impl RandomSource for LoggedRandom {
    fn next_u64(&mut self) -> u64 {
        let word = self.random.next_u64();
        self.events.lock().unwrap().push(Event::Word(word));
        word
    }
}
fn scalar(value: &ExactScalar) -> Value {
    json!([
        value.coefficients().each_ref().map(ToString::to_string),
        value.power()
    ])
}
fn field(value: &rmagna::field::ExactField) -> Value {
    json!(value.coefficients().each_ref().map(ToString::to_string))
}
/// Every physics field and actual controller invocation in TaskEvaluation.
/// Logical compilation/query counts and decomposition sizes describe each
/// execution route separately, rather than changing the physical stream.
fn physics(d: &TaskEvaluation) -> Value {
    json!({
        "accepted":field(&d.accepted), "error":field(&d.error),
        "target_success":d.target_success.as_ref().map(field),
        "moments":d.moments.iter().map(field).collect::<Vec<_>>(),
        "raw_acceptance":scalar(&d.raw_acceptance), "raw_error":scalar(&d.raw_error),
        "raw_moments":d.raw_moments.iter().map(scalar).collect::<Vec<_>>(),
        "raw_target_success":d.raw_target_success.as_ref().map(scalar),
        "target_normalization":d.target_normalization.as_ref().map(scalar),
        "history_mass":scalar(&d.history_mass),
        "history_proposal_mass":d.history_proposal_mass.as_ref().map(scalar),
        "history_weight":field(&d.history_weight),
        "noise_probability":d.noise_probability.to_string(),
        "noise_likelihood_ratio":d.noise_likelihood_ratio.to_string(),
        "sampled_records":d.sampled_records,
        "calls":d.controller_calls.iter().map(|c|json!([c.call,c.name,c.inputs,c.outputs])).collect::<Vec<_>>(),
    })
}
fn source(magic: bool) -> String {
    format!(
        "H 0 1 2\n{}\
M(1/5) 0 1\n\
DETECTOR rec[-2]\nCONTROL[record] det[-1] -> ctrl[0]\nCS ctrl[0] 2\n\
MX(1/7) 2\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n\
CONTROL[record] det[-2] det[-1] -> ctrl[1] ctrl[2]\nCH ctrl[1] 2\nZ_ERROR(1/3) ctrl[2] 2\n\
MX 2\nDETECTOR rec[-4]\nDETECTOR rec[-1]\n\
CONTROL[record] det[-2] det[-1] -> ctrl[3] ctrl[4]\nCZ ctrl[3] 2\nX_ERROR(1/4) ctrl[4] 3\n\
M 3\nDETECTOR rec[-1]\nCONTROL[record] det[-1] -> ctrl[5]\nCX ctrl[5] 4\nCX 4 5\n\
DETECTOR rec[-5] rec[-4]\nPOSTSELECT det[6]\n",
        if magic { "T 0 2\n" } else { "" }
    )
}
#[test]
fn complete_incremental_history_stream_matches_preoptimization_baselines_and_real_work_budgets() {
    struct Files(std::path::PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let files =
        Files(std::env::temp_dir().join(format!("rmagna-full-incremental-{}", std::process::id())));
    std::fs::create_dir(&files.0).unwrap();
    std::fs::write(files.0.join("ideal.magna"), "I 0 1\n").unwrap();
    let baseline: Value = serde_json::from_str(include_str!(
        "../data/fixed-reference-incremental-streams.json"
    ))
    .unwrap();
    assert_eq!(
        baseline["rust_baseline_commit"],
        "fbe56895852a92afb547e15e6ef3a016c4b67917"
    );
    let mut draws = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for magic in [false, true] {
            for sampler in [HistorySampler::AffineSupport, HistorySampler::ExactMarginal] {
                for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
                    for budgeted in [false, true] {
                        let events = Arc::new(Mutex::new(vec![]));
                        let seen = Arc::clone(&events);
                        let mut registry = ControllerRegistry::default();
                        registry
                            .register("record", "1", move |bits: &[bool]| {
                                seen.lock().unwrap().push(Event::Callback(bits.to_vec()));
                                Ok(bits.to_vec())
                            })
                            .unwrap();
                        let parse = ParseOptions {
                            controllers: registry.names(),
                            ..Default::default()
                        };
                        let circuit = parse_circuit(&source(magic), &parse).unwrap();
                        let modules = parse_task(r#"{"outputs":[4,5],"sample_records":[0,1,2,3,4],"ideal_state":"ideal.magna"}"#,&files.0,&TaskOptions::default()).unwrap().load_modules(&circuit,&parse).unwrap();
                        let mut options = EvaluationOptions::default();
                        options.runtime.strategy = strategy;
                        options.runtime.history_sampler = sampler;
                        options.runtime.compile.compiler_strategy = compiler;
                        let evaluator =
                            TaskEvaluator::new(&circuit, &modules, &registry, options).unwrap();
                        assert!(
                            events.lock().unwrap().is_empty(),
                            "preparation executed a real callback"
                        );
                        let meter = ExecutionMeter::new(ExecutionLimits {
                            max_queries: if budgeted { 10_000 } else { u64::MAX },
                            ..Default::default()
                        });
                        let mut rows = vec![];
                        let mut seen_outputs = std::collections::BTreeSet::new();
                        for seed in 0..32 {
                            events.lock().unwrap().clear();
                            let mut random = LoggedRandom {
                                random: SplitMix64::new(26090400 + seed),
                                events: Arc::clone(&events),
                            };
                            let draw = evaluator.sample_with_costs(&mut random, &meter).unwrap();
                            let word = &draw.sampled_records;
                            assert_eq!(word.len(), 5);
                            let calls = events
                                .lock()
                                .unwrap()
                                .iter()
                                .filter_map(|event| match event {
                                    Event::Callback(b) => Some(b.clone()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(
                                calls,
                                vec![
                                    vec![word[0]],
                                    vec![word[1], word[2]],
                                    vec![word[0], word[3]],
                                    vec![word[4]]
                                ]
                            );
                            assert_eq!(draw.controller_calls.len(), 4);
                            assert_eq!(
                                draw.raw_error,
                                if word[4] {
                                    draw.raw_acceptance.clone()
                                } else {
                                    ExactScalar::zero()
                                }
                            );
                            if word[0] != word[1] {
                                assert!(draw.raw_acceptance.is_zero());
                            }
                            assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                            assert_eq!(draw.noise_likelihood_ratio.to_string(), "1");
                            assert!(
                                draw.noise_probability
                                    > num_rational::BigRational::from_integer(0.into())
                            );
                            seen_outputs.insert(word[4]);
                            let transcript = events.lock().unwrap().clone();
                            let next = random.next_u64();
                            rows.push((physics(&draw), transcript, next));
                            draws += 1;
                        }
                        assert_eq!(seen_outputs.len(), 2);
                        let key = format!("{compiler:?}/{magic}/{sampler:?}/{strategy:?}");
                        let digest =
                            format!("{:x}", Sha256::digest(serde_json::to_vec(&rows).unwrap()));
                        assert_eq!(
                            digest,
                            baseline["cells"][&key].as_str().unwrap(),
                            "preoptimization complete stream changed for {key}/budget={budgeted}"
                        );
                        assert!(meter.snapshot().query.attempted < 10_000);
                    }
                }
            }
        }
    }
    assert_eq!(draws, 1024);
    println!(
        "1024 complete source incremental draws: 2 compilers x 2 magic modes x 2 history samplers x fixed/cold x metered/unbounded x 32 seeds; every physics field, actual callback/RNG interleaving and next caller word match each preoptimization Rust stream"
    );
}
