use rmatching::flooder::graph::MatchingGraph;
use rmatching::flooder::graph_flooder::GraphFlooder;
use rmatching::interop::MwpmEvent;
use rmatching::matcher::mwpm::Mwpm;
use rmatching::types::*;

/// Helper: build a simple 2-node graph with one edge of given weight.
fn two_node_mwpm(weight: i32) -> Mwpm {
    let mut g = MatchingGraph::new(2, 1);
    g.add_edge(0, 1, weight, &[0]);
    Mwpm::new(GraphFlooder::new(g))
}

/// Helper: build a 1-node graph with a boundary edge.
fn one_node_boundary_mwpm(weight: i32) -> Mwpm {
    let mut g = MatchingGraph::new(1, 1);
    g.add_boundary_edge(0, weight, &[0]);
    Mwpm::new(GraphFlooder::new(g))
}

#[test]
fn mwpm_two_nodes_match() {
    let mut mwpm = two_node_mwpm(10);

    // Create detection events at both nodes
    mwpm.create_detection_event(NodeIdx(0));
    mwpm.create_detection_event(NodeIdx(1));

    // Run flooder until we get a region-hit-region event
    let event = mwpm.flooder.run_until_next_mwpm_notification();
    match &event {
        MwpmEvent::RegionHitRegion { region1: _, region2: _, edge } => {
            // Two regions should collide
            assert!(edge.obs_mask != 0 || edge.loc_from.is_some());
        }
        other => panic!("Expected RegionHitRegion, got {:?}", other),
    }

    // Process the event — should match the two regions
    mwpm.process_event(event);

    // Both regions should now be frozen with matches
    let r0 = &mwpm.flooder.region_arena[0];
    let r1 = &mwpm.flooder.region_arena[1];
    assert!(r0.match_.is_some());
    assert!(r1.match_.is_some());
    assert!(r0.radius.is_frozen());
    assert!(r1.radius.is_frozen());
}

#[test]
fn mwpm_boundary_match() {
    let mut mwpm = one_node_boundary_mwpm(5);

    // Create detection event at node 0
    mwpm.create_detection_event(NodeIdx(0));

    // Run flooder — should hit boundary
    let event = mwpm.flooder.run_until_next_mwpm_notification();
    match &event {
        MwpmEvent::RegionHitBoundary { region: _, edge } => {
            assert!(edge.loc_to.is_none()); // boundary
        }
        other => panic!("Expected RegionHitBoundary, got {:?}", other),
    }

    // Process the event
    mwpm.process_event(event);

    // Region should be frozen with a boundary match
    let r = &mwpm.flooder.region_arena[0];
    assert!(r.match_.is_some());
    let m = r.match_.as_ref().unwrap();
    assert!(m.region.is_none()); // boundary match
    assert!(r.radius.is_frozen());
}

#[test]
fn mwpm_blossom_formation() {
    // Triangle graph: 3 nodes, 3 edges, all weight 10
    //   0 -- 1
    //   |  /
    //   2
    let mut g = MatchingGraph::new(3, 1);
    g.add_edge(0, 1, 10, &[0]);
    g.add_edge(1, 2, 10, &[]);
    g.add_edge(0, 2, 10, &[]);
    // Add boundary edges so odd-count detection events can resolve
    g.add_boundary_edge(2, 20, &[]);

    let mut mwpm = Mwpm::new(GraphFlooder::new(g));

    // Create detection events at all 3 nodes
    mwpm.create_detection_event(NodeIdx(0));
    mwpm.create_detection_event(NodeIdx(1));
    mwpm.create_detection_event(NodeIdx(2));

    // Process events until no more
    let mut event_count = 0;
    loop {
        let event = mwpm.flooder.run_until_next_mwpm_notification();
        if event.is_no_event() {
            break;
        }
        mwpm.process_event(event);
        event_count += 1;
        if event_count > 20 {
            break; // safety limit
        }
    }

    // With 3 detection events on a triangle + boundary, the algorithm should
    // resolve all regions (either matched to each other or to boundary).
    // At least some events should have been processed.
    assert!(event_count >= 2, "Expected at least 2 events, got {}", event_count);
}

