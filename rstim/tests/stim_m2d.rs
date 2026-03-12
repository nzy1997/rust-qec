// Ported from Stim's measurements_to_detection_events.test.cc
// Tests: measurements_to_detection_events conversion.
// Avoids overlap with existing m2d.rs tests.

use rstim::data_path::ReferenceSampleMode;
use rstim::m2d::{measurements_to_detections, measurements_to_detections_with_options, M2dOptions};
use rstim::parser::parse_lines;
use rstim::sim::bit_table::BitTable;

fn single_shot_table(bits: usize, vals: &[bool]) -> BitTable {
    let mut t = BitTable::new(bits, 1);
    for (i, &v) in vals.iter().enumerate() {
        if v {
            t.set(i, 0, true);
        }
    }
    t
}

// --- single detector, matches false expectation ---
#[test]
fn m2d_single_detector_false_expectation() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert!(!out.detections.get(0, 0), "no detection expected");
}

// --- single detector, violates true expectation ---
#[test]
fn m2d_violates_true_expectation_inverted() {
    let instrs = parse_lines("M !0\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    // M !0 expects true, actual is false -> detection fires
    assert!(out.detections.get(0, 0), "detection should fire");
}

#[test]
fn m2d_violates_true_expectation_x() {
    let instrs = parse_lines("X 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    // Reference expects true (X flips), actual is false -> detection fires
    assert!(out.detections.get(0, 0));
}

// --- violates false expectation ---
#[test]
fn m2d_violates_false_expectation() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[true]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    // Reference expects false, actual is true -> detection fires
    assert!(out.detections.get(0, 0));
}

// --- matches true expectation ---
#[test]
fn m2d_matches_true_expectation_inverted() {
    let instrs = parse_lines("M !0\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[true]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    // M !0 expects true, actual is true -> no detection
    assert!(!out.detections.get(0, 0));
}

#[test]
fn m2d_matches_true_expectation_x() {
    let instrs = parse_lines("X 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[true]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    // X 0 means reference expects true, actual is true -> no detection
    assert!(!out.detections.get(0, 0));
}

// --- indexing: rec[-2] vs rec[-1] ---
#[test]
fn m2d_indexing_rec_minus_2() {
    // M 0 1, DETECTOR rec[-2] references measurement index 0
    let instrs = parse_lines("M 0 1\nDETECTOR rec[-2]\n").unwrap();
    let meas = single_shot_table(2, &[true, false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    // rec[-2] is M0 which is true, reference is false -> detection fires
    assert!(out.detections.get(0, 0));
}

#[test]
fn m2d_indexing_rec_minus_1() {
    let instrs = parse_lines("M 0 1\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(2, &[true, false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    // rec[-1] is M1 which is false, reference is false -> no detection
    assert!(!out.detections.get(0, 0));
}

// --- XOR of two recs ---
#[test]
fn m2d_xor_two_recs() {
    let instrs = parse_lines("M 0 1\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let meas = single_shot_table(2, &[true, false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    // XOR of actual: true ^ false = true. XOR of reference: false ^ false = false. -> detection fires.
    assert!(out.detections.get(0, 0));
}

// --- empty detector ---
#[test]
fn m2d_empty_detector() {
    let instrs = parse_lines("M 0 1\nDETECTOR\n").unwrap();
    let meas = single_shot_table(2, &[true, false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    // Empty detector: XOR of nothing = false. Reference also false -> no detection.
    assert!(!out.detections.get(0, 0));
}

// --- empty circuit ---
#[test]
fn m2d_empty_circuit() {
    let instrs = parse_lines("").unwrap();
    let meas = BitTable::new(0, 1);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert_eq!(out.detections.num_major(), 0);
}

// --- circuit with measurements but no detectors ---
#[test]
fn m2d_no_detectors() {
    let instrs = parse_lines("X 0\nM 0\n").unwrap();
    let meas = single_shot_table(1, &[true]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert_eq!(out.detections.num_major(), 0);
}

// --- observable flip ---
#[test]
fn m2d_observable_flip() {
    let instrs =
        parse_lines("M 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[true]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert!(out.observable_flips.get(0, 0), "observable should flip");
}

#[test]
fn m2d_observable_no_flip() {
    let instrs =
        parse_lines("M 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert!(!out.observable_flips.get(0, 0), "observable should not flip");
}

// --- multiple shots ---
#[test]
fn m2d_multiple_shots() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]\n").unwrap();
    let mut meas = BitTable::new(1, 4);
    meas.set(0, 0, false);
    meas.set(0, 1, true);
    meas.set(0, 2, false);
    meas.set(0, 3, true);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert!(!out.detections.get(0, 0));
    assert!(out.detections.get(0, 1));
    assert!(!out.detections.get(0, 2));
    assert!(out.detections.get(0, 3));
}

// --- repeat block detectors ---
#[test]
fn m2d_repeat_block() {
    let instrs = parse_lines(
        "M !0\nDETECTOR rec[-1]\nREPEAT 2 {\n    M !0\n    DETECTOR rec[-1]\n}\n",
    )
    .unwrap();
    // 3 measurements total (1 + 2 from repeat), all expected true (M !0).
    // Actual: all false -> all detections fire.
    let meas = single_shot_table(3, &[false, false, false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert_eq!(out.detections.num_major(), 3);
    assert!(out.detections.get(0, 0));
    assert!(out.detections.get(1, 0));
    assert!(out.detections.get(2, 0));
}

// --- detector with repeated measurement reference ---
#[test]
fn m2d_repeated_ref() {
    // REPEAT 50 { DETECTOR rec[-2]; DETECTOR rec[-1] } references the same M 0 !1
    let instrs = parse_lines(
        "M 0 !1\nREPEAT 50 {\n    DETECTOR rec[-2]\n    DETECTOR rec[-1]\n}\n",
    )
    .unwrap();
    let meas = single_shot_table(2, &[false, false]);
    let out = measurements_to_detections(&instrs, &meas).unwrap();
    assert_eq!(out.detections.num_major(), 100);
    // rec[-2] = M0 = false, reference false -> no detection for even detectors
    // rec[-1] = M1 = false, reference true (M !1) -> detection fires for odd detectors
    for d in 0..100 {
        if d % 2 == 0 {
            assert!(!out.detections.get(d, 0), "det {d} should not fire");
        } else {
            assert!(out.detections.get(d, 0), "det {d} should fire");
        }
    }
}

#[test]
fn m2d_skip_reference_sample_preserves_zero_reference_behavior() {
    let instrs = parse_lines("R 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let meas = single_shot_table(1, &[true]);
    let default_out = measurements_to_detections(&instrs, &meas).unwrap();
    let skipped_out = measurements_to_detections_with_options(
        &instrs,
        &meas,
        None,
        M2dOptions {
            reference_sample_mode: ReferenceSampleMode::AssumeAllZero,
            ran_without_feedback: false,
        },
    )
    .unwrap();
    assert_eq!(default_out.detections.get(0, 0), skipped_out.detections.get(0, 0));
}
