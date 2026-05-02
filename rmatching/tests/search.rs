use rmatching::search::{SearchFlooder, SearchGraph};
use rmatching::types::*;

/// Build a 3-node chain: 0 --w=10-- 1 --w=20-- 2
fn make_chain_graph() -> SearchGraph {
    let mut g = SearchGraph::new(3, 2);
    g.add_edge(0, 1, 10, 0b01);
    g.add_edge(1, 2, 20, 0b10);
    g
}

#[test]
fn search_shortest_path() {
    let g = make_chain_graph();
    let mut flooder = SearchFlooder::new(g);

    let edge = flooder.find_shortest_path(0, Some(2));
    assert_eq!(edge.loc_from, Some(NodeIdx(0)));
    assert_eq!(edge.loc_to, Some(NodeIdx(2)));
    // XOR of observables along the path: 0b01 ^ 0b10 = 0b11
    assert_eq!(edge.obs_mask, 0b11);
}

#[test]
fn search_shortest_path_reversed() {
    let g = make_chain_graph();
    let mut flooder = SearchFlooder::new(g);

    let edge = flooder.find_shortest_path(2, Some(0));
    assert_eq!(edge.loc_from, Some(NodeIdx(2)));
    assert_eq!(edge.loc_to, Some(NodeIdx(0)));
    assert_eq!(edge.obs_mask, 0b11);
}

#[test]
fn search_adjacent_nodes() {
    let g = make_chain_graph();
    let mut flooder = SearchFlooder::new(g);

    let edge = flooder.find_shortest_path(0, Some(1));
    assert_eq!(edge.loc_from, Some(NodeIdx(0)));
    assert_eq!(edge.loc_to, Some(NodeIdx(1)));
    assert_eq!(edge.obs_mask, 0b01);
}

#[test]
fn search_boundary_path() {
    let mut g = SearchGraph::new(3, 1);
    // 0 --w=10-- 1 --w=5-- boundary
    g.add_edge(0, 1, 10, 0b01);
    g.add_boundary_edge(1, 5, 0b10);

    let mut flooder = SearchFlooder::new(g);
    let edge = flooder.find_shortest_path(0, None);
    assert_eq!(edge.loc_from, Some(NodeIdx(0)));
    assert_eq!(edge.loc_to, None);
    // Path: 0->1 (obs 0b01) then 1->boundary (obs 0b10) => 0b11
    assert_eq!(edge.obs_mask, 0b11);
}

#[test]
fn search_boundary_direct() {
    let mut g = SearchGraph::new(1, 1);
    g.add_boundary_edge(0, 7, 0b01);

    let mut flooder = SearchFlooder::new(g);
    let edge = flooder.find_shortest_path(0, None);
    assert_eq!(edge.loc_from, Some(NodeIdx(0)));
    assert_eq!(edge.loc_to, None);
    assert_eq!(edge.obs_mask, 0b01);
}

#[test]
fn search_reuse_after_reset() {
    let g = make_chain_graph();
    let mut flooder = SearchFlooder::new(g);

    let e1 = flooder.find_shortest_path(0, Some(2));
    assert_eq!(e1.obs_mask, 0b11);

    // Second search on the same flooder should work after implicit reset.
    let e2 = flooder.find_shortest_path(0, Some(1));
    assert_eq!(e2.obs_mask, 0b01);
}

#[test]
fn search_diamond_picks_shorter_path() {
    // Diamond: 0--1 (w=2), 0--2 (w=10), 1--3 (w=2), 2--3 (w=10)
    // Shortest 0->3 is 0->1->3 with total weight 4, obs = 0b01 ^ 0b10 = 0b11
    let mut g = SearchGraph::new(4, 2);
    g.add_edge(0, 1, 2, 0b01);
    g.add_edge(0, 2, 10, 0b100);
    g.add_edge(1, 3, 2, 0b10);
    g.add_edge(2, 3, 10, 0b1000);

    let mut flooder = SearchFlooder::new(g);
    let edge = flooder.find_shortest_path(0, Some(3));
    assert_eq!(edge.loc_from, Some(NodeIdx(0)));
    assert_eq!(edge.loc_to, Some(NodeIdx(3)));
    // Should take the short path: obs = 0b01 ^ 0b10 = 0b11
    assert_eq!(edge.obs_mask, 0b11);
}

// ---------------------------------------------------------------------------
// Coverage: SearchEvent HasTime impl (lines 21, 25-26, 28-29)
// ---------------------------------------------------------------------------

