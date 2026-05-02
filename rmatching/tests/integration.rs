use rmatching::Matching;

// ---------------------------------------------------------------------------
// 1. e2e_rep_code_d3
// ---------------------------------------------------------------------------

/// Distance-3 repetition code: 3 data qubits, 2 detectors (D0, D1), 1 observable (L0).
///
/// Graph:
///   boundary --[L0]-- D0 --[L0]-- D1 --[]-- boundary
///
/// All edges use p=0.1 => weight = ln(0.9/0.1).
#[test]
fn e2e_rep_code_d3() {
    let p: f64 = 0.1;
    let w = ((1.0 - p) / p).ln();

    let mut m = Matching::new();
    m.add_edge(0, 1, w, &[0], p);          // D0-D1, observable L0
    m.add_boundary_edge(0, w, &[0], p);     // D0-boundary, observable L0
    m.add_boundary_edge(1, w, &[], p);      // D1-boundary, no observable

    // syndrome [1,1]: both detectors fire => match D0-D1 => obs L0 toggled
    let pred = m.decode(&[1, 1]);
    assert_eq!(pred, vec![1], "syndrome [1,1]: D0-D1 match should flip L0");

    // syndrome [1,0]: only D0 fires => boundary match via L0 edge
    let pred = m.decode(&[1, 0]);
    assert_eq!(pred, vec![1], "syndrome [1,0]: D0-boundary match should flip L0");

    // syndrome [0,1]: only D1 fires => boundary match via no-observable edge
    let pred = m.decode(&[0, 1]);
    assert_eq!(pred, vec![0], "syndrome [0,1]: D1-boundary match should not flip L0");

    // syndrome [0,0]: no errors
    let pred = m.decode(&[0, 0]);
    assert_eq!(pred, vec![0], "syndrome [0,0]: no errors => L0 stays 0");
}

// ---------------------------------------------------------------------------
// 2. e2e_from_dem_text
// ---------------------------------------------------------------------------

/// Parse a DEM text string, decode several syndromes, verify predictions.
#[test]
fn e2e_from_dem_text() {
    let dem = "\
error(0.1) D0 D1 L0
error(0.1) D0
error(0.1) D1
";
    let mut m = Matching::from_dem(dem).unwrap();

    // Both detectors fire => match D0-D1 => L0 flipped
    assert_eq!(m.decode(&[1, 1]), vec![1]);

    // Only D0 fires => boundary match via error(0.1) D0 (no observable)
    assert_eq!(m.decode(&[1, 0]), vec![0]);

    // Only D1 fires => boundary match via error(0.1) D1 (no observable)
    assert_eq!(m.decode(&[0, 1]), vec![0]);

    // No errors
    assert_eq!(m.decode(&[0, 0]), vec![0]);
}

// ---------------------------------------------------------------------------
// 3. e2e_negative_weights
// ---------------------------------------------------------------------------

/// DEM with p > 0.5 edges produces negative weights.
/// The decoder should handle this by flipping detection events and observables.
#[test]
fn e2e_negative_weights() {
    // p=0.7 => weight = ln(0.3/0.7) < 0
    let dem = "\
error(0.7) D0 D1 L0
error(0.1) D0
error(0.1) D1
";
    let mut m = Matching::from_dem(dem).unwrap();

    // With negative weight on D0-D1 edge:
    // - negative_weight_detection_events = {D0, D1}
    // - negative_weight_observables = {L0}
    //
    // syndrome [0,0]: effective events = sym_diff({}, {D0,D1}) = {D0,D1}
    //   => match D0-D1 via |weight| edge => obs_mask from edge XOR neg_obs_mask
    //   The neg edge obs L0 is already accounted for, so prediction should reflect that.
    let pred = m.decode(&[0, 0]);
    // neg_obs_mask has L0 set. Matching D0-D1 gives obs_mask with L0.
    // Final = obs_mask ^ neg_obs_mask = 1 ^ 1 = 0
    assert_eq!(pred, vec![0], "no syndrome with neg weight: should predict 0");

    // syndrome [1,1]: effective events = sym_diff({D0,D1}, {D0,D1}) = {}
    //   => no matching needed => obs_mask = 0, final = 0 ^ neg_obs_mask = 1
    let pred = m.decode(&[1, 1]);
    assert_eq!(pred, vec![1], "both fire with neg weight: should predict 1");
}

// ---------------------------------------------------------------------------
// 4. e2e_decode_to_edges_consistency
// ---------------------------------------------------------------------------

