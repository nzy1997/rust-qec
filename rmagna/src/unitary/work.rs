//! Monotonic observations from actual scalar-closure preparation factories.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Work observed for one immutable insertion-program owner and all its views.
/// These are factory/evaluation events, not cache sizes or Native private
/// object counts. Bare phase APIs and unrelated compiled owners are excluded.
/// Snapshot at quiescent boundaries when comparing concurrent workloads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct QueryWorkStatistics {
    pub affine_closures_prepared: u64,
    pub phase_maps_prepared: u64,
    pub phase_resources_prepared: u64,
    pub closure_evaluations: u64,
    pub cold_evaluations: u64,
    pub fixed_evaluations: u64,
    pub saturated: bool,
}
#[derive(Default)]
pub(super) struct QueryWorkCounters {
    pub affine_closures: AtomicU64,
    pub phase_maps: AtomicU64,
    pub phase_resources: AtomicU64,
    pub evaluations: AtomicU64,
    pub cold: AtomicU64,
    pub fixed: AtomicU64,
    saturated: AtomicBool,
}
impl QueryWorkCounters {
    pub fn record(&self, counter: &AtomicU64) {
        if counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .is_err()
        {
            self.saturated.store(true, Ordering::Relaxed);
        }
    }
    pub fn snapshot(&self) -> QueryWorkStatistics {
        let read = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        QueryWorkStatistics {
            affine_closures_prepared: read(&self.affine_closures),
            phase_maps_prepared: read(&self.phase_maps),
            phase_resources_prepared: read(&self.phase_resources),
            closure_evaluations: read(&self.evaluations),
            cold_evaluations: read(&self.cold),
            fixed_evaluations: read(&self.fixed),
            saturated: self.saturated.load(Ordering::Relaxed),
        }
    }
}
