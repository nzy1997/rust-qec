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
