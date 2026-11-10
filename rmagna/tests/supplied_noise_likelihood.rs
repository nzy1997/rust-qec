use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::FaultSelection;
use rmagna::noise::{NoiseError, NoiseSampling, RandomSource, SplitMix64};
use rmagna::runtime::{
    CausalProgram, CausalStrategy, ControllerRegistry, HistorySampler, RuntimeError,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{EvaluationOptions, TaskEvaluation, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "rmagna-supplied-noise-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn make_evaluator(
    files: &Files,
    source: &str,
    task: &str,
    registry: &ControllerRegistry,
    options: EvaluationOptions,
) -> TaskEvaluator {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let task = parse_task(task, &files.0, &TaskOptions::default()).unwrap();
    let modules = task.load_modules(&circuit, &parse).unwrap();
    TaskEvaluator::new_with_sampling(
        &circuit,
        &modules,
        registry,
        options,
        &NoiseSampling::Importance {
            proposal_scale: rational(2, 1),
        },
    )
    .unwrap()
}

fn assert_scaled(weighted: &TaskEvaluation, mut unit: TaskEvaluation, likelihood: &BigRational) {
    let fields = FieldArithmetic::default();
    let scale = |value: &ExactField| fields.scale(value, likelihood).unwrap();
    unit.accepted = scale(&unit.accepted);
    unit.error = scale(&unit.error);
    unit.target_success = unit.target_success.as_ref().map(scale);
    unit.moments = unit.moments.iter().map(scale).collect();
    unit.noise_likelihood_ratio = likelihood.clone();
    assert!(
        weighted == &unit,
        "weighted contributions or unweighted diagnostics differ"
    );
}

#[test]
fn supplied_linear_factors_normalize_and_scale_once_without_drawing_noise() {
    let files = Files::new();
    files.put("ideal.magna", "I 0");
    let evaluator = make_evaluator(
        &files,
        "X_ERROR(1/5) 0",
        r#"{"outputs":[0],"ideal_state":"ideal.magna","observables":[[0,1]]}"#,
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
    );
    assert_eq!(evaluator.execution_route(), "linear_affine");
    let fields = FieldArithmetic::default();
    let faults = [FaultSelection {
        site: 0,
        alternative: 0,
    }];
    for (supplied, canonical) in [
        (rational(1, 1), rational(1, 1)),
        (BigRational::new_raw(0.into(), 10.into()), rational(0, 1)),
        (BigRational::new_raw(18.into(), 10.into()), rational(9, 5)),
    ] {
        let mut weighted_rng = SplitMix64::new(30);
        let mut unit_rng = weighted_rng;
        let weighted = evaluator
            .sample_supplied_noise(&faults, &supplied, &mut weighted_rng)
            .unwrap();
        let unit = evaluator.sample_prescribed(&faults, &mut unit_rng).unwrap();
        assert_eq!(weighted_rng, unit_rng);
        assert_eq!(weighted_rng.state(), 30);
        assert_eq!(weighted.noise_probability, rational(1, 5));
        let factor = fields.from_rational(canonical.clone()).unwrap();
        assert_eq!(weighted.accepted, factor);
        assert_eq!(weighted.error, factor);
        assert_eq!(weighted.target_success, Some(ExactField::zero()));
        assert_eq!(weighted.moments, [factor.negated()]);
        assert_scaled(&weighted, unit, &canonical);
    }
}

#[test]
fn supplied_causal_factors_preserve_report_flip_correlation_postselection_and_recovery() {
    let files = Files::new();
    files.put("ideal.magna", "H 1\nCX 1 0");
    files.put("recovery.magna", "X 0");
    let source = "H 0\nT 0\nH 0\nM(1/10) 0\nE(1/4) rec[-1] Z1\nELSE_CORRELATED_ERROR(1/3) rec[-1] X1 X2\nH 3\nM 3\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
    let task = r#"{"outputs":[1],"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"1":0}},"recovery":"recovery.magna","sample_records":[0],"observables":[[0,1]]}"#;
    let faults = [
        FaultSelection {
            site: 0,
            alternative: 0,
        },
        FaultSelection {
            site: 1,
            alternative: 1,
        },
    ];
    let likelihood = rational(9, 5);
    let scalar = ScalarArithmetic::default();
    let fields = FieldArithmetic::default();
    let half = scalar.omega(0, -2).unwrap();
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        for history_sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            let mut options = EvaluationOptions::default();
            options.runtime.strategy = strategy;
            options.runtime.history_sampler = history_sampler;
            let evaluator = make_evaluator(
                &files,
                source,
                task,
                &ControllerRegistry::default(),
                options,
            );
            let mut seen = [false; 2];
            for seed in 0..32 {
                let mut weighted_rng = SplitMix64::new(seed);
                let mut unit_rng = weighted_rng;
                let mut zero_rng = weighted_rng;
                let mut supplied_unit_rng = weighted_rng;
                let weighted = evaluator
                    .sample_supplied_noise(&faults, &likelihood, &mut weighted_rng)
                    .unwrap();
                let unit = evaluator.sample_prescribed(&faults, &mut unit_rng).unwrap();
                let zero_weighted = evaluator
                    .sample_supplied_noise(&faults, &rational(0, 1), &mut zero_rng)
                    .unwrap();
                let supplied_unit = evaluator
                    .sample_supplied_noise(&faults, &rational(1, 1), &mut supplied_unit_rng)
                    .unwrap();
                assert_eq!(weighted_rng, unit_rng);
                assert_eq!(zero_rng, unit_rng);
                assert_eq!(supplied_unit_rng, unit_rng);
                assert!(supplied_unit == unit);

                let [report] = weighted.sampled_records.as_slice() else {
                    panic!("expected the single reported guard");
                };
                let report = *report;
                seen[usize::from(report)] = true;
                let sign = if report { 1 } else { -1 };
                let p = scalar
                    .scalar([2.into(), sign.into(), 0.into(), (-sign).into()], -2)
                    .unwrap();
                let raw_acceptance = scalar.multiply(&p, &half).unwrap();
                let joint = scalar.multiply(&raw_acceptance, &half).unwrap();
                let zero = ExactScalar::zero();
                assert_eq!(weighted.history_mass, p);
                assert_eq!(weighted.raw_acceptance, raw_acceptance);
                assert_eq!(weighted.target_normalization, Some(half.clone()));
                assert_eq!(
                    weighted.raw_error,
                    if report { zero.clone() } else { joint.clone() }
                );
                assert_eq!(
                    weighted.raw_target_success,
                    Some(if report { joint } else { zero })
                );
                assert_eq!(
                    weighted.raw_moments,
                    [if report {
                        raw_acceptance
                    } else {
                        raw_acceptance.negated()
                    }]
                );
                assert_eq!(
                    weighted.noise_probability,
                    if report {
                        rational(1, 40)
                    } else {
                        rational(1, 10)
                    }
                );
                let accepted = match history_sampler {
                    HistorySampler::ExactMarginal => {
                        assert!(weighted.history_proposal_mass.is_none());
                        fields.from_rational(rational(9, 10)).unwrap()
                    }
                    HistorySampler::AffineSupport => {
                        assert_eq!(weighted.history_proposal_mass, Some(half.clone()));
                        fields
                            .scale(&fields.from_scalar(&p).unwrap(), &likelihood)
                            .unwrap()
                    }
                };
                assert_eq!(weighted.accepted, accepted);
                assert_eq!(
                    weighted.error,
                    if report {
                        ExactField::zero()
                    } else {
                        accepted.clone()
                    }
                );
                assert_eq!(
                    weighted.target_success,
                    Some(if report {
                        accepted.clone()
                    } else {
                        ExactField::zero()
                    })
                );
                assert_eq!(
                    weighted.moments,
                    [if report { accepted } else { accepted.negated() }]
                );
                assert_scaled(&zero_weighted, supplied_unit, &rational(0, 1));
                assert_scaled(&weighted, unit, &likelihood);
            }
            assert_eq!(seen, [true, true]);
        }
    }
}

