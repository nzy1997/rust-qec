use rstim::dem::DemTarget;
use rstim::dem_provenance::{
    SourceBranch, TrackedDemResult, TrackedErrorTerm, TrackedSource,
};
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;

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

#[test]
fn tracked_dem_records_repeat_iteration_target_slot_and_branch() {
    let circuit = parse_lines(
        "REPEAT 2 {\n  DEPOLARIZE1(0.3) 5 7\n  TICK\n}\nM 5 7\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();
    let source = tracked
        .sources
        .iter()
        .find(|source| {
            source.repeat_iterations == vec![1]
                && source.target_slots == vec![1]
                && source.branch.label() == "Y"
        })
        .unwrap();

    assert_eq!(source.op_path, vec![0, 0]);
    assert_eq!(source.target_qubits, vec![7]);
}

#[test]
fn tracked_dem_merge_keeps_exact_source_union() {
    let circuit = parse_lines(
        "R 0\nX_ERROR(0.1) 0\nX_ERROR(0.2) 0\nM 0\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();

    assert_eq!(tracked.dem.instructions().len(), 1);
    assert_eq!(tracked.dem_error_to_sources.len(), 1);
    assert_eq!(tracked.dem_error_to_sources[0].len(), 2);
    assert_eq!(tracked.source_to_dem_errors[0], vec![0]);
    assert_eq!(tracked.source_to_dem_errors[1], vec![0]);
}
