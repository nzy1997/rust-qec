//! Factory observations and live symbolic-map retention.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Retained structure for one captured program, observed without running a
/// query. Geometry slots include expired weak entries and allocated capacity.
/// This is a structural inventory, not allocator bytes or process RSS. Callers
/// holding independent plans/continuations must account for those separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ReferenceStorageStatistics {
    pub physical_width: usize,
    pub raw_boundary_width: usize,
    pub query_boundary_width: usize,
    pub unique_reference_boundaries: usize,
    pub raw_phase_variables: usize,
    pub original_positions: usize,
    pub expanded_clifford_gates: usize,
    pub geometry_weak_slots: usize,
    pub geometry_slot_capacity: usize,
    pub live_geometries: usize,
}

/// Work observed for one immutable insertion-program owner and all its views.
/// Factory/evaluation counts are monotonic; live geometry retention is separate.
/// Direct unprepared expansion and bare phase operations are outside this scope.
/// These observations are not logical query-cache sizes or Native private
/// object counts. Bare phase APIs and unrelated compiled owners are excluded.
/// Snapshot at quiescent boundaries when comparing concurrent workloads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct QueryWorkStatistics {
    pub affine_closures_prepared: u64,
    pub affine_geometry_preparations: u64,
    pub affine_geometry_reuses: u64,
    /// Actual live interned projection/contraction owners. None means the
    /// retention mutex could not be inspected; no fake zero is returned.
    pub live_affine_geometries: Option<usize>,
    pub support_projections_prepared: u64,
    pub support_boundary_maps_prepared: u64,
    pub support_contractions_prepared: u64,
    pub support_phase_maps_prepared: u64,
    pub support_phase_resources_prepared: u64,
    pub support_parents_prepared: u64,
    pub support_evaluations: u64,
    pub support_changed_boundary_fallbacks: u64,
    pub phase_maps_prepared: u64,
    pub phase_resources_prepared: u64,
    pub closure_evaluations: u64,
    pub cold_evaluations: u64,
    pub fixed_evaluations: u64,
    pub saturated: bool,
}
#[derive(Default)]
pub(super) struct QueryWorkCounters {
    pub assignments: AtomicU64,
    pub affine_closures: AtomicU64,
    pub geometry_preparations: AtomicU64,
    pub geometry_reuses: AtomicU64,
    pub support_projections: AtomicU64,
    pub support_boundary_maps: AtomicU64,
    pub support_contractions: AtomicU64,
    pub support_phase_maps: AtomicU64,
    pub support_phase_resources: AtomicU64,
    pub support_parents: AtomicU64,
    pub support_evaluations: AtomicU64,
    pub support_changed_boundary_fallbacks: AtomicU64,
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
            affine_geometry_preparations: read(&self.geometry_preparations),
            affine_geometry_reuses: read(&self.geometry_reuses),
            live_affine_geometries: None,
            support_projections_prepared: read(&self.support_projections),
            support_boundary_maps_prepared: read(&self.support_boundary_maps),
            support_contractions_prepared: read(&self.support_contractions),
            support_phase_maps_prepared: read(&self.support_phase_maps),
            support_phase_resources_prepared: read(&self.support_phase_resources),
            support_parents_prepared: read(&self.support_parents),
            support_evaluations: read(&self.support_evaluations),
            support_changed_boundary_fallbacks: read(&self.support_changed_boundary_fallbacks),
            phase_maps_prepared: read(&self.phase_maps),
            phase_resources_prepared: read(&self.phase_resources),
            closure_evaluations: read(&self.evaluations),
            cold_evaluations: read(&self.cold),
            fixed_evaluations: read(&self.fixed),
            saturated: self.saturated.load(Ordering::Relaxed),
        }
    }
}
