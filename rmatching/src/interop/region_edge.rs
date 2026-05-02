use crate::types::*;
use super::compressed_edge::CompressedEdge;

#[derive(Debug, Clone)]
pub struct RegionEdge {
    pub region: RegionIdx,
    pub edge: CompressedEdge,
}

#[derive(Debug, Clone)]
pub struct Match {
    pub region: Option<RegionIdx>, // None = boundary match
    pub edge: CompressedEdge,
}
