use rstim::parser::parse_lines;
use rstim::m2d::measurements_to_detections;
use rstim::sim::bit_table::BitTable;
use rstim::cli::run_m2d;

fn single_shot_table(bits: usize, vals: &[bool]) -> BitTable {
    let mut t = BitTable::new(bits, 1);
    for (i, &v) in vals.iter().enumerate() {
        if v { t.set(i, 0, true); }
    }
    t
}

#[test]
fn m2d_no_errors_no_detectors_fire() {
    // R 0 1; CX 0 1; M 0 1; DETECTOR rec[-2] rec[-1]
    // Reference: M0=0, M1=0 (CX with control=0 -> no flip)
    // Actual: same -> detector = 0 XOR 0 = 0
    let circuit = "R 0 1\nCX 0 1\nM 0 1\nDETECTOR rec[-2] rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let meas = single_shot_table(2, &[false, false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert_eq!(out.detections.num_major(), 1);
    assert!(!out.detections.get(0, 0));
}

#[test]
fn m2d_error_fires_detector() {
    // Same circuit, but M0 is flipped (error)
    // Reference: M0=0, M1=0. Actual: M0=1, M1=0 -> detector = 1 XOR 0 = 1
    let circuit = "R 0 1\nCX 0 1\nM 0 1\nDETECTOR rec[-2] rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let meas = single_shot_table(2, &[true, false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert!(out.detections.get(0, 0));
}

#[test]
fn m2d_observable_flip() {
    // M 0; OBSERVABLE_INCLUDE(0) rec[-1]
    // Reference: M0=0. Actual: M0=1 -> observable flip
    let circuit = "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let meas = single_shot_table(1, &[true]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert_eq!(out.observable_flips.num_major(), 1);
    assert!(out.observable_flips.get(0, 0));
}

#[test]
fn m2d_meas_count_mismatch() {
    let circuit = "R 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let meas = BitTable::new(5, 1); // 5 bits but circuit has 1 measurement
    let result = measurements_to_detections(&instrs, &meas);
    assert!(result.as_ref().is_err_and(|e| e.contains("5 bits but circuit has 1")));
}

#[test]
fn m2d_with_repeat_block() {
    // REPEAT block containing measurements and detectors
    let circuit = "R 0\nREPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n}";
    let instrs = parse_lines(circuit).unwrap();
    // 2 measurements total, both = 0 (matching reference)
    let meas = single_shot_table(2, &[false, false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert_eq!(out.detections.num_major(), 2);
    assert!(!out.detections.get(0, 0));
    assert!(!out.detections.get(1, 0));
}

#[test]
fn m2d_multiple_shots() {
    let circuit = "R 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let mut meas = BitTable::new(1, 3);
    meas.set(0, 1, true); // shot 1 has flipped measurement
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert!(!out.detections.get(0, 0));
    assert!(out.detections.get(0, 1));
    assert!(!out.detections.get(0, 2));
}

#[test]
fn run_m2d_01_to_01() {
    let circuit = "R 0\nM 0\nDETECTOR rec[-1]";
    let data = b"1\n"; // 1 meas, 1 shot, bit=1 (flipped)
    let mut out = Vec::new();
    run_m2d(circuit, data, "01", "01", None, false, &mut out).unwrap();
    assert_eq!(out, b"1\n"); // detector fires
}

#[test]
fn run_m2d_b8_input() {
    let circuit = "R 0\nM 0\nDETECTOR rec[-1]";
    let data = vec![1u8]; // b8: 1 bit set
    let mut out = Vec::new();
    run_m2d(circuit, &data, "b8", "01", None, false, &mut out).unwrap();
    assert_eq!(out, b"1\n");
}

#[test]
fn run_m2d_dets_output() {
    let circuit = "R 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]";
    let data = b"1\n";
    let mut out = Vec::new();
    run_m2d(circuit, data, "01", "dets", None, false, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("D0"));
}

#[test]
fn run_m2d_append_observables() {
    let circuit = "R 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]";
    let data = b"1\n";
    let mut out = Vec::new();
    run_m2d(circuit, data, "01", "01", None, true, &mut out).unwrap();
    // 1 detector + 1 observable = 2 bits per shot
    assert_eq!(out, b"11\n");
}

#[test]
fn run_m2d_unknown_format_errors() {
    let circuit = "R 0\nM 0\nDETECTOR rec[-1]";
    let mut out = Vec::new();
    assert!(run_m2d(circuit, b"", "xyz", "01", None, false, &mut out).is_err());
}
