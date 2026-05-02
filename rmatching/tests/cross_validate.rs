#[test]
#[ignore = "requires pymatching and stim Python packages"]
fn cross_validate_rep_code_d5() {
    let status = std::process::Command::new("python3")
        .arg("tests/cross_validate.py")
        .status()
        .expect("failed to run python3 tests/cross_validate.py");
    assert!(status.success(), "cross_validate.py reported mismatches");
}
