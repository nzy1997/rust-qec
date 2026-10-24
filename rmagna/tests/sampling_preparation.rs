use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionCosts, ExecutionLimits, ExecutionMeter};
use rmagna::instrument::{PreparedQueryStatistics, QueryPreparation};
use rmagna::noise::{RandomSource, SplitMix64};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::task::{
    EvaluationOptions, SamplingPreparationOptions, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-sampling-preparation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, name: &str, source: &str) {
        std::fs::write(self.0.join(name), source).unwrap();
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn evaluator(
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
    let modules = parse_task(task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    TaskEvaluator::new(&circuit, &modules, registry, options).unwrap()
}
fn options(
    mode: QueryPreparation,
    sampler: HistorySampler,
    compiler: CompilerStrategy,
) -> EvaluationOptions {
    let mut options = EvaluationOptions::default();
    options.runtime.compile.probability_preparation = mode;
    options.runtime.compile.density_preparation = mode;
    options.runtime.compile.moment_preparation = mode;
    options.runtime.compile.compiler_strategy = compiler;
    options.runtime.history_sampler = sampler;
    options
}
fn caches(
    e: &TaskEvaluator,
) -> (
    Option<PreparedQueryStatistics>,
    Option<PreparedQueryStatistics>,
    Option<PreparedQueryStatistics>,
) {
    (
        e.prepared_probability_statistics().unwrap(),
        e.prepared_density_statistics().unwrap(),
        e.prepared_moment_statistics().unwrap(),
    )
}
struct Counted {
    random: SplitMix64,
    words: usize,
}
impl Counted {
    fn new(seed: u64) -> Self {
        Self {
            random: SplitMix64::new(seed),
            words: 0,
        }
    }
}
impl RandomSource for Counted {
    fn next_u64(&mut self) -> u64 {
        self.words += 1;
        self.random.next_u64()
    }
}
fn compare(prepared: &TaskEvaluator, baseline: &TaskEvaluator, draws: usize) -> Vec<Vec<bool>> {
    let before = caches(prepared);
    let (mut a, mut b) = (Counted::new(932), Counted::new(932));
    let mut records = Vec::new();
    for index in 0..draws {
        let actual = prepared.sample(&mut a).unwrap();
        let expected = baseline.sample(&mut b).unwrap();
        assert!(actual == expected, "exact draw {index} changed");
        assert_eq!(a.words, b.words);
        assert_eq!(
            caches(prepared),
            before,
            "draw {index} constructed a missing family"
        );
        if let Some(success) = &actual.raw_target_success {
            // The Bell target projected on its second wire is |1><1| / 2.
            // Thus success = (accepted - Z moment) / 4, independently of noise.
            let a = rmagna::scalar::ScalarArithmetic::default();
            assert_eq!(actual.target_normalization, Some(a.omega(0, -2).unwrap()));
            let difference = a
                .subtract(&actual.raw_acceptance, &actual.raw_moments[0])
                .unwrap();
            assert_eq!(
                *success,
                a.multiply(&difference, &a.omega(0, -4).unwrap()).unwrap()
            );
            assert_eq!(
                a.add(&actual.raw_error, success).unwrap(),
                a.multiply(&actual.raw_acceptance, &a.omega(0, -2).unwrap())
                    .unwrap()
            );
        }
        records.push(actual.sampled_records);
    }
    assert!(a.words > 0);
    records
}
#[test]
fn linear_target_and_error_families_are_prepared_without_fake_execution_work() {
    let files = Files::new();
    files.put("ideal.magna", "H 0\nCX 0 1\nI 2");
    let registry = ControllerRegistry::default();
    let source = "H 0\nT 0\nH 0\nCX 0 1\nM(1/7) 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nX_ERROR(1/4) 0\nI 2\nOBSERVABLE_INCLUDE(0) rec[-1]";
    for task in [
        r#"{"outputs":[0],"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"1":1}},"observables":[[0,1],[1,1]]}"#,
        r#"{"error_observables":[0]}"#,
    ] {
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let o = options(
                QueryPreparation::FixedFibers(Default::default()),
                HistorySampler::AffineSupport,
                compiler,
            );
            let e = evaluator(&files, source, task, &registry, o);
            let base = evaluator(&files, source, task, &registry, o);
            let meter = ExecutionMeter::new(ExecutionLimits {
                max_queries: 0,
                ..Default::default()
            });
            let report = e.prepare_sampling_queries(Default::default(), &meter);
            assert_eq!(report.result, Ok(()));
            assert!(report.all_families_retained());
            assert_eq!(report.execution_route, "linear_affine");
            assert_eq!(report.history_rows, 0);
            assert_eq!(
                report.counts.planned,
                if task.contains("ideal_state") { 4 } else { 2 }
            );
            assert_eq!(report.counts.attempted, report.counts.planned);
            assert_eq!(report.counts.created, report.counts.planned);
            assert_eq!(report.counts.failed, 0);
            assert_eq!(report.counts.cold, 0);
            assert!(!report.by_kind.contains_key("reported_support"));
            assert!(report.elapsed_seconds.is_finite() && report.elapsed_seconds >= 0.0);
            assert_eq!(meter.snapshot(), ExecutionCosts::default());
            compare(&e, &base, 24);
            let again = e.prepare_sampling_queries(Default::default(), &meter);
            assert_eq!(again.result, Ok(()));
            assert_eq!(again.counts.created, 0);
            assert_eq!(again.counts.reused, report.counts.planned);
            assert_eq!(meter.snapshot(), ExecutionCosts::default());
        }
    }
}
const SOURCE: &str = "CONTROL[one] -> ctrl[0]\nH 0\nT 0\nH 0\nM(1/7) 0\nDETECTOR rec[-1]\nCONTROL[copy] !det[0] ctrl[0] -> ctrl[1] ctrl[2]\nCH ctrl[1] 1\nE(1/5) ctrl[1] X2\nELSE_CORRELATED_ERROR(1/7) ctrl[1] Z2\nM 1\nDETECTOR rec[-1]\nCONTROL[xor] det[1] ctrl[1] -> ctrl[3]\nCX ctrl[3] 2\nM 2\nDETECTOR rec[-1]\nPOSTSELECT ctrl[3] det[2]\nOBSERVABLE_INCLUDE(0) ctrl[3] det[2]";
fn registry(calls: &Arc<AtomicUsize>) -> ControllerRegistry {
    let mut registry = ControllerRegistry::default();
    let c = calls.clone();
    registry
        .register("one", "1", move |b: &[bool]| {
            assert!(b.is_empty());
            c.fetch_add(1, Ordering::Relaxed);
            Ok(vec![true])
        })
        .unwrap();
    let c = calls.clone();
    registry
        .register("copy", "1", move |b: &[bool]| {
            c.fetch_add(1, Ordering::Relaxed);
            Ok(b.to_vec())
        })
        .unwrap();
    let c = calls.clone();
    registry
        .register("xor", "1", move |b: &[bool]| {
            c.fetch_add(1, Ordering::Relaxed);
            Ok(vec![b[0] ^ b[1]])
        })
        .unwrap();
    registry
}
#[test]
fn feedback_prefixes_decision_constants_and_guarded_categories_cover_actual_noisy_draws() {
    let files = Files::new();
    files.put("ideal.magna", "H 0\nCX 0 1");
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = registry(&calls);
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for task in [
                r#"{"outputs":[2],"sample_records":[0,2],"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"1":1}},"observables":[[0,1],[1,1]]}"#,
                r#"{"sample_records":[0,2],"error_observables":[0]}"#,
            ] {
                calls.store(0, Ordering::Relaxed);
                let o = options(
                    QueryPreparation::FixedFibers(Default::default()),
                    sampler,
                    compiler,
                );
                let e = evaluator(&files, SOURCE, task, &registry, o);
                let base = evaluator(&files, SOURCE, task, &registry, o);
                let meter = ExecutionMeter::default();
                let report = e.prepare_sampling_queries(Default::default(), &meter);
                assert_eq!(report.result, Ok(()));
                assert_eq!(
                    report.all_families_retained(),
                    sampler == HistorySampler::ExactMarginal
                );
                assert_eq!(
                    report.counts.cold,
                    usize::from(sampler == HistorySampler::AffineSupport)
                );
                assert_eq!(
                    report.history_rows, 7,
                    "zero-input control and one guard per correlated category"
                );
                let reported = &report.by_kind["reported_probability"];
                assert_eq!(
                    reported.planned,
                    if sampler == HistorySampler::ExactMarginal {
                        13
                    } else {
                        1
                    }
                );
                assert_eq!(calls.load(Ordering::Relaxed), 0);
                assert_eq!(meter.snapshot(), ExecutionCosts::default());
                if sampler == HistorySampler::AffineSupport {
                    assert_eq!(report.by_kind["reported_support"].cold, 1);
                    assert_eq!(report.by_kind["reported_support"].created, 0);
                }
                let records = compare(&e, &base, 24);
                assert_eq!(calls.load(Ordering::Relaxed), 24 * 2 * 3);
                assert!(records.iter().any(|b| b[0]));
                assert!(records.iter().any(|b| !b[0]));
                let again = e.prepare_sampling_queries(Default::default(), &meter);
                assert_eq!(again.result, Ok(()));
                assert_eq!(again.counts.created, 0);
                assert_eq!(
                    again.counts.reused + again.counts.cold,
                    again.counts.planned
                );
                assert_eq!(
                    again.counts.cold,
                    usize::from(sampler == HistorySampler::AffineSupport)
                );
                assert_eq!(caches(&e), caches(&base));
            }
        }
    }
}
#[test]
fn preflight_caps_deadlines_and_partial_failures_retain_honest_receipts_and_retry() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    let source = "H 0\nT 0\nH 0\nM 0\nI 1 2";
    let task = r#"{"outputs":[1,2],"sample_records":[0],"observables":[[0,1],[0,2]]}"#;
    let o = options(
        QueryPreparation::FixedFibers(Default::default()),
        HistorySampler::ExactMarginal,
        CompilerStrategy::ParityCopies,
    );
    let e = evaluator(&files, source, task, &registry, o);
    let empty = caches(&e);
    for limits in [
        SamplingPreparationOptions {
            max_requests: 0,
            ..Default::default()
        },
        SamplingPreparationOptions {
            max_requests: 3,
            ..Default::default()
        },
        SamplingPreparationOptions {
            max_history_rows: 0,
            ..Default::default()
        },
        SamplingPreparationOptions {
            max_layout_rows: 1,
            ..Default::default()
        },
    ] {
        let meter = ExecutionMeter::default();
        let report = e.prepare_sampling_queries(limits, &meter);
        assert!(report.result.is_err());
        assert!(!report.all_families_retained());
        assert_eq!(report.counts.attempted, 0);
        assert_eq!(report.counts.failed, 0);
        assert_eq!(caches(&e), empty);
        assert_eq!(meter.snapshot(), ExecutionCosts::default());
    }
    let meter = ExecutionMeter::new(ExecutionLimits {
        max_duration: Some(Duration::ZERO),
        ..Default::default()
    });
    let expired = e.prepare_sampling_queries(Default::default(), &meter);
    assert!(expired.result.unwrap_err().message.contains("max_seconds"));
    assert_eq!(expired.counts.planned, 0);
    assert_eq!(expired.counts.attempted, 0);
    assert_eq!(caches(&e), empty);
    let good = e.prepare_sampling_queries(Default::default(), &ExecutionMeter::default());
    assert_eq!(good.result, Ok(()));
    assert!(good.counts.created > 0);
    assert_eq!(good.counts.planned, 6);
    let mut capped = o;
    capped.runtime.compile.max_prepared_moments = 1;
    let e = evaluator(&files, source, task, &registry, capped);
    let meter = ExecutionMeter::default();
    let failed = e.prepare_sampling_queries(Default::default(), &meter);
    let error = failed.result.unwrap_err();
    assert!(error.field.contains("requests[5].pauli_moment"));
    assert!(error.message.contains("max_prepared_moments"));
    assert_eq!(failed.counts.planned, 6);
    assert_eq!(failed.counts.attempted, 6);
    assert_eq!(failed.counts.created, 3);
    assert_eq!(failed.counts.reused, 2);
    assert_eq!(failed.counts.failed, 1);
    assert_eq!(
        e.prepared_moment_statistics()
            .unwrap()
            .unwrap()
            .cached_queries,
        1
    );
    let retry = e.prepare_sampling_queries(Default::default(), &meter);
    assert!(retry.result.is_err());
    assert_eq!(retry.counts.created, 0);
    assert_eq!(retry.counts.reused, 5);
    assert_eq!(retry.counts.failed, 1);
    assert_eq!(meter.snapshot(), ExecutionCosts::default());
    let mut terms = o;
    terms.runtime.sampling.max_terms = 0;
    let e = evaluator(&files, source, task, &registry, terms);
    let failed = e.prepare_sampling_queries(Default::default(), &meter);
    assert!(failed.result.is_err());
    assert_eq!(failed.counts.attempted, 1);
    assert_eq!(failed.counts.failed, 1);
    assert_eq!(
        e.prepared_probability_statistics()
            .unwrap()
            .unwrap()
            .cached_queries,
        0
    );
}
#[test]
fn cold_policies_and_cold_prefix_scope_never_claim_retained_warm_resources() {
    let files = Files::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = registry(&calls);
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        let mut o = options(
            QueryPreparation::Cold,
            sampler,
            CompilerStrategy::ParityCopies,
        );
        let task = r#"{"outputs":[2],"sample_records":[0],"observables":[[0,1]]}"#;
        let e = evaluator(&files, SOURCE, task, &registry, o);
        let meter = ExecutionMeter::default();
        let report = e.prepare_sampling_queries(Default::default(), &meter);
        assert_eq!(report.result, Ok(()));
        assert!(!report.all_families_retained());
        assert!(report.counts.cold >= 3);
        assert_eq!(report.counts.created, 0);
        assert_eq!(report.counts.cold, report.counts.planned);
        assert_eq!(
            e.prepared_probability_statistics()
                .unwrap()
                .unwrap()
                .cached_queries,
            0
        );
        assert_eq!(
            e.prepared_moment_statistics()
                .unwrap()
                .unwrap()
                .cached_queries,
            0
        );
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        o.runtime.strategy = CausalStrategy::ColdPrefix;
        let e = evaluator(&files, SOURCE, task, &registry, o);
        let report = e.prepare_sampling_queries(Default::default(), &meter);
        let error = report.result.as_ref().unwrap_err();
        assert!(error.message.contains("cold_prefix") && error.message.contains("fixed_template"));
        assert_eq!(report.execution_route, "causal_cold_prefix");
        assert_eq!(report.counts.planned, 0);
        assert_eq!(report.counts.attempted, 0);
        assert!(!report.all_families_retained());
        assert_eq!(caches(&e), (None, None, None));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(meter.snapshot(), ExecutionCosts::default());
    }
}

