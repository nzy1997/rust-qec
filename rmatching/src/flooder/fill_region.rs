use crate::interop::{Match, QueuedEventTracker, RegionEdge};
use crate::types::*;
use crate::util::varying::VaryingCT;

#[derive(Debug, Clone)]
pub struct GraphFillRegion {
    pub blossom_parent: Option<RegionIdx>,
    pub blossom_parent_top: Option<RegionIdx>,
    pub alt_tree_node: Option<AltTreeIdx>,
    pub radius: VaryingCT,
    pub shrink_event_tracker: QueuedEventTracker,
    pub match_: Option<Match>,
    pub blossom_children: Vec<RegionEdge>,
    pub shell_area: Vec<NodeIdx>,
    /// Node anchoring the parent-side edge (set when creating a blossom)
    pub blossom_in_parent_loc: Option<NodeIdx>,
    /// Node anchoring the child-side edge (set when creating a blossom)
    pub blossom_in_child_loc: Option<NodeIdx>,
}

impl Default for GraphFillRegion {
    fn default() -> Self {
        GraphFillRegion {
            blossom_parent: None,
            blossom_parent_top: None,
            alt_tree_node: None,
            radius: VaryingCT::frozen(0),
            shrink_event_tracker: QueuedEventTracker::default(),
            match_: None,
            blossom_children: Vec::new(),
            shell_area: Vec::new(),
            blossom_in_parent_loc: None,
            blossom_in_child_loc: None,
        }
    }
}

impl GraphFillRegion {
    pub fn reset_for_reuse(&mut self) {
        self.blossom_parent = None;
        self.blossom_parent_top = None;
        self.alt_tree_node = None;
        self.radius = VaryingCT::frozen(0);
        self.shrink_event_tracker = QueuedEventTracker::default();
        self.match_ = None;
        self.blossom_children.clear();
        self.shell_area.clear();
        self.blossom_in_parent_loc = None;
        self.blossom_in_child_loc = None;
    }

    pub fn tree_equal(&self, other: &GraphFillRegion) -> bool {
        self.alt_tree_node.is_some() && self.alt_tree_node == other.alt_tree_node
    }

    pub fn clear_blossom_parent(&mut self) {
        self.blossom_parent = None;
        self.blossom_parent_top = None;
    }
}
