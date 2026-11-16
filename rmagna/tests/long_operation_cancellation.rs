use rmagna::{
    cancellation::{CancellationToken, CheckpointObserver, SafePoint},
    gf2::BitVector,
    phase::{ParityPhase, PhaseError, PreparedPhaseSum},
    quadratic::{PreparedQuadraticSum, QuadraticError, QuadraticForm},
    scalar::{ExactScalar, ScalarArithmetic, ScalarError},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

fn boundary(width: usize) -> QuadraticForm {
    QuadraticForm::new(
        0,
        vec![0; width],
        (0..width)
            .map(|_| BitVector::zeros(width).unwrap())
            .collect(),
    )
    .unwrap()
}
fn phase(width: usize) -> ParityPhase {
    ParityPhase::new(
        width,
        0,
        (0..width).map(|j| (BitVector::from_indices(width, [j]).unwrap(), 1)),
    )
    .unwrap()
}

#[test]
fn one_inflight_phase_contraction_stops_at_a_deterministic_rendezvous() {
    let arithmetic = ScalarArithmetic::default();
    let phase = phase(12);
    let prepared = PreparedPhaseSum::new(&phase, 1 << 12, &arithmetic).unwrap();
    let normal = prepared
        .evaluate(&phase, &boundary(12), &ExactScalar::one(), &arithmetic)
        .unwrap();
    let token = CancellationToken::new();
    let other = token.clone();
    let terms = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&terms);
    let (entered, ready) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let wait = Mutex::new(wait);
    let observer: CheckpointObserver = Arc::new(move |point| {
        if point.kind == SafePoint::PhaseTerm && seen.fetch_add(1, Ordering::SeqCst) == 3 {
            entered.send(()).unwrap();
            wait.lock().unwrap().recv().unwrap();
        }
    });
    let worker = std::thread::spawn(move || {
        let report = other.with_cooperative_scope(Some(observer), || {
            prepared.evaluate(&phase, &boundary(12), &ExactScalar::one(), &arithmetic)
        });
        assert!(report.interrupted);
        assert!(matches!(
            report.value,
            Err(PhaseError::Scalar(ScalarError::Cancelled))
        ));
        // The prepared object is still usable; no partial scalar was committed.
        let restored = prepared
            .evaluate(&phase, &boundary(12), &ExactScalar::one(), &arithmetic)
            .unwrap();
        assert_eq!(restored, normal);
        report.checkpoints
    });
    ready
        .recv_timeout(Duration::from_secs(10))
        .expect("single contraction never reached its inner safe point");
    token.cancel();
    release.send(()).unwrap();
    let checkpoints = worker.join().unwrap();
    assert_eq!(terms.load(Ordering::SeqCst), 4);
    assert!(checkpoints >= 4);
    println!(
        "interrupted one4096-term contraction at term4;{checkpoints} actual checkpoints;prepared reuse recovered"
    );
}

#[test]
fn cold_and_prepared_quadratic_work_refuse_inside_elimination() {
    let arithmetic = ScalarArithmetic::default();
    let form = boundary(16);
    let prepared = PreparedQuadraticSum::new(&form).unwrap();
    for warm in [false, true] {
        let token = CancellationToken::new();
        let captured = token.clone();
        let observer: CheckpointObserver = Arc::new(move |point| {
            if point.kind == SafePoint::QuadraticStep && point.sequence == 3 {
                captured.cancel();
            }
        });
        let report = token.with_cooperative_scope(Some(observer), || {
            if warm {
                prepared.evaluate(&form, &arithmetic)
            } else {
                form.gauss_sum(&arithmetic)
            }
        });
        assert!(report.interrupted);
        assert_eq!(report.checkpoints, 3);
        assert!(matches!(
            report.value,
            Err(QuadraticError::Scalar(ScalarError::Cancelled))
        ));
        assert_eq!(
            prepared.evaluate(&form, &arithmetic).unwrap(),
            arithmetic.integer(1 << 16).unwrap()
        );
    }
    println!(
        "cold and warm quadratic contractions interrupted at actual step3;no partial sum escaped"
    );
}

