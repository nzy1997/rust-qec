// SPDX-License-Identifier: MPL-2.0
//! Original FileReuse dynamic callback/guarded-noise fixture through real caches.
use super::*;
use rmagna::instrument::{QueryCachePolicy, QueryPreparation};
const SOURCE: &str = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 1\nX_ERROR(1/2) ctrl[0] 1\nM 1";
const TASK: &str = r#"{"outputs":[1],"ideal_state":"zero.magna"}"#;
fn registry() -> (ControllerRegistry, Arc<Mutex<Vec<Vec<bool>>>>) {
    let calls = Arc::new(Mutex::new(vec![]));
    let seen = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |b: &[bool]| {
            seen.lock().unwrap().push(b.to_vec());
            Ok(b.to_vec())
        })
        .unwrap();
    (registry, calls)
}
fn evaluator(
    files: &Files,
    task: &str,
    registry: &ControllerRegistry,
    options: EvaluationOptions,
) -> TaskEvaluator {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(SOURCE, &parse).unwrap();
    let modules = parse_task(task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&physical, &parse)
        .unwrap();
    TaskEvaluator::new(&physical, &modules, registry, options).unwrap()
}
fn configured(
    mut options: EvaluationOptions,
    mode: QueryPreparation,
    lru: bool,
) -> EvaluationOptions {
    let c = &mut options.runtime.compile;
    c.probability_preparation = mode;
    c.density_preparation = mode;
    c.moment_preparation = mode;
    if lru {
        c.query_cache_policy = QueryCachePolicy::Lru;
        c.max_prepared_probabilities = 1;
        c.max_prepared_densities = 1;
        c.max_prepared_moments = 1;
    }
    options
}
pub fn original_dynamic_file_samples_use_automatic_eviction_without_memoizing_callbacks_or_changing_rng()
 {
    let files = Files::new();
    let mut draws = 0;
    for options in configurations()
        .into_iter()
        .filter(|o| o.runtime.strategy == CausalStrategy::FixedTemplate)
    {
        for mode in [
            QueryPreparation::Trace,
            QueryPreparation::FixedFibers(Default::default()),
            QueryPreparation::FixedCatalogue(Default::default()),
            QueryPreparation::Native(Default::default()),
        ] {
            let base = configured(options, mode, false);
            let lru = configured(options, mode, true);
            let (registry, calls) = registry();
            let full = evaluator(&files, TASK, &registry, base);
            let small = evaluator(&files, TASK, &registry, lru);
            let mut first = SplitMix64::new(5);
            let mut second = SplitMix64::new(5);
            let expected = full
                .samples(&mut first, 30)
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            let expected_calls = calls.lock().unwrap().clone();
            assert_eq!(expected_calls.len(), 30);
            calls.lock().unwrap().clear();
            let actual = small
                .samples(&mut second, 30)
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            assert!(actual == expected);
            assert_eq!(first.state(), second.state());
            assert_eq!(*calls.lock().unwrap(), expected_calls);
            assert_eq!(
                expected_calls
                    .iter()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>(),
                [vec![false], vec![true]].into_iter().collect()
            );
            assert_eq!(
                small
                    .prepared_probability_statistics()
                    .unwrap()
                    .unwrap()
                    .cached_queries,
                1
            );
            // ExactMarginal requests both the empty and one-record layout. The
            // affine sampler needs only one probability layout in this source.
            if options.runtime.history_sampler == HistorySampler::ExactMarginal {
                assert!(
                    small
                        .prepared_probability_statistics()
                        .unwrap()
                        .unwrap()
                        .cache_evictions
                        > 0
                );
            }
            draws += 60;
            calls.lock().unwrap().clear();
            let file = files.load(SOURCE, TASK, &registry, lru, NoiseSampling::Physical);
            let baseline = files.load(SOURCE, TASK, &registry, base, NoiseSampling::Physical);
            let run = TaskRunOptions {
                shots: 30,
                seed: 5,
                execution_limits: ExecutionLimits {
                    max_queries: 10000,
                    ..Default::default()
                },
                ..Default::default()
            };
            let a = file.run(&run).unwrap();
            let ac = calls.lock().unwrap().clone();
            calls.lock().unwrap().clear();
            let b = baseline.run(&run).unwrap();
            let ar = a.raw_result();
            let br = b.raw_result();
            assert_eq!(ar.status, RunStatus::Complete);
            assert_eq!(ar.completed, 30);
            assert_eq!(ar.statistics, br.statistics);
            assert_eq!(ar.efficiency, br.efficiency);
            assert_eq!(ar.sampled_records, br.sampled_records);
            assert_eq!(ar.completed_history_groups, br.completed_history_groups);
            assert_eq!(ac, *calls.lock().unwrap());
            assert_eq!(ac.len(), 30);
            assert_eq!(
                a.metadata()["compilation"]["retained_caches"]["probability"]["cached_queries"],
                1
            );
            if options.runtime.history_sampler == HistorySampler::ExactMarginal {
                assert!(
                    a.metadata()["compilation"]["retained_caches"]["probability"]["cache_evictions"]
                        .as_u64()
                        .unwrap() > 0
                );
            }
            draws += 60;
        }
    }
    assert_eq!(draws, 1920);
}
#[test]
fn lru_preparation_receipt_distinguishes_completed_construction_from_retention_and_does_not_run_callbacks()
 {
    let files = Files::new();
    let task = r#"{"outputs":[1],"ideal_state":"zero.magna","observables":[[0,1],[1,0],[1,1]]}"#;
    for options in configurations()
        .into_iter()
        .filter(|o| o.runtime.strategy == CausalStrategy::FixedTemplate)
    {
        let (registry, calls) = registry();
        let small = evaluator(
            &files,
            task,
            &registry,
            configured(
                options,
                QueryPreparation::FixedFibers(Default::default()),
                true,
            ),
        );
        let meter = ExecutionMeter::default();
        let receipt = small.prepare_sampling_queries(Default::default(), &meter);
        assert!(receipt.result.is_ok(), "{:?}", receipt.result);
        assert!(!receipt.all_families_retained());
        assert!(receipt.cache_evictions >= 2);
        assert_eq!(receipt.counts.failed, 0);
        assert_eq!(meter.snapshot(), ExecutionCosts::default());
        assert!(calls.lock().unwrap().is_empty());
        let full = evaluator(
            &files,
            task,
            &registry,
            configured(
                options,
                QueryPreparation::FixedFibers(Default::default()),
                false,
            ),
        );
        let mut first = SplitMix64::new(51);
        let mut second = SplitMix64::new(51);
        for _ in 0..30 {
            assert!(small.sample(&mut first).unwrap() == full.sample(&mut second).unwrap());
        }
        assert_eq!(first.state(), second.state());
        assert_eq!(
            small
                .prepared_moment_statistics()
                .unwrap()
                .unwrap()
                .cached_queries,
            1
        );
        assert!(
            small
                .prepared_moment_statistics()
                .unwrap()
                .unwrap()
                .cache_evictions
                >= 92
        );
        assert_eq!(calls.lock().unwrap().len(), 60);
    }
}

