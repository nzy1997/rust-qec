use std::num::Wrapping;

/// Index into Vec<DetectorNode> — replaces C++ DetectorNode*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIdx(pub u32);

/// Index into Arena<GraphFillRegion> — replaces C++ GraphFillRegion*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionIdx(pub u32);

/// Index into Arena<AltTreeNode> — replaces C++ AltTreeNode*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AltTreeIdx(pub u32);

/// Index into Vec<SearchDetectorNode>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SearchNodeIdx(pub u32);

// Integer type aliases matching PyMatching's ints.h
pub type ObsMask = u64;
pub type Weight = u32;
pub type SignedWeight = i32;
pub type CumulativeTime = i64;
pub type TotalWeight = i64;
pub type CyclicTime = Wrapping<u32>;

pub const NO_NEIGHBOR: usize = usize::MAX;
