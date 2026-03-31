#![allow(unexpected_cfgs)]

use rstim::codegen::{
    color_code::memory_xyz, repetition_code_memory, surface_code::rotated_memory_x,
};
use rstim::dem::DetectorErrorModel;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::ir::circuit_to_string;
use rstim::showcase::dem_semantic_summary;
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn stim_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_stim_env() -> MutexGuard<'static, ()> {
    stim_env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    stim_analyze_errors_flags(circuit_text, &[])
}

fn stim_python_generated_surface_code_circuit_text() -> String {
    let output = Command::new("python3")
        .arg("-c")
        .arg(
            "import stim; print(stim.Circuit.generated('surface_code:rotated_memory_z', rounds=9, distance=3, after_clifford_depolarization=0.001, after_reset_flip_probability=0.001, before_measure_flip_probability=0.001, before_round_data_depolarization=0.001))",
        )
        .output()
        .expect("failed to run python3");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "python3 stim generation failed: {stderr}"
    );
    String::from_utf8(output.stdout).unwrap()
}

fn stim_analyze_errors_flags(circuit_text: &str, flags: &[&str]) -> String {
    let stim_cmd = std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string());
    let mut child = Command::new(stim_cmd)
        .arg("analyze_errors")
        .args(flags)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("stim CLI not found");
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(circuit_text.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "stim failed: {stderr}");
    String::from_utf8(output.stdout).unwrap()
}

fn parse_dem_errors_multi(dem_text: &str) -> BTreeMap<String, Vec<f64>> {
    let mut errors = BTreeMap::new();
    for line in dem_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("error(") {
            if let Some(paren_end) = rest.find(')') {
                let prob: f64 = rest[..paren_end].parse().unwrap();
                let targets = canonicalize_error_targets(rest[paren_end + 1..].trim());
                errors.entry(targets).or_insert_with(Vec::new).push(prob);
            }
        }
    }
    for probs in errors.values_mut() {
        probs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    }
    errors
}

fn parse_dem_error_semantics(dem_text: &str) -> BTreeMap<String, f64> {
    let mut errors = BTreeMap::new();
    for line in dem_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("error(") {
            if let Some(paren_end) = rest.find(')') {
                let prob: f64 = rest[..paren_end].parse().unwrap();
                let targets = canonicalize_error_targets(rest[paren_end + 1..].trim());
                errors
                    .entry(targets)
                    .and_modify(|existing| {
                        *existing = *existing + prob - 2.0 * *existing * prob;
                    })
                    .or_insert(prob);
            }
        }
    }
    errors
}

fn parse_dem_errors(dem_text: &str) -> BTreeMap<String, f64> {
    let mut errors = BTreeMap::new();
    for line in dem_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("error(") {
            if let Some(paren_end) = rest.find(')') {
                let prob: f64 = rest[..paren_end].parse().unwrap();
                let targets = canonicalize_error_targets(rest[paren_end + 1..].trim());
                errors.insert(targets, prob);
            }
        }
    }
    errors
}

fn canonicalize_error_targets(targets: &str) -> String {
    let mut components: Vec<String> = targets
        .split('^')
        .map(|component| {
            let mut terms: Vec<&str> = component.split_whitespace().collect();
            terms.sort();
            terms.join(" ")
        })
        .filter(|component| !component.is_empty())
        .collect();
    components.sort();
    components.join(" ^ ")
}

