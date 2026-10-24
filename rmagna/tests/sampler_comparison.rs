use rmagna::costs::ExecutionLimits;
use rmagna::file_api::{
    FileLoadOptions, FileSamplerComparisonOptions, FileTask, FileTaskOptions,
    SamplerComparisonStatus,
};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::task::{RunStatus, TaskRunOptions, TaskRunResult};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Files(PathBuf);
impl Files {
    fn new(source: &str, task: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-sampler-comparison-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("circuit.magna"), source).unwrap();
        std::fs::write(path.join("task.json"), task).unwrap();
        Self(path)
    }
    fn load(&self, registry: &ControllerRegistry, options: FileTaskOptions) -> FileTask {
        FileTask::load(
            &self.0.join("circuit.magna"),
            Some(&self.0.join("task.json")),
            registry,
            NoiseSampling::Physical,
            options,
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn options(sampler: HistorySampler) -> FileTaskOptions {
    let mut options = FileTaskOptions::default();
    options.evaluation.runtime.history_sampler = sampler;
    options.evaluation.runtime.compile.probability_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options.evaluation.runtime.compile.density_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options.evaluation.runtime.compile.moment_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options
}
fn same(a: &TaskRunResult, b: &TaskRunResult) {
    assert_eq!(
        (
            a.status,
            a.requested,
            a.completed,
            a.failed,
            a.not_attempted,
            a.seed
        ),
        (
            b.status,
            b.requested,
            b.completed,
            b.failed,
            b.not_attempted,
            b.seed
        )
    );
    assert_eq!(a.statistics, b.statistics);
    assert_eq!(a.partial_statistics, b.partial_statistics);
    assert_eq!(a.efficiency, b.efficiency);
    assert_eq!(a.sampled_records, b.sampled_records);
    assert_eq!(a.failures, b.failures);
    assert_eq!(
        a.execution_costs.compilation.attempted,
        b.execution_costs.compilation.attempted
    );
    assert_eq!(
        a.execution_costs.query.attempted,
        b.execution_costs.query.attempted
    );
    assert_eq!(
        a.execution_costs.decoder.attempted,
        b.execution_costs.decoder.attempted
    );
    assert_eq!(
        a.execution_costs.support.attempted,
        b.execution_costs.support.attempted
    );
}
fn assert_support_cache_transition(
    before: &Value,
    after: &Value,
    sampler: HistorySampler,
    labels: usize,
    quotient_rank: usize,
    terms: usize,
) {
    assert!(before["support"].is_null());
    let mut expected = before.clone();
    if sampler == HistorySampler::AffineSupport {
        expected["support"] = json!({
            "labels": labels,
            "quotient_rank": quotient_rank,
            "terms": terms,
            "method": "capped_probability_parent_affine_envelope"
        });
    }
    assert_eq!(after, &expected);
}
const SOURCE: &str = "X_ERROR(1/4) 3\nH 0 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3\nX_ERROR(1/5) ctrl[0] 3\nM 3\nOBSERVABLE_INCLUDE(0) rec[-1]";
fn make_registry(calls: &Arc<AtomicUsize>, fail_first: bool) -> ControllerRegistry {
    let mut r = ControllerRegistry::default();
    let calls = calls.clone();
    r.register("majority", "1", move |bits: &[bool]| {
        let at = calls.fetch_add(1, Ordering::Relaxed);
        if fail_first && at == 0 {
            return Err("injected decoder failure".into());
        }
        Ok(vec![bits.iter().filter(|b| **b).count() >= 2])
    })
    .unwrap();
    r
}
#[test]
fn actual_pilots_match_independent_prepared_channels_without_mutating_configuration() {
    let files = Files::new(SOURCE, r#"{"sample_records":[],"error_observables":[0]}"#);
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = make_registry(&calls, false);
    let file = files.load(&registry, options(HistorySampler::ExactMarginal));
    let mut config = FileSamplerComparisonOptions::default();
    config.pilot.shots = 32;
    config.pilot.seed = 49;
    let before = file.inspect(&config.pilot).unwrap();
    let comparison = file.compare_samplers(&config).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 64);
    assert_eq!(comparison.status(), SamplerComparisonStatus::Complete);
    assert!(comparison.reason().is_none());
    let d = comparison.decision().unwrap();
    assert!(d.gamma.is_finite() && d.gamma >= 0.0);
    assert_eq!(file.inspect(&config.pilot).unwrap(), before);
    assert_eq!(comparison.affine_support().seed(), 49);
    assert_eq!(comparison.exact_marginal().seed(), 50);
    for (pilot, seed, method) in [
        (
            &comparison.affine_support(),
            49,
            HistorySampler::AffineSupport,
        ),
        (
            &comparison.exact_marginal(),
            50,
            HistorySampler::ExactMarginal,
        ),
    ] {
        assert!(pilot.is_complete());
        let independent = files
            .load(&registry, options(method))
            .prepare_sampling(&config.preparation)
            .unwrap();
        let run = TaskRunOptions {
            seed,
            ..config.pilot
        };
        let batch = independent.run(&run).unwrap();
        same(pilot.raw_result().unwrap(), batch.raw_result());
        let saved = pilot.to_json();
        assert_support_cache_transition(
            &saved["preparation"]["compilation"]["retained_caches"],
            &saved["sampling"]["compilation"]["retained_caches"],
            method,
            3,
            0,
            1,
        );
        assert_eq!(
            saved["preparation"]["execution_costs"]["compilation"]["succeeded"],
            1
        );
        assert_eq!(
            saved["preparation"]["execution_costs"]["query"]["attempted"],
            0
        );
        assert_eq!(
            saved["sampling_receipt"]["execution_costs"]["compilation"]["attempted"],
            0
        );
        assert_eq!(
            saved["sampling"]["fingerprints"],
            batch.metadata()["fingerprints"]
        );
        assert_eq!(
            saved["sampling"]["compilation"]["query_preparation_included_in_this_call"],
            false
        );
    }
    assert_eq!(calls.load(Ordering::Relaxed), 128);
    let json = comparison.to_json();
    assert_eq!(json["automatic_switching"], false);
    assert_eq!(json["production_started"], false);
    assert_eq!(json["pilot_shots_per_channel"], 32);
    let kappa = json["pilots"]["affine_support"]["sampling"]["efficiency"]["kappa"]
        .as_f64()
        .unwrap();
    let affine = comparison
        .affine_support()
        .raw_result()
        .unwrap()
        .timings
        .sampling_seconds;
    let exact = comparison
        .exact_marginal()
        .raw_result()
        .unwrap()
        .timings
        .sampling_seconds;
    assert_eq!(d.gamma, kappa * (affine / exact));
    assert_eq!(
        d.recommended_sampler,
        if d.gamma > 1.0 {
            HistorySampler::ExactMarginal
        } else {
            HistorySampler::AffineSupport
        }
    );
    drop(file);
    drop(files);
    assert_eq!(comparison.to_json(), json);
}
#[test]
fn invalid_pilot_counts_seed_overflow_and_unprepared_policies_fail_before_callbacks() {
    let files = Files::new(SOURCE, r#"{"error_observables":[0]}"#);
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = make_registry(&calls, false);
    let baseline = options(HistorySampler::AffineSupport);
    for shots in [0, 1] {
        let mut c = FileSamplerComparisonOptions::default();
        c.pilot.shots = shots;
        let e = files
            .load(&registry, baseline)
            .compare_samplers(&c)
            .unwrap_err();
        assert!(e.to_string().contains("at least two"));
    }
    let mut c = FileSamplerComparisonOptions::default();
    c.pilot.seed = u64::MAX;
    assert!(
        files
            .load(&registry, baseline)
            .compare_samplers(&c)
            .unwrap_err()
            .to_string()
            .contains("overflows")
    );
    for mode in [QueryPreparation::Cold, QueryPreparation::Trace] {
        let mut o = baseline;
        o.evaluation.runtime.compile.probability_preparation = mode;
        assert!(
            files
                .load(&registry, o)
                .compare_samplers(&Default::default())
                .unwrap_err()
                .to_string()
                .contains("probability_preparation")
        );
    }
    let mut o = baseline;
    o.evaluation.runtime.strategy = CausalStrategy::ColdPrefix;
    assert!(
        files
            .load(&registry, o)
            .compare_samplers(&Default::default())
            .unwrap_err()
            .to_string()
            .contains("fixed_template")
    );
    let mut c = FileSamplerComparisonOptions::default();
    c.pilot.statistics.max_dimensions = 1;
    assert!(
        files
            .load(&registry, baseline)
            .compare_samplers(&c)
            .is_err()
    );
    let projected = Files::new(
        SOURCE,
        r#"{"outputs":[3],"ideal_state":"ideal.magna","observables":[[0,1]]}"#,
    );
    std::fs::write(projected.0.join("ideal.magna"), "I 0").unwrap();
    for density in [true, false] {
        let mut o = baseline;
        if density {
            o.evaluation.runtime.compile.density_preparation = QueryPreparation::Trace;
        } else {
            o.evaluation.runtime.compile.moment_preparation = QueryPreparation::Cold;
        }
        let error = projected
            .load(&registry, o)
            .compare_samplers(&Default::default())
            .unwrap_err();
        assert!(error.to_string().contains(if density {
            "density_preparation"
        } else {
            "moment_preparation"
        }));
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}
#[test]
fn an_incomplete_first_pilot_does_not_prevent_the_second_channel_or_discard_typed_results() {
    let files = Files::new(SOURCE, r#"{"error_observables":[0]}"#);
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = make_registry(&calls, true);
    let file = files.load(&registry, options(HistorySampler::AffineSupport));
    let comparison = file.compare_samplers(&Default::default()).unwrap();
    assert_eq!(comparison.status(), SamplerComparisonStatus::Failed);
    assert_eq!(comparison.reason(), Some("pilot_incomplete"));
    assert!(comparison.decision().is_none());
    assert_eq!(
        comparison.affine_support().raw_result().unwrap().status,
        RunStatus::Failed
    );
    assert_eq!(comparison.affine_support().raw_result().unwrap().failed, 1);
    assert_eq!(
        comparison.exact_marginal().raw_result().unwrap().status,
        RunStatus::Complete
    );
    assert_eq!(
        comparison.exact_marginal().raw_result().unwrap().completed,
        2
    );
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    let json = comparison.to_json();
    assert_eq!(
        json["pilots"]["affine_support"]["sampling_receipt"]["failed"],
        1
    );
    assert!(json["gamma"].is_null());
    // An exact-prefix cache cap fails only the second preparation on this task.
    let f = Files::new("H 0\nM 0\nI 1 2", r#"{"sample_records":[0]}"#);
    let mut o = options(HistorySampler::AffineSupport);
    o.evaluation.runtime.compile.max_prepared_probabilities = 1;
    let capped = f
        .load(&ControllerRegistry::default(), o)
        .compare_samplers(&Default::default())
        .unwrap();
    assert_eq!(capped.status(), SamplerComparisonStatus::Failed);
    assert!(capped.affine_support().is_complete());
    assert!(capped.exact_marginal().batch().is_none());
    assert!(!capped.exact_marginal().preparation().is_complete());
    assert!(
        capped
            .exact_marginal()
            .preparation()
            .raw_result()
            .unwrap_err()
            .message
            .contains("max_prepared_probabilities")
    );
    assert_eq!(capped.affine_support().raw_result().unwrap().completed, 2);
}
#[test]
fn unavailable_efficiency_and_numeric_or_query_failures_never_fabricate_a_recommendation() {
    let files = Files::new("H 0\nM 0\nI 1 2", r#"{"sample_records":[0]}"#);
    let registry = ControllerRegistry::default();
    let file = files.load(&registry, options(HistorySampler::AffineSupport));
    let result = file.compare_samplers(&Default::default()).unwrap();
    assert_eq!(result.status(), SamplerComparisonStatus::Inconclusive);
    assert_eq!(result.reason(), Some("no_error_metric"));
    assert!(result.decision().is_none());
    assert!(result.affine_support().is_complete() && result.exact_marginal().is_complete());
    let files = Files::new(
        "I 0 1 2\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0]}"#,
    );
    let result = files
        .load(&registry, options(HistorySampler::AffineSupport))
        .compare_samplers(&Default::default())
        .unwrap();
    assert_eq!(result.status(), SamplerComparisonStatus::Inconclusive);
    assert_eq!(result.reason(), Some("zero_residual_variance"));
    assert!(result.to_json()["recommended_sampler"].is_null());
    let files = Files::new(SOURCE, r#"{"error_observables":[0]}"#);
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = make_registry(&calls, false);
    let mut config = FileSamplerComparisonOptions::default();
    config.pilot.execution_limits = ExecutionLimits {
        max_queries: 0,
        ..Default::default()
    };
    let result = files
        .load(&registry, options(HistorySampler::AffineSupport))
        .compare_samplers(&config)
        .unwrap();
    assert_eq!(result.status(), SamplerComparisonStatus::Failed);
    for p in [&result.affine_support(), &result.exact_marginal()] {
        assert_eq!(p.raw_result().unwrap().status, RunStatus::Failed);
        assert_eq!(p.raw_result().unwrap().execution_costs.query.attempted, 0);
    }
    let mut o = options(HistorySampler::AffineSupport);
    o.numeric.max_work_bits = 0;
    let result = files
        .load(&registry, o)
        .compare_samplers(&Default::default())
        .unwrap();
    assert_eq!(result.status(), SamplerComparisonStatus::Failed);
    for p in [&result.affine_support(), &result.exact_marginal()] {
        assert!(p.export_failure().is_some());
        assert_eq!(p.raw_result().unwrap().status, RunStatus::Complete);
        let json = p.to_json();
        assert!(json["sampling"].is_null());
        assert_eq!(json["sampling_receipt"]["completed"], 2);
        assert_eq!(json["sampling_receipt"]["estimates_persisted"], false);
    }
    assert_eq!(result.to_json()["automatic_switching"], json!(false));
}

#[test]
fn nonclifford_weighted_pilots_keep_true_sampler_statistics_and_prepared_caches() {
    let files = Files::new(
        "H 0\nT 0\nH 0\nM(1/7) 0\nI 1 2\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"sample_records":[0],"error_observables":[0]}"#,
    );
    let registry = ControllerRegistry::default();
    for compiler in [
        rmagna::unitary::CompilerStrategy::ParityCopies,
        rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
    ] {
        let mut configured = options(HistorySampler::ExactMarginal);
        configured.evaluation.runtime.compile.compiler_strategy = compiler;
        let file = files.load(&registry, configured);
        let mut config = FileSamplerComparisonOptions::default();
        config.pilot.shots = 32;
        config.pilot.seed = 71;
        let result = file.compare_samplers(&config).unwrap();
        assert_eq!(result.status(), SamplerComparisonStatus::Complete);
        let field = rmagna::field::FieldArithmetic::default();
        let thirty_two = field
            .from_rational(num_rational::BigRational::from_integer(32.into()))
            .unwrap();
        let affine = result
            .affine_support()
            .raw_result()
            .unwrap()
            .efficiency
            .as_ref()
            .unwrap();
        let exact = result
            .exact_marginal()
            .raw_result()
            .unwrap()
            .efficiency
            .as_ref()
            .unwrap();
        assert_eq!(affine.zero_weights, 0);
        assert!(affine.kappa.is_some());
        assert_eq!(
            field
                .subtract(&affine.effective_sample_size, &thirty_two)
                .unwrap()
                .real_sign()
                .unwrap(),
            std::cmp::Ordering::Less
        );
        assert_eq!(exact.effective_sample_size, thirty_two);
        for (pilot, method, seed) in [
            (result.affine_support(), HistorySampler::AffineSupport, 71),
            (result.exact_marginal(), HistorySampler::ExactMarginal, 72),
        ] {
            let mut configured = configured;
            configured.evaluation.runtime.history_sampler = method;
            let independent = files
                .load(&registry, configured)
                .prepare_sampling(&config.preparation)
                .unwrap();
            let batch = independent
                .run(&TaskRunOptions {
                    seed,
                    ..config.pilot
                })
                .unwrap();
            same(pilot.raw_result().unwrap(), batch.raw_result());
            let value = pilot.to_json();
            assert_support_cache_transition(
                &value["preparation"]["compilation"]["retained_caches"],
                &value["sampling"]["compilation"]["retained_caches"],
                method,
                1,
                2,
                2,
            );
        }
    }
}

#[test]
fn valid_loaded_metadata_failures_remain_two_owned_preparation_receipts() {
    let files = Files::new(SOURCE, r#"{"error_observables":[0]}"#);
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = make_registry(&calls, false);
    let mut configured = options(HistorySampler::ExactMarginal);
    let admitted = files.load(&registry, configured);
    let mut base_options = admitted.inspect(&TaskRunOptions::default()).unwrap()["options"].clone();
    base_options.as_object_mut().unwrap().remove("run");
    configured.fingerprints.max_json_bytes = serde_json::to_vec(&base_options).unwrap().len();
    let file = files.load(&registry, configured);
    let comparison = file.compare_samplers(&Default::default()).unwrap();
    assert_eq!(comparison.status(), SamplerComparisonStatus::Failed);
    for pilot in [comparison.affine_support(), comparison.exact_marginal()] {
        assert!(!pilot.preparation().is_complete());
        assert!(pilot.preparation().metadata_failure().is_some());
        assert!(pilot.preparation().stage().is_none());
        assert_eq!(pilot.preparation().execution_costs(), Default::default());
        assert!(pilot.batch().is_none() && pilot.raw_result().is_none());
        let receipt = pilot.to_json();
        assert!(receipt["preparation"]["fingerprints"].is_null());
        assert_eq!(receipt["preparation"]["failure"]["phase"], "metadata");
        assert_eq!(
            receipt["preparation"]["compilation"]["status"],
            "not_started"
        );
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    drop(file);
    drop(files);
    assert_eq!(
        comparison.to_json()["pilots"]["exact_marginal"]["preparation"]["history_sampler"],
        "exact_marginal"
    );
}

#[test]
fn later_run_descriptor_boundary_retains_completed_first_pilot_and_second_preparation() {
    let files = Files::new(SOURCE, r#"{"error_observables":[0]}"#);
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = make_registry(&calls, false);
    let mut configured = options(HistorySampler::AffineSupport);
    let run = TaskRunOptions {
        shots: 2,
        seed: 9,
        ..Default::default()
    };
    let admitted = files
        .load(&registry, configured)
        .prepare_sampling(&Default::default())
        .unwrap();
    let current = admitted.inspect(&run).unwrap();
    let next = admitted
        .inspect(&TaskRunOptions { seed: 10, ..run })
        .unwrap();
    let first_size = serde_json::to_vec(&current["options"]).unwrap().len();
    assert_eq!(
        serde_json::to_vec(&next["options"]).unwrap().len(),
        first_size + 1
    );
    assert!(
        serde_json::to_vec(&admitted.report().metadata()["options"])
            .unwrap()
            .len()
            < first_size
    );
    configured.fingerprints.max_json_bytes = first_size;
    let file = files.load(&registry, configured);
    let result = file
        .compare_samplers(&FileSamplerComparisonOptions {
            pilot: run,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(result.status(), SamplerComparisonStatus::Failed);
    assert!(result.affine_support().is_complete());
    assert_eq!(result.affine_support().raw_result().unwrap().completed, 2);
    let second = result.exact_marginal();
    assert!(second.preparation().is_complete());
    assert!(second.failure().is_some());
    assert!(second.batch().is_none() && second.raw_result().is_none());
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    let saved = result.to_json();
    drop(file);
    drop(files);
    assert_eq!(result.to_json(), saved);
}

#[test]
fn injected_second_pilot_decoder_failure_preserves_the_completed_first_channel() {
    let files = Files::new(SOURCE, r#"{"error_observables":[0]}"#);
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut registry = ControllerRegistry::default();
    // Deliberate stateful fault injection, not a production callback contract.
    registry
        .register("majority", "1", move |bits: &[bool]| {
            if counter.fetch_add(1, Ordering::Relaxed) == 2 {
                return Err("injected second-channel failure".into());
            }
            Ok(vec![bits.iter().filter(|b| **b).count() >= 2])
        })
        .unwrap();
    let result = files
        .load(&registry, options(HistorySampler::AffineSupport))
        .compare_samplers(&Default::default())
        .unwrap();
    assert!(result.affine_support().is_complete());
    assert_eq!(result.affine_support().raw_result().unwrap().completed, 2);
    assert_eq!(
        result.exact_marginal().raw_result().unwrap().status,
        RunStatus::Failed
    );
    assert_eq!(result.exact_marginal().raw_result().unwrap().failed, 1);
    assert_eq!(result.status(), SamplerComparisonStatus::Failed);
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    assert!(result.decision().is_none());
}

#[test]
fn projected_magic_target_pilots_preserve_literal_feedback_moments_and_normalization() {
    use num_rational::BigRational;
    use rmagna::field::{ExactField, FieldArithmetic};
    use rmagna::scalar::ScalarArithmetic;
    use rmagna::unitary::CompilerStrategy;
    let field = FieldArithmetic::default();
    let scalar = ScalarArithmetic::default();
    let rational = |n: i64, d: i64| {
        field
            .from_rational(BigRational::new(n.into(), d.into()))
            .unwrap()
    };
    let root_half_scalar = scalar
        .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
        .unwrap();
    let root_half = field.from_scalar(&root_half_scalar).unwrap();
    let half = scalar
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    let two = rational(2, 1);
    // Independent derivation fixed before evaluating this scenario:
    // E[(-1)^a]=sqrt(2)/2, readout flip1/3 and X noise1/5 give
    // E[Y]=(sqrt(2)/2)^2*(1/3)*(3/5)=1/10.
    // X remains sqrt(2)/2, Z=0, normalized |+i> target success11/20
    // and error9/20. Its independent projected auxiliary has raw norm1/2.
    let expected_error = rational(9, 20);
    let expected_success = rational(11, 20);
    let expected_y = rational(1, 10);
    let source = "H 0 1\nT 0 1\nH 0\nM(1/3) 0\nDETECTOR rec[-1]\nCONTROL[relay] det[0] -> ctrl[0]\nCX ctrl[0] 1\nX_ERROR(1/5) 1\nI 2";
    let task = r#"{"outputs":[1],"sample_records":[0,0],"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"1":1}},"observables":[[1,0],[1,1],[0,1]]}"#;
    let enumeration_task = r#"{"outputs":[1],"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"1":1}},"observables":[[1,0],[1,1],[0,1]]}"#;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let files = Files::new(source, task);
        let exact_files = Files::new(source, enumeration_task);
        for dir in [&files, &exact_files] {
            std::fs::write(dir.0.join("ideal.magna"), "H 0\nS 0\nH 1").unwrap();
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("relay", "literal-projected-v1", move |bits: &[bool]| {
                counter.fetch_add(1, Ordering::Relaxed);
                Ok(vec![bits[0]])
            })
            .unwrap();
        let mut configured = options(HistorySampler::ExactMarginal);
        configured.evaluation.runtime.compile.compiler_strategy = compiler;
        let file = files.load(&registry, configured);
        let mut affine_options = configured;
        affine_options.evaluation.runtime.history_sampler = HistorySampler::AffineSupport;
        let affine_single = files.load(&registry, affine_options);
        let exact_single = files.load(&registry, configured);
        let physical = exact_files.load(&registry, configured);
        drop(files);
        drop(exact_files);
        let mut exhaustive = rmagna::file_api::FileEnumerationOptions::default();
        exhaustive.enumeration.noise.max_weight = 2;
        let enumeration = physical.enumerate(&exhaustive).unwrap();
        let all = enumeration.raw_result().unwrap();
        assert_eq!(all.target_normalization.as_ref(), Some(&half));
        assert_eq!(
            all.covered_probability,
            rmagna::probability::Probability::one()
        );
        assert_eq!(
            all.omitted_probability,
            rmagna::probability::Probability::zero()
        );
        assert_eq!(all.masses.accepted, ExactField::one());
        assert_eq!(all.masses.error, expected_error);
        assert_eq!(all.masses.target_success.as_ref(), Some(&expected_success));
        assert_eq!(
            all.masses.moments,
            vec![root_half.clone(), expected_y.clone(), ExactField::zero()]
        );
        let config = FileSamplerComparisonOptions {
            pilot: TaskRunOptions {
                shots: 32,
                seed: 0x918,
                ..Default::default()
            },
            ..Default::default()
        };
        let unchanged = file.inspect(&config.pilot).unwrap();
        let count_before = calls.load(Ordering::Relaxed);
        let result = file.compare_samplers(&config).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed) - count_before, 64);
        assert_eq!(file.inspect(&config.pilot).unwrap(), unchanged);
        for (pilot, single, seed, method) in [
            (
                result.affine_support(),
                &affine_single,
                0x918,
                HistorySampler::AffineSupport,
            ),
            (
                result.exact_marginal(),
                &exact_single,
                0x919,
                HistorySampler::ExactMarginal,
            ),
        ] {
            assert!(pilot.is_complete());
            let prepared = single.prepare_sampling(&config.preparation).unwrap();
            assert_eq!(prepared.report().execution_costs().query.succeeded, 1);
            let batch = prepared
                .run(&TaskRunOptions {
                    seed,
                    ..config.pilot
                })
                .unwrap();
            same(pilot.raw_result().unwrap(), batch.raw_result());
            let statistics = pilot.raw_result().unwrap().statistics.as_ref().unwrap();
            assert_eq!(statistics.accepted, statistics.mean_weight);
            assert_eq!(statistics.moments[2], ExactField::zero());
            assert_eq!(
                statistics.moments[0],
                field.multiply(&root_half, &statistics.mean_weight).unwrap()
            );
            assert_eq!(
                field
                    .add(
                        &field
                            .multiply(statistics.error.as_ref().unwrap(), &two)
                            .unwrap(),
                        &statistics.moments[1]
                    )
                    .unwrap(),
                statistics.accepted
            );
            assert!(
                pilot
                    .raw_result()
                    .unwrap()
                    .sampled_records
                    .as_ref()
                    .unwrap()
                    .iter()
                    .all(|r| r.len() == 2 && r[0] == r[1])
            );
            let receipt = pilot.to_json();
            assert_support_cache_transition(
                &receipt["preparation"]["compilation"]["retained_caches"],
                &receipt["sampling"]["compilation"]["retained_caches"],
                method,
                1,
                2,
                2,
            );
            assert_eq!(
                receipt["sampling"]["fingerprints"],
                batch.metadata()["fingerprints"]
            );
        }
        assert_ne!(result.status(), SamplerComparisonStatus::Failed);
        if let Some(decision) = result.decision() {
            let value = result.to_json();
            let kappa = value["pilots"]["affine_support"]["sampling"]["efficiency"]["kappa"]
                .as_f64()
                .unwrap();
            let affine = result
                .affine_support()
                .raw_result()
                .unwrap()
                .timings
                .sampling_seconds;
            let exact = result
                .exact_marginal()
                .raw_result()
                .unwrap()
                .timings
                .sampling_seconds;
            assert_eq!(decision.gamma, kappa * (affine / exact));
        } else {
            assert!(result.reason().is_some());
        }
        let saved = result.to_json();
        drop(file);
        drop(affine_single);
        drop(exact_single);
        drop(physical);
        assert_eq!(result.to_json(), saved);
    }
}
