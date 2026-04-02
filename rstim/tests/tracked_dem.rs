use rstim::dem::DemTarget;
use rstim::dem_provenance::{
    SourceBranch, TrackedDemResult, TrackedErrorTerm, TrackedSource,
};

#[test]
fn tracked_result_builds_reverse_indices() {
    let sources = vec![
        TrackedSource {
            source_id: 0,
            op_path: vec![3, 1],
            repeat_iterations: vec![2],
            instr_name: "DEPOLARIZE1".to_string(),
            target_slots: vec![0],
            target_qubits: vec![5],
            branch: SourceBranch::Y,
            probability_fragment: 0.125,
        },
        TrackedSource {
            source_id: 1,
            op_path: vec![3, 1],
            repeat_iterations: vec![2],
            instr_name: "DEPOLARIZE1".to_string(),
            target_slots: vec![1],
            target_qubits: vec![7],
            branch: SourceBranch::X,
            probability_fragment: 0.125,
        },
    ];
    let dem_terms = vec![
        TrackedErrorTerm {
            probability: 0.2,
            targets: vec![DemTarget::Detector(0)],
            source_ids: vec![0],
        },
        TrackedErrorTerm {
            probability: 0.3,
            targets: vec![DemTarget::Detector(1)],
            source_ids: vec![0, 1],
        },
    ];

    let result = TrackedDemResult::from_terms_and_sources(sources, dem_terms);

    assert_eq!(result.dem_error_to_sources[0], vec![0]);
    assert_eq!(result.dem_error_to_sources[1], vec![0, 1]);
    assert_eq!(result.source_to_dem_errors[0], vec![0, 1]);
    assert_eq!(result.source_to_dem_errors[1], vec![1]);
}