fn strip_stim_comments(circuit_text: &str) -> String {
    circuit_text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_all_graphlike_dem_text(dem_text: &str) {
    for line in dem_text.lines() {
        let line = line.trim();
        if line.starts_with("error(") {
            let targets = line.split(')').nth(1).unwrap_or("").trim();
            for comp in targets.split('^') {
                let det_count = comp
                    .split_whitespace()
                    .filter(|term| term.starts_with('D'))
                    .count();
                assert!(det_count <= 2, "non-graphlike component in: {line}");
            }
        }
    }
}

fn assert_semantic_dem_parity(stim_dem_text: &str, rstim_dem_text: &str) {
    assert_prob_maps_close(
        &parse_dem_error_semantics(stim_dem_text),
        &parse_dem_error_semantics(rstim_dem_text),
        &format!("semantic error mismatch:\nstim:\n{stim_dem_text}\n\nrstim:\n{rstim_dem_text}"),
    );
    let stim_det_lines: Vec<&str> = stim_dem_text
        .lines()
        .filter(|line| line.starts_with("detector") || line.starts_with("shift_detectors"))
        .collect();
    let rstim_det_lines: Vec<&str> = rstim_dem_text
        .lines()
        .filter(|line| line.starts_with("detector") || line.starts_with("shift_detectors"))
        .collect();
    assert_eq!(
        stim_det_lines, rstim_det_lines,
        "detector annotations differ:\nstim:\n{stim_dem_text}\n\nrstim:\n{rstim_dem_text}"
    );
}

fn assert_repeat_aware_dem_parity(stim_dem_text: &str, rstim_dem_text: &str) {
    let stim_dem = DetectorErrorModel::parse(stim_dem_text).unwrap();
    let rstim_dem = DetectorErrorModel::parse(rstim_dem_text).unwrap();
    let stim_summary = dem_semantic_summary(&stim_dem);
    let rstim_summary = dem_semantic_summary(&rstim_dem);
    let context = format!(
        "repeat-aware semantic error mismatch:\nstim:\n{stim_dem_text}\n\nrstim:\n{rstim_dem_text}"
    );
    assert_prob_maps_close(
        &stim_summary.error_probabilities,
        &rstim_summary.error_probabilities,
        &context,
    );
    assert_eq!(
        stim_summary.annotation_lines, rstim_summary.annotation_lines,
        "{context}"
    );
}

fn assert_prob_maps_close(
    expected: &BTreeMap<String, f64>,
    actual: &BTreeMap<String, f64>,
    context: &str,
) {
    assert_eq!(expected.len(), actual.len(), "{context}");
    for (key, expected_prob) in expected {
        let actual_prob = actual.get(key).unwrap_or_else(|| panic!("{context}"));
        let scale = expected_prob.abs().max(actual_prob.abs()).max(1.0);
        let diff = (expected_prob - actual_prob).abs();
        assert!(diff <= 1e-12 * scale, "{context}");
    }
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
fn parse_dem_errors_multi_collects_duplicate_targets() {
    let dem_text = "\
error(0.125) D0 D1
detector(1, 2, 3) D0
error(0.5) D2 L0
error(0.25) D0 D1
";
    let errors = parse_dem_errors_multi(dem_text);
    assert_eq!(errors.len(), 2);
    assert_eq!(errors["D0 D1"], vec![0.125, 0.25]);
    assert_eq!(errors["D2 L0"], vec![0.5]);
}

#[test]
fn assert_all_graphlike_dem_text_accepts_separator_split_components() {
    assert_all_graphlike_dem_text("error(0.1) D0 D1 ^ D2\n");
}

#[test]
fn assert_semantic_dem_parity_accepts_odd_parity_merged_duplicates() {
    let stim_dem_text = "\
error(0.1) D0 ^ D1
error(0.2) D1 ^ D0
";
    let rstim_dem_text = "\
error(0.26) D0 ^ D1
";

    assert_semantic_dem_parity(stim_dem_text, rstim_dem_text);
}

#[test]
fn assert_repeat_aware_dem_parity_tolerates_float_roundoff() {
    let stim_dem_text = "error(0.1) D0\nrepeat 2 {\n    error(0.2) D0 D1\n    shift_detectors 2\n    detector(5, 0) D0\n}\n";
    let rstim_dem_text = "error(0.10000000000000002) D0\nrepeat 2 {\n    error(0.20000000000000004) D0 D1\n    shift_detectors 2\n    detector(5, 0) D0\n}\n";

    assert_repeat_aware_dem_parity(stim_dem_text, rstim_dem_text);
}

#[test]
fn raw_repeat_surface_code_analysis_matches_explicitly_flattened_circuit() {
    let circuit_text = strip_stim_comments(&stim_python_generated_surface_code_circuit_text());
    let circuit = rstim::parser::parse_lines(&circuit_text).unwrap();
    let flat_circuit = rstim::transforms::flattened(&circuit);

    let raw_dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let flat_dem = ErrorAnalyzer::circuit_to_dem(&flat_circuit).unwrap();

    assert_repeat_aware_dem_parity(&raw_dem.to_string(), &flat_dem.to_string());
}

#[test]
fn panic_message_handles_static_str_and_non_string_payloads() {
    let owned = std::panic::catch_unwind(|| panic!("{}", String::from("owned panic"))).unwrap_err();
    assert_eq!(panic_message(owned), "owned panic");

    let literal = std::panic::catch_unwind(|| panic!("literal panic")).unwrap_err();
    assert_eq!(panic_message(literal), "literal panic");

    let non_string = std::panic::catch_unwind(|| std::panic::panic_any(5usize)).unwrap_err();
    assert_eq!(panic_message(non_string), "non-string panic payload");
}

#[test]
fn lock_stim_env_recovers_from_poisoning() {
    let panic = std::panic::catch_unwind(|| {
        let _guard = lock_stim_env();
        panic!("poison the stim env mutex");
    });
    assert!(panic.is_err());

    let _guard = lock_stim_env();
}

#[test]
fn canonicalize_error_targets_sorts_terms_and_components() {
    assert_eq!(
        canonicalize_error_targets(" D1   D0 ^  ^ L0 D2 "),
        "D0 D1 ^ D2 L0"
    );
}

#[test]
fn parse_dem_helpers_ignore_malformed_error_lines_without_closing_paren() {
    let malformed = "error(0.125 D0 D1\nerror(0.25) D2\n";
    assert_eq!(parse_dem_errors(malformed).len(), 1);
    assert_eq!(parse_dem_errors_multi(malformed)["D2"], vec![0.25]);
    assert_eq!(parse_dem_error_semantics(malformed)["D2"], 0.25);
}

#[test]
fn strip_stim_comments_removes_only_comment_lines() {
    assert_eq!(
        strip_stim_comments("# generated by stim\nM 0\n    # indented comment\nDETECTOR rec[-1]"),
        "M 0\nDETECTOR rec[-1]"
    );
}

#[test]
fn assert_prob_maps_close_reports_missing_keys() {
    let mut expected = BTreeMap::new();
    expected.insert("D0 D1".to_string(), 0.125);
    let actual = BTreeMap::new();

    let panic = std::panic::catch_unwind(|| {
        assert_prob_maps_close(&expected, &actual, "missing probability");
    })
    .unwrap_err();
    let text = panic_message(panic);
    assert!(text.contains("missing probability"));
}

#[test]
fn assert_prob_maps_close_reports_probability_mismatch() {
    let mut expected = BTreeMap::new();
    expected.insert("D0 D1".to_string(), 0.125);
    let mut actual = BTreeMap::new();
    actual.insert("D0 D1".to_string(), 0.25);

    let panic = std::panic::catch_unwind(|| {
        assert_prob_maps_close(&expected, &actual, "probability mismatch");
    })
    .unwrap_err();
    let text = panic_message(panic);
    assert!(text.contains("probability mismatch"));
}

#[test]
fn assert_all_graphlike_dem_text_rejects_non_graphlike_component() {
    let panic = std::panic::catch_unwind(|| {
        assert_all_graphlike_dem_text("error(0.1) D0 D1 D2\n");
    })
    .unwrap_err();
    let text = panic_message(panic);
    assert!(text.contains("non-graphlike component"));
}

#[test]
fn assert_semantic_dem_parity_reports_annotation_mismatch() {
    let panic = std::panic::catch_unwind(|| {
        assert_semantic_dem_parity(
            "error(0.1) D0 D1\ndetector(0, 0) D0\n",
            "error(0.1) D0 D1\ndetector(1, 0) D0\n",
        );
    })
    .unwrap_err();
    let text = panic_message(panic);
    assert!(text.contains("detector annotations differ"));
}

#[test]
fn assert_repeat_aware_dem_parity_reports_annotation_mismatch() {
    let panic = std::panic::catch_unwind(|| {
        assert_repeat_aware_dem_parity(
            "repeat 2 {\n    error(0.1) D0\n    detector(0, 0) D0\n}\n",
            "repeat 2 {\n    error(0.1) D0\n    detector(1, 0) D0\n}\n",
        );
    })
    .unwrap_err();
    let text = panic_message(panic);
    assert!(text.contains("repeat-aware semantic error mismatch"));
}

#[test]
fn stim_python_generated_surface_code_circuit_text_reports_python_failure() {
    let _guard = lock_stim_env();
    let dir = tempfile::tempdir().unwrap();
    let python_path = dir.path().join("python3");
    fs::write(
        &python_path,
        "#!/bin/sh\necho 'python exploded' >&2\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&python_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&python_path, perms).unwrap();
    }

    let old_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.path());
    }
    let result = std::panic::catch_unwind(stim_python_generated_surface_code_circuit_text);
    match old_path {
        Some(path) => unsafe { std::env::set_var("PATH", path) },
        None => unsafe { std::env::remove_var("PATH") },
    }

    let panic_text = panic_message(result.unwrap_err());
    assert!(panic_text.contains("python3 stim generation failed: python exploded"));
}