#[test]
fn interference_zero_affine_history_retains_factor_and_zero_contributions() {
    let files = Files::new();
    let source = "H 0 1 2 3\nCCZ 0 1 2\nCCZ 0 1 3\nCCZ 0 2 3\nH 0 1 2 3\nM 0 1 2 3";
    let task = r#"{"outputs":[0],"sample_records":[0,1,2,3],"observables":[[0,1]]}"#;
    let likelihood = rational(9, 5);
    let a = ScalarArithmetic::default();
    let fields = FieldArithmetic::default();
    let mut options = EvaluationOptions::default();
    options.runtime.history_sampler = HistorySampler::AffineSupport;
    options.runtime.compile.compiler_strategy = CompilerStrategy::SharedCommutingLayer;
    options.runtime.compile.probability_preparation =
        rmagna::instrument::QueryPreparation::FixedFibers(Default::default());
    options.runtime.compile.density_preparation =
        rmagna::instrument::QueryPreparation::FixedFibers(Default::default());
    let evaluator = make_evaluator(
        &files,
        source,
        task,
        &ControllerRegistry::default(),
        options,
    );
    let mut seen = [false; 2];
    for seed in 0..32 {
        let mut weighted_rng = SplitMix64::new(seed);
        let mut unit_rng = weighted_rng;
        let weighted = evaluator
            .sample_supplied_noise(&[], &likelihood, &mut weighted_rng)
            .unwrap();
        let unit = evaluator.sample_prescribed(&[], &mut unit_rng).unwrap();
        assert_eq!(weighted_rng, unit_rng);
        let mask = weighted
            .sampled_records
            .iter()
            .enumerate()
            .fold(0_u32, |mask, (i, &bit)| mask | (u32::from(bit) << i));
        let amplitude_numerator: i64 = (0_u32..16)
            .map(|input| {
                let phase = (mask & input).count_ones()
                    + [7, 11, 13]
                        .iter()
                        .filter(|&&ccz| input & ccz == ccz)
                        .count() as u32;
                if phase.is_multiple_of(2) { 1 } else { -1 }
            })
            .sum();
        let mass = a
            .scalar(
                [
                    (amplitude_numerator * amplitude_numerator).into(),
                    0.into(),
                    0.into(),
                    0.into(),
                ],
                -8,
            )
            .unwrap();
        assert_eq!(weighted.history_mass, mass);
        assert_eq!(
            weighted.history_proposal_mass,
            Some(a.omega(0, -8).unwrap())
        );
        assert_eq!(weighted.noise_probability, rational(1, 1));
        assert_eq!(weighted.noise_likelihood_ratio, likelihood);
        assert_eq!(
            weighted.accepted,
            fields
                .from_rational(rational(9 * amplitude_numerator * amplitude_numerator, 80))
                .unwrap()
        );
        seen[usize::from(mass.is_zero())] = true;
        assert_scaled(&weighted, unit, &likelihood);
    }
    assert_eq!(seen, [true, true]);
}