#[test]
fn nested_scopes_unwinding_and_other_threads_restore_their_own_contexts() {
    let arithmetic = ScalarArithmetic::default();
    let form = boundary(3);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let outer = cancelled.with_cooperative_scope(None, || {
        let inner =
            CancellationToken::new().with_cooperative_scope(None, || form.gauss_sum(&arithmetic));
        assert!(!inner.interrupted);
        assert_eq!(inner.value.unwrap(), arithmetic.integer(8).unwrap());
        let isolated =
            std::thread::spawn(|| boundary(3).gauss_sum(&ScalarArithmetic::default()).unwrap());
        assert_eq!(isolated.join().unwrap(), arithmetic.integer(8).unwrap());
        form.gauss_sum(&arithmetic)
    });
    assert!(outer.interrupted);
    assert!(matches!(
        outer.value,
        Err(QuadraticError::Scalar(ScalarError::Cancelled))
    ));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cancelled.with_cooperative_scope(None, || panic!("injected scope unwind"));
    }));
    assert!(panic.is_err());
    assert_eq!(
        form.gauss_sum(&arithmetic).unwrap(),
        arithmetic.integer(8).unwrap()
    );
    let deliberate =
        CancellationToken::new().with_cooperative_scope(None, || Err::<(), _>("deliberate error"));
    assert_eq!(deliberate.value, Err("deliberate error"));
    assert!(!deliberate.interrupted);
}

