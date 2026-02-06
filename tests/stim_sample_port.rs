mod stim_sample_helpers;
use stim_sample_helpers::sample_lines;

#[test]
fn sample_flag_single_shot() {
    let out = sample_lines("M 0\n", 1, None);
    assert_eq!(out.trim(), "0");
}
