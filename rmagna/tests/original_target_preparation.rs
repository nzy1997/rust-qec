// SPDX-License-Identifier: MPL-2.0
// Six complete TargetPreparationTests methods, itpyi/magna
// 56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_target_preparation.py.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionMeter;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskError, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const QUADRATIC: &str = "H 0 1 2 3 4\nCCZ 0 3 4\nCCZ 1 2 4\nH 4";
fn scalar(n: i64, power: i64) -> ExactScalar {
    ScalarArithmetic::default()
        .scalar([n.into(), 0.into(), 0.into(), 0.into()], power)
        .unwrap()
}
fn field(s: &str) -> ExactField {
    FieldArithmetic::default()
        .from_rational(s.parse::<BigRational>().unwrap())
        .unwrap()
}
fn configs() -> impl Iterator<Item = (CompilerStrategy, QueryPreparation)> {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
    .into_iter()
    .flat_map(|c| {
        [
            QueryPreparation::Cold,
            QueryPreparation::FixedFibers(Default::default()),
        ]
        .into_iter()
        .map(move |p| (c, p))
    })
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-target-preparation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn evaluator(
        &self,
        physical: &str,
        target: &str,
        mut spec: Value,
        mut task: Value,
        config: (CompilerStrategy, QueryPreparation),
        sampling: (HistorySampler, usize),
    ) -> Result<TaskEvaluator, TaskError> {
        std::fs::write(self.0.join("target.magna"), target).unwrap();
        if spec.is_object() {
            spec["circuit"] = json!("target.magna");
        } else {
            spec = json!("target.magna");
        }
        task["ideal_state"] = spec;
        let parse = ParseOptions::default();
        let source = parse_circuit(physical, &parse).unwrap();
        let modules = parse_task(&task.to_string(), &self.0, &TaskOptions::default())?
            .load_modules(&source, &parse)?;
        let mut options = EvaluationOptions::default();
        options.runtime.compile.compiler_strategy = config.0;
        options.runtime.compile.probability_preparation = config.1;
        options.runtime.compile.density_preparation = config.1;
        options.runtime.history_sampler = sampling.0;
        options.runtime.sampling.max_terms = sampling.1;
        let meter = ExecutionMeter::default();
        let result = TaskEvaluator::new_with_costs(
            &source,
            &modules,
            &ControllerRegistry::default(),
            options,
            &NoiseSampling::Physical,
            &meter,
        )?;
        // Actual Rust pure-target and protocol preparations replace Native's
        // private reference-coordinate/resource-construction counter scope.
        assert_eq!(meter.snapshot().compilation.succeeded, 2);
        assert_eq!(meter.snapshot().compilation.failed, 0);
        assert_eq!(meter.snapshot().query.succeeded, 1);
        Ok(result)
    }
    fn ordinary(
        &self,
        physical: &str,
        target: &str,
        spec: Value,
        config: (CompilerStrategy, QueryPreparation),
    ) -> TaskEvaluator {
        match self.evaluator(
            physical,
            target,
            spec,
            json!({}),
            config,
            (HistorySampler::ExactMarginal, 256),
        ) {
            Ok(e) => e,
            Err(error) => panic!("valid original target rejected: {error}"),
        }
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn projected(branch: usize) -> Value {
    json!({"outputs":[0,1,2,3],"postselection":{"4":branch}})
}
#[test]
fn original_quadratic_target_branches_match_all_basis_probabilities() {
    let files = Files::new();
    for config in configs() {
        for (branch, count, norm) in [(0, 10, scalar(5, -3)), (1, 6, scalar(3, -3))] {
            let mut support = 0;
            for word in 0..16 {
                let mut physical = String::from("I 0 1 2 3\n");
                for q in 0..4 {
                    if word & (1 << q) != 0 {
                        physical.push_str(&format!("X {q}\n"));
                    }
                }
                let evaluator = files.ordinary(&physical, QUADRATIC, projected(branch), config);
                let draw = evaluator.sample(&mut SplitMix64::new(0)).unwrap();
                let parity =
                    ((word & 1) * ((word >> 3) & 1)) ^ (((word >> 1) & 1) * ((word >> 2) & 1));
                let selected = parity == branch;
                support += usize::from(selected);
                let overlap = if selected {
                    format!("1/{count}")
                } else {
                    "0".into()
                };
                assert_eq!(draw.accepted, ExactField::one());
                assert_eq!(draw.target_normalization, Some(norm.clone()));
                assert_eq!(draw.history_mass, ExactScalar::one());
                assert_eq!(
                    draw.raw_target_success,
                    Some(if selected {
                        scalar(1, -4)
                    } else {
                        ExactScalar::zero()
                    })
                );
                assert_eq!(draw.target_success, Some(field(&overlap)));
                assert_eq!(
                    draw.error,
                    FieldArithmetic::default()
                        .subtract(&ExactField::one(), &field(&overlap))
                        .unwrap()
                );
                assert_eq!(draw.raw_acceptance, ExactScalar::one());
                assert_eq!(draw.network_compilations, 0);
                let stats = evaluator.prepared_density_statistics().unwrap().unwrap();
                let _ = evaluator.sample(&mut SplitMix64::new(1)).unwrap();
                assert_eq!(
                    stats,
                    evaluator.prepared_density_statistics().unwrap().unwrap()
                );
            }
            assert_eq!(support, count);
        }
    }
}
#[test]
fn original_partial_trace_and_encoded_target_output_order() {
    let files = Files::new();
    for config in configs() {
        for physical in ["I 0", "X 0", "H 0\nT 0"] {
            let evaluator = files.ordinary(physical, "H 0\nCX 0 1", json!({"outputs":[1]}), config);
            let draw = evaluator.sample(&mut SplitMix64::new(1)).unwrap();
            assert_eq!(draw.accepted, ExactField::one());
            assert_eq!(draw.error, field("1/2"));
            assert_eq!(draw.target_success, Some(field("1/2")));
            assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
        }
        let evaluator = files.ordinary(
            "H 1\nT 1\nCX 1 0",
            "H 0 1\nT 1\nCX 1 2",
            json!({"outputs":[2,1]}),
            config,
        );
        let draw = evaluator.sample(&mut SplitMix64::new(2)).unwrap();
        assert_eq!(draw.accepted, ExactField::one());
        assert_eq!(draw.error, ExactField::zero());
        assert_eq!(draw.target_success, Some(ExactField::one()));
        assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
    }
}
#[test]
fn original_full_output_target_preserves_acceptance_and_omitted_sector_bounds() {
    let files = Files::new();
    for config in configs() {
        let evaluator = files.ordinary(
            "I 0 1 2 3\nH 4",
            "H 0 1 2 3 5\nCCZ 0 3 5\nCCZ 1 2 5\nH 5",
            json!({"outputs":[0,1,2,3,4],"postselection":{"5":0}}),
            config,
        );
        let mut options = TaskEnumerationOptions::default();
        options.noise.max_weight = 0;
        options.include_patterns = true;
        let result = evaluator.enumerate_physical(&options).unwrap();
        assert_eq!(result.masses.accepted, ExactField::one());
        assert_eq!(result.masses.error, field("19/20"));
        assert_eq!(result.masses.target_success, Some(field("1/20")));
        assert_eq!(result.target_normalization, Some(scalar(5, -3)));
        assert_eq!(result.patterns, 1);
        let noisy = files.ordinary("X 3\nX_ERROR(1/4) 0", QUADRATIC, projected(0), config);
        options.noise.max_weight = 1;
        let complete = noisy.enumerate_physical(&options).unwrap();
        assert_eq!(complete.masses.accepted, ExactField::one());
        assert_eq!(complete.masses.error, field("37/40"));
        assert_eq!(
            complete.covered_probability.value(),
            &"1".parse::<BigRational>().unwrap()
        );
        assert_eq!(
            complete.omitted_probability.value(),
            &"0".parse::<BigRational>().unwrap()
        );
        assert_eq!(complete.patterns, 2);
        options.noise.max_weight = 0;
        let partial = noisy.enumerate_physical(&options).unwrap();
        assert_eq!(partial.masses.accepted, field("3/4"));
        assert_eq!(partial.masses.error, field("27/40"));
        assert_eq!(partial.masses.target_success, Some(field("3/40")));
        assert_eq!(
            partial.covered_probability.value(),
            &"3/4".parse::<BigRational>().unwrap()
        );
        assert_eq!(
            partial.omitted_probability.value(),
            &"1/4".parse::<BigRational>().unwrap()
        );
        assert_eq!(partial.full_error.lower, field("27/40"));
        assert_eq!(partial.full_error.upper, field("37/40"));
        assert_eq!(partial.target_normalization, complete.target_normalization);
        assert_eq!(partial.patterns, 1);
    }
}
#[test]
fn original_invalid_and_zero_target_boundaries_reject() {
    let files = Files::new();
    for config in configs() {
        for (target, spec) in [
            ("I 0 1", json!({"outputs":[0],"postselection":{"1":1}})),
            ("I 0 1", json!({"outputs":[0],"postselection":{"0":0}})),
            ("I 0 1", json!({"outputs":[2]})),
            ("H 0\nM 0", json!({"outputs":[0]})),
        ] {
            let error = match files.evaluator(
                "I 0",
                target,
                spec,
                json!({}),
                config,
                (HistorySampler::ExactMarginal, 256),
            ) {
                Err(e) => e,
                Ok(_) => panic!("invalid or zero original boundary was accepted"),
            };
            assert!(error.field.starts_with("ideal_state"), "{error}");
        }
    }
}
#[test]
fn original_encoded_target_retains_full_physical_output_and_rejects_target_checks() {
    let files = Files::new();
    for config in configs() {
        let evaluator = files.ordinary(
            "H 0\nCX 0 1\nX_ERROR(1/4) 1",
            "H 0\nCX 0 1",
            Value::Null,
            config,
        );
        let mut options = TaskEnumerationOptions::default();
        options.noise.max_weight = 1;
        let result = evaluator.enumerate_physical(&options).unwrap();
        assert_eq!(result.masses.accepted, ExactField::one());
        assert_eq!(result.masses.error, field("1/4"));
        for (gates, error) in [("", "0"), ("X 1", "1"), ("Z 0\nZ 1", "0")] {
            let evaluator = files.ordinary(
                &format!("H 0\nCX 0 1\n{gates}"),
                "H 0\nCX 0 1",
                Value::Null,
                config,
            );
            let draw = evaluator.sample(&mut SplitMix64::new(1)).unwrap();
            assert_eq!(draw.accepted, ExactField::one());
            assert_eq!(draw.error, field(error));
            assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
        }
        let error = match files.evaluator(
            "H 0\nCX 0 1",
            "H 0\nCX 0 1",
            Value::Null,
            json!({"target_checks":[[0]]}),
            config,
            (HistorySampler::ExactMarginal, 256),
        ) {
            Err(e) => e,
            Ok(_) => panic!("unknown original target_checks accepted"),
        };
        assert!(error.message.contains("unknown") && error.message.contains("target_checks"));
    }
}
#[test]
fn original_cancelled_magic_preserves_one_sided_pauli_updates_and_one_term_queries() {
    let files = Files::new();
    let cases = [
        (1, "H 0\nT 0", ["1", "1/2", "1/2", "0"]),
        (2, "H 0 1\nCS 0 1", ["1", "1/4", "1/4", "0"]),
        (3, "H 0 1 2\nCCZ 0 1 2", ["1", "1/4", "0", "0"]),
    ];
    for config in configs() {
        for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            for (width, preparation, overlaps) in cases {
                for history in [false, true] {
                    for (pauli, overlap) in ['I', 'X', 'Y', 'Z'].into_iter().zip(overlaps) {
                        let mut physical = preparation.to_string();
                        if pauli != 'I' {
                            physical.push_str(&format!("\n{pauli}_ERROR(1) 0"));
                        }
                        let mut task = json!({"outputs":(0..width).collect::<Vec<_>>()});
                        if history {
                            physical.push_str(&format!("\nH {width}\nM {width}"));
                            task["sample_records"] = json!([0]);
                        }
                        let evaluator = files
                            .evaluator(
                                &physical,
                                preparation,
                                Value::Null,
                                task,
                                config,
                                (sampler, 1),
                            )
                            .unwrap();
                        let mut warm = None;
                        for seed in 5..9 {
                            let draw = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
                            assert_eq!(draw.accepted, ExactField::one());
                            assert_eq!(draw.target_success, Some(field(overlap)));
                            assert_eq!(
                                draw.error,
                                FieldArithmetic::default()
                                    .subtract(&ExactField::one(), &field(overlap))
                                    .unwrap()
                            );
                            assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                            assert_eq!(
                                draw.history_mass,
                                if history {
                                    scalar(1, -1)
                                } else {
                                    ExactScalar::one()
                                }
                            );
                            assert_eq!(draw.sampled_records.len(), usize::from(history));
                            assert_eq!(draw.network_compilations, 0);
                            assert_eq!(draw.acceptance_resources.quotient_rank, 0);
                            assert!(draw.acceptance_resources.terms <= 1);
                            let target = draw.target_resources.unwrap();
                            assert_eq!(target.quotient_rank, 0);
                            assert!(target.terms <= 1);
                            let stats = evaluator.prepared_density_statistics().unwrap().unwrap();
                            if let Some(previous) = warm {
                                assert_eq!(stats, previous);
                            } else {
                                warm = Some(stats);
                            }
                        }
                    }
                }
            }
        }
    }
}