struct Files(std::path::PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-inner-cancel-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn compile(
        &self,
        source: &str,
        registry: &rmagna::runtime::ControllerRegistry,
    ) -> rmagna::file_api::CompiledFileTask {
        use rmagna::file_api::{FileLoadOptions, FileTask, FileTaskOptions};
        let circuit = self.0.join("physical.magna");
        let task = self.0.join("task.json");
        std::fs::write(&circuit, source).unwrap();
        std::fs::write(&task, r#"{"outputs":[0],"sample_records":[0]}"#).unwrap();
        FileTask::load(
            &circuit,
            Some(&task),
            registry,
            rmagna::noise::NoiseSampling::Physical,
            FileTaskOptions::default(),
            &FileLoadOptions::default(),
            Default::default(),
        )
        .unwrap()
        .compile()
        .unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_cancelled_inflight_row_is_distinct_from_committed_failed_and_unattempted_rows() {
    use rmagna::{
        runtime::ControllerRegistry,
        task::{BatchProgress, ProgressObserver, RunStatus, TaskRunOptions},
    };
    let files = Files::new();
    let task = files.compile(
        "H 0\nT 0\nH 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\n",
        &ControllerRegistry::default(),
    );
    let baseline = task
        .run(&TaskRunOptions {
            shots: 1,
            seed: 8,
            ..Default::default()
        })
        .unwrap();
    for shots in [2, 5] {
        let token = CancellationToken::new();
        let captured = token.clone();
        let completed = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&completed);
        let checkpoint: CheckpointObserver = Arc::new(move |_| {
            if seen.load(Ordering::SeqCst) == 1 {
                captured.cancel();
            }
        });
        let mut callback = |progress: &BatchProgress| {
            completed.store(progress.completed as usize, Ordering::SeqCst);
            Ok(())
        };
        let mut progress =
            ProgressObserver::new(std::num::NonZeroU64::new(1).unwrap(), &mut callback);
        let report = token.with_cooperative_scope(Some(checkpoint), || {
            task.run_cancellable_with_progress(
                &TaskRunOptions {
                    shots,
                    seed: 8,
                    ..Default::default()
                },
                &token,
                &mut progress,
            )
        });
        assert!(report.interrupted);
        let report = report.value.unwrap();
        let raw = report.raw_result();
        assert_eq!(raw.status, RunStatus::Cancelled);
        assert_eq!(
            (
                raw.completed,
                raw.failed,
                raw.interrupted,
                raw.not_attempted
            ),
            (1, 0, 1, shots - 2)
        );
        assert_eq!(raw.sampled_records, baseline.raw_result().sampled_records);
        assert_eq!(
            raw.partial_statistics.as_ref().unwrap(),
            baseline.raw_result().statistics.as_ref().unwrap()
        );
        assert_eq!(raw.efficiency.as_ref().unwrap().completed, 1);
        assert!(raw.failures.is_empty());
        assert!(
            raw.execution_costs.query.attempted
                >= baseline.raw_result().execution_costs.query.attempted
        );
        for cost in [
            raw.execution_costs.query,
            raw.execution_costs.compilation,
            raw.execution_costs.decoder,
            raw.execution_costs.support,
        ] {
            assert_eq!(cost.attempted, cost.succeeded + cost.failed);
        }
        let json = report.to_json().unwrap();
        assert_eq!(json["status"], "cancelled");
        assert_eq!(json["interrupted"], 1);
        assert_eq!(json["not_attempted"], shots - 2);
    }
    println!(
        "committed1/failed0/interrupted1/unattempted0or3;first-row statistics and reported record preserved"
    );
}

fn blocked_process(files: &Files) -> rmagna::runtime::process::ControllerProcess {
    use rmagna::runtime::process::{ControllerProcess, ProcessOptions};
    let mut command = std::process::Command::new("python3");
    command.args(["-I","-B","-u","-c",concat!(
        "import os,sys,json,time,pathlib\n",
        "print(json.dumps({'protocol':'rmagna.controller.v1','bindings':{'toy':'1'},'manifest':{'pid':os.getpid()}}),flush=True)\n",
        "sys.stdin.readline()\npathlib.Path('entered').write_text('request received')\n",
        "while True: time.sleep(1)\n")]).current_dir(&files.0);
    ControllerProcess::spawn(
        command,
        ProcessOptions {
            timeout: Duration::from_secs(5),
            ..Default::default()
        },
    )
    .unwrap()
}
fn wait_for_request(files: &Files) {
    let started = std::time::Instant::now();
    while !files.0.join("entered").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "managed worker never received its request"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn assert_reaped(worker: &rmagna::runtime::process::ControllerProcess, pid: u64) {
    assert!(
        worker.try_wait().unwrap().is_some(),
        "managed decoder was not reaped"
    );
    #[cfg(unix)]
    {
        let output=std::process::Command::new("python3").args(["-I","-B","-c",
        "import os,sys\ntry: os.kill(int(sys.argv[1]),0)\nexcept ProcessLookupError: sys.exit(0)\nelse: sys.exit(1)",
        &pid.to_string()]).status().unwrap();
        assert!(
            output.success(),
            "managed decoder survived claimed successful cancellation"
        );
    }
    #[cfg(not(unix))]
    let _ = pid;
}

#[test]
fn blocked_process_response_is_cancelled_only_after_the_managed_child_is_reaped() {
    let files = Files::new();
    let worker = blocked_process(&files);
    let pid = worker.manifest()["pid"].as_u64().unwrap();
    let token = CancellationToken::new();
    let captured = token.clone();
    let running = worker.clone();
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let report = captured.with_cooperative_scope(None, || running.evaluate("toy", &[false]));
        tx.send(report).unwrap();
    });
    wait_for_request(&files);
    token.cancel();
    let report = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    thread.join().unwrap();
    assert!(report.interrupted);
    assert!(report.checkpoints >= 2);
    assert!(
        report
            .value
            .unwrap_err()
            .to_string()
            .contains("cancelled at a safe point")
    );
    assert_reaped(&worker, pid);
    assert!(worker.evaluate("toy", &[]).is_err());
    println!(
        "blocked response cancelled;actual managed PID{pid} reaped and shared session poisoned"
    );
}

#[test]
fn cancellation_while_waiting_for_the_session_lock_terminates_the_shared_worker() {
    let files = Files::new();
    let worker = blocked_process(&files);
    let pid = worker.manifest()["pid"].as_u64().unwrap();
    let running = worker.clone();
    let (tx, rx) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        tx.send(running.evaluate("toy", &[])).unwrap();
    });
    wait_for_request(&files);
    let token = CancellationToken::new();
    token.cancel();
    let report = token.with_cooperative_scope(None, || worker.evaluate("toy", &[]));
    assert!(report.interrupted);
    assert!(report.value.is_err());
    assert!(rx.recv_timeout(Duration::from_secs(10)).unwrap().is_err());
    owner.join().unwrap();
    assert_reaped(&worker, pid);
    assert!(worker.evaluate("toy", &[]).is_err());
    println!(
        "contending scoped caller terminated/reaped shared PID{pid};original exchange also failed without retry"
    );
}

