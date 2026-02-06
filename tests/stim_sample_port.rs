mod stim_sample_helpers;
use stim_sample_helpers::{sample_lines, deviation};
use std::collections::HashMap;

#[test]
fn sample_flag_single_shot() {
    let out = sample_lines("M 0\n", 1, None);
    assert_eq!(out.trim(), "0");
}

#[test]
fn sample_flag_x_then_m() {
    let out = sample_lines("X 0\nM 0\n", 1, None);
    assert_eq!(out.trim(), "1");
}

#[test]
fn sample_flag_inverted_target() {
    let out = sample_lines("M !0\n", 1, None);
    assert_eq!(out.trim(), "1");
}

#[test]
fn basic_distributions_bell() {
    let mut expected: HashMap<&str, f32> = HashMap::new();
    expected.insert("00", 0.5);
    expected.insert("11", 0.5);
    let out = sample_lines("H 0\nCNOT 0 1\nM 0 1\n", 10_000, None);
    assert_eq!(deviation(&out, &expected), "");
}

#[test]
fn sample_x_error_distribution() {
    let mut expected: HashMap<&str, f32> = HashMap::new();
    expected.insert("00", 0.9 * 0.9);
    expected.insert("01", 0.9 * 0.1);
    expected.insert("10", 0.9 * 0.1);
    expected.insert("11", 0.1 * 0.1);
    let out = sample_lines("X_ERROR(0.1) 0 1\nM 0 1\n", 100_000, None);
    assert_eq!(deviation(&out, &expected), "");
}

#[test]
fn sample_z_error_distribution() {
    let mut expected: HashMap<&str, f32> = HashMap::new();
    expected.insert("00", 0.9 * 0.9);
    expected.insert("01", 0.9 * 0.1);
    expected.insert("10", 0.9 * 0.1);
    expected.insert("11", 0.1 * 0.1);
    let out = sample_lines("H 0 1\nZ_ERROR(0.1) 0 1\nH 0 1\nM 0 1\n", 100_000, None);
    assert_eq!(deviation(&out, &expected), "");
}

#[test]
fn sample_depolarize1_distribution() {
    let mut expected: HashMap<&str, f32> = HashMap::new();
    expected.insert("00", 0.8 * 0.8);
    expected.insert("01", 0.8 * 0.2);
    expected.insert("10", 0.8 * 0.2);
    expected.insert("11", 0.2 * 0.2);
    let out = sample_lines("DEPOLARIZE1(0.3) 0 1\nM 0 1\n", 100_000, None);
    assert_eq!(deviation(&out, &expected), "");
}

#[test]
fn sample_depolarize2_distribution() {
    let mut expected: HashMap<&str, f32> = HashMap::new();
    expected.insert("00", 0.1 * 3.0 / 15.0 + 0.9);
    expected.insert("01", 0.1 * 4.0 / 15.0);
    expected.insert("10", 0.1 * 4.0 / 15.0);
    expected.insert("11", 0.1 * 4.0 / 15.0);
    let out = sample_lines("DEPOLARIZE2(0.1) 0 1\nM 0 1\n", 100_000, None);
    assert_eq!(deviation(&out, &expected), "");
}
