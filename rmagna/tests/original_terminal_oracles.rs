// SPDX-License-Identifier: MPL-2.0
// Complete first four TerminalEvaluationTests methods from itpyi/magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f, tests/core/test_terminal_evaluation.py.
// Public loaded modules carry width/ancilla metadata; exact task draws and
// two-draw batches translate native result fields without ABI/RNG identity.
use num_traits::ToPrimitive;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::fibers::FiberOptions;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::{ProbabilityPreparation, QueryPreparation};
use rmagna::noise::SplitMix64;
use rmagna::numeric::NumericOptions;
use rmagna::runtime::ControllerRegistry;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, RunStatus, TaskEvaluation, TaskEvaluator, TaskOptions, TaskRunOptions,
    export_batch_metrics, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-original-terminal-oracles-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    #[allow(clippy::too_many_arguments)]
    fn check(
        &self,
        physical: &str,
        target: &str,
        recovery: Option<&str>,
        outputs: Option<&[usize]>,
        expected: &ExactScalar,
        compiler: CompilerStrategy,
        compact: bool,
    ) -> (TaskEvaluation, usize, usize) {
        std::fs::write(self.0.join("target.magna"), target).unwrap();
        let mut task = json!({"ideal_state":"target.magna"});
        if let Some(outputs) = outputs {
            task["outputs"] = json!(outputs);
        }
        if let Some(recovery) = recovery {
            std::fs::write(self.0.join("recovery.magna"), recovery).unwrap();
            task["recovery"] = json!("recovery.magna");
        }
        let parse = ParseOptions::default();
        let physical = parse_circuit(physical, &parse).unwrap();
        let spec = parse_task(&task.to_string(), &self.0, &TaskOptions::default()).unwrap();
        let modules = spec.load_modules(&physical, &parse).unwrap();
        let width = modules.ideal_state().unwrap().circuit().num_qubits();
        let ancillas = modules.recovery().map_or(0, |r| {
            r.num_qubits()
                .saturating_sub(modules.task().outputs().len())
        });
        let mut options = EvaluationOptions::default();
        options.runtime.compile.compiler_strategy = compiler;
        if compact {
            let fiber = FiberOptions {
                max_search_rank: 2,
                ..Default::default()
            };
            options.runtime.compile.probability_preparation =
                ProbabilityPreparation::FixedFibers(fiber);
            options.runtime.compile.density_preparation = QueryPreparation::FixedFibers(fiber);
            options.runtime.sampling.max_terms = 4;
        }
        let evaluator =
            TaskEvaluator::new(&physical, &modules, &ControllerRegistry::default(), options)
                .unwrap();
        let draw = evaluator.sample(&mut SplitMix64::new(7)).unwrap();
        let arithmetic = ScalarArithmetic::default();
        let field = FieldArithmetic::default().from_scalar(expected).unwrap();
        assert_eq!(draw.accepted, ExactField::one());
        assert_eq!(draw.raw_acceptance, ExactScalar::one());
        assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
        assert_eq!(&draw.raw_error, expected);
        assert_eq!(draw.error, field);
        assert_eq!(
            draw.raw_target_success,
            Some(arithmetic.subtract(&ExactScalar::one(), expected).unwrap())
        );
        assert!(draw.sampled_records.is_empty());
        assert!(draw.controller_calls.is_empty());
        let batch = evaluator
            .run_batch(&TaskRunOptions {
                shots: 2,
                seed: 19,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(batch.status, RunStatus::Complete);
        assert_eq!(
            (batch.completed, batch.failed, batch.not_attempted),
            (2, 0, 0)
        );
        let summary = batch.statistics.as_ref().unwrap();
        assert_eq!(summary.accepted, ExactField::one());
        assert_eq!(summary.error, Some(field.clone()));
        assert_eq!(summary.conditional_error, Some(field));
        assert_eq!(summary.variances.error, Some(ExactField::zero()));
        assert_eq!(
            summary.variances.conditional_error,
            Some(ExactField::zero())
        );
        let exported = export_batch_metrics(&batch, NumericOptions::default()).unwrap();
        assert_eq!(exported["estimates"]["acceptance"], 1.0);
        let coefficients = expected
            .coefficients()
            .each_ref()
            .map(|c| c.to_f64().unwrap());
        let want = (coefficients[0]
            + (coefficients[1] - coefficients[3]) * std::f64::consts::FRAC_1_SQRT_2)
            * 2.0_f64.powi(expected.power().try_into().unwrap());
        for key in ["error", "conditional_error"] {
            let actual = exported["estimates"][key].as_f64().unwrap();
            assert!(actual.is_finite() && (actual - want).abs() < 1e-11);
            assert_eq!(exported["standard_errors"][key], 0.0);
        }
        (draw, width, ancillas)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scalar(c: [i32; 4], power: i64) -> ExactScalar {
    ScalarArithmetic::default()
        .scalar(c.map(Into::into), power)
        .unwrap()
}
fn compilers() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
#[test]
fn original_t_targets_and_empty_recovery() {
    let files = Files::new();
    let mut cases = 0;
    for compiler in compilers() {
        for (suffix, error) in [
            ("", ExactScalar::zero()),
            ("\nZ 0", ExactScalar::one()),
            ("\nS 0", scalar([1, 0, 0, 0], -1)),
        ] {
            for recovery in [None, Some(""), Some("I 0")] {
                let (_, width, ancillas) = files.check(
                    &format!("H 0\nT 0{suffix}"),
                    "H 0\nT 0",
                    recovery,
                    None,
                    &error,
                    compiler,
                    false,
                );
                assert_eq!(width, 1);
                assert_eq!(ancillas, 0);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 18);
}
#[test]
fn original_target_inverse_against_independent_dense_amplitudes() {
    let files = Files::new();
    let target = "H 0 1\nT 0 1\nY 0\nCX 0 1\nCS 0 1\nCH 0 1";
    // Literal matrix probabilities: 1/4, 1/4+sqrt(2)/8, 1/4,
    // 1/4-sqrt(2)/8. Qubit zero is the low bit; errors are one minus these.
    let errors = [
        scalar([3, 0, 0, 0], -2),
        scalar([6, -1, 0, 1], -3),
        scalar([3, 0, 0, 0], -2),
        scalar([6, 1, 0, -1], -3),
    ];
    let mut cases = 0;
    for compiler in compilers() {
        for (word, error) in errors.iter().enumerate() {
            let mut physical = String::from("I 0 1\n");
            for q in 0..2 {
                if word >> q & 1 != 0 {
                    physical.push_str(&format!("X {q}\n"));
                }
            }
            let (_, width, ancillas) =
                files.check(&physical, target, None, None, error, compiler, false);
            assert_eq!(width, 2);
            assert_eq!(ancillas, 0);
            cases += 1;
        }
    }
    assert_eq!(cases, 8);
}
#[test]
fn original_wide_clifford_encoded_t_target_uses_compact_native_boundary() {
    let files = Files::new();
    let mut cases = 0;
    for compiler in compilers() {
        for width in [3, 65] {
            let mut target = String::from("H 0\nT 0\n");
            for q in 1..width {
                target.push_str(&format!("CX 0 {q}\n"));
            }
            let physical = format!("{target}\nX_ERROR(1) 1");
            let (draw, actual_width, ancillas) = files.check(
                &physical,
                &target,
                Some("MPP Z0*Z1\nCX rec[-1] 1"),
                None,
                &ExactScalar::zero(),
                compiler,
                true,
            );
            assert_eq!(actual_width, width);
            assert_eq!(ancillas, 0);
            let resources = draw.target_resources.unwrap();
            assert!(resources.quotient_rank <= 2);
            assert!(resources.terms <= 4);
            cases += 1;
        }
    }
    assert_eq!(cases, 4);
}
#[test]
fn original_output_mapping_and_unobserved_environment() {
    let files = Files::new();
    let half = scalar([1, 0, 0, 0], -1);
    let mut cases = 0;
    for compiler in compilers() {
        for (physical, target, recovery, outputs, error, ancillas) in [
            ("X 2", "X 0\nI 1", None, &[2, 0][..], ExactScalar::zero(), 0),
            ("X 2", "X 0\nI 1", None, &[0, 2][..], ExactScalar::one(), 0),
            ("H 0\nCX 0 1", "I 0", None, &[0][..], half.clone(), 0),
            ("H 2", "H 0", Some("CX 0 1"), &[2][..], half.clone(), 1),
        ] {
            let (_, _, actual) = files.check(
                physical,
                target,
                recovery,
                Some(outputs),
                &error,
                compiler,
                false,
            );
            assert_eq!(actual, ancillas);
            cases += 1;
        }
    }
    assert_eq!(cases, 8);
}