#[test]
fn registered_process_cancellation_preserves_typed_batch_interruption_after_string_errors() {
    use rmagna::{
        runtime::ControllerRegistry,
        task::{RunStatus, TaskRunOptions},
    };
    let files = Files::new();
    let worker = blocked_process(&files);
    let pid = worker.manifest()["pid"].as_u64().unwrap();
    let mut registry = ControllerRegistry::default();
    worker.register_all(&mut registry).unwrap();
    let task = files.compile(
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[toy] det[0] -> ctrl[0]\nCX ctrl[0] 1\n",
        &registry,
    );
    let token = CancellationToken::new();
    let captured = token.clone();
    let (tx, rx) = mpsc::channel();
    let running = std::thread::spawn(move || {
        tx.send(captured.with_cooperative_scope(None, || {
            task.run_cancellable(
                &TaskRunOptions {
                    shots: 3,
                    seed: 19,
                    ..Default::default()
                },
                &captured,
            )
        }))
        .unwrap();
    });
    wait_for_request(&files);
    token.cancel();
    let report = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    running.join().unwrap();
    assert!(report.interrupted);
    let report = report.value.unwrap();
    let raw = report.raw_result();
    assert_eq!(raw.status, RunStatus::Cancelled);
    assert_eq!(
        (
            raw.completed,
            raw.interrupted,
            raw.failed,
            raw.not_attempted
        ),
        (0, 1, 0, 2)
    );
    assert!(raw.failures.is_empty());
    assert!(raw.sampled_records.as_ref().unwrap().is_empty());
    assert_eq!(raw.execution_costs.decoder.attempted, 1);
    assert_eq!(raw.execution_costs.decoder.failed, 1);
    assert!(report.to_json().is_ok());
    assert_reaped(&worker, pid);
    println!(
        "registered decoder cancellation survives String boundary;no fake row/callback retry;managed PID{pid} reaped"
    );
}

#[test]
fn a_safe_point_after_the_actual_callback_consumes_no_more_caller_owned_entropy() {
    use rmagna::{
        circuit::{ParseOptions, parse_circuit},
        noise::{RandomSource, SplitMix64},
        runtime::{CausalProgram, ControllerRegistry, HistorySampler, RuntimeOptions},
    };
    struct Counted {
        rng: SplitMix64,
        words: Arc<AtomicUsize>,
    }
    impl RandomSource for Counted {
        fn next_u64(&mut self) -> u64 {
            self.words.fetch_add(1, Ordering::SeqCst);
            self.rng.next_u64()
        }
    }
    let token = CancellationToken::new();
    let captured = token.clone();
    let words = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&words);
    let at_callback = Arc::new(AtomicUsize::new(0));
    let saved = Arc::clone(&at_callback);
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |bits: &[bool]| {
            count.fetch_add(1, Ordering::SeqCst);
            saved.store(seen.load(Ordering::SeqCst), Ordering::SeqCst);
            captured.cancel();
            Ok(bits.to_vec())
        })
        .unwrap();
    let circuit = parse_circuit(
        "H 0\nT 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1\n",
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap();
    let arithmetic = ScalarArithmetic::default();
    let mut options = RuntimeOptions::default();
    options.history_sampler = HistorySampler::ExactMarginal;
    let program = CausalProgram::new(&circuit, &registry, options, &arithmetic).unwrap();
    let mut random = Counted {
        rng: SplitMix64::new(2012),
        words: Arc::clone(&words),
    };
    let report =
        token.with_cooperative_scope(None, || program.resolve(&[], &mut random, &arithmetic));
    assert!(report.interrupted);
    assert!(report.value.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        words.load(Ordering::SeqCst),
        at_callback.load(Ordering::SeqCst)
    );
    assert!(words.load(Ordering::SeqCst) > 0);
    let mut replay = SplitMix64::new(2012);
    for _ in 0..words.load(Ordering::SeqCst) {
        replay.next_u64();
    }
    assert_eq!(random.rng.state(), replay.state());
    println!(
        "actual callback1;{} caller words consumed before cancellation;no further word/retry/rewind",
        words.load(Ordering::SeqCst)
    );
}

