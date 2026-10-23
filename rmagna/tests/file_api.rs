use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionLimits, OperationCost};
use rmagna::file_api::{FileApiError, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::instrument::{InsertionPreparation, QueryPreparation};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::task::{TaskOptions, TaskRunOptions, TaskRunResult, parse_task, run_task_batch};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-file-report-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
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
fn options() -> FileTaskOptions {
    let mut o = FileTaskOptions::default();
    o.evaluation.runtime.compile.insertion_preparation =
        InsertionPreparation::Prepared(Default::default());
    o.evaluation.runtime.compile.probability_preparation =
        QueryPreparation::FixedFibers(Default::default());
    o.evaluation.runtime.compile.density_preparation =
        QueryPreparation::FixedFibers(Default::default());
    o
}
fn loaded(
    source: &str,
    task: &str,
    path: &Path,
    registry: &ControllerRegistry,
    options: FileTaskOptions,
    sampling: NoiseSampling,
) -> FileTask {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, path, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    FileTask::from_loaded(
        circuit,
        modules,
        registry,
        sampling,
        options,
        BTreeMap::new(),
    )
    .unwrap()
}
fn counts(c: OperationCost) -> (u64, u64, u64) {
    (c.attempted, c.succeeded, c.failed)
}
fn same_rows(a: &TaskRunResult, b: &TaskRunResult) {
    assert_eq!(
        (a.requested, a.completed, a.failed, a.not_attempted, a.seed),
        (b.requested, b.completed, b.failed, b.not_attempted, b.seed)
    );
    assert_eq!(a.status, b.status);
    assert_eq!(a.statistics, b.statistics);
    assert_eq!(a.partial_statistics, b.partial_statistics);
    assert_eq!(a.efficiency, b.efficiency);
    assert_eq!(a.sampled_records, b.sampled_records);
    assert_eq!(a.failures, b.failures);
}
#[test]
fn owned_and_reused_reports_bind_projected_target_recovery_and_exact_metrics() {
    let f = Files::new();
    f.put("target.magna", "H 0\nCX 0 1");
    f.put("recovery.magna", "X 0");
    let source = "H 0\nCX 0 1\nM 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
    let task = r#"{"outputs":[0],"ideal_state":{"circuit":"target.magna","outputs":[1],"postselection":{"0":0}},"recovery":"recovery.magna","observables":[[0,1]]}"#;
    let registry = ControllerRegistry::default();
    let o = options();
    let file = loaded(source, task, &f.0, &registry, o, NoiseSampling::Physical);
    let run = TaskRunOptions {
        shots: 4,
        seed: 91,
        ..Default::default()
    };
    let inspection = file.inspect(&run).unwrap();
    assert_eq!(inspection["kind"], "inspection");
    assert_eq!(inspection["route"], "not_compiled");
    let compiled = file.compile().unwrap();
    let before = compiled.inspect(&run).unwrap();
    assert_eq!(before["compilation"]["query_preparation"], "on_demand");
    assert_eq!(
        before["compilation"]["retained_caches"]["probability"]["cached_queries"],
        0
    );
    let fresh = file.run(&run).unwrap();
    let first = compiled.run(&run).unwrap();
    let second = compiled.run(&run).unwrap();
    same_rows(fresh.raw_result(), first.raw_result());
    same_rows(first.raw_result(), second.raw_result());
    let a = fresh.to_json().unwrap();
    let b = first.to_json().unwrap();
    let c = second.to_json().unwrap();
    assert_eq!(a["schema_version"], 2);
    assert_eq!(a["software"]["name"], "rmagna");
    assert!(
        a["software_version"]
            .as_str()
            .unwrap()
            .starts_with("rmagna-")
    );
    assert_eq!(a["status"], "complete");
    assert_eq!(a["route"], "linear_affine");
    assert_eq!(a["estimates"]["acceptance"], 0.5);
    assert_eq!(a["estimates"]["error"], 0.5);
    assert_eq!(a["estimates"]["conditional_error"], 1.0);
    assert_eq!(a["estimates"]["moments"], json!([-0.5]));
    assert_eq!(a["standard_errors"]["acceptance"], 0.0);
    assert_eq!(a["standard_errors"]["conditional_error"], 0.0);
    assert_eq!(a["fingerprints"], b["fingerprints"]);
    assert_eq!(b["fingerprints"], c["fingerprints"]);
    assert_eq!(a["options"]["run"]["seed"], run.seed);
    assert_eq!(a["resources"]["limits"], a["options"]);
    assert_eq!(
        a["evaluation"]["ideal_state"]["postselection"],
        json!({"0":false})
    );
    assert_eq!(a["evaluation"]["output_mapping"], json!([0]));
    assert_eq!(
        a["evaluation"]["observables"],
        "after recovery, before target projection"
    );
    assert_eq!(a["compilation"]["preparation_included_in_this_call"], true);
    assert_eq!(b["compilation"]["preparation_included_in_this_call"], false);
    assert!(a["timings"]["preparation_seconds"].is_number());
    assert!(b["timings"]["preparation_seconds"].is_null());
    assert_eq!(b["resources"]["compilations"], 0);
    assert_eq!(c["resources"]["compilations"], 0);
    assert_eq!(
        a["resources"]["queries"].as_u64().unwrap(),
        b["resources"]["queries"].as_u64().unwrap()
            + b["compilation"]["preparation_costs"]["query"]["attempted"]
                .as_u64()
                .unwrap()
    );
    assert_eq!(b["compilation"], c["compilation"]);
    assert_eq!(b["timings"]["components_are_additive"], false);
    assert_eq!(
        b["resources"]["queries"],
        b["execution_costs"]["query"]["attempted"]
    );
    assert_eq!(
        b["compilation"]["retained_caches"]["insertions"]["catalog_entries"],
        1 // The measurement's zero-probability report-flip alternative is retained.
    );
    let parse = ParseOptions::default();
    let circuit = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, &f.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    let raw = run_task_batch(
        &circuit,
        &modules,
        &registry,
        o.evaluation,
        &NoiseSampling::Physical,
        &run,
    )
    .unwrap();
    same_rows(fresh.raw_result(), &raw);
    for (x, y) in [
        (
            fresh.raw_result().execution_costs.query,
            raw.execution_costs.query,
        ),
        (
            fresh.raw_result().execution_costs.compilation,
            raw.execution_costs.compilation,
        ),
    ] {
        assert_eq!(counts(x), counts(y));
    }
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&serde_json::to_string(&a).unwrap()).unwrap(),
        a
    );
}
#[test]
fn loaded_files_and_controller_bindings_remain_snapshots_after_mutation() {
    let a = Files::new();
    let b = Files::new();
    for f in [&a, &b] {
        f.put(
            "physical.magna",
            "H 0\nCX 0 1\nM 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
        );
        f.put("target.magna", "H 0\nCX 0 1");
        f.put("recovery.magna", "X 0");
        f.put("task.json",r#"{"outputs":[0],"ideal_state":{"circuit":"target.magna","outputs":[1],"postselection":{"0":0}},"recovery":"recovery.magna","observables":[[0,1]]}"#);
    }
    let mut registry = ControllerRegistry::default();
    registry
        .register("unused", "1", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    let load = |f: &Files, registry: &ControllerRegistry| {
        FileTask::load(
            &f.0.join("physical.magna"),
            Some(&f.0.join("task.json")),
            registry,
            NoiseSampling::Physical,
            options(),
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
    };
    let x = load(&a, &registry);
    let y = load(&b, &registry);
    let run = TaskRunOptions {
        shots: 3,
        seed: 8,
        ..Default::default()
    };
    let identity = x.fingerprints(&run).unwrap();
    assert_eq!(identity, y.fingerprints(&run).unwrap());
    registry
        .register("late", "2", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    let changed = load(&b, &registry);
    assert_ne!(
        changed.fingerprints(&run).unwrap().controllers,
        identity.controllers
    );
    a.put("physical.magna", "X 0");
    a.put("task.json", "{}");
    std::fs::remove_file(a.0.join("target.magna")).unwrap();
    std::fs::remove_file(a.0.join("recovery.magna")).unwrap();
    let report = x.run(&run).unwrap();
    let expected = y.run(&run).unwrap();
    same_rows(report.raw_result(), expected.raw_result());
    assert_eq!(identity, x.fingerprints(&run).unwrap());
    assert_eq!(
        report.to_json().unwrap()["fingerprints"],
        identity.to_json()
    );
    assert_eq!(report.to_json().unwrap()["estimates"]["error"], 0.5);
}
#[test]
fn causal_routes_histories_importance_and_records_match_existing_batches() {
    let source = "H 0\nT 0\nH 0\nM(1/7) 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nX_ERROR(1/3) ctrl[0] 1\nCX ctrl[0] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]";
    let task = r#"{"error_observables":[0],"sample_records":[0,1]}"#;
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "v1", {
            let calls = Arc::clone(&calls);
            move |b: &[bool]| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(b.to_vec())
            }
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, Path::new("."), &TaskOptions::default())
        .unwrap()
        .load_modules(&physical, &parse)
        .unwrap();
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        for history in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            for sampling in [
                NoiseSampling::Physical,
                NoiseSampling::Importance {
                    proposal_scale: "2".parse().unwrap(),
                },
            ] {
                let mut o = options();
                o.evaluation.runtime.strategy = strategy;
                o.evaluation.runtime.history_sampler = history;
                let file = FileTask::from_loaded(
                    physical.clone(),
                    modules.clone(),
                    &registry,
                    sampling.clone(),
                    o,
                    BTreeMap::new(),
                )
                .unwrap();
                let run = TaskRunOptions {
                    shots: 4,
                    seed: 771,
                    ..Default::default()
                };
                let before = calls.load(Ordering::SeqCst);
                file.inspect(&run).unwrap();
                let compiled = file.compile().unwrap();
                compiled.inspect(&run).unwrap();
                assert_eq!(calls.load(Ordering::SeqCst), before);
                let actual = compiled.run(&run).unwrap();
                let raw = run_task_batch(
                    &physical,
                    &modules,
                    &registry,
                    o.evaluation,
                    &sampling,
                    &run,
                )
                .unwrap();
                same_rows(actual.raw_result(), &raw);
                let json = actual.to_json().unwrap();
                assert_eq!(json["completed"], 4);
                assert_eq!(json["sampled_records"].as_array().unwrap().len(), 4);
                assert_eq!(json["evaluation"]["error_observables"], json!([0]));
                assert_eq!(
                    json["history_sampler"],
                    if history == HistorySampler::ExactMarginal {
                        "exact_marginal"
                    } else {
                        "affine_support"
                    }
                );
                assert_eq!(
                    json["route"],
                    if strategy == CausalStrategy::FixedTemplate {
                        "causal_fixed_template"
                    } else {
                        "causal_cold_prefix"
                    }
                );
                assert_eq!(
                    json["fingerprints"],
                    file.fingerprints(&run).unwrap().to_json()
                );
                if matches!(sampling, NoiseSampling::Importance { .. }) {
                    assert_eq!(json["sampling"]["method"], "importance");
                    assert_eq!(json["proposal_summary"]["excluded_categories"], 1);
                } else {
                    assert!(json["proposal_summary"].is_null());
                }
                if strategy == CausalStrategy::ColdPrefix {
                    assert!(json["compilation"]["retained_caches"]["probability"].is_null());
                    assert!(json["compilation"]["retained_caches"]["insertions"].is_null());
                } else {
                    assert!(
                        json["compilation"]["retained_caches"]["probability"]["cached_queries"]
                            .as_u64()
                            .unwrap()
                            > 0
                    );
                }
            }
        }
    }
    let file = FileTask::from_loaded(
        physical,
        modules,
        &registry,
        NoiseSampling::Importance {
            proposal_scale: "2".parse().unwrap(),
        },
        options(),
        BTreeMap::new(),
    )
    .unwrap();
    let compiled = file.compile().unwrap();
    let runs = (0..4)
        .map(|seed| TaskRunOptions {
            shots: 3,
            seed,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let expected = runs
        .iter()
        .map(|r| compiled.run(r).unwrap())
        .collect::<Vec<_>>();
    std::thread::scope(|scope| {
        let handles = runs
            .iter()
            .map(|run| {
                scope.spawn({
                    let compiled = &compiled;
                    move || compiled.run(run).unwrap()
                })
            })
            .collect::<Vec<_>>();
        for (handle, expected) in handles.into_iter().zip(expected) {
            let actual = handle.join().unwrap();
            same_rows(actual.raw_result(), expected.raw_result());
            assert_eq!(
                counts(actual.raw_result().execution_costs.query),
                counts(expected.raw_result().execution_costs.query)
            );
            assert_eq!(
                actual.to_json().unwrap()["fingerprints"],
                expected.to_json().unwrap()["fingerprints"]
            );
        }
    });
}
#[test]
fn preparation_partial_sampling_and_numeric_export_failures_retain_their_scope() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", {
            let calls = Arc::clone(&calls);
            move |b: &[bool]| {
                if calls.fetch_add(1, Ordering::SeqCst) > 0 {
                    Err("transient decoder failure".to_owned())
                } else {
                    Ok(b.to_vec())
                }
            }
        })
        .unwrap();
    let file = loaded(
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0]}"#,
        Path::new("."),
        &registry,
        options(),
        NoiseSampling::Physical,
    );
    let run = TaskRunOptions {
        shots: 4,
        seed: 1,
        ..Default::default()
    };
    let report = file.run(&run).unwrap();
    let value = report.to_json().unwrap();
    assert_eq!(value["status"], "failed");
    assert_eq!(value["completed"], 1);
    assert_eq!(value["failed"], 1);
    assert_eq!(value["not_attempted"], 2);
    assert!(value["estimates"].is_null());
    assert_eq!(value["partial_statistics"]["estimates"]["acceptance"], 1.0);
    assert_eq!(value["failures"][0]["phase"], "sampling");
    assert_eq!(value["execution_costs"]["decoder"]["attempted"], 2);
    assert_eq!(value["execution_costs"]["decoder"]["failed"], 1);
    let run = TaskRunOptions {
        execution_limits: ExecutionLimits {
            max_duration: Some(std::time::Duration::ZERO),
            ..Default::default()
        },
        ..run
    };
    let failed = file.run(&run).unwrap().to_json().unwrap();
    assert_eq!(failed["completed"], 0);
    assert_eq!(failed["failed"], 0);
    assert_eq!(failed["not_attempted"], 4);
    assert_eq!(failed["failures"][0]["phase"], "preparation");
    assert_eq!(failed["compilation"]["status"], "preparation_failed");
    assert_eq!(failed["resources"]["queries"], 0);
    assert_eq!(failed["resources"]["compilations"], 0);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let mut o = options();
    o.numeric.max_work_bits = 0;
    let scalar = loaded(
        "",
        "{}",
        Path::new("."),
        &ControllerRegistry::default(),
        o,
        NoiseSampling::Physical,
    );
    let raw = scalar.run(&TaskRunOptions::default()).unwrap();
    assert_eq!(raw.raw_result().completed, 1);
    assert!(raw.to_json().is_err());
    assert!(raw.to_json().is_err());
    assert!(raw.raw_result().statistics.is_some());
    assert_eq!(raw.into_raw_result().completed, 1);
}
#[test]
fn file_byte_boundaries_utf8_and_malformed_modules_fail_explicitly() {
    let f = Files::new();
    let registry = ControllerRegistry::default();
    let source = "# café Ω\nH 0";
    let task = "{}";
    f.put("p.magna", source);
    f.put("t.json", task);
    let mut load = FileLoadOptions::default();
    load.circuit.max_source_bytes = source.len();
    load.task.max_json_bytes = task.len();
    let call = |load: &FileLoadOptions| {
        FileTask::load(
            &f.0.join("p.magna"),
            Some(&f.0.join("t.json")),
            &registry,
            NoiseSampling::Physical,
            options(),
            load,
            BTreeMap::new(),
        )
    };
    assert_eq!(
        call(&load)
            .unwrap()
            .inspect(&TaskRunOptions::default())
            .unwrap()["circuit"]["qubits"],
        1
    );
    load.circuit.max_source_bytes -= 1;
    assert!(call(&load).unwrap_err().to_string().contains("input limit"));
    load.circuit.max_source_bytes += 1;
    load.task.max_json_bytes -= 1;
    assert!(call(&load).unwrap_err().to_string().contains("input limit"));
    load.task.max_json_bytes += 1;
    std::fs::write(f.0.join("p.magna"), [255]).unwrap();
    assert!(call(&load).unwrap_err().to_string().contains("UTF-8"));
    f.put("p.magna", "");
    std::fs::write(f.0.join("t.json"), [255]).unwrap();
    assert!(call(&load).unwrap_err().to_string().contains("UTF-8"));
    f.put("t.json", "{");
    assert!(call(&load).is_err());
    f.put(
        "t.json",
        r#"{"outputs":[],"ideal_state":{"circuit":"target.magna","outputs":[]}}"#,
    );
    load.task.max_json_bytes = 1024;
    load.circuit.max_source_bytes = 4;
    f.put("target.magna", "H 0\nH 1");
    assert!(
        call(&load)
            .unwrap_err()
            .to_string()
            .contains("max_source_bytes")
    );
    std::fs::write(f.0.join("target.magna"), [255]).unwrap();
    assert!(call(&load).unwrap_err().to_string().contains("UTF-8"));
    let none = FileTask::load(
        &f.0.join("p.magna"),
        None,
        &registry,
        NoiseSampling::Physical,
        options(),
        &FileLoadOptions::default(),
        BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        none.run(&TaskRunOptions::default())
            .unwrap()
            .to_json()
            .unwrap()["estimates"]["acceptance"],
        1.0
    );
}
#[test]
fn identity_and_run_validation_precede_callbacks_and_preserve_declared_configuration() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "declared-v1", {
            let calls = Arc::clone(&calls);
            move |b: &[bool]| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(b.to_vec())
            }
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(
        "M 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]",
        &parse,
    )
    .unwrap();
    let modules = parse_task(
        r#"{"sample_records":[0]}"#,
        Path::new("."),
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&physical, &parse)
    .unwrap();
    let make = |o, ids| {
        FileTask::from_loaded(
            physical.clone(),
            modules.clone(),
            &registry,
            NoiseSampling::Physical,
            o,
            ids,
        )
    };
    assert!(matches!(
        make(
            options(),
            BTreeMap::from([("ghost".to_owned(), json!("v1"))])
        ),
        Err(FileApiError::Fingerprint(_))
    ));
    let mut o = options();
    o.fingerprints.max_json_bytes = 0;
    assert!(matches!(
        make(o, BTreeMap::new()),
        Err(FileApiError::Fingerprint(_))
    ));
    let a = make(
        options(),
        BTreeMap::from([("copy".to_owned(), json!({"code_hash":"declared-one"}))]),
    )
    .unwrap();
    let b = make(
        options(),
        BTreeMap::from([("copy".to_owned(), json!({"code_hash":"declared-two"}))]),
    )
    .unwrap();
    assert_ne!(
        a.fingerprints(&TaskRunOptions::default())
            .unwrap()
            .controllers,
        b.fingerprints(&TaskRunOptions::default())
            .unwrap()
            .controllers
    );
    assert_eq!(
        a.inspect(&TaskRunOptions::default()).unwrap()["fingerprints"]["controller_attestation"],
        "caller_declared_unverified"
    );
    let compiled = a.compile().unwrap();
    assert!(
        compiled
            .run(&TaskRunOptions {
                shots: 0,
                ..Default::default()
            })
            .is_err()
    );
    assert!(
        compiled
            .run(&TaskRunOptions {
                shots: 2,
                max_retained_record_values: 1,
                ..Default::default()
            })
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let done = compiled
        .run(&TaskRunOptions::default())
        .unwrap()
        .to_json()
        .unwrap();
    assert_eq!(done["options"]["run"]["shots"], 1);
    assert_eq!(
        done["resources"]["queries"],
        done["execution_costs"]["query"]["attempted"]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let missing = FileTask::from_loaded(
        physical,
        modules,
        &ControllerRegistry::default(),
        NoiseSampling::Physical,
        options(),
        BTreeMap::new(),
    )
    .unwrap()
    .run(&TaskRunOptions::default())
    .unwrap()
    .to_json()
    .unwrap();
    assert_eq!(missing["status"], "failed");
    assert_eq!(missing["failures"][0]["phase"], "preparation");
    assert_eq!(missing["completed"], 0);
    assert_eq!(missing["execution_costs"]["decoder"]["attempted"], 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
