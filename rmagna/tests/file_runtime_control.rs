// SPDX-License-Identifier: MPL-2.0
// Original hold-decoder protocol and long-batch request from
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_file_api.py.
use rmagna::file_api::{
    CompiledFileTask, ExclusiveFileTaskError, FileLoadOptions, FileTask, FileTaskOptions,
};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{CancellationToken, ProgressObserver, RunStatus, TaskRunOptions};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

const HOLD: &str = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[hold] det[-1] -> ctrl[0]\nCX ctrl[0] 1\n";
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-runtime-control-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn load(&self, source: &str, task: &str, registry: &ControllerRegistry) -> FileTask {
        let circuit = self.0.join("physical.magna");
        let spec = self.0.join("task.json");
        std::fs::write(&circuit, source).unwrap();
        std::fs::write(&spec, task).unwrap();
        FileTask::load(
            &circuit,
            Some(&spec),
            registry,
            NoiseSampling::Physical,
            FileTaskOptions::default(),
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
    }
    fn compile(&self, source: &str, task: &str, registry: &ControllerRegistry) -> CompiledFileTask {
        self.load(source, task, registry).compile().unwrap()
    }
}

#[test]
fn reentrant_use_is_rejected_and_distinct_exclusive_objects_are_independent() {
    use rmagna::file_api::ExclusiveCompiledFileTask;
    use std::sync::{OnceLock, Weak};
    let files = Files::new();
    let owner = Arc::new(OnceLock::<Weak<ExclusiveCompiledFileTask>>::new());
    let nested = Arc::clone(&owner);
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("hold", "1", move |bits: &[bool]| {
            count.fetch_add(1, Ordering::SeqCst);
            let task = nested.get().unwrap().upgrade().unwrap();
            assert!(matches!(
                task.run(&TaskRunOptions::default()),
                Err(ExclusiveFileTaskError::InUse)
            ));
            Ok(bits.to_vec())
        })
        .unwrap();
    let task = Arc::new(
        files
            .compile(HOLD, r#"{"outputs":[1]}"#, &registry)
            .into_exclusive(),
    );
    owner.set(Arc::downgrade(&task)).unwrap();
    assert_eq!(
        task.run(&TaskRunOptions::default())
            .unwrap()
            .raw_result()
            .status,
        RunStatus::Complete
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let mut callback = |_: &rmagna::task::BatchProgress| {
        assert!(matches!(
            task.inspect(&TaskRunOptions::default()),
            Err(ExclusiveFileTaskError::InUse)
        ));
        Ok(())
    };
    let mut observer = ProgressObserver::new(NonZeroU64::new(1).unwrap(), &mut callback);
    assert_eq!(
        task.run_with_progress(&TaskRunOptions::default(), &mut observer)
            .unwrap()
            .raw_result()
            .status,
        RunStatus::Complete
    );
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release = Mutex::new(release_rx);
    let mut held = ControllerRegistry::default();
    held.register("hold", "1", move |bits: &[bool]| {
        entered_tx.send(()).unwrap();
        release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        Ok(bits.to_vec())
    })
    .unwrap();
    let first = files
        .compile(HOLD, r#"{"outputs":[1]}"#, &held)
        .into_exclusive();
    let second = files
        .compile(
            "H 0\nM 0\n",
            r#"{"outputs":[0]}"#,
            &ControllerRegistry::default(),
        )
        .into_exclusive();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _ = done_tx.send(first.run(&TaskRunOptions::default()));
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let independent = second.run(&TaskRunOptions::default());
    release_tx.send(()).unwrap();
    let first = done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(first.raw_result().status, RunStatus::Complete);
    assert_eq!(
        independent.unwrap().raw_result().status,
        RunStatus::Complete
    );
}

#[test]
fn cancellation_does_not_replace_a_decoder_error_or_a_finished_last_draw() {
    let files = Files::new();
    let token = CancellationToken::new();
    let captured = token.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("hold", "1", move |_: &[bool]| {
            captured.cancel();
            Err("declared failure wins".into())
        })
        .unwrap();
    let task = files.compile(HOLD, r#"{"outputs":[1]}"#, &registry);
    let result = task
        .run_cancellable(
            &TaskRunOptions {
                shots: 3,
                ..Default::default()
            },
            &token,
        )
        .unwrap();
    let raw = result.raw_result();
    assert_eq!(raw.status, RunStatus::Failed);
    assert_eq!((raw.completed, raw.failed, raw.not_attempted), (0, 1, 2));
    assert_eq!(raw.failures.len(), 1);
    assert!(raw.failures[0].message.contains("declared failure wins"));
    assert!(result.to_json().unwrap()["estimates"].is_null());
    let token = CancellationToken::new();
    let captured = token.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("hold", "1", move |bits: &[bool]| {
            captured.cancel();
            Ok(bits.to_vec())
        })
        .unwrap();
    let task = files.compile(HOLD, r#"{"outputs":[1]}"#, &registry);
    let complete = task
        .run_cancellable(&TaskRunOptions::default(), &token)
        .unwrap();
    assert_eq!(complete.raw_result().status, RunStatus::Complete);
    assert!(complete.raw_result().statistics.is_some());
}

#[test]
fn cancelled_prepared_task_retains_its_receipt_and_reuses_captured_inputs() {
    let files = Files::new();
    let file = files.load(
        "H 0\nT 0\nH 0\nM 0\n",
        r#"{"outputs":[0],"sample_records":[0]}"#,
        &ControllerRegistry::default(),
    );
    let prepared = file
        .prepare_sampling(&rmagna::file_api::FilePreparationOptions::default())
        .unwrap();
    assert!(prepared.report().is_complete());
    let before = prepared.report().to_json();
    let token = CancellationToken::new();
    token.cancel();
    let report = prepared
        .run_cancellable(&TaskRunOptions::default(), &token)
        .unwrap();
    assert_eq!(report.raw_result().status, RunStatus::Cancelled);
    assert!(report.to_json().is_ok());
    assert_eq!(prepared.report().to_json(), before);
    let reused = prepared
        .run_cancellable(
            &TaskRunOptions {
                shots: 3,
                seed: 9,
                ..Default::default()
            },
            &CancellationToken::new(),
        )
        .unwrap();
    let fresh = file
        .run(&TaskRunOptions {
            shots: 3,
            seed: 9,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        reused.raw_result().statistics,
        fresh.raw_result().statistics
    );
    assert_eq!(
        reused.raw_result().sampled_records,
        fresh.raw_result().sampled_records
    );
    assert_eq!(reused.raw_result().completed_network_compilations, 0);
    assert_eq!(prepared.report().to_json(), before);
}

#[test]
fn original_long_batch_request_has_responsive_between_draw_cancellation_in_a_child() {
    use std::process::{Command, Stdio};
    use std::time::Instant;
    let output =
        std::env::temp_dir().join(format!("rmagna-cancel-child-{}.out", std::process::id()));
    let file = std::fs::File::create(&output).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "long_batch_child_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("RMAGNA_CANCELLATION_CHILD", "1")
        .stdout(Stdio::from(file.try_clone().unwrap()))
        .stderr(Stdio::from(file))
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("long batch cancellation child exceeded bounded wait");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let text = std::fs::read_to_string(&output).unwrap();
    let _ = std::fs::remove_file(&output);
    assert!(status.success(), "child output: {text}");
    assert!(text.contains("cancelled original 10000000-draw request"));
    print!("{text}");
}

#[test]
#[ignore = "executed by bounded parent process, without process-wide signal handlers"]
fn long_batch_child_entry() {
    assert_eq!(
        std::env::var("RMAGNA_CANCELLATION_CHILD").as_deref(),
        Ok("1")
    );
    let files = Files::new();
    let task = files.compile(
        "H 0\nT 0\nH 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\n",
        r#"{"outputs":[0]}"#,
        &ControllerRegistry::default(),
    );
    let token = CancellationToken::new();
    let other = token.clone();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let ready = Arc::clone(&barrier);
    let canceller = std::thread::spawn(move || {
        ready.wait();
        std::thread::sleep(Duration::from_millis(200));
        other.cancel();
    });
    let mut events = 0;
    let mut callback = |_: &rmagna::task::BatchProgress| {
        events += 1;
        Ok(())
    };
    let mut observer = ProgressObserver::new(NonZeroU64::new(10_000_000).unwrap(), &mut callback);
    barrier.wait();
    let start = std::time::Instant::now();
    let report = task
        .run_cancellable_with_progress(
            &TaskRunOptions {
                shots: 10_000_000,
                ..Default::default()
            },
            &token,
            &mut observer,
        )
        .unwrap();
    canceller.join().unwrap();
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(report.raw_result().status, RunStatus::Cancelled);
    assert!(report.raw_result().completed < 10_000_000);
    assert_eq!(report.raw_result().failed, 0);
    assert_eq!(observer.receipt().attempted, 0);
    assert_eq!(events, 0);
    assert!(report.to_json().unwrap()["estimates"].is_null());
    println!(
        "cancelled original 10000000-draw request after {} committed rows",
        report.raw_result().completed
    );
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn original_compiled_circuit_rejects_use_from_a_second_thread() {
    let files = Files::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release = Mutex::new(release_rx);
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("hold", "1", move |bits: &[bool]| {
            if seen.fetch_add(1, Ordering::SeqCst) == 0 {
                entered_tx.send(()).unwrap();
                release
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|e| e.to_string())?;
            }
            Ok(bits.to_vec())
        })
        .unwrap();
    let compiled = Arc::new(
        files
            .compile(HOLD, r#"{"outputs":[1]}"#, &registry)
            .into_exclusive(),
    );
    let first = Arc::clone(&compiled);
    let (done_tx, done_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = first.run(&TaskRunOptions {
            seed: 1,
            ..Default::default()
        });
        let _ = done_tx.send(result);
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let second = compiled.run(&TaskRunOptions {
        seed: 2,
        ..Default::default()
    });
    let inspection = compiled.inspect(&TaskRunOptions::default());
    release_tx.send(()).unwrap();
    let first = done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(first.raw_result().status, RunStatus::Complete);
    assert!(
        matches!(&second, Err(ExclusiveFileTaskError::InUse)),
        "original second-use rejection"
    );
    assert!(
        second
            .unwrap_err()
            .to_string()
            .contains("in use by another thread")
    );
    assert!(matches!(inspection, Err(ExclusiveFileTaskError::InUse)));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "rejection invokes no second decoder"
    );
    let later = compiled
        .run(&TaskRunOptions {
            seed: 3,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(later.raw_result().status, RunStatus::Complete);
    assert!(later.to_json().unwrap()["estimates"].is_object());
    assert_eq!(
        later.raw_result().statistics.as_ref().unwrap().accepted,
        rmagna::field::ExactField::one()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn exclusive_guard_releases_after_validation_error_decoder_error_unwind_and_cancellation() {
    let files = Files::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&counter);
    let mut registry = ControllerRegistry::default();
    registry
        .register("hold", "1", move |bits: &[bool]| {
            match calls.fetch_add(1, Ordering::SeqCst) {
                0 => Err("declared failure".into()),
                1 => panic!("declared unwind"),
                _ => Ok(bits.to_vec()),
            }
        })
        .unwrap();
    let task = files
        .compile(HOLD, r#"{"outputs":[1]}"#, &registry)
        .into_exclusive();
    assert!(
        task.run(&TaskRunOptions {
            shots: 0,
            ..Default::default()
        })
        .is_err()
    );
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    assert_eq!(
        task.run(&TaskRunOptions::default())
            .unwrap()
            .raw_result()
            .status,
        RunStatus::Failed
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || task.run(&TaskRunOptions::default())
        ))
        .is_err()
    );
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        task.run_cancellable(&TaskRunOptions::default(), &token)
            .unwrap()
            .raw_result()
            .status,
        RunStatus::Cancelled
    );
    assert_eq!(counter.load(Ordering::SeqCst), 2);
    assert_eq!(
        task.run(&TaskRunOptions::default())
            .unwrap()
            .raw_result()
            .status,
        RunStatus::Complete
    );
}

#[test]
fn asynchronous_cancellation_commits_one_draw_without_sparse_progress_or_extra_callback() {
    let files = Files::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release = Mutex::new(release_rx);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("hold", "1", move |bits: &[bool]| {
            observed.fetch_add(1, Ordering::SeqCst);
            entered_tx.send(()).unwrap();
            release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?;
            Ok(bits.to_vec())
        })
        .unwrap();
    let task = files.compile(
        HOLD,
        r#"{"outputs":[1],"sample_records":[0],"observables":[[0,1]]}"#,
        &registry,
    );
    let token = CancellationToken::new();
    let other = token.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut events = 0;
        let mut callback = |_: &rmagna::task::BatchProgress| {
            events += 1;
            Ok(())
        };
        let mut observer =
            ProgressObserver::new(NonZeroU64::new(10_000_000).unwrap(), &mut callback);
        let result = task
            .run_cancellable_with_progress(
                &TaskRunOptions {
                    shots: 10_000_000,
                    max_retained_record_values: 10_000_000,
                    seed: 8,
                    ..Default::default()
                },
                &other,
                &mut observer,
            )
            .unwrap();
        let receipt = observer.receipt();
        let _ = done_tx.send((result, receipt, events));
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    token.cancel();
    release_tx.send(()).unwrap();
    let (report, receipt, events) = done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
    let raw = report.raw_result();
    assert_eq!(raw.status, RunStatus::Cancelled);
    assert_eq!(
        (raw.completed, raw.failed, raw.not_attempted),
        (1, 0, 9_999_999)
    );
    assert!(raw.failures.is_empty());
    assert!(raw.statistics.is_none());
    assert_eq!(raw.partial_statistics.as_ref().unwrap().completed, 1);
    assert_eq!(raw.efficiency.as_ref().unwrap().completed, 1);
    assert_eq!(raw.sampled_records.as_ref().unwrap().len(), 1);
    assert_eq!(raw.execution_costs.decoder.succeeded, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!((receipt.attempted, events), (0, 0));
    let json = report.to_json().unwrap();
    assert_eq!(json["status"], "cancelled");
    assert!(json["estimates"].is_null());
    assert!(json["partial_statistics"]["estimates"].is_object());
}

#[test]
fn pre_cancelled_and_finished_runs_have_precise_status_and_reusable_tokens() {
    let files = Files::new();
    let task = files.compile(
        "H 0\nM 0\n",
        r#"{"outputs":[0],"sample_records":[0]}"#,
        &ControllerRegistry::default(),
    );
    let token = CancellationToken::new();
    token.cancel();
    for seed in [0, 91] {
        let options = TaskRunOptions {
            shots: 3,
            seed,
            ..Default::default()
        };
        let report = task.run_cancellable(&options, &token).unwrap();
        let raw = report.raw_result();
        assert_eq!((raw.completed, raw.failed, raw.not_attempted), (0, 0, 3));
        assert_eq!(raw.status, RunStatus::Cancelled);
        assert!(raw.statistics.is_none() && raw.partial_statistics.is_none());
        assert!(raw.sampled_records.as_ref().unwrap().is_empty());
        assert_eq!(raw.execution_costs.query.attempted, 0);
        assert!(report.to_json().unwrap()["partial_statistics"].is_null());
        let fresh = CancellationToken::new();
        let cancelled_api = task.run_cancellable(&options, &fresh).unwrap();
        let ordinary = task.run(&options).unwrap();
        assert_eq!(cancelled_api.raw_result().status, RunStatus::Complete);
        assert_eq!(
            cancelled_api.raw_result().statistics,
            ordinary.raw_result().statistics
        );
        assert_eq!(
            cancelled_api.raw_result().sampled_records,
            ordinary.raw_result().sampled_records
        );
    }
}
