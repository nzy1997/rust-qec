use rmatching::driver::dem_parse::parse_dem;

#[test]
fn parse_simple_dem() {
    let dem = "error(0.1) D0 D1 L0";
    let g = parse_dem(dem).unwrap();
    assert_eq!(g.edges.len(), 1);
    let e = &g.edges[0];
    assert_eq!(e.node1, 0);
    assert_eq!(e.node2, 1);
    assert_eq!(e.observable_indices, vec![0]);
    assert!((e.error_probability - 0.1).abs() < 1e-9);
}

#[test]
fn parse_boundary_dem() {
    let dem = "error(0.1) D0 L0";
    let g = parse_dem(dem).unwrap();
    assert_eq!(g.edges.len(), 1);
    let e = &g.edges[0];
    assert_eq!(e.node1, 0);
    assert_eq!(e.node2, usize::MAX); // boundary sentinel
    assert_eq!(e.observable_indices, vec![0]);
}

#[test]
fn parse_repeat_dem() {
    // repeat 3 iterations, each with shift_detectors 2
    let dem = "\
repeat 3 {
    error(0.1) D0 D1 L0
    shift_detectors 2
}";
    let g = parse_dem(dem).unwrap();
    // 3 iterations → 3 edges
    assert_eq!(g.edges.len(), 3);
    // Iteration 0: D0-D1, iteration 1: D2-D3, iteration 2: D4-D5
    assert_eq!(g.edges[0].node1, 0);
    assert_eq!(g.edges[0].node2, 1);
    assert_eq!(g.edges[1].node1, 2);
    assert_eq!(g.edges[1].node2, 3);
    assert_eq!(g.edges[2].node1, 4);
    assert_eq!(g.edges[2].node2, 5);
}

#[test]
fn parse_dem_roundtrip() {
    let dem = "\
error(0.1) D0 D1 L0
error(0.05) D1 D2
error(0.1) D0 L1
detector D0
detector D1
detector D2
";
    let g = parse_dem(dem).unwrap();
    // 2 normal edges + 1 boundary edge
    assert_eq!(g.edges.len(), 3);
    assert_eq!(g.get_num_nodes(), 3); // D0, D1, D2
    assert_eq!(g.num_observables, 2); // L0 and L1
}

#[test]
fn parse_correlated_segments_from_single_error_instruction() {
    let dem = "error(0.1) D0 D1 L0 ^ D2 L1 ^ D3 D4";
    let g = parse_dem(dem).unwrap();

    assert_eq!(g.edges.len(), 3);

    assert_eq!(g.edges[0].node1, 0);
    assert_eq!(g.edges[0].node2, 1);
    assert_eq!(g.edges[0].observable_indices, vec![0]);

    assert_eq!(g.edges[1].node1, 2);
    assert_eq!(g.edges[1].node2, usize::MAX);
    assert_eq!(g.edges[1].observable_indices, vec![1]);

    assert_eq!(g.edges[2].node1, 3);
    assert_eq!(g.edges[2].node2, 4);
    assert!(g.get_num_nodes() >= 5);
}

#[test]
fn parse_repeat_with_coordinate_shift_and_detector_shift() {
    let dem = "\
repeat 2 {
    error(0.1) D0 D1
    shift_detectors(0, 0, 1) 0
    shift_detectors 2
}";
    let g = parse_dem(dem).unwrap();

    assert_eq!(g.edges.len(), 2);
    assert_eq!((g.edges[0].node1, g.edges[0].node2), (0, 1));
    assert_eq!((g.edges[1].node1, g.edges[1].node2), (2, 3));
}
