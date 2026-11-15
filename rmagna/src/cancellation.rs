//! Explicit same-thread cooperative scopes around fallible synchronous work.
//!
//! A token alone retains the existing between-draw contract. Opting into a
//! scope additionally makes documented kernel safe points cooperative. Scopes
//! do not preempt arbitrary Rust callbacks or propagate into spawned threads.
use crate::scalar::ScalarError;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafePoint {
    PhaseTerm,
    QuadraticStep,
    Fiber,
    SignatureWork,
    DecoderWait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub kind: SafePoint,
    pub sequence: u64,
}

/// Optional synchronous instrumentation, also useful for deterministic
/// rendezvous. It must return; arbitrary blocked observer code is not preempted.
pub type CheckpointObserver = Arc<dyn Fn(Checkpoint) + Send + Sync>;

/// Actual safe-point observations, independent of successful batch-row counts.
pub struct CooperativeResult<T> {
    pub value: T,
    pub interrupted: bool,
    pub checkpoints: u64,
}
struct State {
    token: CancellationToken,
    observed: Cell<bool>,
    checkpoints: Cell<u64>,
    observer: Option<CheckpointObserver>,
}
thread_local! {
    static ACTIVE: RefCell<Option<Rc<State>>> = const { RefCell::new(None) };
}
struct Scope(Option<Rc<State>>);
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| *active.borrow_mut() = self.0.take());
    }
}
impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// Opt into cooperative kernel safe points for this synchronous operation.
    /// A nested scope temporarily replaces the outer scope; normal return,
    /// errors and unwinding restore the previous thread-local context. The
    /// returned value is preserved verbatim; `interrupted` records an observed
    /// safe-point refusal, not merely a concurrently cancelled token.
    pub fn with_cooperative_scope<T>(
        &self,
        observer: Option<CheckpointObserver>,
        operation: impl FnOnce() -> T,
    ) -> CooperativeResult<T> {
        let state = Rc::new(State {
            token: self.clone(),
            observed: Cell::new(false),
            checkpoints: Cell::new(0),
            observer,
        });
        let _scope = Scope(ACTIVE.with(|active| active.replace(Some(Rc::clone(&state)))));
        let value = operation();
        CooperativeResult {
            value,
            interrupted: state.observed.get(),
            checkpoints: state.checkpoints.get(),
        }
    }
}

pub(crate) fn observed() -> bool {
    ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .is_some_and(|state| state.observed.get())
    })
}

pub(crate) fn checkpoint(kind: SafePoint) -> Result<(), ScalarError> {
    let state = ACTIVE.with(|active| active.borrow().clone());
    let Some(state) = state else { return Ok(()) };
    let sequence = state.checkpoints.get().saturating_add(1);
    state.checkpoints.set(sequence);
    // Release the TLS borrow before user instrumentation. Reentrant or nested
    // scopes must not panic because the parent holds a RefCell borrow.
    if let Some(observer) = &state.observer {
        observer(Checkpoint { kind, sequence });
    }
    if state.token.is_cancelled() {
        state.observed.set(true);
        Err(ScalarError::Cancelled)
    } else {
        Ok(())
    }
}