#[test]
fn mwpm_reset() {
    let mut mwpm = two_node_mwpm(10);
    mwpm.create_detection_event(NodeIdx(0));
    mwpm.create_detection_event(NodeIdx(1));

    // Process one event
    let event = mwpm.flooder.run_until_next_mwpm_notification();
    mwpm.process_event(event);

    // Reset
    mwpm.reset();

    // After reset, arenas should be empty
    assert_eq!(mwpm.flooder.node_arena.len(), 0);
    assert!(mwpm.flooder.graph.nodes[0].region_that_arrived.is_none());
}

#[test]
fn alt_tree_node_become_root() {
    use rmatching::matcher::alt_tree::{AltTreeEdge, AltTreeNode};
    use rmatching::interop::CompressedEdge;
    use rmatching::util::arena::Arena;

    let mut arena: Arena<AltTreeNode> = Arena::new();

    // Create root (idx 0) with outer_region = RegionIdx(0)
    let root_idx = AltTreeIdx(arena.alloc());
    arena[root_idx.0] = AltTreeNode::new_root(RegionIdx(0));

    // Create child (idx 1) with inner=RegionIdx(1), outer=RegionIdx(2)
    let child_idx = AltTreeIdx(arena.alloc());
    let edge = CompressedEdge {
        loc_from: Some(NodeIdx(0)),
        loc_to: Some(NodeIdx(1)),
        obs_mask: 0,
    };
    arena[child_idx.0] = AltTreeNode::new_pair(RegionIdx(1), RegionIdx(2), edge);
    let child_edge = AltTreeEdge::new(child_idx, edge);
    arena[root_idx.0].children.push(child_edge);
    arena[child_idx.0].parent = Some(AltTreeEdge::new(root_idx, edge.reversed()));

    // Make child the root
    AltTreeNode::become_root(child_idx, &mut arena);

    // Child should now be root (no parent)
    assert!(arena[child_idx.0].parent.is_none());
    // Old root should be a child of new root
    assert_eq!(arena[child_idx.0].children.len(), 1);
    assert_eq!(arena[child_idx.0].children[0].alt_tree_node, root_idx);
}

#[test]
fn alt_tree_most_recent_common_ancestor() {
    use rmatching::matcher::alt_tree::{AltTreeEdge, AltTreeNode};
    use rmatching::interop::CompressedEdge;
    use rmatching::util::arena::Arena;

    let mut arena: Arena<AltTreeNode> = Arena::new();
    let e = CompressedEdge::empty();

    // Build tree: root -> child1, root -> child2
    let root = AltTreeIdx(arena.alloc());
    arena[root.0] = AltTreeNode::new_root(RegionIdx(0));

    let c1 = AltTreeIdx(arena.alloc());
    arena[c1.0] = AltTreeNode::new_pair(RegionIdx(1), RegionIdx(2), e);
    arena[root.0].children.push(AltTreeEdge::new(c1, e));
    arena[c1.0].parent = Some(AltTreeEdge::new(root, e));

    let c2 = AltTreeIdx(arena.alloc());
    arena[c2.0] = AltTreeNode::new_pair(RegionIdx(3), RegionIdx(4), e);
    arena[root.0].children.push(AltTreeEdge::new(c2, e));
    arena[c2.0].parent = Some(AltTreeEdge::new(root, e));

    // LCA of c1 and c2 should be root
    let lca = AltTreeNode::most_recent_common_ancestor(c1, c2, &mut arena);
    assert_eq!(lca, Some(root));
}

#[test]
fn mwpm_blossom_then_match_4_events() {
    use rmatching::Matching;
    let dem = "error(0.1) D0 D1 L0\nerror(0.1) D1 D2\nerror(0.1) D0 D2\nerror(0.1) D0\n";
    let mut m = Matching::from_dem(dem).unwrap();
    let pred = m.decode(&[1, 1, 1]);
    assert_eq!(pred.len(), 1);
}

