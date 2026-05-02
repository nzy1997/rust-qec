use rmatching::Matching;

/// 3-node chain: D0 -- D1 -- D2, with L0 on the D0-D1 edge.
/// Fire D0 and D1 => should predict L0 flipped.
#[test]
fn decode_simple_chain() {
    let mut m = Matching::new();
    // D0 -- D1 with observable L0, weight 1.0
    m.add_edge(0, 1, 1.0, &[0], 0.1);
    // D1 -- D2 with no observable, weight 1.0
    m.add_edge(1, 2, 1.0, &[], 0.1);
    // Boundary edges so odd-parity components can resolve
    m.add_boundary_edge(0, 2.0, &[], 0.1);
    m.add_boundary_edge(2, 2.0, &[], 0.1);

    // Syndrome: D0=1, D1=1, D2=0
    let syndrome = vec![1u8, 1, 0];
    let prediction = m.decode(&syndrome);

    // D0 and D1 should match via the D0-D1 edge which carries L0
    assert_eq!(prediction.len(), 1);
    assert_eq!(prediction[0], 1, "Expected L0 to be flipped");
}

/// Single detection near boundary should match to boundary.
#[test]
fn decode_boundary() {
    let mut m = Matching::new();
    m.add_boundary_edge(0, 1.0, &[0], 0.1);
    m.add_edge(0, 1, 3.0, &[], 0.1);
    m.add_boundary_edge(1, 3.0, &[], 0.1);

    // Only D0 fires
    let syndrome = vec![1u8, 0];
    let prediction = m.decode(&syndrome);

    assert_eq!(prediction.len(), 1);
    assert_eq!(prediction[0], 1, "Expected L0 flipped via boundary match");
}

/// Empty syndrome => no observable flips.
#[test]
fn decode_no_errors() {
    let mut m = Matching::new();
    m.add_edge(0, 1, 1.0, &[0], 0.1);
    m.add_boundary_edge(0, 2.0, &[], 0.1);
    m.add_boundary_edge(1, 2.0, &[], 0.1);

    let syndrome = vec![0u8, 0];
    let prediction = m.decode(&syndrome);

    assert_eq!(prediction.len(), 1);
    assert_eq!(prediction[0], 0, "No errors => no observable flips");
}

/// Batch results should match individual decodes.
#[test]
fn decode_batch_matches_single() {
    let mut m = Matching::new();
    m.add_edge(0, 1, 1.0, &[0], 0.1);
    m.add_boundary_edge(0, 2.0, &[], 0.1);
    m.add_boundary_edge(1, 2.0, &[], 0.1);

    let syndromes = vec![
        vec![1u8, 1],
        vec![0, 0],
        vec![1, 0],
    ];

    // Get individual results
    let mut m2 = Matching::new();
    m2.add_edge(0, 1, 1.0, &[0], 0.1);
    m2.add_boundary_edge(0, 2.0, &[], 0.1);
    m2.add_boundary_edge(1, 2.0, &[], 0.1);

    let individual: Vec<Vec<u8>> = syndromes.iter().map(|s| m2.decode(s)).collect();
    let batch = m.decode_batch(&syndromes);

    assert_eq!(batch, individual);
}

/// Verify matched pairs returned by decode_to_edges.
#[test]
fn decode_to_edges_simple() {
    let mut m = Matching::new();
    m.add_edge(0, 1, 1.0, &[0], 0.1);
    m.add_boundary_edge(0, 3.0, &[], 0.1);
    m.add_boundary_edge(1, 3.0, &[], 0.1);

    // D0 and D1 both fire => should match to each other
    let syndrome = vec![1u8, 1];
    let edges = m.decode_to_edges(&syndrome);

    assert_eq!(edges.len(), 1, "Expected one matched pair");
    let (a, b) = edges[0];
    // Should be (0, 1) or (1, 0)
    assert!(
        (a == 0 && b == 1) || (a == 1 && b == 0),
        "Expected edge (0,1), got ({}, {})",
        a,
        b
    );
}

/// DEM-based decode test.
#[test]
fn decode_from_dem() {
    let dem = "\
error(0.1) D0 D1 L0
error(0.1) D1 D2
error(0.05) D0
error(0.05) D2
";
    let mut m = Matching::from_dem(dem).unwrap();

    // Fire D0 and D1
    let syndrome = vec![1u8, 1, 0];
    let prediction = m.decode(&syndrome);
    assert_eq!(prediction.len(), 1);
    assert_eq!(prediction[0], 1, "Expected L0 flipped from DEM decode");
}
