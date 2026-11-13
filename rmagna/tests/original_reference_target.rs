// SPDX-License-Identifier: MPL-2.0
// Two complete ReferenceLayerCompilerTests target-boundary methods at
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_reference_layer_compiler.py.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionMeter;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::{CompileOptions, FaultSelection, QueryPreparation, compile_affine};
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::runtime::ControllerRegistry;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{EvaluationOptions, TaskEvaluation, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-reference-target-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn root_mass(sign: i64) -> ExactScalar {
    ScalarArithmetic::default()
        .scalar([2, sign, 0, -sign].map(Into::into), -2)
        .unwrap()
}
fn check(draw: &TaskEvaluation, target: &ExactScalar, probability: &str) {
    let a = ScalarArithmetic::default();
    let f = FieldArithmetic::default();
    let error = a.add(&ExactScalar::one(), &target.negated()).unwrap();
    assert_eq!(draw.raw_acceptance, ExactScalar::one());
    assert_eq!(draw.raw_target_success, Some(target.clone()));
    assert_eq!(draw.raw_error, error);
    assert_eq!(draw.accepted, ExactField::one());
    assert_eq!(draw.target_success, Some(f.from_scalar(target).unwrap()));
    assert_eq!(draw.error, f.from_scalar(&error).unwrap());
    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
    assert_eq!(draw.history_mass, ExactScalar::one());
    assert_eq!(draw.history_weight, ExactField::one());
    assert_eq!(
        draw.noise_probability,
        probability.parse::<BigRational>().unwrap()
    );
    assert_eq!(
        draw.noise_likelihood_ratio,
        "1".parse::<BigRational>().unwrap()
    );
    assert!(draw.controller_calls.is_empty());
    assert!(draw.sampled_records.is_empty());
    assert!(draw.moments.is_empty());
    assert_eq!(draw.network_compilations, 0);
    assert_eq!(draw.history_queries, 0);
}
fn run_contract(physical: &str, target: &str, has_noise: bool) {
    let files = Files::new();
    std::fs::write(files.0.join("target.magna"), target).unwrap();
    let parse = ParseOptions::default();
    let source = parse_circuit(physical, &parse).unwrap();
    let modules = parse_task(
        r#"{"outputs":[0],"ideal_state":"target.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&source, &parse)
    .unwrap();
    let a = ScalarArithmetic::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for preparation in [
            QueryPreparation::Cold,
            QueryPreparation::FixedFibers(Default::default()),
        ] {
            let compile = CompileOptions {
                compiler_strategy: compiler,
                density_preparation: preparation,
                ..Default::default()
            };
            let bare = compile_affine(&source, &compile, &a).unwrap();
            let mut options = EvaluationOptions::default();
            options.runtime.compile = compile;
            let meter = ExecutionMeter::default();
            let evaluator = TaskEvaluator::new_with_costs(
                &source,
                &modules,
                &ControllerRegistry::default(),
                options,
                &NoiseSampling::Physical,
                &meter,
            )
            .unwrap();
            assert_eq!(
                evaluator.compilation(),
                bare.compilation(),
                "pure-target preparation preserves protocol diagnostics"
            );
            if compiler == CompilerStrategy::SharedCommutingLayer {
                assert_eq!(
                    evaluator.compilation().reference_layer,
                    has_noise.then_some(0)
                );
            }
            // Rust constructs a pure target and protocol, then prepares a
            // doubled-density overlap family. Native target suffix counters
            // describe different layers and ancillas; no equal scope claimed.
            assert_eq!(meter.snapshot().compilation.attempted, 2);
            assert_eq!(meter.snapshot().compilation.succeeded, 2);
            assert_eq!(meter.snapshot().compilation.failed, 0);
            assert_eq!(meter.snapshot().query.attempted, 1);
            assert_eq!(meter.snapshot().query.succeeded, 1);
            for fault in [false, true] {
                if fault && !has_noise {
                    continue;
                }
                let selections = if fault {
                    vec![FaultSelection {
                        site: 0,
                        alternative: 0,
                    }]
                } else {
                    vec![]
                };
                let draw = evaluator
                    .sample_prescribed(&selections, &mut SplitMix64::new(4))
                    .unwrap();
                let mass = if !has_noise || fault {
                    root_mass(1)
                } else {
                    root_mass(-1)
                };
                check(
                    &draw,
                    &mass,
                    if has_noise {
                        if fault { "1/4" } else { "3/4" }
                    } else {
                        "1"
                    },
                );
                let stats = evaluator.prepared_density_statistics().unwrap().unwrap();
                let expected = usize::from(!matches!(preparation, QueryPreparation::Cold));
                assert_eq!(stats.cached_queries, expected);
                assert_eq!(stats.fixed_resources, expected);
            }
            let warm = evaluator.prepared_density_statistics().unwrap();
            let mut seen = [false; 2];
            for seed in [3, 4, 16, 17, 18, 19, 20, 21] {
                let draw = evaluator
                    .sample_with_costs(&mut SplitMix64::new(seed), &meter)
                    .unwrap();
                let repeated = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
                assert!(draw == repeated);
                let fault =
                    has_noise && draw.noise_probability == "1/4".parse::<BigRational>().unwrap();
                seen[usize::from(fault)] = true;
                check(
                    &draw,
                    &if !has_noise || fault {
                        root_mass(1)
                    } else {
                        root_mass(-1)
                    },
                    if has_noise {
                        if fault { "1/4" } else { "3/4" }
                    } else {
                        "1"
                    },
                );
                assert_eq!(evaluator.prepared_density_statistics().unwrap(), warm);
            }
            assert!(seen[0]);
            if has_noise {
                assert!(seen[1]);
            }
            assert_eq!(meter.snapshot().compilation.attempted, 2);
            assert_eq!(meter.snapshot().compilation.succeeded, 2);
            assert_eq!(
                meter.snapshot().query.attempted,
                17,
                "one target normalization plus acceptance/target for eight draws"
            );
            assert_eq!(meter.snapshot().query.succeeded, 17);
            assert_eq!(meter.snapshot().query.failed, 0);
            assert_eq!(evaluator.compilation(), bare.compilation());
        }
    }
}
#[test]
fn original_target_suffix_preserves_protocol_reference_and_reusable_overlap() {
    run_contract("H 0\nT 0\nX_ERROR(1/4) 0", "H 0\nT 0\nT 0\nH 0", true);
}
#[test]
fn original_clifford_parent_retains_reference_with_first_target_layer() {
    run_contract("H 0", "H 0\nT 0", false);
}
