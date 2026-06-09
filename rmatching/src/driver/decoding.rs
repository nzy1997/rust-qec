use crate::driver::dem_parse::parse_dem;
use crate::driver::user_graph::UserGraph;
use crate::matcher::mwpm::{MatchingResult, Mwpm};
use crate::types::*;

/// Public-facing decoder wrapping a `UserGraph` and its cached `Mwpm`.
pub struct Matching {
    user_graph: UserGraph,
    detection_events_buf: Vec<usize>,
    effective_events_buf: Vec<usize>,
}

impl Matching {
    /// Build a `Matching` from a Stim DEM text string.
    pub fn from_dem(dem_text: &str) -> Result<Self, String> {
        let user_graph = parse_dem(dem_text)?;
        Ok(Matching {
            user_graph,
            detection_events_buf: Vec::new(),
            effective_events_buf: Vec::new(),
        })
    }

    /// Create an empty `Matching` (edges added manually).
    pub fn new() -> Self {
        Matching {
            user_graph: UserGraph::new(),
            detection_events_buf: Vec::new(),
            effective_events_buf: Vec::new(),
        }
    }

    pub fn add_edge(
        &mut self,
        n1: usize,
        n2: usize,
        weight: f64,
        observables: &[usize],
        error_probability: f64,
    ) {
        self.user_graph
            .add_edge(n1, n2, observables.to_vec(), weight, error_probability);
    }

    pub fn add_boundary_edge(
        &mut self,
        node: usize,
        weight: f64,
        observables: &[usize],
        error_probability: f64,
    ) {
        self.user_graph
            .add_boundary_edge(node, observables.to_vec(), weight, error_probability);
    }

    pub fn set_boundary(&mut self, boundary: &[usize]) {
        self.user_graph
            .set_boundary(boundary.iter().copied().collect());
    }

    /// Decode a syndrome bit-vector into observable predictions.
    ///
    /// `syndrome` has one byte per detector; non-zero means that detector fired.
    /// Returns one byte per observable (0 or 1).
    pub fn decode(&mut self, syndrome: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        self.decode_into(syndrome, &mut out);
        out
    }

    /// Decode a syndrome into a caller-provided output buffer.
    pub fn decode_into(&mut self, syndrome: &[u8], out: &mut Vec<u8>) {
        let user_graph = &mut self.user_graph;
        let detection_events_buf = &mut self.detection_events_buf;
        let effective_events_buf = &mut self.effective_events_buf;
        let mwpm = user_graph.get_mwpm();
        let num_observables = mwpm.flooder.graph.num_observables;
        let neg_obs_mask =
            compute_neg_obs_mask(&mwpm.flooder.graph.negative_weight_observables_set);

        syndrome_to_detection_events_into(syndrome, detection_events_buf);
        apply_negative_weight_events_into(
            detection_events_buf,
            &mwpm.flooder.graph.negative_weight_detection_events_sorted,
            &mwpm.flooder.graph.is_user_graph_boundary_node,
            effective_events_buf,
        );

        decode_events_to_prediction_into(
            mwpm,
            effective_events_buf,
            num_observables,
            neg_obs_mask,
            out,
        );
    }

    /// Decode multiple syndromes. Each result matches `decode` on the same input.
    pub fn decode_batch(&mut self, syndromes: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        self.decode_batch_into(syndromes, &mut out);
        out
    }

    /// Decode multiple syndromes into caller-provided output buffers.
    pub fn decode_batch_into(&mut self, syndromes: &[Vec<u8>], out: &mut Vec<Vec<u8>>) {
        let user_graph = &mut self.user_graph;
        let detection_events_buf = &mut self.detection_events_buf;
        let effective_events_buf = &mut self.effective_events_buf;
        let mwpm = user_graph.get_mwpm();
        let num_observables = mwpm.flooder.graph.num_observables;
        let neg_obs_mask =
            compute_neg_obs_mask(&mwpm.flooder.graph.negative_weight_observables_set);

        if out.len() < syndromes.len() {
            out.resize_with(syndromes.len(), Vec::new);
        }

        for (syndrome, prediction_out) in syndromes.iter().zip(out.iter_mut()) {
            syndrome_to_detection_events_into(syndrome, detection_events_buf);
            apply_negative_weight_events_into(
                detection_events_buf,
                &mwpm.flooder.graph.negative_weight_detection_events_sorted,
                &mwpm.flooder.graph.is_user_graph_boundary_node,
                effective_events_buf,
            );
            decode_events_to_prediction_into(
                mwpm,
                effective_events_buf,
                num_observables,
                neg_obs_mask,
                prediction_out,
            );
        }

        out.truncate(syndromes.len());
    }

