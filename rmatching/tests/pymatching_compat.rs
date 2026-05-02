//! Tests ported from PyMatching's test suite.
//!
//! These verify that rmatching produces identical decode results to PyMatching
//! for the same graph topologies and syndromes.

use rmatching::Matching;

// ---------------------------------------------------------------------------
// From PyMatching: tests/matching/decode_test.py
// ---------------------------------------------------------------------------

/// Ported from test_negative_weight_repetition_code.
/// 6-node ring with all negative weights.
#[test]
fn pm_negative_weight_repetition_code() {
    let mut m = Matching::new();
    m.add_edge(0, 1, -1.0, &[0], -1.0);
    m.add_edge(1, 2, -1.0, &[1], -1.0);
    m.add_edge(2, 3, -1.0, &[2], -1.0);
    m.add_edge(3, 4, -1.0, &[3], -1.0);
    m.add_edge(4, 5, -1.0, &[4], -1.0);
    m.add_edge(5, 0, -1.0, &[5], -1.0);

    let c = m.decode(&[0, 1, 1, 0, 0, 0]);
    assert_eq!(c, vec![1, 0, 1, 1, 1, 1]);
}

/// Ported from test_isolated_negative_weight.
/// 4-node ring with one isolated negative-weight edge.
#[test]
fn pm_isolated_negative_weight() {
    let mut m = Matching::new();
    m.add_edge(0, 1, 1.0, &[0], -1.0);
    m.add_edge(1, 2, -10.0, &[1], -1.0);
    m.add_edge(2, 3, 1.0, &[2], -1.0);
    m.add_edge(3, 0, 1.0, &[3], -1.0);

    let c = m.decode(&[0, 1, 1, 0]);
    assert_eq!(c, vec![0, 1, 0, 0]);
}

/// Ported from test_negative_and_positive_in_matching.
/// 4-node ring with mixed positive and negative weights.
#[test]
fn pm_negative_and_positive_in_matching() {
    let mut m = Matching::new();
    m.add_edge(0, 1, 1.0, &[0], -1.0);
    m.add_edge(1, 2, -10.0, &[1], -1.0);
    m.add_edge(2, 3, 1.0, &[2], -1.0);
    m.add_edge(3, 0, 1.0, &[3], -1.0);

    let c = m.decode(&[0, 1, 0, 1]);
    assert_eq!(c, vec![0, 1, 1, 0]);
}

/// Ported from test_decode_to_matched_detection_events.
/// 21-node chain with boundary edges at both ends, using decode_to_edges.
#[test]
fn pm_decode_to_matched_detection_events() {
    let num_nodes = 20;
    let mut m = Matching::new();
    m.add_boundary_edge(0, 1.0, &[], -1.0);
    for i in 0..num_nodes {
        m.add_edge(i, i + 1, 1.0, &[], -1.0);
    }
    m.add_boundary_edge(num_nodes, 1.0, &[], -1.0);

    // Detection events at nodes 2, 10, 12, 18
    let mut syndrome = vec![0u8; num_nodes + 1];
    syndrome[2] = 1;
    syndrome[10] = 1;
    syndrome[12] = 1;
    syndrome[18] = 1;

    let edges = m.decode_to_edges(&syndrome);

    // Expected matched pairs: (2, boundary), (10, 12), (18, boundary)
    // rmatching deduplicates with from <= to, so:
    //   (2, -1), (10, 12), (18, -1)
    assert_eq!(edges.len(), 3);

    // Collect into a sorted set for order-independent comparison
    let mut sorted_edges: Vec<(i64, i64)> = edges.clone();
    sorted_edges.sort();
    assert_eq!(sorted_edges, vec![(2, -1), (10, 12), (18, -1)]);
}

