//! Explicit addressable Rust streams; no native/Python trajectory claim.
use rmagna::noise::{RandomSource, SplitMix64};

pub(crate) const PROTOCOL: &str = "splitmix64-addressed-u32-seed-u32-index-v1";

pub(crate) fn draw_stream(seed: u32, index: u32) -> SplitMix64 {
    // SplitMix's first-output permutation is bijective on u64. Distinct
    // (seed,index) addresses therefore receive distinct child initial states.
    let mut address = SplitMix64::new((u64::from(seed) << 32) | u64::from(index));
    SplitMix64::new(address.next_u64())
}