    pub(crate) fn graph_num_observables(&mut self) -> usize {
        self.user_graph.get_mwpm().flooder.graph.num_observables
    }

    pub(crate) fn decode_bit_packed_into(
        &mut self,
        packed_dets: &[u8],
        num_dets: usize,
        num_obs: usize,
        out: &mut Vec<u8>,
    ) {
        let user_graph = &mut self.user_graph;
        let detection_events_buf = &mut self.detection_events_buf;
        let effective_events_buf = &mut self.effective_events_buf;
        let mwpm = user_graph.get_mwpm();
        let graph_num_observables = mwpm.flooder.graph.num_observables;
        debug_assert_eq!(num_obs, graph_num_observables);
        let neg_obs_mask =
            compute_neg_obs_mask(&mwpm.flooder.graph.negative_weight_observables_set);

        packed_dets_to_detection_events_into(packed_dets, num_dets, detection_events_buf);
        apply_negative_weight_events_into(
            detection_events_buf,
            &mwpm.flooder.graph.negative_weight_detection_events_sorted,
            &mwpm.flooder.graph.is_user_graph_boundary_node,
            effective_events_buf,
        );

        process_timeline_until_completion(mwpm, effective_events_buf);
        let mut res = shatter_and_extract(mwpm, effective_events_buf);
        res.obs_mask ^= neg_obs_mask;
        obs_mask_to_bit_packed_predictions_into(res.obs_mask, graph_num_observables, out);
        mwpm.reset();
    }

    /// Decode a syndrome and return matched pairs as `(node1, node2)`.
    /// Boundary matches use `-1` for the boundary node.
    pub fn decode_to_edges(&mut self, syndrome: &[u8]) -> Vec<(i64, i64)> {
        let mwpm = self.user_graph.get_mwpm();

        let detection_events = syndrome_to_detection_events(syndrome);

        let effective_events = apply_negative_weight_events(
            &detection_events,
            &mwpm.flooder.graph.negative_weight_detection_events_sorted,
            &mwpm.flooder.graph.is_user_graph_boundary_node,
        );

        process_timeline_until_completion(mwpm, &effective_events);

        let edges = extract_match_edges(mwpm, &effective_events);

        mwpm.reset();

        edges
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn syndrome_to_detection_events(syndrome: &[u8]) -> Vec<usize> {
    let mut detection_events = Vec::new();
    syndrome_to_detection_events_into(syndrome, &mut detection_events);
    detection_events
}

#[cfg(test)]
fn decode_events_to_prediction(
    mwpm: &mut Mwpm,
    effective_events: &[usize],
    num_observables: usize,
    neg_obs_mask: ObsMask,
) -> Vec<u8> {
    let mut predictions = Vec::new();
    decode_events_to_prediction_into(
        mwpm,
        effective_events,
        num_observables,
        neg_obs_mask,
        &mut predictions,
    );
    predictions
}

fn decode_events_to_prediction_into(
    mwpm: &mut Mwpm,
    effective_events: &[usize],
    num_observables: usize,
    neg_obs_mask: ObsMask,
    out: &mut Vec<u8>,
) {
    process_timeline_until_completion(mwpm, effective_events);

    let mut res = shatter_and_extract(mwpm, effective_events);
    res.obs_mask ^= neg_obs_mask;
    obs_mask_to_predictions_into(res.obs_mask, num_observables, out);
    mwpm.reset();
}

fn syndrome_to_detection_events_into(syndrome: &[u8], out: &mut Vec<usize>) {
    out.clear();
    out.extend(
        syndrome
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != 0)
            .map(|(i, _)| i),
    );
}

fn packed_dets_to_detection_events_into(packed: &[u8], num_dets: usize, out: &mut Vec<usize>) {
    out.clear();
    let det_bytes = num_dets.div_ceil(8);
    for (byte_index, &byte) in packed.iter().take(det_bytes).enumerate() {
        let mut bits = byte;
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            let det = byte_index * 8 + bit;
            if det < num_dets {
                out.push(det);
            }
            bits &= bits - 1;
        }
    }
}

