// SPDX-License-Identifier: MPL-2.0
// Complete RuntimeOwnershipTests.test_reentrant_callbacks_and_exception_leave_fresh_control_values,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_runtime_ownership.py.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::noise::{RandomSource, SplitMix64};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{EvaluationOptions, TaskEvaluation, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
fn check(draw: &TaskEvaluation) {
    let half = ScalarArithmetic::default()
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    assert_eq!(draw.sampled_records.len(), 2);
    assert_eq!(draw.sampled_records[0], draw.sampled_records[1]);
    assert_eq!(draw.history_mass, half);
    assert_eq!(draw.raw_acceptance, half);
    assert_eq!(draw.raw_error, ExactScalar::zero());
    assert_eq!(draw.accepted, ExactField::one());
    assert_eq!(draw.error, ExactField::zero());
    assert_eq!(draw.history_weight, ExactField::one());
    assert_eq!(draw.controller_calls.len(), 1);
    let call = &draw.controller_calls[0];
    assert_eq!(call.name, "echo");
    assert_eq!(call.call, 0);
    assert_eq!(call.inputs, [draw.sampled_records[0]]);
    assert_eq!(call.outputs, call.inputs);
}
#[test]
fn original_reentrant_callbacks_and_exception_leave_fresh_control_values() {
    let mut configurations = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let holder = Arc::new(Mutex::new(None::<Arc<TaskEvaluator>>));
                let calls = Arc::new(Mutex::new(Vec::<Vec<bool>>::new()));
                let nested = Arc::new(Mutex::new(Vec::<(TaskEvaluation, SplitMix64)>::new()));
                let enabled = Arc::new(AtomicBool::new(true));
                let active = Arc::new(AtomicBool::new(false));
                let fail = Arc::new(AtomicBool::new(false));
                let (h, c, n, en, ac, fl) = (
                    holder.clone(),
                    calls.clone(),
                    nested.clone(),
                    enabled.clone(),
                    active.clone(),
                    fail.clone(),
                );
                let mut registry = ControllerRegistry::default();
                registry
                    .register("echo", "1", move |bits: &[bool]| {
                        c.lock().unwrap().push(bits.to_vec());
                        if fl.load(Ordering::SeqCst) {
                            return Err("intentional decoder failure".into());
                        }
                        if en.load(Ordering::SeqCst) && !ac.load(Ordering::SeqCst) {
                            ac.store(true, Ordering::SeqCst);
                            let evaluator = h.lock().unwrap().as_ref().unwrap().clone();
                            let mut random = SplitMix64::new(19);
                            let result = evaluator.sample(&mut random);
                            ac.store(false, Ordering::SeqCst);
                            match result {
                                Ok(draw) => n.lock().unwrap().push((draw, random)),
                                Err(e) => return Err(e.to_string()),
                            }
                        }
                        Ok(bits.to_vec())
                    })
                    .unwrap();
                let parse = ParseOptions {
                    controllers: registry.names(),
                    ..Default::default()
                };
                let physical=parse_circuit("H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 1\nM 1",&parse).unwrap();
                let modules = parse_task(
                    r#"{"sample_records":[0,1]}"#,
                    Path::new("."),
                    &TaskOptions::default(),
                )
                .unwrap()
                .load_modules(&physical, &parse)
                .unwrap();
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = causal;
                options.runtime.history_sampler = sampler;
                let evaluator =
                    Arc::new(TaskEvaluator::new(&physical, &modules, &registry, options).unwrap());
                *holder.lock().unwrap() = Some(evaluator.clone());
                let mut actual_rng = SplitMix64::new(6);
                let actual = evaluator.sample(&mut actual_rng).unwrap();
                check(&actual);
                let mut one_word = SplitMix64::new(6);
                one_word.next_u64();
                assert_eq!(actual_rng, one_word);
                assert_eq!(calls.lock().unwrap().len(), 2);
                assert_eq!(nested.lock().unwrap().len(), 1);
                assert!(!active.load(Ordering::SeqCst));
                enabled.store(false, Ordering::SeqCst);
                let mut reference_rng = SplitMix64::new(6);
                let reference = evaluator.sample(&mut reference_rng).unwrap();
                assert!(actual == reference);
                assert_eq!(actual_rng, reference_rng);
                let mut nested_rng = SplitMix64::new(19);
                let reference_nested = evaluator.sample(&mut nested_rng).unwrap();
                let (actual_nested, actual_nested_rng) = nested.lock().unwrap().pop().unwrap();
                check(&actual_nested);
                assert!(actual_nested == reference_nested);
                assert_eq!(actual_nested_rng, nested_rng);
                fail.store(true, Ordering::SeqCst);
                let mut failure_rng = SplitMix64::new(6);
                let Err(error) = evaluator.sample(&mut failure_rng) else {
                    panic!("declared callback exception was swallowed");
                };
                assert!(error.message.contains("intentional decoder failure"));
                assert_eq!(failure_rng, one_word);
                assert_eq!(calls.lock().unwrap().len(), 5);
                fail.store(false, Ordering::SeqCst);
                let mut recovered_rng = SplitMix64::new(6);
                let recovered = evaluator.sample(&mut recovered_rng).unwrap();
                assert!(actual == recovered);
                assert_eq!(actual_rng, recovered_rng);
                assert_eq!(calls.lock().unwrap().len(), 6);
                for bit in [false, true] {
                    let draw = evaluator
                        .evaluate_prescribed_history(&[], &[vec![bit], vec![bit, bit]])
                        .unwrap();
                    check(&draw);
                    assert_eq!(draw.sampled_records, [bit, bit]);
                }
                assert_eq!(calls.lock().unwrap().len(), 8);
                let log = calls.lock().unwrap();
                assert!(log.contains(&vec![false]) && log.contains(&vec![true]));
                drop(log);
                // Break the intentionally installed callback/evaluator ownership cycle.
                holder.lock().unwrap().take();
                assert_eq!(Arc::strong_count(&evaluator), 1);
                configurations += 1;
            }
        }
    }
    assert_eq!(configurations, 8);
}