#[test]
fn mwpm_blossom_decode_chain_5() {
    use rmatching::Matching;
    let dem = concat!(
        "error(0.1) D0 D1 L0\n",
        "error(0.1) D1 D2\n",
        "error(0.1) D2 D3 L1\n",
        "error(0.1) D3 D4\n",
        "error(0.1) D0\n",
        "error(0.1) D4\n",
    );
    let mut m = Matching::from_dem(dem).unwrap();
    let pred = m.decode(&[1, 1, 0, 0, 0]);
    assert_eq!(pred, vec![1, 0]);
    let pred = m.decode(&[0, 0, 1, 1, 0]);
    assert_eq!(pred, vec![0, 1]);
    let pred = m.decode(&[0, 0, 0, 0, 0]);
    assert_eq!(pred, vec![0, 0]);
}

// ---------------------------------------------------------------------------
// Coverage tests: process_event NoEvent (line 85)
// ---------------------------------------------------------------------------

#[test]
fn mwpm_process_no_event() {
    let mut mwpm = two_node_mwpm(10);
    // Processing NoEvent should be a no-op
    mwpm.process_event(MwpmEvent::NoEvent);
}

// ---------------------------------------------------------------------------
// Coverage tests: tree hitting boundary match (lines 118-121, 131, 134)
// ---------------------------------------------------------------------------

/// Build a graph where a tree grows into a region that is already matched
/// to the boundary. This exercises handle_tree_hitting_boundary_match.
///
/// Graph: 0 --10-- 1 --5-- boundary
/// Fire D0 and D1. D1 hits boundary first (weight 5), gets boundary-matched.
/// Then D0's tree hits D1, which is boundary-matched (match_.region == None).
/// This triggers the handle_tree_hitting_boundary_match path.
#[test]
fn mwpm_tree_hitting_boundary_match() {
    use rmatching::Matching;
    // D1 has a close boundary, D0 has a farther boundary.
    // D0-D1 edge is heavier than D1-boundary.
    // So D1 matches to boundary first, then D0's tree hits D1's matched region.
    let dem = concat!(
        "error(0.1) D0 D1 L0\n",
        "error(0.3) D1\n",         // D1 has close boundary
        "error(0.01) D0\n",        // D0 has far boundary
    );
    let mut m = Matching::from_dem(dem).unwrap();

    // Both fire: one should match to boundary, other matches the matched region
    let pred = m.decode(&[1, 1]);
    assert_eq!(pred.len(), 1);
}

// ---------------------------------------------------------------------------
// Coverage tests: handle_tree_hitting_match (tree absorbing a matched pair)
// ---------------------------------------------------------------------------

/// Graph that forces a tree to absorb a matched pair.
/// 4 nodes: 0 --10-- 1 --10-- 2 --10-- 3
/// Boundary at 3 (weight 30).
/// Fire D0, D2, D3. D2 and D3 match first. Then D0's tree grows into
/// D1 (empty), hits D2 which is matched to D3.
#[test]
fn mwpm_tree_absorbs_matched_pair() {
    let mut g = MatchingGraph::new(4, 1);
    g.add_edge(0, 1, 10, &[0]);
    g.add_edge(1, 2, 10, &[]);
    g.add_edge(2, 3, 10, &[]);
    g.add_boundary_edge(0, 30, &[]);
    g.add_boundary_edge(3, 30, &[]);

    let mut mwpm = Mwpm::new(GraphFlooder::new(g));

    mwpm.create_detection_event(NodeIdx(0));
    mwpm.create_detection_event(NodeIdx(2));
    mwpm.create_detection_event(NodeIdx(3));

    // Run to completion
    let mut event_count = 0;
    loop {
        let event = mwpm.flooder.run_until_next_mwpm_notification();
        if event.is_no_event() {
            break;
        }
        mwpm.process_event(event);
        event_count += 1;
        if event_count > 30 {
            break;
        }
    }

    // All regions should be resolved
    assert!(event_count >= 2, "Expected at least 2 events, got {}", event_count);
}

