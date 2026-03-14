use std::process::Command;

use rstim::parser::parse_lines;
use rstim::showcase::{showcase_cases, strip_comment_preamble, structural_circuit_summary};

fn stim_cmd() -> String {
    std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string())
}

fn run_capture(cmd: &str, args: &[String]) -> String {
    let output = Command::new(cmd).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {cmd} {args:?}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn showcase_gen_parity_matches_structurally() {
    for case in showcase_cases() {
        let args = vec![
            "gen".to_string(),
            "--code".to_string(),
            case.code.to_string(),
            "--task".to_string(),
            case.task.to_string(),
            "--distance".to_string(),
            case.distance.to_string(),
            "--rounds".to_string(),
            case.rounds.to_string(),
        ];
        let stim_text = run_capture(&stim_cmd(), &args);
        let rstim_text = run_capture(env!("CARGO_BIN_EXE_rstim"), &args);

        let stim_norm = strip_comment_preamble(&stim_text);
        let stim_instrs = parse_lines(stim_norm).unwrap();
        let rstim_instrs = parse_lines(&rstim_text).unwrap();
        assert_eq!(
            structural_circuit_summary(&stim_instrs),
            structural_circuit_summary(&rstim_instrs),
            "gen mismatch for {}",
            case.label(),
        );
    }
}
