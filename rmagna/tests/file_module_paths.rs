// SPDX-License-Identifier: MPL-2.0
// Complete TerminalEvaluationTests.test_three_file_cli_task_relative_paths_and_fingerprints,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_terminal_evaluation.py.
use rmagna::field::ExactField;
use rmagna::file_api::{FileInputPaths, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{RunStatus, TaskRunOptions};
use rmagna::unitary::CompilerStrategy;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-file-module-paths-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        let files = Self(p);
        files.put("physical.magna", "X 0");
        files.put("target.magna", "I 0");
        files.put("recovery.magna", "M 0\nCX rec[-1] 0");
        files
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn task(
        &self,
        task: Option<&Path>,
        ideal: Option<&Path>,
        recovery: Option<&Path>,
        compiler: CompilerStrategy,
        load: &FileLoadOptions,
    ) -> Result<FileTask, rmagna::file_api::FileApiError> {
        let mut options = FileTaskOptions::default();
        options.evaluation.runtime.compile.compiler_strategy = compiler;
        FileTask::load_paths(
            FileInputPaths {
                physical: &self.0.join("physical.magna"),
                task,
                ideal_state: ideal,
                recovery,
            },
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            options,
            load,
            BTreeMap::new(),
        )
    }
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn cli_json(&self, args: &[&str]) -> Value {
        let out = self.cli(args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(out.stderr.is_empty());
        serde_json::from_slice(&out.stdout).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn compilers() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
fn run() -> TaskRunOptions {
    TaskRunOptions {
        shots: 2,
        seed: 8,
        ..Default::default()
    }
}
fn error(task: &FileTask) -> ExactField {
    let report = task.run(&run()).unwrap();
    assert_eq!(report.raw_result().status, RunStatus::Complete);
    assert_eq!(report.raw_result().completed, 2);
    report
        .raw_result()
        .statistics
        .as_ref()
        .unwrap()
        .error
        .clone()
        .unwrap()
}
#[test]
fn original_three_file_cli_task_relative_paths_and_fingerprints() {
    let files = Files::new();
    files.put(
        "task.json",
        r#"{"ideal_state":"target.magna","recovery":"recovery.magna"}"#,
    );
    for mode in ["validate", "inspect", "run"] {
        let mut args = vec![mode, "physical.magna", "--task", "task.json", "--json"];
        if mode == "run" {
            args.extend(["--shots", "2"]);
        }
        let report = files.cli_json(&args);
        assert!(!report["evaluation"]["ideal_state"].is_null());
        if mode == "run" {
            assert_eq!(report["estimates"]["error"], 0.0);
        }
    }
    for compiler in compilers() {
        files.put("target.magna", "I 0");
        let task_path = files.0.join("task.json");
        let before = files
            .task(Some(&task_path), None, None, compiler, &Default::default())
            .unwrap();
        let before_fp = before.fingerprints(&run()).unwrap().to_json();
        assert_eq!(error(&before), ExactField::zero());
        files.put("target.magna", "X 0");
        let after = files
            .task(Some(&task_path), None, None, compiler, &Default::default())
            .unwrap();
        let after_fp = after.fingerprints(&run()).unwrap().to_json();
        assert_ne!(before_fp["ideal_state"], after_fp["ideal_state"]);
        assert_eq!(error(&after), ExactField::one());
        let missing = files.0.join("missing-target.magna");
        let Err(conflict) = files.task(
            Some(&task_path),
            Some(&missing),
            None,
            compiler,
            &Default::default(),
        ) else {
            panic!("duplicate target accepted");
        };
        assert!(conflict.to_string().contains("declared in both"));
    }
}
#[test]
fn explicit_snapshots_fingerprints_and_projected_target_objects() {
    let files = Files::new();
    let target = files.0.join("target.magna");
    let recovery = files.0.join("recovery.magna");
    for compiler in compilers() {
        files.put("target.magna", "I 0");
        let captured = files
            .task(
                None,
                Some(&target),
                Some(&recovery),
                compiler,
                &Default::default(),
            )
            .unwrap();
        let fp = captured.fingerprints(&run()).unwrap().to_json();
        assert_eq!(error(&captured), ExactField::zero());
        files.put("target.magna", "X 0");
        let changed = files
            .task(
                None,
                Some(&target),
                Some(&recovery),
                compiler,
                &Default::default(),
            )
            .unwrap();
        assert_eq!(error(&changed), ExactField::one());
        assert_ne!(
            fp["ideal_state"],
            changed.fingerprints(&run()).unwrap().to_json()["ideal_state"]
        );
        assert_eq!(error(&captured), ExactField::zero());
        assert_eq!(fp, captured.fingerprints(&run()).unwrap().to_json());
        files.put("target.magna", "I 0");
        files.put("task.json",r#"{"outputs":[0],"ideal_state":{"circuit":"target.magna","outputs":[0],"postselection":{}}}"#);
        let task_path = files.0.join("task.json");
        let projected = files
            .task(
                Some(&task_path),
                None,
                Some(&recovery),
                compiler,
                &Default::default(),
            )
            .unwrap();
        assert_eq!(error(&projected), ExactField::zero());
    }
}
#[test]
fn duplicate_sources_dependencies_and_module_quotas() {
    let files = Files::new();
    let task_path = files.0.join("task.json");
    let missing = files.0.join("missing.magna");
    let target = files.0.join("target.magna");
    let recovery = files.0.join("recovery.magna");
    for compiler in compilers() {
        for (text, ideal, recovery) in [
            (r#"{"ideal_state":null}"#, Some(missing.as_path()), None),
            (
                r#"{"ideal_state":"target.magna"}"#,
                Some(missing.as_path()),
                None,
            ),
            (
                r#"{"ideal_state":"target.magna","recovery":null}"#,
                None,
                Some(missing.as_path()),
            ),
            (
                r#"{"ideal_state":"target.magna","recovery":"recovery.magna"}"#,
                None,
                Some(missing.as_path()),
            ),
        ] {
            files.put("task.json", text);
            let Err(e) = files.task(
                Some(&task_path),
                ideal,
                recovery,
                compiler,
                &Default::default(),
            ) else {
                panic!("duplicate source accepted");
            };
            assert!(e.to_string().contains("declared in both"), "{e}");
            assert!(!e.to_string().contains("cannot open"));
        }
        for (text, ideal, recovery, field) in [
            (
                r#"{"error_observables":[]}"#,
                Some(target.as_path()),
                None,
                "error_observables",
            ),
            (
                r#"{"error_observables":[]}"#,
                Some(target.as_path()),
                Some(recovery.as_path()),
                "error_observables",
            ),
            ("{}", None, Some(missing.as_path()), "recovery"),
        ] {
            files.put("task.json", text);
            let Err(e) = files.task(
                Some(&task_path),
                ideal,
                recovery,
                compiler,
                &Default::default(),
            ) else {
                panic!("invalid task accepted");
            };
            assert!(e.to_string().contains(field), "{e}");
            assert!(!e.to_string().contains("cannot open"));
        }
        let mut load = FileLoadOptions::default();
        load.circuit.max_source_bytes = 3;
        files.put("target.magna", "I 0\nI 0");
        let Err(e) = files.task(None, Some(&target), Some(&recovery), compiler, &load) else {
            panic!("oversized target accepted");
        };
        assert!(e.to_string().contains("ideal_state"));
        files.put("target.magna", "I 0");
        files.put("task.json", "{ } ");
        let mut task_limit = FileLoadOptions::default();
        task_limit.task.max_json_bytes = 3;
        assert!(
            files
                .task(
                    Some(&task_path),
                    Some(&target),
                    Some(&recovery),
                    compiler,
                    &task_limit
                )
                .is_err()
        );
        let loaded = files
            .task(
                None,
                Some(&target),
                Some(&recovery),
                compiler,
                &Default::default(),
            )
            .unwrap();
        let limited = loaded
            .run(&TaskRunOptions {
                shots: 1,
                execution_limits: rmagna::costs::ExecutionLimits {
                    max_queries: 1,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
        assert_eq!(limited.raw_result().status, RunStatus::Failed);
        assert_eq!(
            (
                limited.raw_result().completed,
                limited.raw_result().failed,
                limited.raw_result().not_attempted
            ),
            (0, 1, 0)
        );
        assert!(limited.to_json().unwrap()["estimates"].is_null());
    }
}
#[test]
fn explicit_cli_and_distinct_relative_path_origins() {
    // A child process tests library caller-CWD semantics without changing the
    // parallel harness's process-global working directory.
    if let Some(root) = std::env::var_os("RMAGNA_MODULE_PATH_TEST_CHILD") {
        let root = PathBuf::from(root);
        let physical = root.join("physical.magna");
        let task = root.join("taskdir/task.json");
        let empty = root.join("taskdir/empty.json");
        for compiler in compilers() {
            let mut options = FileTaskOptions::default();
            options.evaluation.runtime.compile.compiler_strategy = compiler;
            let load = |task, ideal, recovery| {
                FileTask::load_paths(
                    FileInputPaths {
                        physical: &physical,
                        task,
                        ideal_state: ideal,
                        recovery,
                    },
                    &ControllerRegistry::default(),
                    NoiseSampling::Physical,
                    options,
                    &Default::default(),
                    BTreeMap::new(),
                )
                .unwrap()
            };
            assert_eq!(
                error(&load(
                    Some(empty.as_path()),
                    Some(Path::new("target.magna")),
                    Some(Path::new("recovery.magna"))
                )),
                ExactField::zero()
            );
            assert_eq!(
                error(&load(Some(task.as_path()), None, None)),
                ExactField::one()
            );
        }
        return;
    }
    let files = Files::new();
    std::fs::create_dir(files.0.join("caller")).unwrap();
    std::fs::create_dir(files.0.join("taskdir")).unwrap();
    files.put("caller/target.magna", "I 0");
    files.put("caller/recovery.magna", "M 0\nCX rec[-1] 0");
    files.put("taskdir/target.magna", "X 0");
    files.put("taskdir/recovery.magna", "M 0\nCX rec[-1] 0");
    files.put(
        "taskdir/task.json",
        r#"{"ideal_state":"target.magna","recovery":"recovery.magna"}"#,
    );
    files.put("taskdir/empty.json", "{}");
    for mode in ["validate", "inspect", "run"] {
        let mut args = vec![
            mode,
            "physical.magna",
            "--ideal-state",
            "target.magna",
            "--recovery",
            "recovery.magna",
            "--json",
        ];
        if mode == "run" {
            args.extend(["--shots", "2"]);
        }
        let report = files.cli_json(&args);
        assert!(!report["evaluation"]["ideal_state"].is_null());
        if mode == "run" {
            assert_eq!(report["estimates"]["error"], 0.0);
            assert_eq!(report["completed"], 2);
        }
        args[3] = "missing-target.magna";
        let failure = files.cli(&args);
        assert!(!failure.status.success());
    }
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "explicit_cli_and_distinct_relative_path_origins",
            "--nocapture",
        ])
        .env("RMAGNA_MODULE_PATH_TEST_CHILD", &files.0)
        .current_dir(files.0.join("caller"))
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(String::from_utf8_lossy(&child.stdout).contains("1 passed; 0 failed"));
}