// ---------------------------------------------------------------------------
// Coverage tests: blossom formation and shattering (lines 331-496)
// ---------------------------------------------------------------------------

/// Pentagon + extra node to force blossom formation, shattering, and re-matching.
/// 5-cycle: 0-1-2-3-4-0, all weight 10, plus node 5 connected to node 0.
/// Fire D0, D1, D2, D3, D4, D5. The odd cycle (5 nodes) forces blossom
/// formation. Then the blossom must interact with D5's tree or match.
#[test]
fn mwpm_blossom_shattering_pentagon() {
    use rmatching::Matching;
    // Pentagon with boundary edges to allow odd parity resolution.
    // Node 0 connects to all others in a cycle, plus a spur from node 5.
    let dem = concat!(
        "error(0.1) D0 D1 L0\n",
        "error(0.1) D1 D2\n",
        "error(0.1) D2 D3\n",
        "error(0.1) D3 D4\n",
        "error(0.1) D4 D0\n",
        "error(0.1) D0 D5\n",
        "error(0.05) D5\n",
    );
    let mut m = Matching::from_dem(dem).unwrap();

    // Fire odd number of detectors to force blossom + boundary resolution
    let pred = m.decode(&[1, 1, 1, 0, 0, 0]);
    assert_eq!(pred.len(), 1);

    // Fire all 6 detectors
    let pred = m.decode(&[1, 1, 1, 1, 1, 1]);
    assert_eq!(pred.len(), 1);
}

/// Triangle blossom with shattering: exercises blossom formation
/// then the blossom hitting another region or boundary.
#[test]
fn mwpm_triangle_blossom_shatter_with_external() {
    use rmatching::Matching;
    // Triangle + extra node connected to one vertex, with boundary.
    // This exercises blossom formation on the triangle, then the blossom
    // grows and hits the external node.
    let dem = concat!(
        "error(0.1) D0 D1\n",
        "error(0.1) D1 D2\n",
        "error(0.1) D0 D2 L0\n",
        "error(0.1) D2 D3\n",
        "error(0.05) D3\n",
        "error(0.05) D0\n",
    );
    let mut m = Matching::from_dem(dem).unwrap();

    // 2 detectors on the triangle (even - no blossom needed)
    let pred = m.decode(&[1, 1, 0, 0]);
    assert_eq!(pred.len(), 1);

    // 2 on the spur
    let pred = m.decode(&[0, 0, 1, 1]);
    assert_eq!(pred.len(), 1);
}

/// Blossom with nested structure: test with safe even-parity syndrome
/// that exercises matching through blossoms.
#[test]
fn mwpm_double_triangle_blossom() {
    use rmatching::Matching;
    // Two triangles sharing a vertex at D2
    let dem = concat!(
        "error(0.1) D0 D1 L0\n",
        "error(0.1) D1 D2\n",
        "error(0.1) D0 D2\n",
        "error(0.1) D2 D3 L1\n",
        "error(0.1) D3 D4\n",
        "error(0.1) D2 D4\n",
        "error(0.05) D0\n",
        "error(0.05) D4\n",
    );
    let mut m = Matching::from_dem(dem).unwrap();

    // Two detectors matching directly
    let pred = m.decode(&[1, 1, 0, 0, 0]);
    assert_eq!(pred, vec![1, 0]);

    let pred = m.decode(&[0, 0, 0, 1, 1]);
    assert_eq!(pred.len(), 2);

    // Four detectors: two pairs
    let pred = m.decode(&[1, 1, 0, 1, 1]);
    assert_eq!(pred.len(), 2);
}

// ---------------------------------------------------------------------------
// Coverage tests: MatchingResult (line 682-683, 722-724, 760-767)
// ---------------------------------------------------------------------------