#[test]
fn stim_analyze_errors_respects_override_command() {
    let _guard = lock_stim_env();
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
    let _guard = lock_stim_env();
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
fn cross_validate_decomposed_handwritten_non_graphlike_failure_mode() {
    let _guard = lock_stim_env();
    let circuit_text = "\
R 0 1 2
X_ERROR(0.1) 0
CX 0 1
CX 1 2
M 0 1 2
DETECTOR rec[-3]
DETECTOR rec[-2]
DETECTOR rec[-1]
";
    let stim_cmd = std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string());
    let mut child = Command::new(stim_cmd)
        .arg("analyze_errors")
        .arg("--decompose_errors")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("stim CLI not found");
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(circuit_text.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let instrs = rstim::parser::parse_lines(circuit_text).unwrap();
    let rstim_result = ErrorAnalyzer::circuit_to_dem_decomposed(&instrs);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let rstim_success = rstim_result
        .as_ref()
        .map(|dem| dem.to_string())
        .unwrap_or_default();

    assert!(
        output.stdout.is_empty(),
        "stim unexpectedly produced stdout:\n{stdout}"
    );
    let stim_stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stim_stderr.contains("Failed to decompose errors into graphlike components"),
        "stim stderr did not report decomposition failure:\n{stim_stderr}"
    );
    assert!(
        rstim_result.is_err(),
        "rstim unexpectedly succeeded:\n{rstim_success}"
    );
}

