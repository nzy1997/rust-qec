use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BLINDED_SEED: u64 = 18_446_744_073_709_551_557;
const BLINDED_SEED_TEXT: &str = "18446744073709551557";

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_cli(args: &[String]) -> Output {
    rstim_cmd().args(args).output().expect("run rstim")
}

#[test]
fn export_decoder_dataset_cli_contract() {
    detectors_mode_writes_public_circuit_and_detector_rows();
    bare_relative_output_paths_write_bundles_in_current_directory();
    blinded_measurements_masks_recomputed_public_observable();
    deterministic_seed_reproduces_bundle_bytes();
    rejection_cases_fail_before_outputs_exist();
    println!("PASS decoder dataset cli detectors=1 relative_paths=1 blinded=1 deterministic=1 rejections=1");
}

fn detectors_mode_writes_public_circuit_and_detector_rows() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(
        &circuit,
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let public_out = root.path().join("public");
    let private_out = root.path().join("private");
    let output = run_cli(&export_args(
        &circuit,
        "detectors",
        &public_out,
        &private_out,
        &[],
    ));
    assert_success(&output, "detectors export");

    assert_eq!(
        sorted_entries(&public_out),
        vec!["circuit.stim", "manifest.json", "shots.b8"]
    );
    assert_eq!(
        sorted_entries(&private_out),
        vec!["answers.b8", "manifest.json"]
    );
    assert_eq!(fs::read(public_out.join("shots.b8")).unwrap(), vec![1]);
    assert_eq!(fs::read(private_out.join("answers.b8")).unwrap(), vec![1]);
    let manifest: Value =
        serde_json::from_slice(&fs::read(public_out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["mode"], "detectors");
    assert_eq!(manifest["row"]["kind"], "detectors");
}

fn bare_relative_output_paths_write_bundles_in_current_directory() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(
        &circuit,
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(root.path()).unwrap();
    let output = run_cli(&[
        "export_decoder_dataset".to_string(),
        "--circuit".to_string(),
        circuit.display().to_string(),
        "--shots".to_string(),
        "1".to_string(),
        "--mode".to_string(),
        "detectors".to_string(),
        "--public_out".to_string(),
        "public-data".to_string(),
        "--private_out".to_string(),
        "private-truth".to_string(),
    ]);
    std::env::set_current_dir(original_dir).unwrap();
    assert_success(&output, "bare relative output paths export");

    let public_out = root.path().join("public-data");
    let private_out = root.path().join("private-truth");
    assert_eq!(
        sorted_entries(&public_out),
        vec!["circuit.stim", "manifest.json", "shots.b8"]
    );
    assert_eq!(
        sorted_entries(&private_out),
        vec!["answers.b8", "manifest.json"]
    );
    assert_eq!(fs::read(public_out.join("shots.b8")).unwrap(), vec![1]);
    assert_eq!(fs::read(private_out.join("answers.b8")).unwrap(), vec![1]);
}

fn blinded_measurements_masks_recomputed_public_observable() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("producer-input.stim");
    fs::write(
        &circuit,
        "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let public_out = root.path().join("public");
    let private_out = root.path().join("private");
    let output = run_cli(&export_args(
        &circuit,
        "measurements_blinded",
        &public_out,
        &private_out,
        &["--logical_x_qubits", "0", "--seed", BLINDED_SEED_TEXT],
    ));
    assert_success(&output, "blinded export");

    let public_manifest = fs::read_to_string(public_out.join("manifest.json")).unwrap();
    assert_no_public_secret_words(&public_manifest, &private_out, &circuit, BLINDED_SEED);
    assert_eq!(fs::read(private_out.join("answers.b8")).unwrap().len(), 1);
    assert_eq!(fs::read(private_out.join("masks.b8")).unwrap().len(), 1);
}

fn deterministic_seed_reproduces_bundle_bytes() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(
        &circuit,
        "R 0\nX_ERROR(0.5) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let public_a = root.path().join("public-a");
    let private_a = root.path().join("private-a");
    let public_b = root.path().join("public-b");
    let private_b = root.path().join("private-b");
    let extra = ["--seed", "17"];

    assert_success(
        &run_cli(&export_args(
            &circuit,
            "detectors",
            &public_a,
            &private_a,
            &extra,
        )),
        "first deterministic export",
    );
    assert_success(
        &run_cli(&export_args(
            &circuit,
            "detectors",
            &public_b,
            &private_b,
            &extra,
        )),
        "second deterministic export",
    );
    assert_tree_bytes_equal(&public_a, &public_b);
    assert_tree_bytes_equal(&private_a, &private_b);
}

fn rejection_cases_fail_before_outputs_exist() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(
        &circuit,
        "R 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();

    for (name, mode, extra) in [
        ("zero-shots", "detectors", vec!["--shots", "0"]),
        ("unknown-mode", "nope", vec![]),
        (
            "detectors-logical-qubits",
            "detectors",
            vec!["--logical_x_qubits", "0"],
        ),
        (
            "blinded-without-logical-qubits",
            "measurements_blinded",
            vec![],
        ),
    ] {
        let public_out = root.path().join(format!("{name}-public"));
        let private_out = root.path().join(format!("{name}-private"));
        let mut args = export_args(&circuit, mode, &public_out, &private_out, &[]);
        if extra.as_slice() == ["--shots", "0"] {
            let shots_index = args.iter().position(|arg| arg == "--shots").unwrap() + 1;
            args[shots_index] = "0".to_string();
        } else {
            args.extend(extra.iter().map(|value| value.to_string()));
        }
        assert_failure(&run_cli(&args), name);
        assert!(!public_out.exists(), "{name} created public output");
        assert!(!private_out.exists(), "{name} created private output");
    }

    let public_out = root.path().join("existing-public");
    let private_out = root.path().join("existing-private");
    fs::create_dir(&public_out).unwrap();
    fs::write(public_out.join("keep"), "keep").unwrap();
    assert_failure(
        &run_cli(&export_args(
            &circuit,
            "detectors",
            &public_out,
            &private_out,
            &[],
        )),
        "pre-existing output directory",
    );
    assert_eq!(fs::read_to_string(public_out.join("keep")).unwrap(), "keep");
    assert!(
        !private_out.exists(),
        "pre-existing public output created private output"
    );

    let public_out = root.path().join("nested");
    let private_out = public_out.join("private");
    assert_failure(
        &run_cli(&export_args(
            &circuit,
            "detectors",
            &public_out,
            &private_out,
            &[],
        )),
        "nested output directories",
    );
    assert!(
        !public_out.exists(),
        "nested output directories created public output"
    );
}

fn generated_repetition_memory_with_marker() -> (String, &'static str) {
    let mut circuit = generated_common_circuit_text("repetition_code", "memory");
    insert_marker_before_first_tick(&mut circuit);
    (circuit, "0,1,2")
}

fn generated_surface_z_memory_with_marker() -> (String, &'static str) {
    let mut circuit = generated_common_circuit_text("surface_code", "rotated_memory_z");
    insert_marker_before_first_tick(&mut circuit);
    (circuit, "1,2,3")
}

fn generated_surface_x_memory_with_marker() -> String {
    let mut circuit = generated_common_circuit_text("surface_code", "rotated_memory_x");
    insert_marker_before_first_tick(&mut circuit);
    circuit
}

fn generated_common_circuit_text(code: &str, task: &str) -> String {
    let args = [
        "gen".to_string(),
        "--code".to_string(),
        code.to_string(),
        "--task".to_string(),
        task.to_string(),
        "--distance".to_string(),
        "3".to_string(),
        "--rounds".to_string(),
        "3".to_string(),
        "--after_clifford_depolarization".to_string(),
        "0.01".to_string(),
    ];
    let output = run_cli(&args);
    assert_success(&output, &format!("generate {code} {task}"));
    String::from_utf8(output.stdout).expect("generated circuit is UTF-8")
}

fn insert_marker_before_first_tick(circuit: &mut String) {
    let needle = "TICK\n";
    let index = circuit
        .find(needle)
        .expect("generated memory circuit has first TICK");
    circuit.insert_str(index, "# RSTIM_LOGICAL_FLIP_POINT\n");
}

#[test]
fn repetition_and_surface_memory_export_in_both_modes() {
    for (name, circuit_text, logical_x_qubits) in [
        {
            let (text, support) = generated_repetition_memory_with_marker();
            ("repetition", text, support)
        },
        {
            let (text, support) = generated_surface_z_memory_with_marker();
            ("surface_z", text, support)
        },
    ] {
        verify_memory_case(name, &circuit_text, logical_x_qubits, "detectors");
        verify_memory_case(
            name,
            &circuit_text,
            logical_x_qubits,
            "measurements_blinded",
        );
    }
}

#[test]
fn surface_memory_x_blinded_export_accepts_logical_z_and_rejects_invalid_choices() {
    const LOGICAL_X_SUPPORT: &str = "1,2,3";
    const LOGICAL_Z_SUPPORT: &str = "1,7,13";

    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("memory-x.stim");
    fs::write(&circuit, generated_surface_x_memory_with_marker()).unwrap();

    let public_out = root.path().join("success-public");
    let private_out = root.path().join("success-private");
    let mut args = export_args(
        &circuit,
        "measurements_blinded",
        &public_out,
        &private_out,
        &[
            "--logical_z_qubits",
            LOGICAL_Z_SUPPORT,
            "--seed",
            "20260813",
        ],
    );
    let shots_index = args.iter().position(|arg| arg == "--shots").unwrap() + 1;
    args[shots_index] = "64".to_string();
    assert_success(&run_cli(&args), "memory-X logical-Z blinded export");

    let public_manifest: Value =
        serde_json::from_slice(&fs::read(public_out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(public_manifest["shots"], 64);
    assert_eq!(public_manifest["row"]["kind"], "measurements");
    let measurement_bits = public_manifest["circuit"]["measurements"]
        .as_u64()
        .unwrap();
    assert_eq!(public_manifest["row"]["bits"], measurement_bits);
    let public_bytes_per_shot = public_manifest["row"]["bytes_per_shot"]
        .as_u64()
        .unwrap() as usize;
    assert_eq!(public_bytes_per_shot, measurement_bits.div_ceil(8) as usize);
    assert_eq!(
        fs::read(public_out.join("shots.b8")).unwrap().len(),
        64 * public_bytes_per_shot
    );

    let private_manifest: Value =
        serde_json::from_slice(&fs::read(private_out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(private_manifest["shots"], 64);
    assert_eq!(private_manifest["answers_file"]["bits"], 1);
    assert_eq!(private_manifest["answers_file"]["bytes_per_shot"], 1);
    assert_eq!(private_manifest["masks_file"]["bits"], 1);
    assert_eq!(private_manifest["masks_file"]["bytes_per_shot"], 1);
    assert_eq!(fs::read(private_out.join("answers.b8")).unwrap().len(), 64);
    assert_eq!(fs::read(private_out.join("masks.b8")).unwrap().len(), 64);

    for (name, mode, extra, expected) in [
        (
            "logical-X-does-not-flip",
            "measurements_blinded",
            vec!["--logical_x_qubits", LOGICAL_X_SUPPORT],
            "injected logical X does not flip observable 0",
        ),
        (
            "both-logical-options",
            "measurements_blinded",
            vec![
                "--logical_x_qubits",
                LOGICAL_X_SUPPORT,
                "--logical_z_qubits",
                LOGICAL_Z_SUPPORT,
            ],
            "--logical_x_qubits and --logical_z_qubits are mutually exclusive",
        ),
        (
            "neither-logical-option",
            "measurements_blinded",
            vec![],
            "measurements_blinded mode requires exactly one of --logical_x_qubits or --logical_z_qubits",
        ),
        (
            "detectors-logical-Z",
            "detectors",
            vec!["--logical_z_qubits", LOGICAL_Z_SUPPORT],
            "detectors mode rejects --logical_z_qubits",
        ),
        (
            "invalid-logical-X",
            "measurements_blinded",
            vec!["--logical_x_qubits", "not-a-qubit"],
            "--logical_x_qubits contains invalid",
        ),
        (
            "invalid-logical-Z",
            "measurements_blinded",
            vec!["--logical_z_qubits", "not-a-qubit"],
            "--logical_z_qubits contains invalid",
        ),
    ] {
        let public_out = root.path().join(format!("{name}-public"));
        let private_out = root.path().join(format!("{name}-private"));
        let output = run_cli(&export_args(
            &circuit,
            mode,
            &public_out,
            &private_out,
            &extra,
        ));
        assert_failure_contains(&output, name, expected);
        assert!(!public_out.exists(), "{name} created public output");
        assert!(!private_out.exists(), "{name} created private output");
    }
}

fn verify_memory_case(name: &str, circuit_text: &str, logical_x_qubits: &str, mode: &str) {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join(format!("{name}.stim"));
    fs::write(&circuit, circuit_text).unwrap();
    let public_out = root.path().join(format!("{name}-{mode}-public"));
    let private_out = root.path().join(format!("{name}-{mode}-private"));
    let mut extra = vec!["--seed", "20260728"];
    if mode == "measurements_blinded" {
        extra.extend(["--logical_x_qubits", logical_x_qubits]);
    }
    let output = run_cli(&export_args(
        &circuit,
        mode,
        &public_out,
        &private_out,
        &extra,
    ));
    assert_success(&output, &format!("{name} {mode}"));
    assert_eq!(
        sorted_entries(&public_out),
        vec!["circuit.stim", "manifest.json", "shots.b8"]
    );
    assert!(private_out.join("answers.b8").exists());
    if mode == "measurements_blinded" {
        assert!(private_out.join("masks.b8").exists());
    }
}

fn export_args(
    circuit: &Path,
    mode: &str,
    public_out: &Path,
    private_out: &Path,
    extra: &[&str],
) -> Vec<String> {
    let mut args = vec![
        "export_decoder_dataset".to_string(),
        "--circuit".to_string(),
        circuit.display().to_string(),
        "--shots".to_string(),
        "1".to_string(),
        "--mode".to_string(),
        mode.to_string(),
        "--public_out".to_string(),
        public_out.display().to_string(),
        "--private_out".to_string(),
        private_out.display().to_string(),
    ];
    args.extend(extra.iter().map(|value| value.to_string()));
    args
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output, context: &str) {
    assert!(!output.status.success(), "{context} unexpectedly succeeded");
}

fn assert_failure_contains(output: &Output, context: &str, expected: &str) {
    assert_failure(output, context);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "{context} stderr did not contain {expected:?}: {stderr}"
    );
}

fn sorted_entries(path: &Path) -> Vec<String> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    entries.sort();
    entries
}

fn assert_no_public_secret_words(
    public_manifest: &str,
    private_out: &Path,
    producer_circuit: &Path,
    forbidden_seed: u64,
) {
    let manifest: Value = serde_json::from_str(public_manifest).expect("public manifest is JSON");
    let private_path = private_out.display().to_string();
    let producer_path = producer_circuit.display().to_string();
    let producer_label = producer_circuit
        .file_name()
        .expect("producer circuit has file name")
        .to_string_lossy()
        .into_owned();
    assert_no_public_secret_value(
        &manifest,
        "$",
        forbidden_seed,
        &[
            private_path.as_str(),
            producer_path.as_str(),
            producer_label.as_str(),
        ],
    );
}

fn assert_no_public_secret_value(
    value: &Value,
    path: &str,
    forbidden_seed: u64,
    forbidden_values: &[&str],
) {
    const FORBIDDEN_WORDS: [&str; 6] = [
        "seed",
        "mask",
        "answer",
        "private",
        "producer",
        "permutation",
    ];

    match value {
        Value::Object(values) => {
            for (key, value) in values {
                let normalized_key = key.to_ascii_lowercase();
                for forbidden in FORBIDDEN_WORDS {
                    assert!(
                        !normalized_key.contains(forbidden),
                        "public manifest leaked {forbidden} key at {path}.{key}"
                    );
                }
                assert_no_public_secret_value(
                    value,
                    &format!("{path}.{key}"),
                    forbidden_seed,
                    forbidden_values,
                );
            }
        }
        Value::Array(_) => {
            panic!("public manifest leaked an array at {path}; arrays can expose row permutations");
        }
        Value::String(text) => {
            let normalized_text = text.to_ascii_lowercase();
            for forbidden in FORBIDDEN_WORDS {
                assert!(
                    !normalized_text.contains(forbidden),
                    "public manifest leaked {forbidden} value at {path}: {text}"
                );
            }
            for forbidden in forbidden_values {
                assert_ne!(
                    text, *forbidden,
                    "public manifest leaked private path or producer-circuit label at {path}: {text}"
                );
            }
        }
        Value::Number(number) => {
            assert_ne!(
                number.as_u64(),
                Some(forbidden_seed),
                "public manifest leaked seed value at {path}: {number}"
            );
        }
        _ => {}
    }
}

fn assert_tree_bytes_equal(left: &Path, right: &Path) {
    assert_eq!(sorted_entries(left), sorted_entries(right));
    for entry in fs::read_dir(left).unwrap() {
        let entry = entry.unwrap();
        let name: PathBuf = entry.file_name().into();
        assert_eq!(
            fs::read(entry.path()).unwrap(),
            fs::read(right.join(name)).unwrap()
        );
    }
}
