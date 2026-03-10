#![allow(unexpected_cfgs)]

use rstim::codegen::repetition_code_memory;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::ir::circuit_to_string;
use rstim::parser::parse_lines;
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

fn stim_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => "non-string panic payload".to_string(),
        },
    }
}

fn stim_analyze_errors(circuit_text: &str) -> String {
    let stim_cmd = std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string());
    let mut child = Command::new(stim_cmd)
        .args(["analyze_errors"])
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
    String::from_utf8(output.stdout).unwrap()
}

fn parse_dem_errors(dem_text: &str) -> BTreeMap<String, f64> {
    let mut errors = BTreeMap::new();
    for line in dem_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("error(") {
            if let Some(paren_end) = rest.find(')') {
                let prob: f64 = rest[..paren_end].parse().unwrap();
                let targets = rest[paren_end + 1..].trim().to_string();
                errors.insert(targets, prob);
            }
        }
    }
    errors
}

#[test]
fn parse_dem_errors_extracts_only_error_lines() {
    let dem_text = "\
error(0.125) D0 D1
detector(1, 2, 3) D0
shift_detectors(2) 0, 1
error(0.5) D2 L0
";
    let errors = parse_dem_errors(dem_text);
    assert_eq!(errors.len(), 2);
    assert_eq!(errors["D0 D1"], 0.125);
    assert_eq!(errors["D2 L0"], 0.5);
}

#[test]
fn stim_analyze_errors_respects_override_command() {
    let _guard = stim_env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let stim_path = dir.path().join("stim");
    fs::write(
        &stim_path,
        "#!/bin/sh\ncat >/dev/null\nprintf 'error(0.25) D0\\n'",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&stim_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&stim_path, perms).unwrap();
    }

    unsafe {
        std::env::set_var("RSTIM_TEST_STIM", &stim_path);
    }
    let output = stim_analyze_errors("M 0\nDETECTOR rec[-1]");
    unsafe {
        std::env::remove_var("RSTIM_TEST_STIM");
    }

    assert_eq!(output, "error(0.25) D0\n");
}

#[test]
fn stim_analyze_errors_propagates_failure_output() {
    let _guard = stim_env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let stim_path = dir.path().join("stim");
    fs::write(
        &stim_path,
        "#!/bin/sh\ncat >/dev/null\necho 'stim exploded' >&2\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&stim_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&stim_path, perms).unwrap();
    }

    unsafe {
        std::env::set_var("RSTIM_TEST_STIM", &stim_path);
    }
    let result = std::panic::catch_unwind(|| stim_analyze_errors("M 0\nDETECTOR rec[-1]"));
    unsafe {
        std::env::remove_var("RSTIM_TEST_STIM");
    }

    let panic_text = panic_message(result.unwrap_err());
    assert!(panic_text.contains("stim failed: stim exploded"));
}

#[test]
#[cfg(not(tarpaulin))]
#[ignore] // requires stim CLI: pip install stim
fn cross_validate_decomposed_dem_rep_code() {
    let circuit = repetition_code_memory(5, 3, 0.01);
    let circuit_text = circuit_to_string(&circuit);

    let stim_dem_text = stim_analyze_errors(&circuit_text);

    // Get rstim's decomposed DEM (via stim analyze_errors --decompose_errors in stim)
    let rstim_dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let rstim_dem_text = rstim_dem.to_string();

    assert!(!stim_dem_text.is_empty(), "stim DEM empty");
    assert!(!rstim_dem_text.is_empty(), "rstim DEM empty");

    let stim_errors = stim_dem_text.lines().filter(|l| l.trim().starts_with("error")).count();
    let rstim_errors = rstim_dem_text.lines().filter(|l| l.trim().starts_with("error")).count();
    assert_eq!(stim_errors, rstim_errors,
        "error count mismatch:\nstim ({}):\n{}\nrstim ({}):\n{}",
        stim_errors, stim_dem_text, rstim_errors, rstim_dem_text);
}

#[test]
#[cfg(not(tarpaulin))]
#[ignore] // requires stim CLI: pip install stim
fn cross_validate_surface_code_dem() {
    let circuit_text = std::fs::read_to_string("../drafts/surface_code_rotated_memory_x_5_0.01.stim")
        .expect("missing ../drafts/surface_code_rotated_memory_x_5_0.01.stim");
    let instrs = parse_lines(&circuit_text).unwrap();

    let stim_dem_text = stim_analyze_errors(&circuit_text);
    let rstim_dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let rstim_dem_text = rstim_dem.to_string();

    let stim_errors = parse_dem_errors(&stim_dem_text);
    let rstim_errors = parse_dem_errors(&rstim_dem_text);

    // Same number of error lines
    assert_eq!(stim_errors.len(), rstim_errors.len(),
        "error count mismatch: stim={} rstim={}", stim_errors.len(), rstim_errors.len());

    // Same target sets
    for key in stim_errors.keys() {
        assert!(rstim_errors.contains_key(key),
            "stim has error target '{}' not in rstim", key);
    }

    // Probabilities match within floating-point tolerance
    let mut max_rel = 0.0f64;
    for (key, stim_p) in &stim_errors {
        let rstim_p = rstim_errors[key];
        let rel = (stim_p - rstim_p).abs() / stim_p.max(1e-20);
        if rel > max_rel {
            max_rel = rel;
        }
        assert!(rel < 1e-12,
            "probability mismatch for '{}': stim={} rstim={} rel={}",
            key, stim_p, rstim_p, rel);
    }

    // Same detector annotations
    let stim_det_lines: Vec<&str> = stim_dem_text.lines()
        .filter(|l| l.starts_with("detector") || l.starts_with("shift_detectors"))
        .collect();
    let rstim_det_lines: Vec<&str> = rstim_dem_text.lines()
        .filter(|l| l.starts_with("detector") || l.starts_with("shift_detectors"))
        .collect();
    assert_eq!(stim_det_lines, rstim_det_lines,
        "detector annotations differ");
}

#[test]
#[cfg(not(tarpaulin))]
#[ignore] // requires stim CLI: pip install stim
fn cross_validate_rep_code_dem_probabilities() {
    let circuit = repetition_code_memory(5, 3, 0.01);
    let circuit_text = circuit_to_string(&circuit);

    let stim_dem_text = stim_analyze_errors(&circuit_text);
    let rstim_dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let rstim_dem_text = rstim_dem.to_string();

    let stim_errors = parse_dem_errors(&stim_dem_text);
    let rstim_errors = parse_dem_errors(&rstim_dem_text);

    assert_eq!(stim_errors.len(), rstim_errors.len(),
        "error count mismatch: stim={} rstim={}", stim_errors.len(), rstim_errors.len());

    for (key, stim_p) in &stim_errors {
        let rstim_p = rstim_errors.get(key).unwrap_or(&0.0);
        let rel = (stim_p - rstim_p).abs() / stim_p.max(1e-20);
        assert!(rel < 1e-12,
            "probability mismatch for '{}': stim={} rstim={}", key, stim_p, rstim_p);
    }
}