/// Ported from test_decode_self_loops.
/// Self-loop edges with negative weights contribute to negative_weight_observables
/// but do not create graph edges.
#[test]
fn pm_decode_self_loops() {
    let mut m = Matching::new();
    m.add_boundary_edge(0, 1.0, &[0], -1.0);
    m.add_edge(0, 1, 3.0, &[1], -1.0);
    m.add_edge(1, 2, 3.0, &[2], -1.0);
    m.add_edge(2, 2, -100.0, &[3], -1.0);
    m.add_edge(3, 3, -200.0, &[4], -1.0);
    m.add_edge(4, 4, 4.0, &[5], -1.0);

    let c = m.decode(&[0, 0, 1, 0, 0]);
    assert_eq!(c, vec![1, 1, 1, 1, 1, 0]);
}

/// Ported from test_syndrome_on_boundary_nodes.
/// Verifies that decoding with boundary nodes set via set_boundary does not panic.
#[test]
fn pm_syndrome_on_boundary_nodes() {
    let mut m = Matching::new();
    m.add_edge(0, 1, 1.0, &[0], -1.0);
    m.add_edge(1, 2, 1.0, &[1], -1.0);
    m.add_edge(2, 3, 1.0, &[2], -1.0);
    m.add_edge(3, 4, 1.0, &[3], -1.0);
    m.set_boundary(&[3, 4]);

    m.decode(&[0, 0, 0, 1, 0]);
    m.decode(&[0, 0, 0, 0, 1]);
    m.decode(&[0, 0, 0, 1, 1]);
    m.decode(&[1, 0, 1, 0, 1]);
}

/// Ported from test_decode_to_edges.
/// 11-node chain with boundary at node 0, 5 detection events.
#[test]
fn pm_decode_to_edges() {
    let mut m = Matching::new();
    m.add_boundary_edge(0, 1.0, &[], -1.0);
    for i in 0..10 {
        m.add_edge(i, i + 1, 1.0, &[], -1.0);
    }

    let edges = m.decode_to_edges(&[0, 1, 0, 1, 0, 0, 1, 0, 1, 1, 0]);

    // Detection events at 1, 3, 6, 8, 9 (5 events, odd → one boundary match)
    // MWPM: 1→boundary (cost 1), 3↔6 (cost 3), 8↔9 (cost 1) = total 5
    let mut sorted_edges: Vec<(i64, i64)> = edges.clone();
    sorted_edges.sort();
    assert_eq!(sorted_edges, vec![(1, -1), (3, 6), (8, 9)]);
}

/// Ported from test_parallel_boundary_edges_decoding (first part).
/// Two boundary nodes with a single detector between them.
#[test]
fn pm_parallel_boundary_edges_basic() {
    let mut m = Matching::new();
    m.set_boundary(&[0, 2]);
    m.add_edge(0, 1, 3.5, &[0], -1.0);
    m.add_edge(1, 2, 2.5, &[1], -1.0);

    // Node 1 fires, nodes 0 and 2 are boundary → 1 detection event → boundary match
    // Cheaper path: 1→2 (weight 2.5, obs {1})
    let c = m.decode(&[0, 1]);
    assert_eq!(c, vec![0, 1]);
}

/// Ported from test_parallel_boundary_edges_decoding (second part).
/// Adding an expensive direct boundary edge should not change the result.
#[test]
fn pm_parallel_boundary_edges_with_extra() {
    let mut m = Matching::new();
    m.set_boundary(&[0, 2]);
    m.add_edge(0, 1, 3.5, &[0], -1.0);
    m.add_edge(1, 2, 2.5, &[1], -1.0);
    m.add_boundary_edge(1, 100.0, &[100], -1.0);

    // Still cheaper to match via node 2 (weight 2.5)
    let c = m.decode(&[0, 1]);
    // Only observable 1 should be set
    assert_eq!(c[1], 1);
    let nonzero_count: usize = c.iter().filter(|&&x| x != 0).count();
    assert_eq!(nonzero_count, 1);
}

/// Ported from test_parallel_boundary_edges_decoding (third part).
/// Star graph with negative weights and no explicit boundary, then with boundary.
#[test]
fn pm_parallel_boundary_edges_negative_weights() {
    let mut m = Matching::new();
    m.add_edge(0, 1, -1.0, &[0], -1.0);
    m.add_edge(0, 2, 3.0, &[1], -1.0);
    m.add_boundary_edge(0, -0.5, &[2], -1.0);
    m.add_edge(0, 3, -3.0, &[3], -1.0);
    m.add_edge(0, 4, -2.0, &[4], -1.0);

    let c = m.decode(&[1, 0, 0, 0, 0]);
    assert_eq!(c, vec![0, 0, 1, 0, 0]);

    // Now set nodes 1-4 as boundary
    m.set_boundary(&[1, 2, 3, 4]);
    let c = m.decode(&[1, 0, 0, 0, 0]);
    assert_eq!(c, vec![0, 0, 0, 1, 0]);
}

