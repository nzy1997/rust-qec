use rmagna::costs::{ExecutionCosts, ExecutionLimits};
use rmagna::file_api::{FileLoadOptions, FilePreparationOptions, FileTask, FileTaskOptions};
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
    time::Duration,
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "rmagna-file-preparation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
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
fn target_files() -> Files {
    let files = Files::new();
    files.put(
        "circuit.magna",
        "H 0\nT 0\nH 0\nCX 0 1\nM(1/7) 1\nX_ERROR(1/4) 0\nI 2",
    );
    files.put("ideal.magna", "H 0\nCX 0 1");
    files.put("recovery.magna", "X 0");
    files.put("task.json",r#"{"outputs":[0],"sample_records":[0],"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"1":1}},"recovery":"recovery.magna","observables":[[0,1]]}"#);
    files
}
fn same_rows(a: &TaskRunResult, b: &TaskRunResult) {
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
    assert_eq!(a.completed_history_groups, b.completed_history_groups);
}
fn retained_after_sampling(
    mut before: Value,
    sampler: HistorySampler,
    labels: usize,
    quotient_rank: usize,
    terms: usize,
) -> Value {
    assert!(before["support"].is_null());
    if sampler == HistorySampler::AffineSupport {
        before["support"] = json!({
            "labels": labels,
            "quotient_rank": quotient_rank,
            "terms": terms,
            "method": "capped_probability_parent_affine_envelope"
        });
    }
    before
}
#[test]
fn fresh_preparation_owns_deleted_sources_and_separates_later_sampling_costs() {
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        let files = target_files();
        let file = files.load(&ControllerRegistry::default(), options(sampler));
        let config = FilePreparationOptions {
            execution_limits: ExecutionLimits {
                max_queries: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let prepared = file.prepare_sampling(&config).unwrap();
        let report = prepared.report();
        assert_eq!(report.raw_result(), Ok(()));
        assert!(report.is_complete());
        assert_eq!(report.execution_costs().compilation.succeeded, 2);
        assert_eq!(report.execution_costs().query.succeeded, 1);
        assert_eq!(report.execution_costs().decoder.attempted, 0);
        assert_eq!(report.execution_costs().support.attempted, 0);
        assert_eq!(
            report.stage().unwrap().all_families_retained(),
            sampler == HistorySampler::ExactMarginal
        );
        let receipt = report.to_json();
        assert_eq!(receipt["kind"], "preparation");
        assert_eq!(receipt["status"], "complete");
        assert!(receipt["options"].get("run").is_none());
        assert_eq!(
            receipt["options"]["sampling_preparation"]["execution_limits"]["max_queries"],
            1
        );
        assert!(receipt.get("estimates").is_none());
        assert!(
            receipt["compilation"]["preparation_included_in_this_call"]
                .as_bool()
                .unwrap()
        );
        assert!(
            receipt["compilation"]["query_preparation_included_in_this_call"]
                .as_bool()
                .unwrap()
        );
        assert!(report.total_seconds() >= report.stage().unwrap().elapsed_seconds);
        let run = TaskRunOptions {
            shots: 16,
            seed: 634,
            ..Default::default()
        };
        let before = prepared.inspect(&run).unwrap();
        let retained = before["compilation"]["retained_caches"].clone();
        let retained = retained_after_sampling(retained, sampler, 1, 2, 2);
        drop(files);
        let baseline = file.run(&run).unwrap();
        drop(file);
        let batch = prepared.run(&run).unwrap();
        same_rows(batch.raw_result(), baseline.raw_result());
        assert_eq!(batch.raw_result().status, RunStatus::Complete);
        assert_eq!(batch.raw_result().execution_costs.compilation.attempted, 0);
        assert_eq!(
            batch.raw_result().execution_costs.query.attempted + 1,
            baseline.raw_result().execution_costs.query.attempted
        );
        assert!(batch.raw_result().timings.preparation_seconds.is_none());
        let result = batch.to_json().unwrap();
        assert_eq!(result["compilation"]["retained_caches"], retained);
        assert_eq!(
            result["compilation"]["query_preparation"],
            "explicit_configured_sampling_families"
        );
        assert_eq!(
            result["compilation"]["query_preparation_included_in_this_call"],
            false
        );
        assert_eq!(
            result["compilation"]["preparation_included_in_this_call"],
            false
        );
        assert_eq!(
            result["compilation"]["query_preparation_receipt"],
            receipt["sampling_preparation"]
        );
        assert_eq!(before["fingerprints"], result["fingerprints"]);
        for category in [
            "circuit",
            "task",
            "ideal_state",
            "recovery",
            "controllers",
            "sampling",
        ] {
            assert_eq!(
                result["fingerprints"][category],
                baseline.metadata()["fingerprints"][category]
            );
        }
        assert_ne!(
            result["fingerprints"]["options"],
            baseline.metadata()["fingerprints"]["options"]
        );
        let again = prepared.run(&run).unwrap();
        same_rows(batch.raw_result(), again.raw_result());
        assert_eq!(
            prepared.inspect(&run).unwrap()["compilation"]["retained_caches"],
            retained
        );
        drop(prepared);
        assert_eq!(batch.to_json().unwrap(), result);
    }
}
#[test]
fn reused_preparation_has_fresh_quotas_and_an_owned_receipt_without_prior_compilation() {
    let files = target_files();
    let file = files.load(
        &ControllerRegistry::default(),
        options(HistorySampler::AffineSupport),
    );
    let compiled = file.compile().unwrap();
    let config = FilePreparationOptions {
        execution_limits: ExecutionLimits {
            max_queries: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let first = compiled.prepare_sampling(&config).unwrap();
    assert!(first.is_complete());
    assert!(first.stage().unwrap().counts.created > 0);
    assert_eq!(first.execution_costs(), ExecutionCosts::default());
    assert_eq!(
        first.metadata()["compilation"]["preparation_included_in_this_call"],
        false
    );
    assert_eq!(
        first.metadata()["compilation"]["preparation_costs"]["query"]["succeeded"],
        1
    );
    let expired = compiled
        .prepare_sampling(&FilePreparationOptions {
            execution_limits: ExecutionLimits {
                max_duration: Some(Duration::ZERO),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    assert!(!expired.is_complete());
    assert_eq!(expired.execution_costs(), ExecutionCosts::default());
    assert_eq!(expired.stage().unwrap().counts.attempted, 0);
    assert_eq!(expired.to_json()["failure"]["phase"], "sampling_families");
    let second = compiled.prepare_sampling(&config).unwrap();
    assert_eq!(second.raw_result(), Ok(()));
    let c = second.stage().unwrap().counts;
    assert_eq!(c.created, 0);
    assert_eq!(c.cold, 1);
    assert_eq!(c.reused + c.cold, c.planned);
    assert_eq!(second.execution_costs(), ExecutionCosts::default());
    let value = second.to_json();
    drop(compiled);
    drop(file);
    drop(files);
    assert_eq!(second.to_json(), value);
    assert_eq!(first.raw_result(), Ok(()));
}
fn noisy_files() -> Files {
    let files = Files::new();
    files.put("circuit.magna","H 0 1 2\nM(1/7) 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nE(1/5) ctrl[0] X3\nELSE_CORRELATED_ERROR(1/7) ctrl[0] Z3\nCX ctrl[0] 3\nM 3");
    files.put(
        "task.json",
        r#"{"outputs":[3],"sample_records":[0,3],"observables":[[0,1]]}"#,
    );
    files
}
#[test]
fn nonlinear_preparation_never_invokes_decoders_and_keeps_both_seeded_samplers() {
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        let files = noisy_files();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("majority", "1", move |bits: &[bool]| {
                count.fetch_add(1, Ordering::Relaxed);
                Ok(vec![bits.iter().filter(|v| **v).count() >= 2])
            })
            .unwrap();
        let file = files.load(&registry, options(sampler));
        let prepared = file.prepare_sampling(&Default::default()).unwrap();
        assert!(prepared.report().is_complete());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(prepared.report().stage().unwrap().history_rows, 6);
        let run = TaskRunOptions {
            shots: 24,
            seed: 179,
            ..Default::default()
        };
        let before = prepared.inspect(&run).unwrap()["compilation"]["retained_caches"].clone();
        let before = retained_after_sampling(before, sampler, 4, 0, 1);
        let a = prepared.run(&run).unwrap();
        let b = file.run(&run).unwrap();
        same_rows(a.raw_result(), b.raw_result());
        assert_eq!(calls.load(Ordering::Relaxed), 48);
        assert_eq!(
            prepared.inspect(&run).unwrap()["compilation"]["retained_caches"],
            before
        );
        assert!(
            a.raw_result()
                .sampled_records
                .as_ref()
                .unwrap()
                .iter()
                .any(|b| b[0])
        );
        assert!(
            a.raw_result()
                .sampled_records
                .as_ref()
                .unwrap()
                .iter()
                .any(|b| !b[0])
        );
    }
}
#[test]
fn failures_keep_compilation_and_family_receipts_and_block_implicit_sampling() {
    let files = target_files();
    let file = files.load(
        &ControllerRegistry::default(),
        options(HistorySampler::ExactMarginal),
    );
    for limits in [
        ExecutionLimits {
            max_queries: 0,
            ..Default::default()
        },
        ExecutionLimits {
            max_duration: Some(Duration::ZERO),
            ..Default::default()
        },
    ] {
        let failed = file
            .prepare_sampling(&FilePreparationOptions {
                execution_limits: limits,
                ..Default::default()
            })
            .unwrap();
        assert!(!failed.report().is_complete());
        assert!(failed.partial_task().is_none());
        assert!(failed.report().stage().is_none());
        assert!(failed.run(&TaskRunOptions::default()).is_err());
        let json = failed.report().to_json();
        assert_eq!(json["status"], "failed");
        assert_eq!(json["failure"]["phase"], "compilation");
        assert!(json["sampling_preparation"].is_null());
        assert_eq!(failed.report().execution_costs().query.attempted, 0);
    }
    let mut config = FilePreparationOptions::default();
    config.queries.max_requests = 1;
    let failed = file.prepare_sampling(&config).unwrap();
    assert!(!failed.report().is_complete());
    assert!(failed.partial_task().is_some());
    assert!(failed.inspect(&TaskRunOptions::default()).is_err());
    assert!(failed.run(&TaskRunOptions::default()).is_err());
    assert_eq!(failed.report().execution_costs().query.succeeded, 1);
    assert_eq!(failed.report().stage().unwrap().counts.attempted, 0);
    assert_eq!(
        failed.report().to_json()["failure"]["phase"],
        "sampling_families"
    );
    let (compiled, receipt) = failed.into_parts();
    let compiled = compiled.unwrap();
    let retry = compiled.prepare_sampling(&Default::default()).unwrap();
    assert!(retry.is_complete());
    assert!(retry.stage().unwrap().counts.created > 0);
    assert_eq!(retry.execution_costs(), ExecutionCosts::default());
    assert!(!receipt.is_complete());
    // A later family-cap failure keeps the earlier probability plans intact.
    let files = noisy_files();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("majority", "1", move |bits: &[bool]| {
            count.fetch_add(1, Ordering::Relaxed);
            Ok(vec![bits[0]])
        })
        .unwrap();
    let mut capped = options(HistorySampler::ExactMarginal);
    capped.evaluation.runtime.compile.max_prepared_probabilities = 1;
    let failed = files
        .load(&registry, capped)
        .prepare_sampling(&Default::default())
        .unwrap();
    assert!(!failed.report().is_complete());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let stage = failed.report().stage().unwrap();
    assert_eq!(stage.counts.created, 1);
    assert_eq!(stage.counts.failed, 1);
    assert!(
        stage
            .result
            .as_ref()
            .unwrap_err()
            .message
            .contains("max_prepared_probabilities")
    );
    let compiled = failed.partial_task().unwrap();
    assert_eq!(
        compiled.inspect(&Default::default()).unwrap()["compilation"]["retained_caches"]["probability"]
            ["cached_queries"],
        1
    );
    let retry = compiled.prepare_sampling(&Default::default()).unwrap();
    assert_eq!(retry.stage().unwrap().counts.created, 0);
    assert_eq!(retry.stage().unwrap().counts.reused, 1);
    assert_eq!(retry.stage().unwrap().counts.failed, 1);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let mut terms = options(HistorySampler::ExactMarginal);
    terms.evaluation.runtime.sampling.max_terms = 0;
    let failed = files
        .load(&registry, terms)
        .prepare_sampling(&Default::default())
        .unwrap();
    assert!(!failed.report().is_complete());
    assert_eq!(failed.report().stage().unwrap().counts.attempted, 1);
    assert_eq!(failed.report().stage().unwrap().counts.failed, 1);
    assert_eq!(failed.report().execution_costs().compilation.succeeded, 1);
    assert_eq!(failed.report().execution_costs().query.attempted, 0);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}
#[test]
fn cold_and_cold_prefix_modes_preserve_explicit_scope_and_configured_fingerprints() {
    let files = target_files();
    let registry = ControllerRegistry::default();
    let mut cold = options(HistorySampler::AffineSupport);
    cold.evaluation.runtime.compile.probability_preparation = QueryPreparation::Cold;
    cold.evaluation.runtime.compile.density_preparation = QueryPreparation::Cold;
    cold.evaluation.runtime.compile.moment_preparation = QueryPreparation::Cold;
    let file = files.load(&registry, cold);
    let prepared = file.prepare_sampling(&Default::default()).unwrap();
    assert!(prepared.report().is_complete());
    assert!(!prepared.report().stage().unwrap().all_families_retained());
    let value = prepared.report().to_json();
    assert_eq!(value["sampling_preparation"]["counts"]["cold"], 5);
    assert_eq!(value["sampling_preparation"]["counts"]["created"], 0);
    let run = TaskRunOptions {
        shots: 4,
        seed: 7,
        ..Default::default()
    };
    let a = prepared.run(&run).unwrap();
    let b = file.run(&run).unwrap();
    same_rows(a.raw_result(), b.raw_result());
    cold.evaluation.runtime.strategy = CausalStrategy::ColdPrefix;
    let failed = files
        .load(&registry, cold)
        .prepare_sampling(&Default::default())
        .unwrap();
    assert!(!failed.report().is_complete());
    assert!(
        failed
            .report()
            .stage()
            .unwrap()
            .result
            .as_ref()
            .unwrap_err()
            .message
            .contains("cold_prefix")
    );
    assert!(failed.partial_task().is_some());
    assert!(failed.run(&run).is_err());
    let file = files.load(&registry, options(HistorySampler::AffineSupport));
    let base = FilePreparationOptions::default();
    let reference = file.prepare_sampling(&base).unwrap().report().to_json();
    let mut configs = Vec::new();
    let mut c = base;
    c.queries.max_requests += 1;
    configs.push(c);
    let mut c = base;
    c.queries.max_history_rows += 1;
    configs.push(c);
    let mut c = base;
    c.queries.max_layout_rows += 1;
    configs.push(c);
    let mut c = base;
    c.execution_limits.max_queries = 30;
    configs.push(c);
    let mut c = base;
    c.execution_limits.max_duration = Some(Duration::new(100, 27));
    configs.push(c);
    for config in configs {
        let changed = file.prepare_sampling(&config).unwrap().report().to_json();
        for key in [
            "circuit",
            "task",
            "ideal_state",
            "recovery",
            "controllers",
            "sampling",
        ] {
            assert_eq!(reference["fingerprints"][key], changed["fingerprints"][key]);
        }
        assert_ne!(
            reference["fingerprints"]["options"],
            changed["fingerprints"]["options"]
        );
    }
    assert_eq!(
        reference["options"]["sampling_preparation"]["method"],
        json!("configured_reusable_sampling_families")
    );
    assert!(reference["options"].get("run").is_none());
    let _: Value = serde_json::from_str(&serde_json::to_string(&reference).unwrap()).unwrap();
}

#[test]
fn metadata_failures_own_fresh_and_reused_receipts_without_starting_families() {
    for padding in [0, 8] {
        let files = target_files();
        let mut opts = options(HistorySampler::ExactMarginal);
        let base = files.load(&ControllerRegistry::default(), opts);
        let mut resolved = base.inspect(&TaskRunOptions::default()).unwrap()["options"].clone();
        resolved.as_object_mut().unwrap().remove("run");
        // Loading admits immutable snapshots without run/preparation options.
        // Leave exactly the admitted base JSON budget; adding preparation
        // configuration then fails through the real public metadata path.
        opts.fingerprints.max_json_bytes = serde_json::to_vec(&resolved).unwrap().len() + padding;
        let file = files.load(&ControllerRegistry::default(), opts);
        let prepared = file.prepare_sampling(&Default::default()).unwrap();
        assert!(!prepared.report().is_complete());
        assert!(prepared.partial_task().is_none());
        assert!(prepared.report().stage().is_none());
        assert_eq!(
            prepared.report().execution_costs(),
            ExecutionCosts::default()
        );
        assert!(prepared.report().metadata_failure().is_some());
        assert_eq!(
            prepared.report().raw_result().unwrap_err().field,
            "preparation.metadata"
        );
        let owned = prepared.report().to_json();
        assert_eq!(owned["failure"]["phase"], "metadata");
        assert_eq!(owned["compilation"]["status"], "not_started");
        assert_eq!(
            owned["compilation"]["preparation_included_in_this_call"],
            true
        );
        assert!(owned["fingerprints"].is_null());
        assert_eq!(
            owned["options"]["sampling_preparation"]["method"],
            "configured_reusable_sampling_families"
        );
        assert!(prepared.run(&TaskRunOptions::default()).is_err());
        // Compilation itself has no provenance requirement. The failed reused
        // preparation must preserve this earlier paid work and a fresh zero quota.
        let compiled = file.compile().unwrap();
        let report = compiled
            .prepare_sampling(&FilePreparationOptions {
                execution_limits: ExecutionLimits {
                    max_queries: 0,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
        assert!(!report.is_complete());
        assert_eq!(report.execution_costs(), ExecutionCosts::default());
        assert!(report.stage().is_none());
        assert!(report.metadata_failure().is_some());
        let json = report.to_json();
        assert_eq!(json["failure"]["phase"], "metadata");
        assert_eq!(json["compilation"]["status"], "compiled");
        assert_eq!(
            json["compilation"]["preparation_included_in_this_call"],
            false
        );
        assert_eq!(
            json["compilation"]["preparation_costs"]["compilation"]["succeeded"],
            2
        );
        assert_eq!(
            json["compilation"]["preparation_costs"]["query"]["succeeded"],
            1
        );
        drop(compiled);
        drop(file);
        drop(files);
        assert_eq!(report.to_json(), json);
        assert_eq!(prepared.report().to_json(), owned);
    }
}

#[test]
fn file_policy_matrix_preserves_literal_projected_target_feedback_and_fresh_batches() {
    use num_rational::BigRational;
    use rmagna::field::{ExactField, FieldArithmetic};
    use rmagna::unitary::CompilerStrategy;
    let field = FieldArithmetic::default();
    let two = field
        .from_rational(BigRational::from_integer(2.into()))
        .unwrap();
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
                let files = Files::new();
                files.put("circuit.magna", "H 0\nT 0\nH 0\nM(1/7) 0\nDETECTOR rec[-1]\nH 1\nS 1\nCONTROL[copy] det[0] -> ctrl[0]\nCONTROL[zero] -> ctrl[1]\nCONTROL[join] ctrl[0] ctrl[1] -> ctrl[2]\nE(1/5) ctrl[0] X0\nELSE_CORRELATED_ERROR(1/7) ctrl[0] Z0\nCX ctrl[2] 1");
                files.put("ideal.magna", "H 0\nS 0\nH 1\nCX 1 2");
                files.put("task.json", r#"{"outputs":[1],"sample_records":[0,0],"ideal_state":{"circuit":"ideal.magna","outputs":[0],"postselection":{"2":0}},"observables":[[1,1],[0,1]]}"#);
                let calls = Arc::new(AtomicUsize::new(0));
                let mut registry = ControllerRegistry::default();
                for name in ["copy", "zero", "join"] {
                    let count = calls.clone();
                    registry
                        .register(name, "matrix-v1", move |bits: &[bool]| {
                            count.fetch_add(1, Ordering::Relaxed);
                            Ok(vec![match name {
                                "copy" => bits[0],
                                "zero" => false,
                                _ => bits[0] ^ bits[1],
                            }])
                        })
                        .unwrap();
                }
                let mut opts = options(sampler);
                opts.evaluation.runtime.compile.compiler_strategy = compiler;
                opts.evaluation.runtime.compile.probability_preparation = policy;
                opts.evaluation.runtime.compile.density_preparation = policy;
                opts.evaluation.runtime.compile.moment_preparation = policy;
                let file = files.load(&registry, opts);
                let prepared = file.prepare_sampling(&Default::default()).unwrap();
                assert!(prepared.report().is_complete());
                assert_eq!(calls.load(Ordering::Relaxed), 0);
                assert_eq!(prepared.report().execution_costs().query.succeeded, 1);
                assert_eq!(prepared.report().execution_costs().decoder.attempted, 0);
                assert_eq!(
                    prepared.report().stage().unwrap().all_families_retained(),
                    sampler == HistorySampler::ExactMarginal
                        && !matches!(policy, QueryPreparation::Cold)
                );
                let run = TaskRunOptions {
                    shots: 64,
                    seed: 0x917,
                    ..Default::default()
                };
                let before = prepared.inspect(&run).unwrap();
                let retained = before["compilation"]["retained_caches"].clone();
                let retained = retained_after_sampling(retained, sampler, 1, 2, 2);
                let receipt = prepared.report().to_json();
                drop(files);
                let baseline = file.run(&run).unwrap();
                let batch = prepared.run(&run).unwrap();
                same_rows(baseline.raw_result(), batch.raw_result());
                assert_eq!(batch.raw_result().status, RunStatus::Complete);
                assert_eq!(calls.load(Ordering::Relaxed), 3 * 128);
                assert_eq!(batch.raw_result().execution_costs.compilation.attempted, 0);
                let statistics = batch.raw_result().statistics.as_ref().unwrap();
                // The projected Bell auxiliary leaves |+i><+i|/2. After target
                // normalization, reported r controls X on |+i>, so error=r,
                // Y=1-2r and Z=0 for every assignment. Guarded faults act only
                // on the already recorded, traced-out qubit and cannot change
                // these identities. They hold with affine importance weights.
                assert_eq!(statistics.accepted, statistics.mean_weight);
                assert_eq!(statistics.moments[1], ExactField::zero());
                assert_eq!(
                    field
                        .add(
                            &field
                                .multiply(statistics.error.as_ref().unwrap(), &two)
                                .unwrap(),
                            &statistics.moments[0]
                        )
                        .unwrap(),
                    statistics.accepted
                );
                let rows = batch.raw_result().sampled_records.as_ref().unwrap();
                assert!(rows.iter().all(|r| r.len() == 2 && r[0] == r[1]));
                if sampler == HistorySampler::ExactMarginal {
                    assert_eq!(statistics.accepted, ExactField::one());
                    let ones = rows.iter().filter(|r| r[0]).count();
                    let error = field
                        .from_rational(BigRational::new(ones.into(), 64.into()))
                        .unwrap();
                    assert_eq!(statistics.error.as_ref().unwrap(), &error);
                    assert_eq!(
                        statistics.moments[0],
                        field
                            .subtract(&ExactField::one(), &field.multiply(&two, &error).unwrap())
                            .unwrap()
                    );
                }
                assert_eq!(
                    prepared.inspect(&run).unwrap()["compilation"]["retained_caches"],
                    retained
                );
                let calls_before = calls.load(Ordering::Relaxed);
                let rejected = prepared
                    .run(&TaskRunOptions {
                        execution_limits: ExecutionLimits {
                            max_queries: 0,
                            ..Default::default()
                        },
                        ..run
                    })
                    .unwrap();
                assert_eq!(rejected.raw_result().status, RunStatus::Failed);
                assert_eq!(rejected.raw_result().completed, 0);
                // Affine support constructs no scalar query before resolving
                // these three callbacks; the zero scalar quota then rejects the
                // final contraction. Exact marginal rejects before callbacks.
                let failed_callbacks = calls.load(Ordering::Relaxed) - calls_before;
                assert_eq!(
                    u64::try_from(failed_callbacks).unwrap(),
                    rejected.raw_result().execution_costs.decoder.attempted
                );
                assert_eq!(
                    failed_callbacks,
                    if sampler == HistorySampler::AffineSupport {
                        3
                    } else {
                        0
                    }
                );
                let again = prepared.run(&run).unwrap();
                same_rows(batch.raw_result(), again.raw_result());
                assert_eq!(prepared.report().to_json(), receipt);
                assert_eq!(
                    prepared.inspect(&run).unwrap()["compilation"]["retained_caches"],
                    retained
                );
                assert_eq!(batch.metadata()["fingerprints"], before["fingerprints"]);
                assert_eq!(
                    batch.metadata()["compilation"]["query_preparation_included_in_this_call"],
                    false
                );
            }
        }
    }
}
