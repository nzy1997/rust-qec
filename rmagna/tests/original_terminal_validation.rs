// SPDX-License-Identifier: MPL-2.0
// Complete TerminalEvaluationTests.test_module_validation_and_mutually_exclusive_metrics,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_terminal_evaluation.py.
use rmagna::circuit::{Circuit, ParseOptions, parse_circuit};
use rmagna::costs::ExecutionLimits;
use rmagna::file_api::{FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{
    LoadedModules, RunPhase, RunStatus, TaskError, TaskOptions, TaskRunOptions, parse_task,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-original-terminal-validation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn modules(
        &self,
        target: Option<&str>,
        recovery: Option<&str>,
        mut task: Value,
    ) -> Result<(Circuit, LoadedModules), TaskError> {
        if let Some(target) = target {
            std::fs::write(self.0.join("target.magna"), target).unwrap();
            task["ideal_state"] = json!("target.magna");
        }
        if let Some(recovery) = recovery {
            std::fs::write(self.0.join("recovery.magna"), recovery).unwrap();
            task["recovery"] = json!("recovery.magna");
        }
        let parse = ParseOptions::default();
        let physical = parse_circuit("I 0", &parse).unwrap();
        let modules = parse_task(&task.to_string(), &self.0, &TaskOptions::default())?
            .load_modules(&physical, &parse)?;
        Ok((physical, modules))
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn original_module_validation_and_mutually_exclusive_metrics() {
    let files = Files::new();
    for target in [
        "M 0",
        "R 0",
        "X_ERROR(0) 0",
        "H 1",
        "CONTROL[XOR](1) -> ctrl[0]",
    ] {
        let error = files
            .modules(Some(target), None, json!({}))
            .expect_err("invalid original target accepted");
        assert!(
            error.field.starts_with("ideal_state"),
            "target={target}: {error}"
        );
    }
    for recovery in ["X_ERROR(0) 0", "M(1/2) 0"] {
        let error = files
            .modules(Some("I 0"), Some(recovery), json!({}))
            .expect_err("noisy original recovery accepted");
        assert!(error.field.starts_with("recovery"), "{error}");
    }
    let error = files
        .modules(None, Some(""), json!({}))
        .expect_err("recovery without target accepted");
    assert_eq!(error.field, "recovery");
    assert!(error.message.contains("requires ideal_state"));
    for (task, field, message) in [
        (
            json!({"error_checks":[]}),
            "error_checks",
            "no longer supported",
        ),
        (json!({"error_effect":{}}), "<root>", "unknown field"),
        (
            json!({"error_observables":[]}),
            "error_observables",
            "classical error definition",
        ),
    ] {
        let error = files
            .modules(Some("I 0"), None, task)
            .expect_err("invalid original metric accepted");
        assert_eq!(error.field, field);
        assert!(error.message.contains(message), "{error}");
        if field == "<root>" {
            assert!(error.message.contains("error_effect"));
        }
    }
    let (physical, modules) = files.modules(Some("I 0"), None, json!({})).unwrap();
    let file = FileTask::from_loaded(
        physical,
        modules,
        &ControllerRegistry::default(),
        NoiseSampling::Physical,
        FileTaskOptions::default(),
        BTreeMap::new(),
    )
    .unwrap();
    let report = file
        .run(&TaskRunOptions {
            shots: 1,
            execution_limits: ExecutionLimits {
                max_queries: 1,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    let result = report.raw_result();
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(
        (result.completed, result.failed, result.not_attempted),
        (0, 1, 0)
    );
    assert!(result.statistics.is_none());
    assert!(result.partial_statistics.is_none());
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.failures[0].phase, RunPhase::Sampling);
    assert_eq!(result.failures[0].shot, Some(0));
    assert!(result.failures[0].message.contains("max_queries = 1"));
    assert_eq!(result.execution_costs.query.attempted, 1);
    assert_eq!(result.execution_costs.query.succeeded, 1);
    assert_eq!(result.execution_costs.query.failed, 0);
    assert_eq!(result.execution_costs.decoder.attempted, 0);
    assert_eq!(result.completed_network_compilations, 0);
    assert_eq!(result.completed_history_groups, 0);
    let value = report.to_json().unwrap();
    assert_eq!(value["status"], "failed");
    assert!(value["estimates"].is_null());
    assert!(value["partial_statistics"].is_null());
}
