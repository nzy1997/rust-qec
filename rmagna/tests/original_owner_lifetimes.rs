// SPDX-License-Identifier: MPL-2.0
// Complete RuntimeOwnershipTests.test_execution_and_noise_schedule_have_independent_owner_lifetimes,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_runtime_ownership.py.
// Rust owns its task and categorical model explicitly; private native capsule
// layout and cross-language RNG identity are not asserted.
use num_rational::BigRational as R;
use rmagna::circuit::{Circuit, ParseOptions, parse_circuit};
use rmagna::fibers::FiberOptions;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::{
    CompileOptions, CompiledInstrument, FaultSelection, InsertionPreparation,
    ProbabilityPreparation, compile_affine,
};
use rmagna::noise::{
    NoiseDraw, NoiseModel, NoiseOptions, NoiseSampler, NoiseSampling, RandomSource, SplitMix64,
};
use rmagna::runtime::ControllerRegistry;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, LoadedModules, TaskEvaluation, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::{collections::BTreeSet, path::Path};
fn r(n: i64, d: i64) -> R {
    R::new(n.into(), d.into())
}
struct SourceOwners {
    _circuit: Circuit,
    _modules: LoadedModules,
    _instrument: CompiledInstrument,
    _registry: ControllerRegistry,
}
fn prepare(compile: CompileOptions) -> (TaskEvaluator, NoiseSampler, SourceOwners) {
    let parse = ParseOptions::default();
    let physical = parse_circuit("H 0\nZ_ERROR(1/7) 0\nM 0", &parse).unwrap();
    let modules = parse_task("{}", Path::new("."), &TaskOptions::default())
        .unwrap()
        .load_modules(&physical, &parse)
        .unwrap();
    let registry = ControllerRegistry::default();
    let sampling = NoiseSampling::Importance {
        proposal_scale: r(2, 1),
    };
    let options = EvaluationOptions {
        runtime: rmagna::runtime::RuntimeOptions {
            compile,
            ..Default::default()
        },
        ..Default::default()
    };
    let evaluator =
        TaskEvaluator::new_with_sampling(&physical, &modules, &registry, options, &sampling)
            .unwrap();
    assert_eq!(evaluator.execution_route(), "linear_affine");
    let instrument = compile_affine(&physical, &compile, &ScalarArithmetic::default()).unwrap();
    let model = NoiseModel::physical(&instrument, &NoiseOptions::default()).unwrap();
    let schedule =
        NoiseSampler::configured(model, &sampling, &BTreeSet::new(), NoiseOptions::default())
            .unwrap();
    (
        evaluator,
        schedule,
        SourceOwners {
            _circuit: physical,
            _modules: modules,
            _instrument: instrument,
            _registry: registry,
        },
    )
}
fn check(row: &TaskEvaluation) {
    assert_eq!(row.raw_acceptance, ExactScalar::one());
    assert_eq!(row.raw_error, ExactScalar::zero());
    assert_eq!(row.history_mass, ExactScalar::one());
    assert_eq!(row.history_weight, ExactField::one());
    assert_eq!(row.error, ExactField::zero());
    assert!(row.sampled_records.is_empty());
    assert!(row.controller_calls.is_empty());
    assert!(row.noise_probability == r(6, 7) || row.noise_probability == r(1, 7));
    let weight = if row.noise_probability == r(6, 7) {
        r(6, 5)
    } else {
        r(1, 2)
    };
    assert_eq!(row.noise_likelihood_ratio, weight);
    assert_eq!(
        row.accepted,
        FieldArithmetic::default().from_rational(weight).unwrap()
    );
}
fn execution_rows(execution: &TaskEvaluator) -> Vec<(TaskEvaluation, SplitMix64)> {
    (0..12)
        .map(|seed| {
            let mut rng = SplitMix64::new(seed);
            let row = execution.sample(&mut rng).unwrap();
            check(&row);
            (row, rng)
        })
        .collect()
}
fn compare_rows(execution: &TaskEvaluator, expected: &[(TaskEvaluation, SplitMix64)]) {
    for (seed, (before, state)) in expected.iter().enumerate() {
        let mut rng = SplitMix64::new(seed as u64);
        let actual = execution.sample(&mut rng).unwrap();
        check(&actual);
        assert!(before == &actual);
        assert_eq!(&rng, state);
    }
}
fn noise_rows(schedule: &NoiseSampler) -> (Vec<NoiseDraw>, SplitMix64) {
    let mut rng = SplitMix64::new(391);
    let draws = (0..12)
        .map(|_| schedule.draw(&mut rng).unwrap())
        .collect::<Vec<_>>();
    (draws, rng)
}
struct Word {
    value: u64,
    used: usize,
}
impl RandomSource for Word {
    fn next_u64(&mut self) -> u64 {
        assert_eq!(self.used, 0, "forced draw requested extra entropy");
        self.used += 1;
        self.value
    }
}
#[test]
fn original_execution_and_noise_schedule_have_independent_owner_lifetimes() {
    let mut configurations = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for preparation in [
            ProbabilityPreparation::Cold,
            ProbabilityPreparation::Trace,
            ProbabilityPreparation::FixedFibers(FiberOptions::default()),
        ] {
            for insertions in [
                InsertionPreparation::Direct,
                InsertionPreparation::Prepared(Default::default()),
            ] {
                let compile = CompileOptions {
                    compiler_strategy: compiler,
                    probability_preparation: preparation,
                    insertion_preparation: insertions,
                    ..Default::default()
                };
                let (execution, schedule, owners) = prepare(compile);
                let expected = execution_rows(&execution);
                let expected_noise = noise_rows(&schedule);
                drop(owners);
                compare_rows(&execution, &expected);
                assert_eq!(noise_rows(&schedule), expected_noise);
                for (word, fault, physical, proposal, weight) in [
                    (0, false, r(6, 7), r(5, 7), r(6, 5)),
                    (5, true, r(1, 7), r(2, 7), r(1, 2)),
                ] {
                    let mut rng = Word {
                        value: word,
                        used: 0,
                    };
                    let actual = schedule.draw(&mut rng).unwrap();
                    assert_eq!(
                        actual.faults,
                        if fault {
                            vec![FaultSelection {
                                site: 0,
                                alternative: 0,
                            }]
                        } else {
                            vec![]
                        }
                    );
                    assert_eq!(actual.physical_probability, physical);
                    assert_eq!(actual.proposal_probability, proposal);
                    assert_eq!(actual.likelihood_ratio, weight);
                    assert_eq!(actual.entropy_words, 1);
                    assert_eq!(rng.used, 1);
                }
                drop(schedule);
                compare_rows(&execution, &expected);
                drop(execution);
                let (reverse_execution, detached_schedule, reverse_owners) = prepare(compile);
                drop(reverse_owners);
                drop(reverse_execution);
                assert_eq!(noise_rows(&detached_schedule), expected_noise);
                configurations += 1;
            }
        }
    }
    assert_eq!(configurations, 12);
}
