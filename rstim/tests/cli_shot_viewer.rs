use std::process::Command;

#[test]
fn shot_viewer_help_documents_loopback_controls() {
    let output = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args(["shot_viewer", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--no_open"));
    assert!(!stdout.contains("--serve-once"));
}
