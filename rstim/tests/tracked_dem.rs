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
fn source_branch_labels_match_expected_markers() {
    let cases = [
        (SourceBranch::X, "X"),
        (SourceBranch::Y, "Y"),
        (SourceBranch::Z, "Z"),
        (SourceBranch::XX, "XX"),
        (SourceBranch::XY, "XY"),
        (SourceBranch::XZ, "XZ"),
        (SourceBranch::YX, "YX"),
        (SourceBranch::YY, "YY"),
        (SourceBranch::YZ, "YZ"),
        (SourceBranch::ZX, "ZX"),
        (SourceBranch::ZY, "ZY"),
        (SourceBranch::ZZ, "ZZ"),
        (SourceBranch::MeasurementFlip, "M"),
        (SourceBranch::CorrelatedBranch { index: 7 }, "E7"),
        (
            SourceBranch::Custom {
                label: "custom".to_string(),
            },
            "custom",
        ),
    ];

    for (branch, expected) in cases {
        assert_eq!(branch.label(), expected);
    }
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

#[test]
fn tracked_dem_decomposition_keeps_reverse_links() {
    let circuit = parse_lines(
        "R 0 1 2\nX_ERROR(0.1) 0\nX_ERROR(0.1) 1\nCX 0 1\nCX 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let tracked = ErrorAnalyzer::circuit_to_tracked_dem_decomposed(&circuit).unwrap();

    let dem_ids = &tracked.source_to_dem_errors[0];
    assert!(!dem_ids.is_empty());
    for &dem_id in dem_ids {
        assert!(tracked.dem_error_to_sources[dem_id].contains(&0));
    }
}

#[test]
fn tracked_dem_decomposition_propagates_failure() {
    let circuit = parse_lines(
        "R 0 1 2\nX_ERROR(0.1) 0\nCX 0 1\nCX 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let err = ErrorAnalyzer::circuit_to_tracked_dem_decomposed(&circuit).unwrap_err();

    assert!(err.contains("failed to decompose non-graphlike error"));
}

#[test]
fn tracked_dem_matches_plain_dem_for_depolarize1() {
    let circuit = parse_lines("R 0\nDEPOLARIZE1(0.3) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();

    let plain = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();

    assert_eq!(tracked.dem.to_string(), plain.to_string());
}

#[test]
fn tracked_dem_matches_plain_dem_for_supported_single_qubit_pauli_errors() {
    let cases = [
        ("X_ERROR", "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n"),
        ("Y_ERROR", "R 0\nY_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n"),
        ("Z_ERROR", "H 0\nZ_ERROR(0.1) 0\nH 0\nM 0\nDETECTOR rec[-1]\n"),
    ];

    for (name, text) in cases {
        let circuit = parse_lines(text).unwrap();
        let plain = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
        let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();

        assert_eq!(tracked.dem.to_string(), plain.to_string(), "{name}");
    }
}

#[test]
fn tracked_dem_matches_plain_dem_for_supported_measurement_noise_ops() {
    let cases = [
        ("M", "R 0\nM(0.125) 0\nDETECTOR rec[-1]\n"),
        ("MZ", "R 0\nMZ(0.125) 0\nDETECTOR rec[-1]\n"),
        ("MX", "RX 0\nMX(0.125) 0\nDETECTOR rec[-1]\n"),
        ("MY", "RY 0\nMY(0.125) 0\nDETECTOR rec[-1]\n"),
        ("MR", "R 0\nMR(0.125) 0\nDETECTOR rec[-1]\n"),
        ("MRZ", "R 0\nMRZ(0.125) 0\nDETECTOR rec[-1]\n"),
        ("MRX", "RX 0\nMRX(0.125) 0\nDETECTOR rec[-1]\n"),
        ("MRY", "RY 0\nMRY(0.125) 0\nDETECTOR rec[-1]\n"),
    ];

    for (name, text) in cases {
        let circuit = parse_lines(text).unwrap();
        let plain = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
        let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();

        assert_eq!(tracked.dem.to_string(), plain.to_string(), "{name}");
    }
}

#[test]
fn tracked_dem_decomposed_matches_plain_dem_decomposed() {
    let circuit = parse_lines(
        "R 0 1 2\nX_ERROR(0.1) 0\nX_ERROR(0.1) 1\nCX 0 1\nCX 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let plain = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let tracked = ErrorAnalyzer::circuit_to_tracked_dem_decomposed(&circuit).unwrap();

    assert_eq!(tracked.dem.to_string(), plain.to_string());
}
