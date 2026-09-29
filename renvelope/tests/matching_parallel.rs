use renvelope::{
    EdgeKind, EnvelopeMatchingCase, EnvelopeMatchingEdge, EnvelopeMatchingShot, decode_matching,
};

fn edge(id: &str, a: usize, b: usize, probability: f64, logical: bool) -> EnvelopeMatchingEdge {
    EnvelopeMatchingEdge {
        id: id.to_string(),
        node1: a,
        node2: Some(b),
        observable_indices: if logical { vec![0] } else { vec![] },
        weight: ((1.0 - probability) / probability).ln(),
        kind: EdgeKind::SpaceLike,
        independent_mechanism: true,
    }
}

#[test]
fn independent_parallel_dem_mechanisms_change_the_logical_prediction() {
    let mut case = EnvelopeMatchingCase {
        schema_version: "atom-loss-envelope-matching.v0".to_string(),
        num_detectors: 3,
        num_observables: 1,
        edges: vec![
            edge("direct-0.1", 0, 1, 0.1, true),
            edge("direct-0.2", 0, 1, 0.2, true),
            edge("via-0", 0, 2, 0.35434369377420455, false),
            edge("via-1", 2, 1, 0.35434369377420455, false),
        ],
        loss_edge_map: vec![],
        shots: vec![EnvelopeMatchingShot {
            observed_detectors: vec![0, 1],
            observed_losses: vec![],
        }],
    };
    assert_eq!(decode_matching(&case).unwrap().predictions, vec![1]);
    case.edges.remove(0);
    assert_eq!(decode_matching(&case).unwrap().predictions, vec![0]);
    case.edges[0].weight = ((1.0_f64 - 0.26) / 0.26).ln();
    assert_eq!(decode_matching(&case).unwrap().predictions, vec![1]);
}
