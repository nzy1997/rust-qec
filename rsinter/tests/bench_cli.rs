use std::process::Command;

#[test]
fn rsinter_cli_help_mentions_bench_subcommands() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .arg("--help")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("bench"));
    assert!(stdout.contains("run"));
    assert!(stdout.contains("merge"));
    assert!(stdout.contains("plot"));
}