struct NoEntropy;
impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("invalid input consumed entropy")
    }
}

#[test]
fn invalid_supplied_inputs_fail_before_entropy_or_controller_calls() {
    let files = Files::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ControllerRegistry::default();
    let counted = Arc::clone(&calls);
    registry
        .register("echo", "1", move |bits: &[bool]| {
            counted.fetch_add(1, Ordering::Relaxed);
            Ok(bits.to_vec())
        })
        .unwrap();
    let source =
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nX_ERROR(1/4) ctrl[0] 1";
    let task = r#"{"outputs":[1]}"#;
    let valid = FaultSelection {
        site: 0,
        alternative: 0,
    };
    let bad_site = FaultSelection {
        site: 2,
        alternative: 0,
    };
    let bad_alternative = FaultSelection {
        site: 0,
        alternative: 1,
    };
    let raw_large = BigRational::new_raw(32.into(), 32.into());
    let one = rational(1, 1);
    let zero = rational(0, 1);
    for (scalars, fields) in [
        (
            ScalarArithmetic::new(5).unwrap(),
            FieldArithmetic::default(),
        ),
        (ScalarArithmetic::default(), FieldArithmetic::new(5)),
    ] {
        let options = EvaluationOptions {
            scalars,
            fields,
            ..Default::default()
        };
        let evaluator = make_evaluator(&files, source, task, &registry, options);
        for (faults, factor, message) in [
            (&[valid][..], rational(-1, 1), "nonnegative numerator"),
            (
                &[valid][..],
                BigRational::new_raw(0.into(), 0.into()),
                "positive denominator",
            ),
            (
                &[valid][..],
                BigRational::new_raw((-1).into(), (-1).into()),
                "positive denominator",
            ),
            (
                &[valid][..],
                raw_large.clone(),
                "6 raw coefficient bits, limit is 5",
            ),
            (&[bad_site][..], zero.clone(), "site 2 has no alternative 0"),
            (
                &[bad_alternative][..],
                one.clone(),
                "site 0 has no alternative 1",
            ),
            (
                &[valid, valid][..],
                one.clone(),
                "site 0 was selected more than once",
            ),
        ] {
            let error = evaluator
                .sample_supplied_noise(faults, &factor, &mut NoEntropy)
                .err()
                .unwrap();
            assert!(error.message.contains(message), "{error}");
            assert_eq!(calls.load(Ordering::Relaxed), 0);
        }
    }
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let scalar = ScalarArithmetic::new(5).unwrap();
    let program = CausalProgram::new(&circuit, &registry, Default::default(), &scalar).unwrap();
    assert!(matches!(
        program.resolve_supplied_noise(&[], &[valid], &raw_large, &mut NoEntropy, &scalar),
        Err(RuntimeError::Noise(NoiseError::SuppliedLikelihoodLimit {
            limit: 5,
            required: 6
        }))
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let scalar = ScalarArithmetic::default();
    let mut random = SplitMix64::new(21);
    let resolved = program
        .resolve_supplied_noise(&[], &[valid], &zero, &mut random, &scalar)
        .unwrap();
    assert_eq!(resolved.noise_likelihood_ratio(), &zero);
    assert_eq!(resolved.entropy_words(), (0, 1));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