#[test]
fn cross_validate_decomposed_rep_code_dem() {
    let _guard = lock_stim_env();
    let circuit = repetition_code_memory(5, 3, 0.01);
    let circuit_text = circuit_to_string(&circuit);
    let stim_dem_text = stim_analyze_errors_flags(&circuit_text, &["--decompose_errors"]);
    let rstim_dem_text = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit)
        .unwrap()
        .to_string();

    assert_all_graphlike_dem_text(&stim_dem_text);
    assert_all_graphlike_dem_text(&rstim_dem_text);
    assert_semantic_dem_parity(&stim_dem_text, &rstim_dem_text);
}

#[test]
fn cross_validate_decomposed_surface_code_dem() {
    let _guard = lock_stim_env();
    let circuit = rotated_memory_x(5, 3, 0.01);
    let circuit_text = circuit_to_string(&circuit);
    let stim_dem_text = stim_analyze_errors_flags(&circuit_text, &["--decompose_errors"]);
    let rstim_dem_text = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit)
        .unwrap()
        .to_string();

    assert_all_graphlike_dem_text(&stim_dem_text);
    assert_all_graphlike_dem_text(&rstim_dem_text);
    assert_semantic_dem_parity(&stim_dem_text, &rstim_dem_text);
}

#[test]
fn cross_validate_surface_code_dem_from_original_stim_generated_repeat_circuit() {
    let _guard = lock_stim_env();
    let circuit_text = strip_stim_comments(&stim_python_generated_surface_code_circuit_text());
    let circuit = rstim::parser::parse_lines(&circuit_text).unwrap();
    let rstim_dem_text = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap().to_string();

    let dir = tempfile::tempdir().unwrap();
    let dem_path = dir.path().join("rstim_repeat_surface_code.dem");
    fs::write(&dem_path, &rstim_dem_text).unwrap();

    // Stim's repeat-preserving DEM factorization is not identical to the
    // factorization produced from an explicitly flattened circuit, so compare
    // semantic sampling behavior instead of comparing DEM terms directly.
    let script = r#"
import pathlib
import sys

import numpy as np
import stim

shots = 200_000
det_tol = 0.0015
obs_tol = 0.0015
pair_tol = 0.00075

circuit = stim.Circuit.generated(
    'surface_code:rotated_memory_z',
    rounds=9,
    distance=3,
    after_clifford_depolarization=0.001,
    after_reset_flip_probability=0.001,
    before_measure_flip_probability=0.001,
    before_round_data_depolarization=0.001,
)
dem = stim.DetectorErrorModel(pathlib.Path(sys.argv[1]).read_text())

circuit_dets, circuit_obs = circuit.compile_detector_sampler(seed=1234).sample(
    shots,
    separate_observables=True,
)
dem_dets, dem_obs, _ = dem.compile_sampler(seed=1235).sample(shots)

det_diff = float(np.max(np.abs(circuit_dets.mean(axis=0) - dem_dets.mean(axis=0))))
obs_diff = float(np.max(np.abs(circuit_obs.mean(axis=0) - dem_obs.mean(axis=0))))

pair_diff = 0.0
for a, b in [(4, 12), (5, 13), (9, 17), (10, 18), (21, 29), (68, 69)]:
    if a < circuit_dets.shape[1] and b < circuit_dets.shape[1]:
        circuit_joint = float(np.mean(circuit_dets[:, a] & circuit_dets[:, b]))
        dem_joint = float(np.mean(dem_dets[:, a] & dem_dets[:, b]))
        pair_diff = max(pair_diff, abs(circuit_joint - dem_joint))

if det_diff > det_tol or obs_diff > obs_tol or pair_diff > pair_tol:
    print(f'max detector mean diff: {det_diff}')
    print(f'max observable mean diff: {obs_diff}')
    print(f'max pairwise joint diff: {pair_diff}')
    sys.exit(1)
"#;
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(&dem_path)
        .output()
        .expect("failed to run python3");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "python3 sampling parity check failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn cross_validate_decomposed_color_code_failure_mode() {
    let _guard = lock_stim_env();
    let circuit = memory_xyz(3, 2, 0.001);
    let circuit_text = circuit_to_string(&circuit);
    let stim_cmd = std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string());
    let mut child = Command::new(stim_cmd)
        .arg("analyze_errors")
        .arg("--decompose_errors")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("stim CLI not found");
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(circuit_text.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let rstim_result = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let rstim_success = rstim_result
        .as_ref()
        .map(|dem| dem.to_string())
        .unwrap_or_default();

    assert!(
        output.stdout.is_empty(),
        "stim unexpectedly produced stdout:\n{stdout}"
    );
    let stim_stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stim_stderr.contains("non-deterministic detectors"),
        "stim stderr did not report non-deterministic detectors:\n{stim_stderr}"
    );
    assert!(
        rstim_result.is_err(),
        "rstim unexpectedly succeeded:\n{rstim_success}"
    );
    let rstim_err = rstim_result.unwrap_err();
    assert!(
        rstim_err.contains("non-deterministic"),
        "rstim error did not report non-deterministic detector state:\n{rstim_err}"
    );
}