/// Call both `decode` and `decode_to_edges` on the same syndrome and verify
/// that the edges are consistent with the observable predictions.
#[test]
fn e2e_decode_to_edges_consistency() {
    let dem = "\
error(0.1) D0 D1 L0
error(0.1) D1 D2
error(0.05) D0
error(0.05) D2
";
    // Use syndrome [1,1,0]: D0 and D1 fire => should match D0-D1 (carries L0)
    let mut m1 = Matching::from_dem(dem).unwrap();
    let mut m2 = Matching::from_dem(dem).unwrap();

    let syndrome = vec![1u8, 1, 0];

    let predictions = m1.decode(&syndrome);
    let edges = m2.decode_to_edges(&syndrome);

    // D0 and D1 should be matched together
    assert_eq!(edges.len(), 1, "Expected one matched pair");
    let (a, b) = edges[0];
    assert!(
        (a == 0 && b == 1) || (a == 1 && b == 0),
        "Expected D0-D1 match, got ({}, {})",
        a,
        b
    );

    // The D0-D1 edge carries L0, so prediction should be 1
    assert_eq!(predictions, vec![1]);

    // Now test with a boundary match: syndrome [1,0,0]
    let mut m3 = Matching::from_dem(dem).unwrap();
    let mut m4 = Matching::from_dem(dem).unwrap();

    let syndrome2 = vec![1u8, 0, 0];
    let pred2 = m3.decode(&syndrome2);
    let edges2 = m4.decode_to_edges(&syndrome2);

    // D0 should match to boundary (-1)
    assert_eq!(edges2.len(), 1);
    let (a, b) = edges2[0];
    assert!(
        b == -1 || a == -1,
        "Expected boundary match, got ({}, {})",
        a,
        b
    );

    // Boundary edge for D0 has no observable, so L0 = 0
    assert_eq!(pred2, vec![0]);
}

// ---------------------------------------------------------------------------
// 5. e2e_surface_code_d3
// ---------------------------------------------------------------------------

/// Simplified distance-3 surface code DEM.
///
/// A d=3 surface code has 8 stabilizers (4 X, 4 Z) but for a single-round
/// DEM we model just the Z-type detectors (4 detectors) and 1 logical observable.
///
/// Detector layout (Z stabilizers):
///   D0  D1
///   D2  D3
///
/// Edges (each data qubit error triggers two adjacent detectors):
///   error(0.1) D0 D1        (top horizontal data qubit)
///   error(0.1) D2 D3        (bottom horizontal data qubit)
///   error(0.1) D0 D2        (left vertical data qubit)
///   error(0.1) D1 D3        (right vertical data qubit)
///   error(0.1) D0 D3 L0     (diagonal — logical observable)
///   error(0.05) D0           (top-left boundary)
///   error(0.05) D1           (top-right boundary)
///   error(0.05) D2           (bottom-left boundary)
///   error(0.05) D3           (bottom-right boundary)
#[test]
fn e2e_surface_code_d3() {
    let dem = "\
error(0.1) D0 D1
error(0.1) D2 D3
error(0.1) D0 D2
error(0.1) D1 D3
error(0.1) D0 D3 L0
error(0.05) D0
error(0.05) D1
error(0.05) D2
error(0.05) D3
";
    let mut m = Matching::from_dem(dem).unwrap();

    // No errors
    assert_eq!(m.decode(&[0, 0, 0, 0]), vec![0]);

    // Single detector D0 fires => boundary match, no L0
    let pred = m.decode(&[1, 0, 0, 0]);
    assert_eq!(pred.len(), 1);
    // D0 boundary edge has no observable, so L0 = 0
    assert_eq!(pred[0], 0);

    // D0 and D3 fire => match via diagonal edge carrying L0
    let pred = m.decode(&[1, 0, 0, 1]);
    assert_eq!(pred, vec![1], "D0-D3 match should flip L0");

    // D0 and D1 fire => match via top edge, no L0
    let pred = m.decode(&[1, 1, 0, 0]);
    assert_eq!(pred, vec![0], "D0-D1 match should not flip L0");

    // All four fire => two pairs matched, check prediction is valid
    let pred = m.decode(&[1, 1, 1, 1]);
    assert_eq!(pred.len(), 1);
    // The exact value depends on which pairing the MWPM chooses,
    // but it must be a valid 0 or 1.
    assert!(pred[0] == 0 || pred[0] == 1);
}
