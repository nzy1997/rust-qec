use rstim::dem::{DemTarget, DetectorErrorModel};
use rstim::ir::{PauliBasis, StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::showcase::{
    dem_semantic_summary, median_duration_ns, render_markdown_table, showcase_cases,
    strip_comment_preamble, structural_circuit_summary,
};
use std::time::Duration;

#[test]
fn showcase_cases_cover_expected_matrix() {
    let labels: Vec<String> = showcase_cases().into_iter().map(|c| c.label()).collect();
    assert_eq!(labels.len(), 6);
    assert!(labels.contains(&"repetition_code/memory d=5 r=5".to_string()));
    assert!(labels.contains(&"repetition_code/memory d=13 r=13".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_x d=5 r=5".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_x d=13 r=13".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_z d=5 r=5".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_z d=13 r=13".to_string()));
}

#[test]
fn strip_comment_preamble_drops_leading_stim_header_only() {
    let text = "# header\n# header\nR 0\n# inline stays comment to parser\nM 0\n";
    assert_eq!(
        strip_comment_preamble(text),
        "R 0\n# inline stays comment to parser\nM 0\n"
    );
}

#[test]
fn structural_circuit_summary_counts_repeat_and_annotations() {
    let instrs = parse_lines(
        "QUBIT_COORDS(1, 2) 0\nR 0\nREPEAT 2 {\n    M 0\n    DETECTOR(1, 0) rec[-1]\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let summary = structural_circuit_summary(&instrs);
    assert_eq!(summary.measurements, 2);
    assert_eq!(summary.detectors, 2);
    assert_eq!(summary.observables, 1);
    assert_eq!(summary.opcode_counts["M"], 2);
    assert!(summary.qubit_coords.contains("QUBIT_COORDS(1,2) 0"));
}

#[test]
fn dem_semantic_summary_flattens_repeat_blocks_and_shifted_detectors() {
    let dem = DetectorErrorModel::parse(
        "error(0.125) D0\nrepeat 2 {\n    error(0.25) D0 D1\n    shift_detectors 2\n    detector(5, 0) D0\n}\n",
    )
    .unwrap();
    let summary = dem_semantic_summary(&dem);
    assert!(summary.error_probabilities.contains_key("D0"));
    assert!(summary.error_probabilities.contains_key("D0 D1"));
    assert!(summary
        .annotation_lines
        .iter()
        .any(|line| line.starts_with("detector(5,0) D2")));
}

#[test]
fn median_duration_ns_picks_middle_value() {
    let values = vec![
        Duration::from_millis(30),
        Duration::from_millis(10),
        Duration::from_millis(20),
    ];
    assert_eq!(median_duration_ns(&values), 20_000_000);
}

#[test]
fn render_markdown_table_includes_expected_headers() {
    let table = render_markdown_table(&[vec![
        "repetition_code/memory d=5 r=5".to_string(),
        "exact".to_string(),
        "match".to_string(),
        "0".to_string(),
        "1.0".to_string(),
        "1.1".to_string(),
        "2.0".to_string(),
        "2.4".to_string(),
        "1.10x".to_string(),
        "1.20x".to_string(),
    ]]);
    assert!(table.contains("| Case | Gen | DEM |"));
    assert!(table.contains("repetition_code/memory d=5 r=5"));
}

#[test]
fn structural_circuit_summary_formats_special_targets_and_normalizes_units() {
    let instrs = vec![
        StimInstr::new("QUBIT_COORDS", vec![1.5, 2.0], vec![StimTarget::Qubit(3)]),
        StimInstr::new("MR", vec![], vec![StimTarget::Qubit(0), StimTarget::Qubit(1)]),
        StimInstr::new("MRX", vec![], vec![StimTarget::Qubit(2)]),
        StimInstr::new("MRY", vec![], vec![StimTarget::Qubit(3)]),
        StimInstr::new("MRZ", vec![], vec![StimTarget::Qubit(4)]),
        StimInstr::new(
            "SWAP",
            vec![],
            vec![
                StimTarget::Qubit(0),
                StimTarget::Qubit(1),
                StimTarget::Qubit(2),
                StimTarget::Qubit(3),
            ],
        ),
        StimInstr::new(
            "MPP",
            vec![],
            vec![
                StimTarget::pauli(0, PauliBasis::X, false),
                StimTarget::Combiner,
                StimTarget::pauli(1, PauliBasis::Y, true),
                StimTarget::pauli(2, PauliBasis::Z, false),
            ],
        ),
        StimInstr::new("S", vec![], vec![]),
        StimInstr::new(
            "OBSERVABLE_INCLUDE",
            vec![2.0],
            vec![
                StimTarget::QubitInv(9),
                StimTarget::Rec(-2),
                StimTarget::pauli(4, PauliBasis::Y, true),
                StimTarget::pauli(5, PauliBasis::X, false),
                StimTarget::pauli(6, PauliBasis::Z, false),
                StimTarget::Combiner,
                StimTarget::Sweep(7),
            ],
        ),
    ];

    let summary = structural_circuit_summary(&instrs);

    assert!(summary.qubit_coords.contains("QUBIT_COORDS(1.5,2) 3"));
    assert_eq!(summary.opcode_counts["M"], 2);
    assert_eq!(summary.opcode_counts["R"], 3);
    assert_eq!(summary.opcode_counts["MX"], 1);
    assert_eq!(summary.opcode_counts["RX"], 1);
    assert_eq!(summary.opcode_counts["MY"], 1);
    assert_eq!(summary.opcode_counts["RY"], 1);
    assert_eq!(summary.opcode_counts["MZ"], 1);
    assert_eq!(summary.opcode_counts["SWAP"], 2);
    assert_eq!(summary.opcode_counts["MPP"], 2);
    assert_eq!(summary.opcode_counts["S"], 1);
    assert_eq!(summary.observable_target_arities[&1], 1);
    assert!(summary
        .observable_includes
        .contains("OBSERVABLE_INCLUDE(2) !9 rec[-2] !Y4 X5 Z6 * sweep[7]"));
}

#[test]
fn dem_semantic_summary_tracks_repeats_shifts_and_observables() {
    let mut body = DetectorErrorModel::new();
    body.add_error(
        0.25,
        vec![
            DemTarget::Detector(0),
            DemTarget::Observable(2),
            DemTarget::Separator,
            DemTarget::Detector(1),
        ],
    );
    body.add_detector(0, vec![1.5, 0.0]);
    body.add_shift_detectors(3, vec![0.0, 0.5]);

    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.125, vec![DemTarget::Detector(0)]);
    dem.add_observable(4);
    dem.add_shift_detectors(2, vec![1.0, 2.5]);
    dem.add_repeat(2, body);

    let summary = dem_semantic_summary(&dem);

    assert_eq!(summary.error_probabilities["D0"], 0.125);
    assert_eq!(summary.error_probabilities["D2 L2 ^ D3"], 0.25);
    assert_eq!(summary.error_probabilities["D5 L2 ^ D6"], 0.25);
    assert!(summary
        .annotation_lines
        .contains(&"logical_observable L4".to_string()));
    assert!(summary
        .annotation_lines
        .contains(&"shift_detectors(1,2.5) 2".to_string()));
    assert!(summary
        .annotation_lines
        .contains(&"shift_detectors(0,0.5) 3".to_string()));
    assert!(summary
        .annotation_lines
        .contains(&"detector(1.5,0) D2".to_string()));
    assert!(summary
        .annotation_lines
        .contains(&"detector(1.5,0) D5".to_string()));
}