fn obs_mask_to_bit_packed_predictions_into(
    obs_mask: ObsMask,
    num_observables: usize,
    out: &mut Vec<u8>,
) {
    let obs_bytes = num_observables.div_ceil(8);
    out.clear();
    out.resize(obs_bytes, 0);
    for obs in 0..num_observables.min(64) {
        if ((obs_mask >> obs) & 1) != 0 {
            out[obs / 8] |= 1 << (obs % 8);
        }
    }
}

fn compute_neg_obs_mask(neg_obs_set: &std::collections::HashSet<usize>) -> ObsMask {
    let mut mask: ObsMask = 0;
    for &obs in neg_obs_set {
        mask ^= 1u64 << obs;
    }
    mask
}

/// Compute the symmetric difference of detection events and negative-weight
/// detection events, filtering out user-graph boundary nodes.
fn apply_negative_weight_events(
    detection_events: &[usize],
    neg_det_sorted: &[usize],
    is_boundary: &[bool],
) -> Vec<usize> {
    let mut result = Vec::new();
    apply_negative_weight_events_into(
        detection_events,
        neg_det_sorted,
        is_boundary,
        &mut result,
    );
    result
}

fn apply_negative_weight_events_into(
    detection_events: &[usize],
    neg_det_sorted: &[usize],
    is_boundary: &[bool],
    out: &mut Vec<usize>,
) {
    if neg_det_sorted.is_empty() {
        out.clear();
        out.extend(
            detection_events
                .iter()
                .copied()
                .filter(|&d| d >= is_boundary.len() || !is_boundary[d]),
        );
        return;
    }

    out.clear();
    let mut det_i = 0;
    let mut neg_i = 0;

    while det_i < detection_events.len() && neg_i < neg_det_sorted.len() {
        let det = detection_events[det_i];
        let neg = neg_det_sorted[neg_i];

        if det == neg {
            det_i += 1;
            neg_i += 1;
            continue;
        }

        let candidate = if det < neg {
            det_i += 1;
            det
        } else {
            neg_i += 1;
            neg
        };

        if candidate >= is_boundary.len() || !is_boundary[candidate] {
            out.push(candidate);
        }
    }

    while det_i < detection_events.len() {
        let det = detection_events[det_i];
        det_i += 1;
        if det >= is_boundary.len() || !is_boundary[det] {
            out.push(det);
        }
    }

    while neg_i < neg_det_sorted.len() {
        let neg = neg_det_sorted[neg_i];
        neg_i += 1;
        if neg >= is_boundary.len() || !is_boundary[neg] {
            out.push(neg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_alloc::{allocation_count, reset_allocation_count};

    #[test]
    fn syndrome_to_detection_events_into_reuses_buffer() {
        let mut out = vec![99, 100];
        syndrome_to_detection_events_into(&[0, 1, 0, 2], &mut out);
        assert_eq!(out, vec![1, 3]);

        syndrome_to_detection_events_into(&[1, 0], &mut out);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn packed_dets_to_detection_events_handles_cross_byte_bits() {
        let packed = [0b0000_0101u8, 0b1111_1110u8];
        let mut out = vec![999];

        packed_dets_to_detection_events_into(&packed, 10, &mut out);

        assert_eq!(out, vec![0, 2, 9]);
    }

    #[test]
    fn obs_mask_to_bit_packed_predictions_into_clears_stale_bits() {
        let mut out = vec![0xFF, 0xFF];

        obs_mask_to_bit_packed_predictions_into(1u64 << 8, 9, &mut out);

        assert_eq!(out, vec![0, 1]);
    }

    #[test]
    fn apply_negative_weight_events_into_filters_and_sorts() {
        let detection_events = vec![0, 2, 4];
        let neg_det_sorted = vec![2usize, 3usize];
        let is_boundary = vec![false, false, false, true, false];
        let mut out = vec![999];

        apply_negative_weight_events_into(
            &detection_events,
            &neg_det_sorted,
            &is_boundary,
            &mut out,
        );

        assert_eq!(out, vec![0, 4]);
    }

    #[test]
    fn apply_negative_weight_events_into_merges_sorted_inputs_without_hashing() {
        let detection_events = vec![1, 3, 6];
        let neg_det_sorted = vec![0, 3, 4, 7];
        let is_boundary = vec![false; 8];
        let mut out = vec![999];

        apply_negative_weight_events_into(
            &detection_events,
            &neg_det_sorted,
            &is_boundary,
            &mut out,
        );

        assert_eq!(out, vec![0, 1, 4, 6, 7]);
    }

    #[test]
    fn apply_negative_weight_events_into_filters_boundary_nodes_from_both_inputs() {
        let detection_events = vec![0, 2, 5];
        let neg_det_sorted = vec![1, 2, 4, 6];
        let is_boundary = vec![false, true, false, false, true, false, false];
        let mut out = vec![999];

        apply_negative_weight_events_into(
            &detection_events,
            &neg_det_sorted,
            &is_boundary,
            &mut out,
        );

        assert_eq!(out, vec![0, 5, 6]);
    }

    #[test]
    fn negative_weight_detector_cache_is_sorted_after_graph_build() {
        let mut matching = Matching::new();
        matching.add_edge(5, 1, -1.0, &[], 0.1);
        matching.add_edge(3, 5, -1.0, &[], 0.1);
        matching.add_boundary_edge(2, -1.0, &[], 0.1);

        let mwpm = matching.user_graph.get_mwpm();

        assert_eq!(
            mwpm.flooder.graph.negative_weight_detection_events_sorted,
            vec![1, 2, 3]
        );
    }

    #[test]
    fn decode_events_to_prediction_matches_public_decode() {
        let mut matching = Matching::new();
        matching.add_edge(0, 1, 1.0, &[0], 0.1);
        matching.add_boundary_edge(0, 2.0, &[], 0.1);
        matching.add_boundary_edge(1, 2.0, &[], 0.1);

        let syndrome = vec![1u8, 1u8];
        let expected = matching.decode(&syndrome);

        let mwpm = matching.user_graph.get_mwpm();
        let num_observables = mwpm.flooder.graph.num_observables;
        let neg_obs_mask = compute_neg_obs_mask(&mwpm.flooder.graph.negative_weight_observables_set);
        let mut detection_events = Vec::new();
        let mut effective_events = Vec::new();

        syndrome_to_detection_events_into(&syndrome, &mut detection_events);
        apply_negative_weight_events_into(
            &detection_events,
            &mwpm.flooder.graph.negative_weight_detection_events_sorted,
            &mwpm.flooder.graph.is_user_graph_boundary_node,
            &mut effective_events,
        );

        let actual =
            decode_events_to_prediction(mwpm, &effective_events, num_observables, neg_obs_mask);
        assert_eq!(actual, expected);
    }

    #[test]
    fn decode_bit_packed_into_matches_byte_syndrome_decode() {
        let mut matching = Matching::new();
        matching.add_edge(0, 8, 1.0, &[0], 0.1);
        matching.add_boundary_edge(0, 3.0, &[], 0.05);
        matching.add_boundary_edge(8, 3.0, &[0], 0.05);

        let syndrome = vec![0, 0, 0, 0, 0, 0, 0, 0, 1];
        let expected = matching.decode(&syndrome);

        let mut packed_out = vec![0xAA];
        matching.decode_bit_packed_into(&[0u8, 1u8], 9, 1, &mut packed_out);

        assert_eq!(expected, vec![1]);
        assert_eq!(packed_out, vec![1]);
    }

    #[test]
    fn decode_bit_packed_into_reuses_matching_buffers_after_warmup() {
        let mut matching = Matching::new();
        matching.add_edge(0, 8, 1.0, &[0], 0.1);
        matching.add_boundary_edge(0, 3.0, &[], 0.05);
        matching.add_boundary_edge(8, 3.0, &[0], 0.05);

        let warmup_packed = [0u8, 1u8];
        let second_packed = [0u8, 0u8];
        let expected_second = matching.decode(&[0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let mut out = Vec::new();
        matching.decode_bit_packed_into(&warmup_packed, 9, 1, &mut out);
        out[0] = 0xAA;

        reset_allocation_count();
        matching.decode_bit_packed_into(&second_packed, 9, 1, &mut out);

        assert_eq!(expected_second.as_slice(), &[0]);
        assert_eq!(out, expected_second);
        assert_eq!(allocation_count(), 0);
    }

    #[test]
    fn graph_num_observables_ignores_declared_but_unused_dem_observables() {
        let mut matching = Matching::from_dem(
            "\
error(0.1) D0 L0
error(0.05) D0
logical_observable L8
",
        )
        .unwrap();

        assert_eq!(matching.graph_num_observables(), 1);
    }

    #[test]
    fn shatter_and_extract_repeated_decode_reuses_cleanup_buffer() {
        let mut matching = Matching::new();
        matching.add_edge(0, 1, 1.0, &[0], 0.1);
        matching.add_edge(2, 3, 1.0, &[], 0.1);
        matching.add_edge(1, 2, 3.0, &[], 0.1);
        matching.add_boundary_edge(0, 5.0, &[], 0.05);
        matching.add_boundary_edge(3, 5.0, &[], 0.05);

        let syndrome = vec![1u8, 1u8, 1u8, 1u8];
        let _ = matching.decode(&syndrome);

        let mwpm = matching.user_graph.get_mwpm();
        let mut detection_events = Vec::new();
        let mut effective_events = Vec::new();
        syndrome_to_detection_events_into(&syndrome, &mut detection_events);
        apply_negative_weight_events_into(
            &detection_events,
            &mwpm.flooder.graph.negative_weight_detection_events_sorted,
            &mwpm.flooder.graph.is_user_graph_boundary_node,
            &mut effective_events,
        );

        process_timeline_until_completion(mwpm, &effective_events);
        reset_allocation_count();
        let _ = shatter_and_extract(mwpm, &effective_events);
        mwpm.reset();

        assert_eq!(allocation_count(), 0);
    }

    #[test]
    fn decode_into_matches_public_decode_and_reuses_buffer() {
        let mut matching = Matching::new();
        matching.add_edge(0, 1, 1.0, &[0], 0.1);
        matching.add_boundary_edge(0, 2.0, &[], 0.1);
        matching.add_boundary_edge(1, 2.0, &[], 0.1);

        let syndrome = vec![1u8, 1u8];
        let expected = matching.decode(&syndrome);
        let mut out = Vec::new();

        matching.decode_into(&syndrome, &mut out);
        assert_eq!(out, expected);

        reset_allocation_count();
        matching.decode_into(&syndrome, &mut out);

        assert_eq!(allocation_count(), 0);
    }

    #[test]
    fn decode_batch_into_matches_public_decode_batch_and_reuses_buffers() {
        let mut matching = Matching::new();
        matching.add_edge(0, 1, 1.0, &[0], 0.1);
        matching.add_edge(2, 3, 1.0, &[], 0.1);
        matching.add_edge(1, 2, 3.0, &[], 0.1);
        matching.add_boundary_edge(0, 5.0, &[], 0.05);
        matching.add_boundary_edge(3, 5.0, &[], 0.05);
        let syndromes = vec![vec![1u8, 1u8, 1u8, 1u8], vec![1u8, 0u8, 0u8, 1u8]];
        let expected = matching.decode_batch(&syndromes);
        let mut out = Vec::new();

        matching.decode_batch_into(&syndromes, &mut out);
        assert_eq!(out, expected);

        reset_allocation_count();
        matching.decode_batch_into(&syndromes, &mut out);

        assert_eq!(allocation_count(), 0);
    }
}

fn process_timeline_until_completion(mwpm: &mut Mwpm, detection_events: &[usize]) {
    // Reset queue time
    mwpm.flooder.queue.cur_time = 0;

    let num_nodes = mwpm.flooder.graph.nodes.len();

    for &det in detection_events {
        if det >= num_nodes {
            // Skip out-of-range detection events
            continue;
        }
        mwpm.create_detection_event(NodeIdx(det as u32));
    }

    loop {
        let event = mwpm.flooder.run_until_next_mwpm_notification();
        if event.is_no_event() {
            break;
        }
        mwpm.process_event(event);
    }
}

fn shatter_and_extract(mwpm: &mut Mwpm, detection_events: &[usize]) -> MatchingResult {
    let mut res = MatchingResult::new();
    let mut nodes_to_clean = std::mem::take(&mut mwpm.flooder.node_cleanup_buffer);
    for &i in detection_events {
        if i < mwpm.flooder.graph.nodes.len()
            && mwpm.flooder.graph.nodes[i].region_that_arrived.is_some()
        {
            let top = mwpm.flooder.graph.nodes[i].region_that_arrived_top.unwrap();
            // Collect shell-area nodes to reset *after* shattering, since
            // pair_and_shatter_subblossoms needs region_that_arrived_top to
            // locate sub-blossoms.
            nodes_to_clean.clear();
            collect_shell_nodes_recursive(mwpm.flooder.region_arena.items(), top, &mut nodes_to_clean);
            let match_region = mwpm.flooder.region_arena[top.0]
                .match_
                .as_ref()
                .and_then(|m| m.region);
            if let Some(mr) = match_region {
                collect_shell_nodes_recursive(
                    mwpm.flooder.region_arena.items(),
                    mr,
                    &mut nodes_to_clean,
                );
            }
            // Shattering reads region_that_arrived_top, so run it first.
            res += mwpm.shatter_blossom_and_extract_matches(top);
            // Now reset the nodes to prevent double-processing.
            for node_idx in nodes_to_clean.drain(..) {
                mwpm.flooder.graph.nodes[node_idx.0 as usize].reset();
            }
        }
    }
    mwpm.flooder.node_cleanup_buffer = nodes_to_clean;
    res
}

fn collect_shell_nodes_recursive(
    regions: &[crate::flooder::fill_region::GraphFillRegion],
    region: RegionIdx,
    out: &mut Vec<NodeIdx>,
) {
    out.extend(regions[region.0 as usize].shell_area.iter().copied());
    for child in &regions[region.0 as usize].blossom_children {
        collect_shell_nodes_recursive(regions, child.region, out);
    }
}

fn extract_match_edges(mwpm: &mut Mwpm, detection_events: &[usize]) -> Vec<(i64, i64)> {
    let mut match_edges = Vec::new();
    let mut nodes_to_clean = std::mem::take(&mut mwpm.flooder.node_cleanup_buffer);
    for &i in detection_events {
        if i < mwpm.flooder.graph.nodes.len()
            && mwpm.flooder.graph.nodes[i].region_that_arrived.is_some()
        {
            let top = mwpm.flooder.graph.nodes[i].region_that_arrived_top.unwrap();
            // Collect shell-area nodes to reset after shattering
            nodes_to_clean.clear();
            collect_shell_nodes_recursive(mwpm.flooder.region_arena.items(), top, &mut nodes_to_clean);
            let match_region = mwpm.flooder.region_arena[top.0]
                .match_
                .as_ref()
                .and_then(|m| m.region);
            if let Some(mr) = match_region {
                collect_shell_nodes_recursive(
                    mwpm.flooder.region_arena.items(),
                    mr,
                    &mut nodes_to_clean,
                );
            }
            // Shatter to collect compressed edges
            mwpm.shatter_blossom_and_extract_match_edges(top, &mut match_edges);
            // Reset nodes to prevent double-processing
            for node_idx in nodes_to_clean.drain(..) {
                mwpm.flooder.graph.nodes[node_idx.0 as usize].reset();
            }
        }
    }
    mwpm.flooder.node_cleanup_buffer = nodes_to_clean;

    // Convert CompressedEdge pairs to (i64, i64) detection event pairs
    let mut edges = Vec::new();
    for ce in &match_edges {
        let from = ce.loc_from.map(|n| n.0 as i64).unwrap_or(-1);
        let to = ce.loc_to.map(|n| n.0 as i64).unwrap_or(-1);
        // Normalize: smaller first (except boundary -1)
        let (a, b) = if to == -1 || (from != -1 && from <= to) {
            (from, to)
        } else {
            (to, from)
        };
        edges.push((a, b));
    }
    // Deduplicate
    edges.sort();
    edges.dedup();
    edges
}

fn obs_mask_to_predictions_into(obs_mask: ObsMask, num_observables: usize, out: &mut Vec<u8>) {
    out.clear();
    out.resize(num_observables, 0);
    for (i, value) in out.iter_mut().take(64).enumerate() {
        *value = ((obs_mask >> i) & 1) as u8;
    }
}
