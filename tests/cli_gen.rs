use std::process::Command;

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

#[test]
fn gen_repetition_code() {
    let output = rstim_cmd()
        .args(["gen", "--code", "repetition_code", "--task", "memory",
               "--distance", "3", "--rounds", "2",
               "--after_clifford_depolarization", "0.001"])
        .output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("R "));
    assert!(s.contains("CX "));
    assert!(s.contains("M "));
    assert!(s.contains("DETECTOR"));
    assert!(s.contains("OBSERVABLE_INCLUDE"));
}

#[test]
fn gen_noiseless() {
    let output = rstim_cmd()
        .args(["gen", "--code", "repetition_code", "--task", "memory",
               "--distance", "3", "--rounds", "1"])
        .output().unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(!s.contains("DEPOLARIZE"));
}

#[test]
fn gen_unknown_code_fails() {
    let output = rstim_cmd()
        .args(["gen", "--code", "unknown", "--task", "memory",
               "--distance", "3", "--rounds", "1"])
        .output().unwrap();
    assert!(!output.status.success());
}