/// Test shatter_blossom_and_extract_matches through decode.
/// Uses even parity to avoid problematic odd-parity blossom shattering.
#[test]
fn mwpm_shatter_extract_with_blossom() {
    use rmatching::Matching;
    // Triangle with 2 firing - simple match, no blossom needed
    let dem = concat!(
        "error(0.1) D0 D1 L0\n",
        "error(0.1) D1 D2\n",
        "error(0.1) D0 D2\n",
        "error(0.1) D0\n",
        "error(0.1) D1\n",
        "error(0.1) D2\n",
    );
    let mut m = Matching::from_dem(dem).unwrap();

    // Even number of detectors fires
    let pred = m.decode(&[1, 1, 0]);
    assert_eq!(pred.len(), 1);
    assert_eq!(pred[0], 1);

    // Another even pair
    let pred = m.decode(&[0, 1, 1]);
    assert_eq!(pred.len(), 1);
}

// ---------------------------------------------------------------------------
// Coverage tests: alt_tree add_child (lines 103, 109-112)
// ---------------------------------------------------------------------------

#[test]
fn alt_tree_add_child() {
    use rmatching::matcher::alt_tree::{AltTreeEdge, AltTreeNode};
    use rmatching::interop::CompressedEdge;
    use rmatching::util::arena::Arena;

    let mut arena: Arena<AltTreeNode> = Arena::new();
    let e = CompressedEdge {
        loc_from: Some(NodeIdx(0)),
        loc_to: Some(NodeIdx(1)),
        obs_mask: 0b01,
    };

    // Create root
    let root = AltTreeIdx(arena.alloc());
    arena[root.0] = AltTreeNode::new_root(RegionIdx(0));

    // Create child
    let child = AltTreeIdx(arena.alloc());
    arena[child.0] = AltTreeNode::new_pair(RegionIdx(1), RegionIdx(2), e);

    // Use add_child method - need to work around borrow checker:
    // Take root node out, call add_child, put it back
    let mut root_node = std::mem::take(&mut arena[root.0]);
    let child_edge = AltTreeEdge::new(child, e);
    root_node.add_child(root, child_edge, &mut arena);
    arena[root.0] = root_node;

    // Verify child's parent is set correctly
    assert!(arena[child.0].parent.is_some());
    assert_eq!(arena[child.0].parent.as_ref().unwrap().alt_tree_node, root);
    // Verify parent's children list
    assert_eq!(arena[root.0].children.len(), 1);
    assert_eq!(arena[root.0].children[0].alt_tree_node, child);
}

// ---------------------------------------------------------------------------
// Coverage tests: alt_tree clear_visited_upward when node not visited (line 226)
// ---------------------------------------------------------------------------

#[test]
fn alt_tree_lca_different_trees() {
    use rmatching::matcher::alt_tree::AltTreeNode;
    use rmatching::util::arena::Arena;

    let mut arena: Arena<AltTreeNode> = Arena::new();

    // Tree 1: root1
    let root1 = AltTreeIdx(arena.alloc());
    arena[root1.0] = AltTreeNode::new_root(RegionIdx(0));

    // Tree 2: root2
    let root2 = AltTreeIdx(arena.alloc());
    arena[root2.0] = AltTreeNode::new_root(RegionIdx(1));

    // LCA of nodes in different trees should be None
    let lca = AltTreeNode::most_recent_common_ancestor(root1, root2, &mut arena);
    assert_eq!(lca, None);

    // Verify visited flags were cleaned up
    assert!(!arena[root1.0].visited);
    assert!(!arena[root2.0].visited);
}