/// All-negative-weight ring with 6 nodes.
/// Detection events at nodes 1 and 4.
#[test]
fn pm_all_negative_ring_6() {
    let mut m = Matching::new();
    for i in 0..6 {
        m.add_edge(i, (i + 1) % 6, -2.0, &[i], -1.0);
    }
    let c = m.decode(&[0, 1, 0, 0, 1, 0]);
    assert_eq!(c, vec![1, 0, 0, 0, 1, 1]);
}

/// All-negative-weight ring with 10 nodes.
/// Detection events at nodes 2 and 5.
#[test]
fn pm_all_negative_ring_10() {
    let mut m = Matching::new();
    for i in 0..10 {
        m.add_edge(i, (i + 1) % 10, -2.0, &[i], -1.0);
    }
    let mut s = vec![0u8; 10];
    s[2] = 1;
    s[5] = 1;
    let c = m.decode(&s);
    // Expected nonzero at: 0, 1, 5, 6, 7, 8, 9
    let nonzero: Vec<usize> = c.iter().enumerate().filter(|&(_, &v)| v != 0).map(|(i, _)| i).collect();
    assert_eq!(nonzero, vec![0, 1, 5, 6, 7, 8, 9]);
}

/// All-negative-weight ring with 20 nodes.
/// Detection events at nodes 2 and 10.
#[test]
fn pm_all_negative_ring_20() {
    let mut m = Matching::new();
    for i in 0..20 {
        m.add_edge(i, (i + 1) % 20, -2.0, &[i], -1.0);
    }
    let mut s = vec![0u8; 20];
    s[2] = 1;
    s[10] = 1;
    let c = m.decode(&s);
    let nonzero: Vec<usize> = c.iter().enumerate().filter(|&(_, &v)| v != 0).map(|(i, _)| i).collect();
    assert_eq!(nonzero, vec![0, 1, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]);
}

/// Mixed positive/negative weight chain with 8 nodes.
/// Adapted from C++ HandleSomeNegativeWeights test.
#[test]
fn pm_mixed_weight_chain_8() {
    let mut m = Matching::new();
    m.add_boundary_edge(0, -4.0, &[0], -1.0);
    for i in (0..7).step_by(2) {
        m.add_edge(i, i + 1, 2.0, &[i + 1], -1.0);
    }
    for i in (1..7).step_by(2) {
        m.add_edge(i, i + 1, -4.0, &[i + 1], -1.0);
    }
    m.add_boundary_edge(7, 2.0, &[8], -1.0);

    let c = m.decode(&[1, 1, 1, 0, 0, 1, 1, 1]);
    assert_eq!(c, vec![1, 0, 1, 0, 0, 0, 1, 0, 1]);
}

/// decode_batch: verify that batched decoding matches individual decodes.
#[test]
fn pm_decode_batch() {
    let mut m = Matching::new();
    m.add_boundary_edge(0, 1.0, &[0], -1.0);
    for i in 0..4 {
        m.add_edge(i, i + 1, 1.0, &[i + 1], -1.0);
    }
    m.add_boundary_edge(4, 1.0, &[5], -1.0);

    let syndromes = vec![
        vec![1, 0, 0, 0, 1],
        vec![0, 1, 0, 1, 0],
        vec![1, 1, 0, 0, 0],
    ];

    let batch_results = m.decode_batch(&syndromes);

    assert_eq!(batch_results[0], vec![1, 0, 0, 0, 0, 1]);
    assert_eq!(batch_results[1], vec![0, 0, 1, 1, 0, 0]);
    assert_eq!(batch_results[2], vec![0, 1, 0, 0, 0, 0]);
}
