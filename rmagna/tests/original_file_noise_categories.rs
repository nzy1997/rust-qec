// SPDX-License-Identifier: MPL-2.0
// Original FileApiTests conditional-noise and exclusive/nonuniform-channel
// methods from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_file_api.py, exercised through the Rust public file/task APIs.

use num_rational::BigRational;
use rmagna::circuit::{Basis, BitExpr, Circuit, NoiseKind, Operation, ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileTask, FileTaskOptions};
use rmagna::instrument::{CompileOptions, InstrumentError, compile_affine};
use rmagna::noise::{
    NoiseModel, NoiseOptions, NoiseSampler, NoiseSampling, OriginalNoiseAction,
    OriginalNoiseOptions, RandomSource, SplitMix64, original_noise_categories,
};
use rmagna::probability::Probability;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, LoadedModules, RunStatus, TaskEnumerationOptions, TaskEvaluator,
    TaskOptions, TaskRunOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct ZeroTarget(PathBuf);

impl ZeroTarget {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "rmagna-original-file-noise-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(
            directory.join("zero.magna"),
            "# The selected quantum outputs have the all-zero ideal target.\n",
        )
        .unwrap();
        Self(directory)
    }

    fn load(&self, source: &str) -> (Circuit, LoadedModules) {
        let parse = ParseOptions::default();
        let circuit = parse_circuit(source, &parse).unwrap();
        let modules = parse_task(
            r#"{"outputs":[0],"ideal_state":"zero.magna"}"#,
            &self.0,
            &TaskOptions::default(),
        )
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
        (circuit, modules)
    }
}

impl Drop for ZeroTarget {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct NoEntropy;

impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("the forced original channel must not request entropy")
    }
}

fn ratio(numerator: i64, denominator: i64) -> BigRational {
    BigRational::new(numerator.into(), denominator.into())
}

fn field(numerator: i64, denominator: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(ratio(numerator, denominator))
        .unwrap()
}

fn assert_file_error(
    physical: &Circuit,
    modules: &LoadedModules,
    options: EvaluationOptions,
    expected: i64,
) {
    let file = FileTask::from_loaded(
        physical.clone(),
        modules.clone(),
        &ControllerRegistry::default(),
        NoiseSampling::Physical,
        FileTaskOptions {
            evaluation: options,
            ..Default::default()
        },
        BTreeMap::new(),
    )
    .unwrap();
    let report = file.run(&TaskRunOptions::default()).unwrap();
    let result = report.raw_result();
    assert_eq!(result.status, RunStatus::Complete);
    assert_eq!(result.completed, 1);
    assert_eq!(result.failed, 0);
    let statistics = result.statistics.as_ref().unwrap();
    assert_eq!(statistics.accepted, ExactField::one());
    assert_eq!(statistics.error, Some(field(expected, 1)));
    assert_eq!(statistics.conditional_error, Some(field(expected, 1)));
    let json = report.to_json().unwrap();
    assert_eq!(json["status"], "complete");
    assert_eq!(json["completed"], 1);
    assert_eq!(json["estimates"]["acceptance"], 1.0);
    assert_eq!(json["estimates"]["error"], expected as f64);
    assert_eq!(json["estimates"]["conditional_error"], expected as f64);

    let evaluation = TaskEvaluator::new(physical, modules, &ControllerRegistry::default(), options)
        .unwrap()
        .sample(&mut NoEntropy)
        .unwrap();
    assert_eq!(evaluation.accepted, ExactField::one());
    assert_eq!(evaluation.error, field(expected, 1));
    assert_eq!(evaluation.target_success, Some(field(1 - expected, 1)));
    assert_eq!(evaluation.raw_acceptance, ExactScalar::one());
    assert_eq!(evaluation.history_mass, ExactScalar::one());
    assert_eq!(evaluation.history_weight, ExactField::one());
    assert_eq!(evaluation.noise_likelihood_ratio, ratio(1, 1));
    assert!(evaluation.sampled_records.is_empty());
}

