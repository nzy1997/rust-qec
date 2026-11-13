// SPDX-License-Identifier: MPL-2.0
// Complete VirtualCliffordFeedbackTests::test_linear_clifford_feedback_stays_coherent_and_enumeration_preserves_mass,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_virtual_clifford_feedback.py.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ScalarArithmetic;
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

fn field(s: &str) -> ExactField {
    FieldArithmetic::default()
        .from_rational(s.parse::<BigRational>().unwrap())
        .unwrap()
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-feedback-enumeration-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("plus.magna"), "H 0").unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn original_linear_feedback_is_coherent_and_enumeration_preserves_mass() {
    let files = Files::new();
    let mut configurations = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut registry = ControllerRegistry::default();
                registry
                    .register("and", "1", |bits: &[bool]| {
                        assert_eq!(bits.len(), 2);
                        Ok(vec![bits[0] & bits[1]])
                    })
                    .unwrap();
                let parse = ParseOptions {
                    controllers: registry.names(),
                    ..Default::default()
                };
                let build = |text: &str, output: usize| {
                    let physical = parse_circuit(text, &parse).unwrap();
                    let task = parse_task(
                        &format!(r#"{{"outputs":[{output}],"ideal_state":"plus.magna"}}"#),
                        &files.0,
                        &TaskOptions::default(),
                    )
                    .unwrap();
                    let modules = task.load_modules(&physical, &parse).unwrap();
                    let mut options = EvaluationOptions::default();
                    options.runtime.compile.compiler_strategy = compiler;
                    options.runtime.strategy = causal;
                    options.runtime.history_sampler = sampler;
                    TaskEvaluator::new(&physical, &modules, &registry, options).unwrap()
                };
                let mut options = TaskEnumerationOptions::default();
                options.noise.max_weight = 0;
                options.include_patterns = true;
                let linear = build("H 0 1\nM 0\nCS rec[-1] 1\nCH rec[-1] 1", 1);
                assert_eq!(linear.execution_route(), "linear_affine");
                assert!(!linear.sampled_history());
                let result = linear.enumerate_physical(&options).unwrap();
                assert_eq!(result.masses.accepted, field("1"));
                assert_eq!(result.masses.error, field("1/4"));
                assert_eq!(result.masses.target_success, Some(field("3/4")));
                assert_eq!(result, linear.enumerate_physical(&options).unwrap());
                for gate in ["CS", "CS_DAG", "CH"] {
                    let text = format!(
                        "H 0 1 2\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCONTROL[XOR] ctrl[0] rec[-2] -> ctrl[1]\n{gate} ctrl[1] 2"
                    );
                    let nonlinear = build(&text, 2);
                    assert!(nonlinear.sampled_history());
                    assert_eq!(
                        nonlinear.execution_route(),
                        match causal {
                            CausalStrategy::ColdPrefix => "causal_cold_prefix",
                            CausalStrategy::FixedTemplate => "causal_fixed_template",
                        }
                    );
                    let result = nonlinear.enumerate_physical(&options).unwrap();
                    assert_eq!(result.patterns, 1);
                    assert_eq!(result.histories, 4);
                    assert_eq!(
                        result.covered_probability.value(),
                        &"1".parse::<BigRational>().unwrap()
                    );
                    assert_eq!(
                        result.omitted_probability.value(),
                        &"0".parse::<BigRational>().unwrap()
                    );
                    assert_eq!(result.masses.accepted, field("1"));
                    assert_eq!(result.masses.error, field("1/8"));
                    assert_eq!(result.masses.target_success, Some(field("7/8")));
                    assert_eq!(result.full_error.lower, field("1/8"));
                    assert_eq!(result.full_error.upper, field("1/8"));
                    assert_eq!(
                        result.conditional_error_within_enumerated_sector,
                        Some(field("1/8"))
                    );
                    let pattern = &result.pattern_responses.as_ref().unwrap()[0];
                    assert!(pattern.choices.is_empty());
                    assert_eq!(pattern.histories, 4);
                    assert_eq!(pattern.response, result.masses);
                    assert_eq!(result, nonlinear.enumerate_physical(&options).unwrap());
                    for a in [false, true] {
                        for b in [false, true] {
                            let draw = nonlinear
                                .evaluate_prescribed_history(&[], &[vec![a, b]])
                                .unwrap();
                            let active = a && !b;
                            assert_eq!(draw.controller_calls.len(), 1);
                            assert_eq!(draw.controller_calls[0].inputs, [a, b]);
                            assert_eq!(draw.controller_calls[0].outputs, [a && b]);
                            assert_eq!(
                                draw.history_mass,
                                ScalarArithmetic::default().omega(0, -4).unwrap()
                            );
                            assert_eq!(draw.accepted, field("1"));
                            assert_eq!(draw.error, field(if active { "1/2" } else { "0" }));
                            assert_eq!(
                                draw.target_success,
                                Some(field(if active { "1/2" } else { "1" }))
                            );
                            assert_eq!(draw.noise_probability, "1".parse::<BigRational>().unwrap());
                        }
                    }
                }
                configurations += 1;
            }
        }
    }
    assert_eq!(configurations, 8);
}
