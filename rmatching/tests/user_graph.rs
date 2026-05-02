use rmatching::driver::user_graph::{UserGraph, NUM_DISTINCT_WEIGHTS};

#[test]
fn user_graph_add_edge() {
    let mut g = UserGraph::new();
    g.add_edge(0, 1, vec![0, 2], 1.5, 0.1);
    g.add_edge(1, 2, vec![1], 2.0, 0.2);

    assert_eq!(g.get_num_edges(), 2);
    assert_eq!(g.get_num_nodes(), 3);
    assert_eq!(g.num_observables, 3); // indices 0,1,2 → num = 3

    let e = &g.edges[0];
    assert_eq!(e.node1, 0);
    assert_eq!(e.node2, 1);
    assert_eq!(e.observable_indices, vec![0, 2]);
    assert!((e.weight - 1.5).abs() < 1e-12);
    assert!((e.error_probability - 0.1).abs() < 1e-12);
}

#[test]
fn user_graph_add_boundary_edge() {
    let mut g = UserGraph::new();
    g.add_edge(0, 1, vec![0], 1.0, 0.1);
    g.add_boundary_edge(0, vec![1], 0.5, 0.05);

    assert_eq!(g.get_num_edges(), 2);
    assert_eq!(g.edges[1].node2, usize::MAX);
}

#[test]
fn user_graph_to_matching_graph() {
    let mut g = UserGraph::new();
    g.add_edge(0, 1, vec![0], 1.0, 0.1);
    g.add_edge(1, 2, vec![1], 2.0, 0.2);
    g.add_boundary_edge(2, vec![0, 1], 0.5, 0.05);

    let mg = g.to_matching_graph(NUM_DISTINCT_WEIGHTS);
    // 3 detector nodes
    assert_eq!(mg.nodes.len(), 3);
    // node 0 has 1 neighbor (node 1)
    assert_eq!(mg.nodes[0].neighbors.len(), 1);
    // node 1 has 2 neighbors (node 0 and node 2)
    assert_eq!(mg.nodes[1].neighbors.len(), 2);
    // node 2 has 2 neighbors (node 1 + boundary)
    assert_eq!(mg.nodes[2].neighbors.len(), 2);
}

#[test]
fn user_graph_to_search_graph() {
    let mut g = UserGraph::new();
    g.add_edge(0, 1, vec![0], 1.0, 0.1);
    g.add_boundary_edge(1, vec![1], 0.5, 0.05);

    let sg = g.to_search_graph(NUM_DISTINCT_WEIGHTS);
    assert_eq!(sg.nodes.len(), 2);
    // node 0: neighbor is node 1
    assert_eq!(sg.nodes[0].neighbors.len(), 1);
    // node 1: boundary edge (inserted at front) + node 0
    assert_eq!(sg.nodes[1].neighbors.len(), 2);
}

#[test]
fn user_graph_dem_instruction() {
    let mut g = UserGraph::new();
    let p = 0.1;
    g.handle_dem_instruction(p, &[0, 1], vec![0]);

    assert_eq!(g.get_num_edges(), 1);
    let expected_weight = ((1.0 - p) / p).ln();
    assert!((g.edges[0].weight - expected_weight).abs() < 1e-12);
    assert!((g.edges[0].error_probability - p).abs() < 1e-12);
}

#[test]
fn user_graph_dem_instruction_boundary() {
    let mut g = UserGraph::new();
    g.handle_dem_instruction(0.2, &[3], vec![0, 1]);

    assert_eq!(g.get_num_edges(), 1);
    assert_eq!(g.edges[0].node1, 3);
    assert_eq!(g.edges[0].node2, usize::MAX);
    let expected = ((1.0_f64 - 0.2) / 0.2).ln();
    assert!((g.edges[0].weight - expected).abs() < 1e-12);
}

#[test]
fn user_graph_set_boundary() {
    let mut g = UserGraph::new();
    g.add_edge(0, 1, vec![], 1.0, 0.1);
    g.add_edge(1, 2, vec![], 1.0, 0.1);

    let boundary: std::collections::HashSet<usize> = [2].into_iter().collect();
    g.set_boundary(boundary);

    assert!(g.is_boundary_node(2));
    assert!(!g.is_boundary_node(0));
    assert!(g.is_boundary_node(usize::MAX));
    assert_eq!(g.get_num_detectors(), 2); // 3 nodes - 1 boundary
}

#[test]
fn user_graph_to_mwpm() {
    let mut g = UserGraph::new();
    g.add_edge(0, 1, vec![0], 1.0, 0.1);
    g.add_edge(1, 2, vec![1], 2.0, 0.2);

    // Should not panic
    let _mwpm = g.to_mwpm();
}

#[test]
fn user_graph_get_mwpm_lazy() {
    let mut g = UserGraph::new();
    g.add_edge(0, 1, vec![0], 1.0, 0.1);

    // First call builds it
    let _ = g.get_mwpm();
    // Second call reuses cached
    let _ = g.get_mwpm();
}

#[test]
fn user_graph_get_mwpm_invalidation() {
    let mut g = UserGraph::new();
    g.add_edge(0, 1, vec![0], 1.0, 0.1);
    let _ = g.get_mwpm();

    // Adding an edge invalidates the cache
    g.add_edge(1, 2, vec![1], 2.0, 0.2);
    // This should rebuild
    let _ = g.get_mwpm();
}
