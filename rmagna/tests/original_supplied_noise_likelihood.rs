// SPDX-License-Identifier: MPL-2.0
// Complete original ConditionalNoiseTests.test_importance_weights_only_tilt_unconditional_channels
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_conditional_noise.py; independent Rust public-API witness.
use num_rational::BigRational;
use rmagna::circuit::{Basis, Operation, ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::FaultSelection;
use rmagna::noise::{
    NoiseSampling, OriginalNoiseAction, OriginalNoiseOptions, SplitMix64, original_noise_categories,
};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ExactScalar;
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;

#[test]
fn original_importance_weights_only_tilt_unconditional_channels() {
    let dir = std::env::temp_dir().join(format!(
        "rmagna-original-supplied-noise-{}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    struct RemoveOnDrop(std::path::PathBuf);
    impl Drop for RemoveOnDrop {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let files = RemoveOnDrop(dir);
    std::fs::write(files.0.join("target.magna"), "I 0").unwrap();
    let parse = ParseOptions::default();
    let circuit =
        parse_circuit("H 0\nM 0\nX_ERROR(1/3) rec[-1] 1\nZ_ERROR(1/10) 1", &parse).unwrap();
    let task = parse_task(
        r#"{"outputs":[1],"ideal_state":"target.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = task.load_modules(&circuit, &parse).unwrap();
    let registry = ControllerRegistry::default();
    let sampling = NoiseSampling::Importance {
        proposal_scale: BigRational::from_integer(5.into()),
    };
    let likelihood = BigRational::new(9.into(), 5.into());
    let field = FieldArithmetic::default();
    let weighted = field.from_rational(likelihood.clone()).unwrap();

    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = strategy;
                options.runtime.history_sampler = sampler;
                let evaluator = TaskEvaluator::new_with_sampling(
                    &circuit, &modules, &registry, options, &sampling,
                )
                .unwrap();
                let summary = evaluator.scaled_proposal_summary().unwrap();
                assert_eq!(summary.excluded_categories, 1);
                assert_eq!(summary.scaled_categories, 1);

                let error_ids: Vec<_> = circuit
                    .operations()
                    .iter()
                    .filter_map(|op| {
                        matches!(op.operation, Operation::Noise { .. }).then_some(op.id)
                    })
                    .collect();
                assert_eq!(error_ids.len(), 2);
                let categories =
                    original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
                let guarded = categories
                    .iter()
                    .find(|category| category.source_operations[0].operation == error_ids[0])
                    .unwrap();
                assert!(guarded.guard.is_some());
                assert_eq!(guarded.channels.len(), 1);
                assert_eq!(
                    guarded.channels[0].action,
                    OriginalNoiseAction::Pauli {
                        factors: vec![(1, Basis::X)]
                    }
                );
                let faults = [FaultSelection {
                    site: guarded.internal_site,
                    alternative: 0,
                }];
                let half = options.scalars.omega(0, -2).unwrap();
                let zero = ExactScalar::zero();
                let mut seen = [false; 2];
                for seed in 0..16 {
                    let draw = evaluator
                        .sample_supplied_noise(&faults, &likelihood, &mut SplitMix64::new(seed))
                        .unwrap();
                    assert_eq!(draw.noise_likelihood_ratio, likelihood);
                    assert_eq!(draw.history_mass, half);
                    assert_eq!(draw.raw_acceptance, half);
                    assert_eq!(draw.accepted, weighted);
                    let active = draw.raw_error == half;
                    seen[usize::from(active)] = true;
                    if active {
                        assert_eq!(draw.raw_target_success, Some(zero.clone()));
                        assert_eq!(draw.error, weighted);
                        assert_eq!(draw.target_success, Some(ExactField::zero()));
                        assert_eq!(
                            draw.noise_probability,
                            BigRational::new(3.into(), 10.into())
                        );
                    } else {
                        assert_eq!(draw.raw_error, zero);
                        assert_eq!(draw.raw_target_success, Some(half.clone()));
                        assert_eq!(draw.error, ExactField::zero());
                        assert_eq!(draw.target_success, Some(weighted.clone()));
                        assert_eq!(
                            draw.noise_probability,
                            BigRational::new(9.into(), 10.into())
                        );
                    }
                }
                assert_eq!(seen, [true, true]);
            }
        }
    }
}