#[test]
fn a_decoder_error_after_requesting_cancellation_remains_a_failed_draw() {
    use rmagna::{
        runtime::ControllerRegistry,
        task::{RunStatus, TaskRunOptions},
    };
    let files = Files::new();
    let token = CancellationToken::new();
    let captured = token.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("toy", "1", move |_: &[bool]| {
            captured.cancel();
            Err("deliberate decoder failure wins".into())
        })
        .unwrap();
    let task = files.compile(
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[toy] det[0] -> ctrl[0]\nCX ctrl[0] 1\n",
        &registry,
    );
    let report = token.with_cooperative_scope(None, || {
        task.run_cancellable(
            &TaskRunOptions {
                shots: 3,
                ..Default::default()
            },
            &token,
        )
    });
    assert!(
        !report.interrupted,
        "a token request alone is not an observed safe-point refusal"
    );
    let report = report.value.unwrap();
    let raw = report.raw_result();
    assert_eq!(raw.status, RunStatus::Failed);
    assert_eq!(
        (
            raw.completed,
            raw.failed,
            raw.interrupted,
            raw.not_attempted
        ),
        (0, 1, 0, 2)
    );
    assert_eq!(raw.failures.len(), 1);
    assert!(
        raw.failures[0]
            .message
            .contains("deliberate decoder failure wins")
    );
    assert!(report.to_json().is_ok());
}

#[test]
fn complete_original_signal_reference_keeps_the_separate_python_exception_contract_explicit() {
    use sha2::{Digest, Sha256};
    let raw = include_bytes!("data/original-long-operation-cancellation.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "92298b8c71b2a93d48031621791898ef5ba3a2f142d84693770e74153444c88c"
    );
    let data: serde_json::Value = serde_json::from_slice(raw).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../benchmarks/magna_reproduction/reference.json"
    ))
    .unwrap();
    assert_eq!(data["upstream_commit"], manifest["upstream"]["commit"]);
    assert_eq!(
        data["reference_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/reference.json"
            ))
        )
    );
    assert_eq!(
        data["producer_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/original_long_operation_cancellation.py"
            ))
        )
    );
    for (name, value) in data["source_sha256"].as_object().unwrap() {
        assert_eq!(*value, manifest["upstream"]["files"][name]);
    }
    assert_eq!(data["original_methods_passed"], 1);
    assert_eq!(
        data["methods"][0],
        "test_file_api.FileApiTests.test_signal_handlers_run_during_one_long_native_batch"
    );
    assert_eq!(data["original_shots"], 10_000_000);
    assert_eq!(data["original_progress_every"], 10_000_000);
    assert_eq!(data["original_alarm_seconds"], "1/5");
    assert_eq!(data["original_exception"], "Stop");
    assert_eq!(data["requests"].as_array().unwrap().len(), 4);
    assert_eq!(data["requests"][0]["callable"], true);
    assert_eq!(data["requests"][1]["delay"], "1/5");
    assert_eq!(data["requests"][2]["delay"], "0");
    assert_eq!(data["requests"][3]["restores_previous"], true);
    assert_eq!(data["signal_handler_restored"], true);
    assert_eq!(data["timer_disabled"], true);
    assert_eq!(
        data["scope"],
        "unchanged_original_signal_contract_not_Rust_Python_ABI"
    );
    println!(
        "unchanged original10million-draw request/0.2secondSIGALRM/Stop passed in Native;handler/timer restored;Rust exposes separate cooperative scope"
    );
}

