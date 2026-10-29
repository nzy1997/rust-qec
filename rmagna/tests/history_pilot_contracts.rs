//! Finite original pilot and decision contracts from
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_history_efficiency.py. Native source is data, never imported.

use rmagna::field::ExactField;
use rmagna::file_api::{
    FileLoadOptions, FileSamplerComparisonOptions, FileTask, FileTaskOptions,
    SamplerComparisonStatus, SamplerDecision,
};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::task::{RunStatus, TaskRunOptions, TaskRunResult};
use serde_json::json;
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Files(PathBuf);
impl Files {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let files = Self(std::env::temp_dir().join(format!(
            "rmagna-original-pilot-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&files.0).unwrap();
        std::fs::write(files.0.join("physical.magna"), source).unwrap();
        std::fs::write(files.0.join("target.magna"), "I 0\n").unwrap();
        std::fs::write(
            files.0.join("task.json"),
            json!({
                "outputs":[0], "ideal_state":{"circuit":"target.magna","outputs":[0]}
            })
            .to_string(),
        )
        .unwrap();
        files
    }
    fn load(&self, options: FileTaskOptions, registry: &ControllerRegistry) -> FileTask {
        FileTask::load(
            &self.0.join("physical.magna"),
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
    // Explicit fixed families match the native prepared-family role. This is
    // not an assertion that the two implementations share planner defaults.
    options.evaluation.runtime.compile.probability_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options.evaluation.runtime.compile.density_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options.evaluation.runtime.compile.moment_preparation =
        QueryPreparation::FixedFibers(Default::default());
    options
}
fn same_draws(a: &TaskRunResult, b: &TaskRunResult) {
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
}

#[test]
fn original_thirty_two_draw_pilots_keep_configuration_and_report_point_gamma() {
    let files = Files::new("X_ERROR(1/2) 0\n");
    let registry = ControllerRegistry::default();
    let configured = options(HistorySampler::ExactMarginal);
    let file = files.load(configured, &registry);
    let config = FileSamplerComparisonOptions {
        pilot: TaskRunOptions {
            shots: 32,
            seed: 13,
            ..Default::default()
        },
        ..Default::default()
    };
    let before = file.inspect(&config.pilot).unwrap();
    assert_eq!(before["history_sampler"], "exact_marginal");
    let result = file.compare_samplers(&config).unwrap();
    assert_eq!(result.status(), SamplerComparisonStatus::Complete);
    assert!(result.reason().is_none());
    assert_eq!(result.pilot_shots_per_channel(), 32);
    assert_eq!(file.inspect(&config.pilot).unwrap(), before);
    assert_eq!(
        configured.evaluation.runtime.history_sampler,
        HistorySampler::ExactMarginal
    );
    assert_eq!((config.pilot.shots, config.pilot.seed), (32, 13));
    for (pilot, sampler, seed, name) in [
        (
            result.affine_support(),
            HistorySampler::AffineSupport,
            13,
            "affine_support",
        ),
        (
            result.exact_marginal(),
            HistorySampler::ExactMarginal,
            14,
            "exact_marginal",
        ),
    ] {
        assert!(pilot.is_complete());
        assert_eq!(pilot.history_sampler(), sampler);
        assert_eq!(pilot.seed(), seed);
        assert!(
            pilot.preparation().total_seconds().is_finite()
                && pilot.preparation().total_seconds() > 0.0
        );
        assert!(pilot.preparation().is_complete());
        let raw = pilot.raw_result().unwrap();
        assert_eq!(
            (
                raw.status,
                raw.requested,
                raw.completed,
                raw.failed,
                raw.not_attempted,
                raw.seed
            ),
            (RunStatus::Complete, 32, 32, 0, 0, seed)
        );
        assert_eq!(raw.execution_costs.decoder.attempted, 0);
        assert_eq!(
            raw.efficiency.as_ref().unwrap().kappa,
            Some(ExactField::one())
        );
        let value = pilot.to_json();
        assert_eq!(value["history_sampler"], name);
        assert_eq!(value["sampling"]["history_sampler"], name);
        assert_eq!(value["sampling"]["efficiency"]["kappa"], 1.0);
        assert_eq!(value["preparation"]["timings"]["sampling_seconds"], 0.0);
        let independent = files
            .load(options(sampler), &registry)
            .prepare_sampling(&config.preparation)
            .unwrap();
        let batch = independent
            .run(&TaskRunOptions {
                seed,
                ..config.pilot
            })
            .unwrap();
        same_draws(raw, batch.raw_result());
    }
    let decision = result.decision().unwrap();
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
    assert!(affine > 0.0 && exact > 0.0);
    assert_eq!(decision.affine_support_seconds_per_draw, affine / 32.0);
    assert_eq!(decision.exact_marginal_seconds_per_draw, exact / 32.0);
    assert_eq!(
        decision.gamma,
        decision.affine_support_seconds_per_draw / decision.exact_marginal_seconds_per_draw
    );
    let value = result.to_json();
    assert_eq!(value["kappa"], 1.0);
    assert_eq!(value["gamma"], decision.gamma);
    assert_eq!(value["automatic_switching"], false);
    assert_eq!(value["production_started"], false);
    assert_eq!(file.inspect(&config.pilot).unwrap(), before);
    let identity = Files::new("I 0\n");
    let inconclusive = identity
        .load(configured, &registry)
        .compare_samplers(&FileSamplerComparisonOptions::default())
        .unwrap();
    assert_eq!(inconclusive.status(), SamplerComparisonStatus::Inconclusive);
    assert!(inconclusive.decision().is_none());
    for pilot in [inconclusive.affine_support(), inconclusive.exact_marginal()] {
        assert!(pilot.is_complete());
        assert_eq!(pilot.raw_result().unwrap().completed, 2);
    }
    assert!(inconclusive.to_json()["gamma"].is_null());
    assert!(inconclusive.to_json()["recommended_sampler"].is_null());
}

#[test]
fn original_typed_count_rejections_and_literal_native_decisions_are_witnessed() {
    assert!(NonZeroU64::new(0).is_none());
    assert!(u64::try_from(-1_i64).is_err());
    let files = Files::new("H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[probe] det[0] -> ctrl[0]\n");
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("probe", "original-count-v1", move |bits: &[bool]| {
            counter.fetch_add(1, Ordering::Relaxed);
            Ok(vec![bits[0]])
        })
        .unwrap();
    let mut configured = options(HistorySampler::ExactMarginal);
    configured
        .evaluation
        .runtime
        .compile
        .max_prepared_probabilities = 0;
    let file = files.load(configured, &registry);
    for shots in [0, 1] {
        let mut comparison = FileSamplerComparisonOptions::default();
        comparison.pilot.shots = shots;
        comparison.preparation.queries.max_requests = 0;
        let error = file.compare_samplers(&comparison).unwrap_err();
        assert!(error.to_string().contains("at least two"));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
    let mut impossible = FileSamplerComparisonOptions::default();
    impossible.preparation.queries.max_requests = 0;
    let canary = file.compare_samplers(&impossible).unwrap();
    assert_eq!(canary.status(), SamplerComparisonStatus::Failed);
    assert!(!canary.affine_support().preparation().is_complete());
    assert!(!canary.exact_marginal().preparation().is_complete());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    // Literal independent native examples: Gamma=2 selects exact, Gamma=1/2
    // selects affine; common count10 gives per-draw point times1/5 and2/5.
    for (kappa, gamma, sampler) in [
        (4.0, 2.0, HistorySampler::ExactMarginal),
        (1.0, 0.5, HistorySampler::AffineSupport),
    ] {
        let decision = SamplerDecision::from_pilot_totals(kappa, 2.0, 4.0, 10).unwrap();
        assert_eq!(decision.gamma, gamma);
        assert_eq!(decision.affine_support_seconds_per_draw, 0.2);
        assert_eq!(decision.exact_marginal_seconds_per_draw, 0.4);
        assert_eq!(decision.recommended_sampler, sampler);
    }
}