#[test]
fn original_conditional_noise_is_one_physical_channel() {
    let target = ZeroTarget::new();
    let mut configurations = 0;
    for bit in [0, 1] {
        for guarded in [false, true] {
            let guard = if guarded { "ctrl[0] " } else { "" };
            let source =
                format!("CONTROL[XOR]({bit}) -> ctrl[0]\nCX ctrl[0] 0\nX_ERROR(1) {guard}0\nM 0");
            let (physical, modules) = target.load(&source);
            let originals =
                original_noise_categories(&physical, &OriginalNoiseOptions::default()).unwrap();
            assert_eq!(originals.len(), 1);
            let category = &originals[0];
            assert_eq!(category.internal_site, 0);
            assert_eq!(category.source_operations.len(), 1);
            assert_eq!(category.source_operations[0].operation, 2);
            assert_eq!(category.channels.len(), 1);
            assert_eq!(category.channels[0].probability, Probability::one());
            assert_eq!(category.identity_probability, Probability::zero());
            assert_eq!(
                category.channels[0].action,
                OriginalNoiseAction::Pauli {
                    factors: vec![(0, Basis::X)]
                }
            );
            assert_eq!(
                category.guard,
                guarded.then_some(BitExpr {
                    constant: bit == 1,
                    ..Default::default()
                })
            );
            assert_eq!(
                physical
                    .operations()
                    .iter()
                    .filter(|op| matches!(
                        op.operation,
                        Operation::Noise {
                            kind: NoiseKind::X,
                            ..
                        }
                    ))
                    .count(),
                1
            );
            let expected = bit ^ if guarded { bit } else { 1 };
            for compiler in [
                CompilerStrategy::ParityCopies,
                CompilerStrategy::SharedCommutingLayer,
            ] {
                let compiled = compile_affine(
                    &physical,
                    &CompileOptions {
                        compiler_strategy: compiler,
                        ..Default::default()
                    },
                    &ScalarArithmetic::default(),
                );
                if guarded {
                    assert!(matches!(
                        compiled,
                        Err(InstrumentError::NeedsHistory { operation: 2 })
                    ));
                } else {
                    let compiled = compiled.unwrap();
                    assert_eq!(compiled.noise_sites().len(), 2);
                    assert_eq!(compiled.noise_sites()[0].source_operations(), &[2]);
                    assert_eq!(compiled.noise_sites()[1].source_operations(), &[3]);
                    assert_eq!(
                        compiled.noise_sites()[1].alternatives()[0].probability(),
                        &Probability::zero()
                    );
                }
                for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
                    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                        let mut options = EvaluationOptions::default();
                        options.runtime.compile.compiler_strategy = compiler;
                        options.runtime.strategy = causal;
                        options.runtime.history_sampler = sampler;
                        assert_file_error(&physical, &modules, options, expected);
                        assert_eq!(physical.to_text(), source);
                        configurations += 1;
                    }
                }
            }
        }
    }
    assert_eq!(configurations, 32);
}

