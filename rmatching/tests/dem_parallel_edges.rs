use rmatching::Matching;
use rmatching::driver::dem_parse::parse_dem;
use rmatching::driver::user_graph::UserGraph;

const TWO_HOP_EDGE_PROBABILITY: f64 = 0.35434369377420455;

#[test]
fn dem_parallel_edges_choose_combined_direct_path() {
    let dem = format!(
        "error(0.1) D0 D1 L0\n\
         error(0.2) D1 D0 L0\n\
         error({TWO_HOP_EDGE_PROBABILITY}) D0 D2\n\
         error({TWO_HOP_EDGE_PROBABILITY}) D2 D1"
    );

    let graph = parse_dem(&dem).unwrap();
    assert_eq!(graph.edges.len(), 3);
    let direct_edge = &graph.edges[0];
    assert!((direct_edge.error_probability - 0.26).abs() < 1e-12);
    assert!((direct_edge.weight - 1.0459685551826876).abs() < 1e-12);

    let mut matching = Matching::from_dem(&dem).unwrap();
    assert_eq!(matching.decode(&[1, 1, 0]), vec![1]);
}

#[test]
fn dem_parallel_edges_single_direct_edge_negative_control() {
    let dem = format!(
        "error(0.2) D0 D1 L0\n\
         error({TWO_HOP_EDGE_PROBABILITY}) D0 D2\n\
         error({TWO_HOP_EDGE_PROBABILITY}) D2 D1"
    );

    let mut matching = Matching::from_dem(&dem).unwrap();
    assert_eq!(matching.decode(&[1, 1, 0]), vec![0]);
}

#[test]
fn dem_parallel_edges_merge_boundary_and_keep_first_observables() {
    let dem = format!(
        "error(0.1) D0 L0\n\
         error(0.2) D0 L1\n\
         error({TWO_HOP_EDGE_PROBABILITY}) D0 D1\n\
         error({TWO_HOP_EDGE_PROBABILITY}) D1"
    );

    let graph = parse_dem(&dem).unwrap();
    assert_eq!(graph.edges.len(), 3);
    assert_eq!(graph.num_observables, 2);
    let direct_boundary_edge = &graph.edges[0];
    assert_eq!(direct_boundary_edge.observable_indices, vec![0]);
    assert!((direct_boundary_edge.error_probability - 0.26).abs() < 1e-12);

    let mut matching = Matching::from_dem(&dem).unwrap();
    assert_eq!(matching.decode(&[1, 0]), vec![1, 0]);
}

#[test]
fn programmatic_parallel_edges_remain_separate() {
    let mut graph = UserGraph::new();
    graph.add_edge(0, 1, vec![0], 2.0, 0.1);
    graph.add_edge(1, 0, vec![1], 1.0, 0.2);

    assert_eq!(graph.edges.len(), 2);
    assert_eq!(graph.edges[0].observable_indices, vec![0]);
    assert_eq!(graph.edges[1].observable_indices, vec![1]);
}
