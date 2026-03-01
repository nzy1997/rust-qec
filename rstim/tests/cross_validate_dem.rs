use rstim::codegen::repetition_code_memory;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::ir::circuit_to_string;
use std::process::Command;

#[test]
#[ignore] // requires stim CLI: pip install stim
fn cross_validate_decomposed_dem_rep_code() {
    let circuit = repetition_code_memory(5, 3, 0.01);
    let circuit_text = circuit_to_string(&circuit);

    // Get Stim's DEM via CLI
    let mut child = Command::new("stim")
        .args(["analyze_errors", "--decompose_errors"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("stim CLI not found");

    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(circuit_text.as_bytes()).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "stim failed: {}", String::from_utf8_lossy(&output.stderr));
    let stim_dem_text = String::from_utf8(output.stdout).unwrap();

    // Get rstim's DEM
    let rstim_dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let rstim_dem_text = rstim_dem.to_string();

    // Both should be non-empty
    assert!(!stim_dem_text.is_empty(), "stim DEM empty");
    assert!(!rstim_dem_text.is_empty(), "rstim DEM empty");

    // Compare error count
    let stim_errors = stim_dem_text.lines().filter(|l| l.trim().starts_with("error")).count();
    let rstim_errors = rstim_dem_text.lines().filter(|l| l.trim().starts_with("error")).count();
    assert_eq!(stim_errors, rstim_errors,
        "error count mismatch:\nstim ({}):\n{}\nrstim ({}):\n{}",
        stim_errors, stim_dem_text, rstim_errors, rstim_dem_text);
}
