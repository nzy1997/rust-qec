// Ported from Stim's detector_error_model.test.cc
// Tests: parse/display round-trips, counting detectors/observables, DEM manipulation.
// Avoids overlap with existing dem_format.rs and dem_ir.rs.

use rstim::dem::{DemTarget, DetectorErrorModel};
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;

// --- round_trip_str ---
#[test]
fn dem_round_trip_str() {
    let input = "\
error(0.125) D0
repeat 100 {
    repeat 200 {
        error(0.25) D0 D1 L0 ^ D2
        shift_detectors(1.5, 3) 10
        detector(0.5) D0
    }
    error(0.375) D0 D1
    shift_detectors 20
}
";
    let dem = DetectorErrorModel::parse(input).unwrap();
    let output = dem.to_string();
    let dem2 = DetectorErrorModel::parse(&output).unwrap();
    assert_eq!(dem, dem2, "round-trip should preserve DEM");
}

// --- parse empty ---
#[test]
fn dem_parse_empty() {
    let dem = DetectorErrorModel::parse("").unwrap();
    assert_eq!(dem.instructions().len(), 0);
    assert_eq!(dem.num_detectors(), 0);
    assert_eq!(dem.num_observables(), 0);
}

// --- count_detectors ---
#[test]
fn dem_count_detectors_simple() {
    let dem = DetectorErrorModel::parse("error(0.3) D2 L1000\n").unwrap();
    assert!(dem.num_detectors() >= 3, "D2 implies at least 3 detectors");
}

#[test]
fn dem_count_detectors_with_shift() {
    let dem = DetectorErrorModel::parse("shift_detectors 5\ndetector(0) D3\n").unwrap();
    // After shift of 5, D3 means detector index 8, so at least 9 detectors.
    assert!(dem.num_detectors() >= 4, "detectors={}", dem.num_detectors());
}

// --- count_observables ---
#[test]
fn dem_count_observables_empty() {
    let dem = DetectorErrorModel::parse("").unwrap();
    assert_eq!(dem.num_observables(), 0);
}

#[test]
fn dem_count_observables_in_error() {
    let dem = DetectorErrorModel::parse("error(0.3) L2 D9999\n").unwrap();
    assert!(dem.num_observables() >= 3, "L2 implies at least 3 observables");
}

#[test]
fn dem_count_observables_with_logical() {
    let dem =
        DetectorErrorModel::parse("shift_detectors 5\nerror(0.01) L3\n").unwrap();
    assert!(dem.num_observables() >= 4, "L3 implies at least 4 observables");
}

// --- DEM from circuit (circuit_to_dem) ---
#[test]
fn dem_from_simple_circuit() {
    let instrs = parse_lines(
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n",
    )
    .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert!(dem.num_detectors() >= 1);
    let text = dem.to_string();
    assert!(text.contains("error"), "DEM should contain error instructions: {text}");
}

#[test]
fn dem_from_bell_circuit() {
    let instrs = parse_lines(
        "R 0 1\nH 0\nCNOT 0 1\nDEPOLARIZE2(0.01) 0 1\nM 0 1\n\
         DETECTOR rec[-1] rec[-2]\n",
    )
    .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert!(dem.num_detectors() >= 1);
}

// --- DEM from rep code circuit ---
#[test]
fn dem_from_rep_code() {
    let instrs = parse_lines(
        "R 0 1 2\nTICK\nCNOT 0 1\nCNOT 2 1\nTICK\n\
         DEPOLARIZE2(0.01) 0 1 2 1\n\
         M 1\nR 1\nDETECTOR rec[-1]\n\
         REPEAT 2 {\n    TICK\n    CNOT 0 1\n    CNOT 2 1\n    TICK\n\
         DEPOLARIZE2(0.01) 0 1 2 1\n    M 1\n    R 1\n    DETECTOR rec[-1] rec[-2]\n}\n\
         M 0 2\nOBSERVABLE_INCLUDE(0) rec[-1] rec[-2]\n",
    )
    .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let text = dem.to_string();
    assert!(dem.num_detectors() >= 3, "rep code should have 3+ detectors");
    assert!(dem.num_observables() >= 1, "should have 1+ observable");
    assert!(text.contains("error"), "should contain errors: {text}");
}

// --- DEM equality ---
#[test]
fn dem_equality() {
    let mut a = DetectorErrorModel::new();
    let mut b = DetectorErrorModel::new();
    assert_eq!(a, b);

    a.add_error(0.1, vec![DemTarget::Detector(0)]);
    assert_ne!(a, b);

    b.add_error(0.1, vec![DemTarget::Detector(0)]);
    assert_eq!(a, b);
}

// --- DEM repeat block ---
#[test]
fn dem_repeat_block_construct() {
    let mut body = DetectorErrorModel::new();
    body.add_error(0.01, vec![DemTarget::Detector(0), DemTarget::Detector(1)]);
    body.add_shift_detectors(1, vec![]);

    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(100, body);
    let text = dem.to_string();
    assert!(text.contains("repeat 100"), "text={text}");
    assert!(text.contains("error(0.01)"), "text={text}");
    assert!(text.contains("shift_detectors"), "text={text}");
}

// --- DEM with separator targets ---
#[test]
fn dem_error_with_separator() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(
        0.25,
        vec![
            DemTarget::Detector(0),
            DemTarget::Detector(1),
            DemTarget::Observable(0),
            DemTarget::Separator,
            DemTarget::Detector(2),
        ],
    );
    let text = dem.to_string();
    assert!(text.contains("^"), "separator should show as ^: {text}");
    // Round-trip
    let dem2 = DetectorErrorModel::parse(&text).unwrap();
    assert_eq!(dem, dem2);
}

// --- Parse with comments and whitespace ---
#[test]
fn dem_parse_comments() {
    let input = "# comment\n\nerror(0.1) D0\n# another comment\n";
    let dem = DetectorErrorModel::parse(input).unwrap();
    assert_eq!(dem.instructions().len(), 1);
}

// --- Parse nested repeat ---
#[test]
fn dem_parse_nested_repeat() {
    let input = "\
repeat 5 {
    repeat 3 {
        error(0.01) D0 D1
        shift_detectors 2
    }
    error(0.02) D0
    shift_detectors 1
}
";
    let dem = DetectorErrorModel::parse(input).unwrap();
    let output = dem.to_string();
    let dem2 = DetectorErrorModel::parse(&output).unwrap();
    assert_eq!(dem, dem2, "nested repeat round-trip");
}

// --- DEM from circuit with observable ---
#[test]
fn dem_observable_from_circuit() {
    let instrs = parse_lines(
        "R 0\nX_ERROR(0.01) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert!(dem.num_observables() >= 1);
}