#[test]
fn original_correlated_alternatives_and_nonuniform_pauli_identity() {
    let target = ZeroTarget::new();
    let mut forced_configurations = 0;
    let mut pauli_draws = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let mut options = EvaluationOptions::default();
        options.runtime.compile.compiler_strategy = compiler;
        for first in [1, 0] {
            let source = format!("CORRELATED_ERROR({first}) X0\nELSE_CORRELATED_ERROR(1) X0\nM 0");
            let (physical, modules) = target.load(&source);
            let originals =
                original_noise_categories(&physical, &OriginalNoiseOptions::default()).unwrap();
            assert_eq!(originals.len(), 1);
            let category = &originals[0];
            assert_eq!(category.source_operations.len(), 2);
            assert_eq!(category.channels.len(), 2);
            assert_eq!(category.identity_probability, Probability::zero());
            for (index, channel) in category.channels.iter().enumerate() {
                assert_eq!(channel.source.operation, index);
                assert_eq!(
                    channel.probability.value(),
                    &ratio(if index == 0 { first } else { 1 - first }, 1)
                );
                assert_eq!(
                    channel.action,
                    OriginalNoiseAction::Pauli {
                        factors: vec![(0, Basis::X)]
                    }
                );
            }
            assert_file_error(&physical, &modules, options, 1);
            forced_configurations += 1;
        }

        let (physical, modules) = target.load("PAULI_CHANNEL_1(0.25,0.5,0) 0");
        let Operation::Noise { probabilities, .. } = &physical.operations()[0].operation else {
            panic!("original Pauli source must remain a noise instruction");
        };
        assert_eq!(
            probabilities
                .iter()
                .map(|p| p.value().clone())
                .collect::<Vec<_>>(),
            vec![ratio(1, 4), ratio(1, 2), ratio(0, 1)]
        );
        let originals =
            original_noise_categories(&physical, &OriginalNoiseOptions::default()).unwrap();
        assert_eq!(originals.len(), 1);
        assert_eq!(originals[0].identity_probability.value(), &ratio(1, 4));
        assert_eq!(originals[0].channels.len(), 3);
        for (channel, basis) in originals[0]
            .channels
            .iter()
            .zip([Basis::X, Basis::Y, Basis::Z])
        {
            assert_eq!(
                channel.action,
                OriginalNoiseAction::Pauli {
                    factors: vec![(0, basis)]
                }
            );
        }
        let compiled = compile_affine(
            &physical,
            &CompileOptions {
                compiler_strategy: compiler,
                ..Default::default()
            },
            &ScalarArithmetic::default(),
        )
        .unwrap();
        assert_eq!(compiled.noise_sites().len(), 1);
        let noise_options = NoiseOptions::default();
        let sampler = NoiseSampler::physical(
            NoiseModel::physical(&compiled, &noise_options).unwrap(),
            noise_options,
        );
        let evaluator =
            TaskEvaluator::new(&physical, &modules, &ControllerRegistry::default(), options)
                .unwrap();
        let mut support = BTreeSet::new();
        for seed in 0..32 {
            let draw = sampler.draw(&mut SplitMix64::new(seed)).unwrap();
            let (sector, expected_probability, expected_error) = match draw.faults.as_slice() {
                [] => (0, ratio(1, 4), 0),
                [choice] if choice.site == 0 && choice.alternative == 0 => (1, ratio(1, 4), 1),
                [choice] if choice.site == 0 && choice.alternative == 1 => (2, ratio(1, 2), 1),
                unexpected => panic!("zero-probability Z or unexpected category {unexpected:?}"),
            };
            support.insert(sector);
            assert_eq!(draw.physical_probability, expected_probability);
            assert_eq!(draw.proposal_probability, expected_probability);
            assert_eq!(draw.likelihood_ratio, ratio(1, 1));
            let response = evaluator
                .sample_prescribed(&draw.faults, &mut NoEntropy)
                .unwrap();
            assert_eq!(response.accepted, ExactField::one());
            assert_eq!(response.error, field(expected_error, 1));
            assert_eq!(response.target_success, Some(field(1 - expected_error, 1)));
            assert_eq!(response.noise_probability, expected_probability);
            pauli_draws += 1;
        }
        assert_eq!(support, BTreeSet::from([0, 1, 2]));
        let mut limits = TaskEnumerationOptions::default();
        limits.noise.max_weight = 1;
        let exact = evaluator.enumerate_physical(&limits).unwrap();
        assert_eq!(exact.covered_probability.value(), &ratio(1, 1));
        assert_eq!(exact.omitted_probability.value(), &ratio(0, 1));
        assert_eq!(exact.masses.accepted, ExactField::one());
        assert_eq!(exact.masses.error, field(3, 4));
        assert_eq!(exact.masses.target_success, Some(field(1, 4)));
    }
    assert_eq!(forced_configurations, 4);
    assert_eq!(pauli_draws, 64);
}