#[test]
fn trace_mode_retains_geometry_while_phase_resources_remain_deferred() {
    let files = Files::new();
    files.put("ideal.magna", "H 0\nCX 0 1");
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = registry(&calls);
    let task = r#"{"outputs":[2],"sample_records":[0,2],"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"1":1}},"observables":[[0,1],[1,1]]}"#;
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        let o = options(
            QueryPreparation::Trace,
            sampler,
            CompilerStrategy::SharedCommutingLayer,
        );
        let e = evaluator(&files, SOURCE, task, &registry, o);
        let base = evaluator(&files, SOURCE, task, &registry, o);
        let report = e.prepare_sampling_queries(Default::default(), &ExecutionMeter::default());
        assert_eq!(report.result, Ok(()));
        assert_eq!(
            report.all_families_retained(),
            sampler == HistorySampler::ExactMarginal
        );
        let (p, d, m) = caches(&e);
        for value in [p, d, m] {
            let value = value.unwrap();
            assert!(value.cached_queries > 0);
            assert_eq!(value.fixed_resources, 0);
        }
        assert_eq!(
            report.counts.cold,
            usize::from(sampler == HistorySampler::AffineSupport)
        );
        compare(&e, &base, 8);
    }
}
#[test]
fn literal_interference_report_faults_and_pauli_moments_survive_static_preparation() {
    use num_rational::BigRational;
    use rmagna::scalar::{ExactScalar, ScalarArithmetic};
    let files = Files::new();
    files.put("ideal.magna", "H 0\nS 0");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ControllerRegistry::default();
    let counted = calls.clone();
    registry
        .register("echo", "1", move |b: &[bool]| {
            counted.fetch_add(1, Ordering::Relaxed);
            Ok(b.to_vec())
        })
        .unwrap();
    // HTH|0> has P(0)=(2+sqrt(2))/4. The reported bit controls X on |+i>.
    let source = "H 0\nT 0\nH 0\nM(1/7) 0\nDETECTOR rec[-1]\nH 1\nS 1\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1";
    let task = r#"{"outputs":[1],"sample_records":[0],"ideal_state":"ideal.magna","observables":[[1,1],[0,1]]}"#;
    let a = ScalarArithmetic::default();
    let p0 = a
        .scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
        .unwrap();
    let p1 = a
        .scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
        .unwrap();
    let flip_probability = BigRational::new(1.into(), 7.into());
    let identity_probability = BigRational::new(6.into(), 7.into());
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            for policy in [
                QueryPreparation::Cold,
                QueryPreparation::Trace,
                QueryPreparation::FixedFibers(Default::default()),
            ] {
                let o = options(policy, sampler, compiler);
                let e = evaluator(&files, source, task, &registry, o);
                let baseline = evaluator(&files, source, task, &registry, o);
                calls.store(0, Ordering::Relaxed);
                let meter = ExecutionMeter::new(ExecutionLimits {
                    max_queries: 0,
                    ..Default::default()
                });
                let report = e.prepare_sampling_queries(Default::default(), &meter);
                assert_eq!(report.result, Ok(()));
                assert_eq!(calls.load(Ordering::Relaxed), 0);
                assert_eq!(meter.snapshot(), ExecutionCosts::default());
                let retained = caches(&e);
                let (mut ra, mut rb) = (Counted::new(145), Counted::new(145));
                let mut seen = [[false; 2]; 2];
                for _ in 0..256 {
                    let value = e.sample(&mut ra).unwrap();
                    assert!(value == baseline.sample(&mut rb).unwrap());
                    assert_eq!(ra.words, rb.words);
                    assert_eq!(caches(&e), retained);
                    let flip = value.noise_probability == flip_probability;
                    assert!(flip || value.noise_probability == identity_probability);
                    let reported = value.sampled_records[0];
                    let truth = reported ^ flip;
                    seen[usize::from(flip)][usize::from(reported)] = true;
                    let mass = if truth { &p1 } else { &p0 };
                    assert_eq!(&value.history_mass, mass);
                    assert_eq!(&value.raw_acceptance, mass);
                    assert_eq!(value.target_normalization, Some(ExactScalar::one()));
                    assert_eq!(
                        value.raw_target_success,
                        Some(if reported {
                            ExactScalar::zero()
                        } else {
                            mass.clone()
                        })
                    );
                    assert_eq!(
                        value.raw_error,
                        if reported {
                            mass.clone()
                        } else {
                            ExactScalar::zero()
                        }
                    );
                    let phase = a.omega(if reported { 4 } else { 0 }, 0).unwrap();
                    assert_eq!(
                        value.raw_moments,
                        vec![a.multiply(mass, &phase).unwrap(), ExactScalar::zero()]
                    );
                }
                assert_eq!(seen, [[true; 2]; 2]);
                assert_eq!(calls.load(Ordering::Relaxed), 512);
            }
        }
    }
}