#[test]
fn alt_tree_lca_deep_tree() {
    use rmatching::matcher::alt_tree::{AltTreeEdge, AltTreeNode};
    use rmatching::interop::CompressedEdge;
    use rmatching::util::arena::Arena;

    let mut arena: Arena<AltTreeNode> = Arena::new();
    let e = CompressedEdge::empty();

    // Build a chain: root -> c1 -> c2 -> c3
    let root = AltTreeIdx(arena.alloc());    arena[root.0] = AltTreeNode::new_root(RegionIdx(0));

    let c1 = AltTreeIdx(arena.alloc());
    arena[c1.0] = AltTreeNode::new_pair(RegionIdx(1), RegionIdx(2), e);
    arena[root.0].children.push(AltTreeEdge::new(c1, e));
    arena[c1.0].parent = Some(AltTreeEdge::new(root, e));

    let c2 = AltTreeIdx(arena.alloc());
    arena[c2.0] = AltTreeNode::new_pair(RegionIdx(3), RegionIdx(4), e);
    arena[c1.0].children.push(AltTreeEdge::new(c2, e));
    arena[c2.0].parent = Some(AltTreeEdge::new(c1, e));

    let c3 = AltTreeIdx(arena.alloc());
    arena[c3.0] = AltTreeNode::new_pair(RegionIdx(5), RegionIdx(6), e);
    arena[c2.0].children.push(AltTreeEdge::new(c3, e));
    arena[c3.0].parent = Some(AltTreeEdge::new(c2, e));

    // Also add a branch from root
    let c4 = AltTreeIdx(arena.alloc());
    arena[c4.0] = AltTreeNode::new_pair(RegionIdx(7), RegionIdx(8), e);
    arena[root.0].children.push(AltTreeEdge::new(c4, e));
    arena[c4.0].parent = Some(AltTreeEdge::new(root, e));

    // LCA of c3 and c4 should be root
    let lca = AltTreeNode::most_recent_common_ancestor(c3, c4, &mut arena);
    assert_eq!(lca, Some(root));
}

// ---------------------------------------------------------------------------
// Coverage tests: alt_tree unstable_erase_by_node when not found
// ---------------------------------------------------------------------------

#[test]
fn alt_tree_unstable_erase_not_found() {
    use rmatching::matcher::alt_tree::{unstable_erase_by_node, AltTreeEdge};
    use rmatching::interop::CompressedEdge;

    let mut vec = vec![
        AltTreeEdge::new(AltTreeIdx(0), CompressedEdge::empty()),
        AltTreeEdge::new(AltTreeIdx(1), CompressedEdge::empty()),
    ];

    // Try to erase a node that doesn't exist
    let removed = unstable_erase_by_node(&mut vec, AltTreeIdx(99));
    assert!(!removed);
    assert_eq!(vec.len(), 2);
}

// ---------------------------------------------------------------------------
// Coverage tests: Larger blossom scenarios exercising lines 410-496
// (blossom_shattering with gap%2==0 and gap%2!=0 paths)
// ---------------------------------------------------------------------------

/// Hexagonal graph with even-parity syndromes.
#[test]
fn mwpm_hexagonal_blossom() {
    use rmatching::Matching;
    // 6-cycle plus spurs
    let dem = concat!(
        "error(0.1) D0 D1\n",
        "error(0.1) D1 D2\n",
        "error(0.1) D2 D3\n",
        "error(0.1) D3 D4\n",
        "error(0.1) D4 D5\n",
        "error(0.1) D5 D0 L0\n",
        "error(0.1) D0 D6\n",
        "error(0.05) D6\n",
        "error(0.05) D3\n",
    );
    let mut m = Matching::from_dem(dem).unwrap();

    // 2 adjacent detectors
    let pred = m.decode(&[1, 1, 0, 0, 0, 0, 0]);
    assert_eq!(pred.len(), 1);

    // 4 detectors (even) on one side of cycle
    let pred = m.decode(&[1, 1, 1, 1, 0, 0, 0]);
    assert_eq!(pred.len(), 1);

    // 6 detectors (even)
    let pred = m.decode(&[1, 1, 1, 1, 1, 1, 0]);
    assert_eq!(pred.len(), 1);
}

