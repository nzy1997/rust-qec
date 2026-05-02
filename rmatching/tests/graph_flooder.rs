use rmatching::flooder::graph::MatchingGraph;
use rmatching::flooder::graph_flooder::GraphFlooder;
use rmatching::interop::MwpmEvent;
use rmatching::types::*;

#[test]
fn flooder_create_detection_event() {
    let mut graph = MatchingGraph::new(3, 1);
    graph.add_edge(0, 1, 10, &[0]);
    graph.add_edge(1, 2, 10, &[]);
    let mut flooder = GraphFlooder::new(graph);
    let _r = flooder.create_detection_event(NodeIdx(0));
    assert!(flooder.graph.nodes[0].region_that_arrived.is_some());
    assert!(flooder.graph.nodes[0].region_that_arrived_top.is_some());
    assert_eq!(flooder.graph.nodes[0].reached_from_source, Some(NodeIdx(0)));
}

#[test]
fn flooder_two_events_collide() {
    let mut graph = MatchingGraph::new(2, 1);
    graph.add_edge(0, 1, 10, &[0]);
    let mut flooder = GraphFlooder::new(graph);
    flooder.create_detection_event(NodeIdx(0));
    flooder.create_detection_event(NodeIdx(1));
    let event = flooder.run_until_next_mwpm_notification();
    match event {
        MwpmEvent::RegionHitRegion { edge, .. } => {
            assert_eq!(edge.obs_mask, 1);
        }
        _ => panic!("Expected RegionHitRegion"),
    }
}

#[test]
fn flooder_boundary_hit() {
    let mut graph = MatchingGraph::new(1, 1);
    graph.add_boundary_edge(0, 5, &[0]);
    let mut flooder = GraphFlooder::new(graph);
    flooder.create_detection_event(NodeIdx(0));
    let event = flooder.run_until_next_mwpm_notification();
    match event {
        MwpmEvent::RegionHitBoundary { edge, .. } => {
            assert_eq!(edge.loc_to, None);
            assert_eq!(edge.obs_mask, 1);
        }
        _ => panic!("Expected RegionHitBoundary"),
    }
}

#[test]
fn flooder_chain_growth() {
    // 3-node chain: 0 --5-- 1 --5-- 2, boundary at 2
    let mut graph = MatchingGraph::new(3, 0);
    graph.add_edge(0, 1, 5, &[]);
    graph.add_edge(1, 2, 5, &[]);
    graph.add_boundary_edge(2, 5, &[]);
    let mut flooder = GraphFlooder::new(graph);
    flooder.create_detection_event(NodeIdx(0));
    let event = flooder.run_until_next_mwpm_notification();
    match event {
        MwpmEvent::RegionHitBoundary { .. } => {}
        _ => panic!("Expected RegionHitBoundary after chain growth"),
    }
}

#[test]
fn flooder_reset() {
    let mut graph = MatchingGraph::new(2, 1);
    graph.add_edge(0, 1, 10, &[0]);
    let mut flooder = GraphFlooder::new(graph);
    flooder.create_detection_event(NodeIdx(0));
    flooder.reset();
    assert!(flooder.graph.nodes[0].region_that_arrived.is_none());
    assert!(flooder.queue.is_empty());
}

#[test]
fn flooder_no_event_on_empty() {
    let graph = MatchingGraph::new(2, 0);
    let mut flooder = GraphFlooder::new(graph);
    let event = flooder.run_until_next_mwpm_notification();
    assert!(event.is_no_event());
}

// ---------------------------------------------------------------------------
// Coverage: set_region_growing, set_region_shrinking, set_region_frozen
// (lines 98, 101-102, 112, 240, 475-476, 480-481)
// ---------------------------------------------------------------------------

/// Test region state transitions: growing -> shrinking -> frozen.
/// Exercises set_region_shrinking (and schedule_tentative_shrink_event).
#[test]
fn flooder_region_state_transitions() {
    let mut graph = MatchingGraph::new(3, 1);
    graph.add_edge(0, 1, 20, &[0]);
    graph.add_edge(1, 2, 20, &[]);
    let mut flooder = GraphFlooder::new(graph);

    let region = flooder.create_detection_event(NodeIdx(0));

    // Region starts growing
    assert!(flooder.region_arena[region.0].radius.is_growing());

    // Set to shrinking
    flooder.set_region_shrinking(region);
    assert!(flooder.region_arena[region.0].radius.is_shrinking());

    // Set to frozen (from shrinking)
    flooder.set_region_frozen(region);
    assert!(flooder.region_arena[region.0].radius.is_frozen());
}

/// Test set_region_frozen when region was growing (not shrinking).
#[test]
fn flooder_freeze_growing_region() {
    let mut graph = MatchingGraph::new(2, 1);
    graph.add_edge(0, 1, 20, &[0]);
    let mut flooder = GraphFlooder::new(graph);

    let region = flooder.create_detection_event(NodeIdx(0));
    assert!(flooder.region_arena[region.0].radius.is_growing());

    // Freeze while growing (not shrinking - exercises the was_shrinking=false branch)
    flooder.set_region_frozen(region);
    assert!(flooder.region_arena[region.0].radius.is_frozen());
}

/// Test that a chain of 3 nodes with detection events at both ends
/// produces a collision, exercising grow-into-empty-node and collision paths.
#[test]
fn flooder_chain_three_nodes_collision() {
    let mut graph = MatchingGraph::new(3, 1);
    graph.add_edge(0, 1, 10, &[0]);
    graph.add_edge(1, 2, 10, &[]);
    let mut flooder = GraphFlooder::new(graph);

    flooder.create_detection_event(NodeIdx(0));
    flooder.create_detection_event(NodeIdx(2));

    // Should get a RegionHitRegion event when the two regions collide
    let event = flooder.run_until_next_mwpm_notification();
    match event {
        MwpmEvent::RegionHitRegion { .. } => {}
        _ => panic!("Expected RegionHitRegion"),
    }
}

/// Tests FloodCheckEvent HasTime trait methods.
#[test]
fn flood_check_event_has_time() {
    use rmatching::interop::FloodCheckEvent;
    use rmatching::util::radix_heap::HasTime;
    use std::num::Wrapping;

    let no = FloodCheckEvent::no_event();
    assert!(no.is_no_event());
    assert_eq!(no.time(), Wrapping(0));

    let node_ev = FloodCheckEvent::LookAtNode {
        node: NodeIdx(0),
        time: Wrapping(42),
    };
    assert!(!node_ev.is_no_event());
    assert_eq!(node_ev.time(), Wrapping(42));

    let shrink_ev = FloodCheckEvent::LookAtShrinkingRegion {
        region: RegionIdx(0),
        time: Wrapping(99),
    };
    assert!(!shrink_ev.is_no_event());
    assert_eq!(shrink_ev.time(), Wrapping(99));

    let search_ev = FloodCheckEvent::LookAtSearchNode {
        node: SearchNodeIdx(0),
        time: Wrapping(7),
    };
    assert!(!search_ev.is_no_event());
    assert_eq!(search_ev.time(), Wrapping(7));
}