#[test]
fn automatic_eviction_keeps_actual_callback_failure_and_the_completed_file_prefix() {
    let files = Files::new();
    for options in configurations()
        .into_iter()
        .filter(|o| o.runtime.strategy == CausalStrategy::FixedTemplate)
    {
        let mut outputs = vec![];
        for lru in [false, true] {
            let calls = Arc::new(Mutex::new(vec![]));
            let seen = Arc::clone(&calls);
            let mut registry = ControllerRegistry::default();
            registry
                .register("echo", "1", move |b: &[bool]| {
                    let mut calls = seen.lock().unwrap();
                    calls.push(b.to_vec());
                    if calls.len() == 4 {
                        Err("dynamic eviction callback failure".into())
                    } else {
                        Ok(b.to_vec())
                    }
                })
                .unwrap();
            let file = files.load(
                SOURCE,
                TASK,
                &registry,
                configured(options, QueryPreparation::Native(Default::default()), lru),
                NoiseSampling::Physical,
            );
            let report = file
                .run(&TaskRunOptions {
                    shots: 30,
                    seed: 5,
                    ..Default::default()
                })
                .unwrap();
            let result = report.into_raw_result();
            assert_eq!(
                (result.completed, result.failed, result.not_attempted),
                (3, 1, 26)
            );
            assert_eq!(result.status, RunStatus::Failed);
            assert_eq!(result.failures[0].shot, Some(3));
            assert!(
                result.failures[0]
                    .message
                    .contains("dynamic eviction callback failure")
            );
            assert_eq!(result.execution_costs.decoder.attempted, 4);
            assert_eq!(result.execution_costs.decoder.failed, 1);
            assert!(result.statistics.is_none());
            outputs.push((result, calls.lock().unwrap().clone()));
        }
        let (small, sc) = outputs.pop().unwrap();
        let (full, fc) = outputs.pop().unwrap();
        assert_eq!(small.partial_statistics, full.partial_statistics);
        assert_eq!(small.efficiency, full.efficiency);
        assert_eq!(small.sampled_records, full.sampled_records);
        assert_eq!(small.failures, full.failures);
        assert_eq!(sc, fc);
        assert_eq!(sc.len(), 4);
    }
}
