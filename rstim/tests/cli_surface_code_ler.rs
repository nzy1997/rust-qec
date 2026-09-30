use std::fs;
use std::process::Command;

const SWEEP: [&str; 6] = [
    "--distances",
    "3,5,7",
    "--rounds",
    "9,15,21",
    "--physical-error-rates",
    "0.008,0.009,0.01,0.011,0.012",
];

#[test]
fn installed_cli_runs_surface_code_sweep_and_plots_measured_failures() {
    let output_dir = tempfile::tempdir().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .current_dir(output_dir.path())
        .arg("surface-code-ler")
        .args(SWEEP)
        .args(["--shots", "2000", "--seed", "86", "--out-dir", "."])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let csv = fs::read_to_string(output_dir.path().join("surface-code-ler.csv")).unwrap();
    let rows: Vec<_> = csv.lines().collect();
    assert_eq!(rows.len(), 16);
    assert_eq!(
        rows[1].split(',').take(7).collect::<Vec<_>>(),
        ["3", "9", "0.008", "2000", "386", "218", "0.109"]
    );
    assert_eq!(
        rows[15].split(',').take(7).collect::<Vec<_>>(),
        ["7", "21", "0.012", "2000", "790", "723", "0.3615"]
    );
    let per_round_d3_at_low_p: f64 = rows[1].split(',').nth(7).unwrap().parse().unwrap();
    let per_round_d7_at_low_p: f64 = rows[11].split(',').nth(7).unwrap().parse().unwrap();
    let per_round_d3_at_high_p: f64 = rows[5].split(',').nth(7).unwrap().parse().unwrap();
    let per_round_d7_at_high_p: f64 = rows[15].split(',').nth(7).unwrap().parse().unwrap();
    let expected_per_round = (1.0 - (1.0 - 2.0 * 0.109_f64).powf(1.0 / 9.0)) / 2.0;
    assert!((per_round_d3_at_low_p - expected_per_round).abs() < 1e-12);
    assert!(per_round_d7_at_low_p < per_round_d3_at_low_p);
    assert!(per_round_d7_at_high_p > per_round_d3_at_high_p);

    let svg = fs::read_to_string(output_dir.path().join("surface-code-ler.svg")).unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("distance 3, rounds 9"));
    assert!(svg.contains("distance 5, rounds 15"));
    assert!(svg.contains("distance 7, rounds 21"));
    assert!(svg.contains("Logical Error Rate per Round"));
    assert!(svg.contains("0.008"));
    assert!(svg.contains("0.012"));
}

#[test]
fn installed_cli_rejects_zero_shots() {
    let result = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .arg("surface-code-ler")
        .args(SWEEP)
        .args(["--shots", "0"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("--shots must be positive"));
}

#[test]
fn installed_cli_plots_single_shot_points() {
    let output_dir = tempfile::tempdir().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .current_dir(output_dir.path())
        .arg("surface-code-ler")
        .args(SWEEP)
        .args(["--shots", "1"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output_dir.path().join("surface-code-ler.svg").is_file());
}

#[test]
fn installed_cli_uses_the_requested_rounds_and_error_rates() {
    let output_dir = tempfile::tempdir().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .current_dir(output_dir.path())
        .args([
            "surface-code-ler",
            "--distances",
            "3",
            "--rounds",
            "2",
            "--physical-error-rates",
            "0.01,0.02",
            "--shots",
            "1",
            "--out-dir",
            ".",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let csv = fs::read_to_string(output_dir.path().join("surface-code-ler.csv")).unwrap();
    let rows: Vec<_> = csv.lines().collect();
    assert_eq!(rows.len(), 3);
    assert!(rows[1].starts_with("3,2,0.01,1,"));
    assert!(rows[2].starts_with("3,2,0.02,1,"));
    let svg = fs::read_to_string(output_dir.path().join("surface-code-ler.svg")).unwrap();
    assert!(svg.contains("distance 3, rounds 2"));
}

#[test]
fn installed_cli_rejects_mismatched_distance_and_round_lists() {
    let result = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args([
            "surface-code-ler",
            "--distances",
            "3,5",
            "--rounds",
            "3",
            "--physical-error-rates",
            "0.01,0.02",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("--distances and --rounds must contain the same nonzero number of values")
    );
}