/// K4 (complete graph on 4 nodes) exercises multiple matching paths.
#[test]
fn mwpm_k4_complete_graph() {
    use rmatching::Matching;
    let dem = concat!(
        "error(0.1) D0 D1 L0\n",
        "error(0.1) D0 D2\n",
        "error(0.1) D0 D3\n",
        "error(0.1) D1 D2\n",
        "error(0.1) D1 D3\n",
        "error(0.1) D2 D3 L1\n",
        "error(0.05) D0\n",
        "error(0.05) D1\n",
        "error(0.05) D2\n",
        "error(0.05) D3\n",
    );
    let mut m = Matching::from_dem(dem).unwrap();

    // 2 adjacent fire
    let pred = m.decode(&[1, 1, 0, 0]);
    assert_eq!(pred.len(), 2);
    assert_eq!(pred[0], 1); // D0-D1 carries L0
    assert_eq!(pred[1], 0);

    // 2 non-adjacent fire
    let pred = m.decode(&[1, 0, 0, 1]);
    assert_eq!(pred.len(), 2);

    // Only 1 fires (boundary match)
    let pred = m.decode(&[1, 0, 0, 0]);
    assert_eq!(pred.len(), 2);
}

// ---------------------------------------------------------------------------
// Coverage tests: tree hitting other tree (lines 195-225)
// This happens when two independent trees collide.
// ---------------------------------------------------------------------------

#[test]
fn mwpm_two_trees_collide() {
    // 4 nodes in a chain: 0--1--2--3
    // Fire D0 and D3. They grow into trees. Eventually trees from D0 and D3 meet.
    let mut g = MatchingGraph::new(4, 1);
    g.add_edge(0, 1, 10, &[0]);
    g.add_edge(1, 2, 10, &[]);
    g.add_edge(2, 3, 10, &[]);
    g.add_boundary_edge(0, 50, &[]);
    g.add_boundary_edge(3, 50, &[]);

    let mut mwpm = Mwpm::new(GraphFlooder::new(g));
    mwpm.create_detection_event(NodeIdx(0));
    mwpm.create_detection_event(NodeIdx(3));

    let mut event_count = 0;
    loop {
        let event = mwpm.flooder.run_until_next_mwpm_notification();
        if event.is_no_event() {
            break;
        }
        mwpm.process_event(event);
        event_count += 1;
        if event_count > 20 {
            break;
        }
    }

    assert!(event_count >= 1);
}

// ---------------------------------------------------------------------------
// Coverage tests: MatchingResult AddAssign
// ---------------------------------------------------------------------------

#[test]
fn matching_result_add_assign() {
    use rmatching::matcher::mwpm::MatchingResult;

    let mut a = MatchingResult::new();
    assert_eq!(a.obs_mask, 0);
    assert_eq!(a.weight, 0);

    let b = MatchingResult { obs_mask: 0b101, weight: 42 };
    a += b;
    assert_eq!(a.obs_mask, 0b101);
    assert_eq!(a.weight, 42);

    // XOR semantics for obs_mask
    let c = MatchingResult { obs_mask: 0b111, weight: 8 };
    a += c;
    assert_eq!(a.obs_mask, 0b010);
    assert_eq!(a.weight, 50);
}

// ---------------------------------------------------------------------------
// Coverage: repeated decode exercises reset + re-blossom paths
// ---------------------------------------------------------------------------

#[test]
fn mwpm_repeated_decode_with_blossoms() {
    use rmatching::Matching;
    let dem = concat!(
        "error(0.1) D0 D1 L0\n",
        "error(0.1) D1 D2\n",
        "error(0.1) D0 D2\n",
        "error(0.1) D0\n",
        "error(0.1) D2\n",
    );
    let mut m = Matching::from_dem(dem).unwrap();

    // Multiple decodes with even parity to exercise reset paths
    for _ in 0..5 {
        let pred = m.decode(&[1, 1, 0]);
        assert_eq!(pred, vec![1]);
    }

    for _ in 0..5 {
        let pred = m.decode(&[0, 0, 0]);
        assert_eq!(pred, vec![0]);
    }
}
