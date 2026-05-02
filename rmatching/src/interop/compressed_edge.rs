use crate::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressedEdge {
    pub loc_from: Option<NodeIdx>,
    pub loc_to: Option<NodeIdx>, // None = boundary
    pub obs_mask: ObsMask,
}

impl CompressedEdge {
    pub fn empty() -> Self {
        CompressedEdge {
            loc_from: None,
            loc_to: None,
            obs_mask: 0,
        }
    }

    pub fn reversed(&self) -> Self {
        CompressedEdge {
            loc_from: self.loc_to,
            loc_to: self.loc_from,
            obs_mask: self.obs_mask,
        }
    }

    pub fn merged_with(&self, other: &CompressedEdge) -> Self {
        CompressedEdge {
            loc_from: self.loc_from,
            loc_to: other.loc_to,
            obs_mask: self.obs_mask ^ other.obs_mask,
        }
    }
}