#[test]
fn cross_validate_surface_code_dem() {
    let _guard = lock_stim_env();
    let instrs = rotated_memory_x(5, 3, 0.01);
    let circuit_text = circuit_to_string(&instrs);

    let stim_dem_text = stim_analyze_errors(&circuit_text);
    let rstim_dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let rstim_dem_text = rstim_dem.to_string();

    let stim_errors = parse_dem_errors(&stim_dem_text);
    let rstim_errors = parse_dem_errors(&rstim_dem_text);
    let count_context = format!(
        "error count mismatch: stim={} rstim={}",
        stim_errors.len(),
        rstim_errors.len()
    );

    assert_eq!(stim_errors.len(), rstim_errors.len(), "{count_context}");

    for key in stim_errors.keys() {
        let missing_key_context = format!("stim has error target '{key}' not in rstim");
        assert!(rstim_errors.contains_key(key), "{missing_key_context}");
    }

    let mut max_rel = 0.0f64;
    for (key, stim_p) in &stim_errors {
        let rstim_p = rstim_errors[key];
        let rel = (stim_p - rstim_p).abs() / stim_p.max(1e-20);
        if rel > max_rel {
            max_rel = rel;
        }
        let rel_context =
            format!("probability mismatch for '{key}': stim={stim_p} rstim={rstim_p} rel={rel}");
        assert!(rel < 1e-12, "{rel_context}");
    }

    let stim_det_lines: Vec<&str> = stim_dem_text
        .lines()
        .filter(|l| l.starts_with("detector") || l.starts_with("shift_detectors"))
        .collect();
    let rstim_det_lines: Vec<&str> = rstim_dem_text
        .lines()
        .filter(|l| l.starts_with("detector") || l.starts_with("shift_detectors"))
        .collect();
    assert_eq!(
        stim_det_lines, rstim_det_lines,
        "detector annotations differ"
    );
}

#[test]
fn cross_validate_rep_code_dem_probabilities() {
    let _guard = lock_stim_env();
    let circuit = repetition_code_memory(5, 3, 0.01);
    let circuit_text = circuit_to_string(&circuit);

    let stim_dem_text = stim_analyze_errors(&circuit_text);
    let rstim_dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    let rstim_dem_text = rstim_dem.to_string();

    let stim_errors = parse_dem_errors(&stim_dem_text);
    let rstim_errors = parse_dem_errors(&rstim_dem_text);
    let count_context = format!(
        "error count mismatch: stim={} rstim={}",
        stim_errors.len(),
        rstim_errors.len()
    );

    assert_eq!(stim_errors.len(), rstim_errors.len(), "{count_context}");

    for (key, stim_p) in &stim_errors {
        let rstim_p = rstim_errors.get(key).unwrap_or(&0.0);
        let rel = (stim_p - rstim_p).abs() / stim_p.max(1e-20);
        let rel_context =
            format!("probability mismatch for '{key}': stim={stim_p} rstim={rstim_p}");
        assert!(rel < 1e-12, "{rel_context}");
    }
}
