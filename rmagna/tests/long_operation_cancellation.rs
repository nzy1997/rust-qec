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
