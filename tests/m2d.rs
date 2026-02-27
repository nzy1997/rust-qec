use rstim::parser::parse_lines;
use rstim::m2d::measurements_to_detections;
use rstim::sim::bit_table::BitTable;

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
