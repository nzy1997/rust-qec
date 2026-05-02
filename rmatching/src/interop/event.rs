use crate::types::*;
use super::compressed_edge::CompressedEdge;

#[derive(Debug, Clone)]
pub enum MwpmEvent {
    NoEvent,
    RegionHitRegion {
        region1: RegionIdx,
        region2: RegionIdx,
        edge: CompressedEdge,
    },
    RegionHitBoundary {
        region: RegionIdx,
        edge: CompressedEdge,
    },
    BlossomShatter {
        blossom: RegionIdx,
        in_parent: RegionIdx,
        in_child: RegionIdx,
    },
}

impl MwpmEvent {
    pub fn is_no_event(&self) -> bool {
        matches!(self, MwpmEvent::NoEvent)
    }
}
