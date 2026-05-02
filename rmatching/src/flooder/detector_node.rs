use crate::interop::QueuedEventTracker;
use crate::types::*;
use crate::util::varying::VaryingCT;
#[cfg(test)]
use std::cell::Cell;

use super::fill_region::GraphFillRegion;

#[cfg(test)]
thread_local! {
    static RESET_CALLS: Cell<usize> = const { Cell::new(0) };
    static LOCAL_RADIUS_CALLS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Debug, Clone)]
pub struct DetectorNode {
    // Permanent (graph structure)
    pub neighbors: Vec<NodeIdx>,
    pub neighbor_weights: Vec<Weight>,
    pub neighbor_observables: Vec<ObsMask>,
    // Ephemeral (reset between decodes)
    pub region_that_arrived: Option<RegionIdx>,
    pub region_that_arrived_top: Option<RegionIdx>,
    pub reached_from_source: Option<NodeIdx>,
    pub observables_crossed_from_source: ObsMask,
    pub radius_of_arrival: CumulativeTime,
    pub wrapped_radius_cached: i32,
    pub node_event_tracker: QueuedEventTracker,
}

impl Default for DetectorNode {
    fn default() -> Self {
        DetectorNode {
            neighbors: Vec::new(),
            neighbor_weights: Vec::new(),
            neighbor_observables: Vec::new(),
            region_that_arrived: None,
            region_that_arrived_top: None,
            reached_from_source: None,
            observables_crossed_from_source: 0,
            radius_of_arrival: 0,
            wrapped_radius_cached: 0,
            node_event_tracker: QueuedEventTracker::default(),
        }
    }
}

impl DetectorNode {
    pub fn new() -> Self {
        Self::default()
    }

    /// The local radius at this node = top_region.radius + wrapped_radius_cached
    pub fn local_radius(&self, regions: &[GraphFillRegion]) -> VaryingCT {
        #[cfg(test)]
        LOCAL_RADIUS_CALLS.with(|calls| calls.set(calls.get() + 1));

        match self.region_that_arrived_top {
            None => VaryingCT::frozen(0),
            Some(top_idx) => {
                regions[top_idx.0 as usize].radius + self.wrapped_radius_cached as i64
            }
        }
    }

    /// Walk blossom hierarchy to compute wrapped radius
    pub fn compute_wrapped_radius(&self, regions: &[GraphFillRegion]) -> i32 {
        if self.reached_from_source.is_none() {
            return 0;
        }
        let mut total: i32 = 0;
        let mut r = self.region_that_arrived;
        while r != self.region_that_arrived_top {
            if let Some(idx) = r {
                total += regions[idx.0 as usize].radius.y_intercept() as i32;
                r = regions[idx.0 as usize].blossom_parent;
            } else {
                break;
            }
        }
        total - self.radius_of_arrival as i32
    }

    pub fn has_same_owner_as(&self, other: &DetectorNode) -> bool {
        self.region_that_arrived_top.is_some()
            && self.region_that_arrived_top == other.region_that_arrived_top
    }

    pub fn reset(&mut self) {
        #[cfg(test)]
        RESET_CALLS.with(|calls| calls.set(calls.get() + 1));
        self.region_that_arrived = None;
        self.region_that_arrived_top = None;
        self.reached_from_source = None;
        self.observables_crossed_from_source = 0;
        self.radius_of_arrival = 0;
        self.wrapped_radius_cached = 0;
        self.node_event_tracker.clear();
    }

    /// Walk blossom parent chain from region_that_arrived up to (but not including)
    /// region_that_arrived_top. Returns the child region directly under top.
    /// Used by do_blossom_shattering to find in_parent and in_child.
    pub fn heir_region_on_shatter(&self, regions: &[GraphFillRegion]) -> Option<RegionIdx> {
        let top = self.region_that_arrived_top?;
        let mut r = self.region_that_arrived?;
        loop {
            let parent = regions[r.0 as usize].blossom_parent;
            if parent == Some(top) || parent.is_none() {
                return Some(r);
            }
            r = parent.unwrap();
        }
    }

    /// Walk blossom parent chain from region_that_arrived to find the child
    /// region directly under the given target blossom. Used when shattering
    /// nested blossoms where region_that_arrived_top may point to an outer blossom.
    pub fn heir_region_for_blossom(
        &self,
        regions: &[GraphFillRegion],
        target_blossom: RegionIdx,
    ) -> Option<RegionIdx> {
        let mut r = self.region_that_arrived?;
        loop {
            let parent = regions[r.0 as usize].blossom_parent;
            if parent == Some(target_blossom) {
                return Some(r);
            }
            if parent.is_none() {
                return None;
            }
            r = parent.unwrap();
        }
    }

    #[cfg(test)]
    pub(crate) fn reset_reset_call_count() {
        RESET_CALLS.with(|calls| calls.set(0));
    }

    #[cfg(test)]
    pub(crate) fn reset_call_count() -> usize {
        RESET_CALLS.with(|calls| calls.get())
    }

    #[cfg(test)]
    pub(crate) fn reset_local_radius_call_count() {
        LOCAL_RADIUS_CALLS.with(|calls| calls.set(0));
    }

    #[cfg(test)]
    pub(crate) fn local_radius_call_count() -> usize {
        LOCAL_RADIUS_CALLS.with(|calls| calls.get())
    }
}