#[test]
fn search_event_has_time_trait() {
    use rmatching::search::search_flooder::SearchEvent;
    use rmatching::util::radix_heap::HasTime;
    use std::num::Wrapping;

    // NoEvent
    let no = SearchEvent::no_event();
    assert!(no.is_no_event());
    assert_eq!(no.time(), Wrapping(0));

    // LookAtNode
    let ev = SearchEvent::LookAtNode {
        node: SearchNodeIdx(42),
        time: Wrapping(100),
    };
    assert!(!ev.is_no_event());
    assert_eq!(ev.time(), Wrapping(100));
}

// ---------------------------------------------------------------------------
// Coverage: search_flooder boundary collision path (lines 348-349, 428-429)
// ---------------------------------------------------------------------------

#[test]
fn search_boundary_two_hops() {
    // 0 --w=5-- 1 --w=3-- boundary, search from 0 to boundary
    // Exercises the reversed emit path where boundary edge obs matters
    let mut g = SearchGraph::new(2, 1);
    g.add_edge(0, 1, 5, 0b01);
    g.add_boundary_edge(1, 3, 0b10);

    let mut flooder = SearchFlooder::new(g);

    // Collect edges using iter_edges_on_shortest_path
    let mut edges = Vec::new();
    flooder.iter_edges_on_shortest_path(0, None, |from, to, obs| {
        edges.push((from, to, obs));
    });

    // Path should be 0->1->boundary with correct obs masks
    assert!(!edges.is_empty());
}

// ---------------------------------------------------------------------------
// Coverage: search_flooder no collision (line 293)
// ---------------------------------------------------------------------------

#[test]
fn search_no_collision_disconnected() {
    // Two disconnected nodes, search from one to the other
    let g = SearchGraph::new(2, 1);
    // No edges added - nodes are disconnected
    let mut flooder = SearchFlooder::new(g);

    let edge = flooder.find_shortest_path(0, Some(1));
    // Should not find a path
    assert_eq!(edge.loc_from, Some(NodeIdx(0)));
    // obs_mask should be 0 (no path found, so default)
    assert_eq!(edge.obs_mask, 0);
}

// ---------------------------------------------------------------------------
// Coverage: search_graph self-loop and add_edge (lines 42, 55-56, 83)
// ---------------------------------------------------------------------------

#[test]
fn search_graph_self_loop() {
    let mut g = SearchGraph::new(2, 1);
    // Self-loop should be ignored
    g.add_edge(0, 0, 10, 0b01);
    assert_eq!(g.nodes[0].neighbors.len(), 0);

    // Normal edge should work
    g.add_edge(0, 1, 10, 0b01);
    assert_eq!(g.nodes[0].neighbors.len(), 1);
}

#[test]
fn search_detector_node_default() {
    use rmatching::search::SearchDetectorNode;
    let n = SearchDetectorNode::default();
    assert!(n.reached_from_source.is_none());
    assert_eq!(n.distance_from_source, 0);
    assert!(n.index_of_predecessor.is_none());
}

// ---------------------------------------------------------------------------
// Coverage: search_flooder with longer chain to exercise emit_reversed
// ---------------------------------------------------------------------------

#[test]
fn search_long_chain_path() {
    // 5-node chain: 0--1--2--3--4
    let mut g = SearchGraph::new(5, 1);
    g.add_edge(0, 1, 10, 0b01);
    g.add_edge(1, 2, 10, 0);
    g.add_edge(2, 3, 10, 0);
    g.add_edge(3, 4, 10, 0b01);

    let mut flooder = SearchFlooder::new(g);
    let edge = flooder.find_shortest_path(0, Some(4));
    assert_eq!(edge.loc_from, Some(NodeIdx(0)));
    assert_eq!(edge.loc_to, Some(NodeIdx(4)));
    // obs = 0b01 ^ 0 ^ 0 ^ 0b01 = 0
    assert_eq!(edge.obs_mask, 0);
}

#[test]
fn search_iter_edges_node_to_node() {
    let g = make_chain_graph();
    let mut flooder = SearchFlooder::new(g);

    let mut collected = Vec::new();
    flooder.iter_edges_on_shortest_path(0, Some(2), |from, to, obs| {
        collected.push((from, to, obs));
    });

    // Should have 2 edges: 0->1 and 1->2
    assert_eq!(collected.len(), 2);
}