fn replying_process(files: &Files) -> rmagna::runtime::process::ControllerProcess {
    use rmagna::runtime::process::{ControllerProcess, ProcessOptions};
    let mut command = std::process::Command::new("python3");
    command.args(["-I","-B","-u","-c",concat!(
        "import os,sys,json,time,pathlib\n",
        "print(json.dumps({'protocol':'rmagna.controller.v1','bindings':{'toy':'1'},'manifest':{'pid':os.getpid()}}),flush=True)\n",
        "request=json.loads(sys.stdin.readline())\n",
        "print(json.dumps({'id':request['id'],'outputs':[True]}),flush=True)\n",
        "pathlib.Path('reply-ready').write_text('complete response flushed')\n",
        "while True: time.sleep(1)\n")]).current_dir(&files.0);
    ControllerProcess::spawn(
        command,
        ProcessOptions {
            timeout: Duration::from_secs(5),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn a_valid_queued_reply_is_rejected_when_shared_termination_commits_first() {
    let files = Files::new();
    let worker = replying_process(&files);
    let pid = worker.manifest()["pid"].as_u64().unwrap();
    let (entered, ready) = mpsc::channel();
    let (release, waiting) = mpsc::channel();
    let waiting = Mutex::new(waiting);
    let observer: CheckpointObserver = Arc::new(move |point| {
        if point.kind == SafePoint::DecoderWait && point.sequence == 2 {
            entered.send(()).unwrap();
            waiting
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
        }
    });
    let owner = worker.clone();
    let (done, result) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        done.send(
            CancellationToken::new()
                .with_cooperative_scope(Some(observer), || owner.evaluate("toy", &[])),
        )
        .unwrap();
    });
    ready.recv_timeout(Duration::from_secs(10)).unwrap();
    let started = std::time::Instant::now();
    while !files.0.join("reply-ready").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "child did not flush its valid reply"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    // The complete response is already in the pipe/queue. The owner cannot
    // commit it while paused, so termination must win without a timing race.
    let cancel = CancellationToken::new();
    cancel.cancel();
    let cancelled = cancel.with_cooperative_scope(None, || worker.evaluate("toy", &[]));
    assert!(cancelled.interrupted);
    assert!(cancelled.value.is_err());
    assert_reaped(&worker, pid);
    release.send(()).unwrap();
    let original = result.recv_timeout(Duration::from_secs(10)).unwrap();
    thread.join().unwrap();
    assert!(
        !original.interrupted,
        "another scope's cancellation is a process failure for this owner"
    );
    let error = original
        .value
        .expect_err("a killed shared worker must poison its in-flight queued valid reply");
    assert!(
        error.to_string().contains("stopped before response commit"),
        "{error}"
    );
    assert!(worker.evaluate("toy", &[]).is_err());
    println!(
        "valid reply flushed before shared cancellation;child reaped before owner resumed;queued reply refused at commit,no retry"
    );
}

#[test]
fn an_owned_committed_reply_remains_valid_after_later_shared_termination() {
    let files = Files::new();
    let worker = replying_process(&files);
    let pid = worker.manifest()["pid"].as_u64().unwrap();
    let committed = worker.evaluate("toy", &[]).unwrap();
    assert_eq!(committed, [true]);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let report = cancel.with_cooperative_scope(None, || worker.evaluate("toy", &[]));
    assert!(report.interrupted);
    assert!(report.value.is_err());
    assert_reaped(&worker, pid);
    assert_eq!(committed, [true]);
    assert!(worker.evaluate("toy", &[]).is_err());
    println!(
        "response committed first;later termination reaped child but retained already-owned output"
    );
}
