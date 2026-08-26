use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SHOTS: usize = 64;

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run(args: &[String]) -> Output {
    rstim_cmd().args(args).output().expect("run rstim")
}

fn export_args(
    circuit: &Path,
    mode: &str,
    public: &Path,
    private: &Path,
    extra: &[&str],
) -> Vec<String> {
    let mut args = vec![
        "export_decoder_dataset".to_string(),
        "--circuit".to_string(),
        circuit.display().to_string(),
        "--shots".to_string(),
        SHOTS.to_string(),
        "--mode".to_string(),
        mode.to_string(),
        "--public_out".to_string(),
        public.display().to_string(),
        "--private_out".to_string(),
        private.display().to_string(),
    ];
    args.extend(extra.iter().map(|arg| arg.to_string()));
    args
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn bit(bytes: &[u8], shot: usize, index: usize) -> bool {
    bytes[shot] >> index & 1 == 1
}

fn sorted_files(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn assert_public_has_no_private_metadata(public: &Path) {
    assert_eq!(
        sorted_files(public)
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        ["circuit.stim", "manifest.json", "shots.b8"]
    );
    for path in sorted_files(public) {
        let bytes = fs::read(&path).unwrap();
        for secret in [
            b"logical_input".as_slice(),
            b"\"support\"".as_slice(),
            b"\"pauli\"".as_slice(),
            b"masks.b8".as_slice(),
            b"answers.b8".as_slice(),
            b"trace.jsonl".as_slice(),
        ] {
            assert!(
                !bytes.windows(secret.len()).any(|window| window == secret),
                "{} leaked {:?}",
                path.display(),
                String::from_utf8_lossy(secret)
            );
        }
    }
}

fn verify_support_case(root: &Path, name: &str, pauli: &str) -> (PathBuf, PathBuf) {
    let circuit = root.join(format!("{name}.stim"));
    let (reset, measurement, option) = match pauli {
        "X" => ("R", "M", "--logical_x_qubits"),
        "Z" => ("RX", "MX", "--logical_z_qubits"),
        _ => unreachable!(),
    };
    fs::write(
        &circuit,
        format!(
            "{reset} 0 1\nTICK[rstim:logical_flip_point]\n{pauli} 1\n{measurement} 0 1\nDETECTOR rec[-2] rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\n"
        ),
    )
    .unwrap();
    let public = root.join(format!("{name}-public"));
    let private = root.join(format!("{name}-private"));
    let output = run(&export_args(
        &circuit,
        "measurements_blinded",
        &public,
        &private,
        &[
            option,
            "0,1",
            "--seed",
            "7",
            "--batch_shots",
            "17",
            "--error_trace",
        ],
    ));
    assert_success(&output, name);

    let measurements = fs::read(public.join("shots.b8")).unwrap();
    let answers = fs::read(private.join("answers.b8")).unwrap();
    let masks = fs::read(private.join("masks.b8")).unwrap();
    assert_eq!(measurements.len(), SHOTS);
    assert_eq!(answers.len(), SHOTS);
    assert_eq!(masks.len(), SHOTS);
    assert!((0..SHOTS).any(|shot| bit(&masks, shot, 0)));
    assert!((0..SHOTS).any(|shot| !bit(&masks, shot, 0)));

    let traces: Vec<Value> = fs::read_to_string(private.join("trace.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(traces.len(), SHOTS);
    for shot in 0..SHOTS {
        let mask = bit(&masks, shot, 0);
        let first_measurement = bit(&measurements, shot, 0);
        let second_measurement = bit(&measurements, shot, 1);
        assert_eq!(first_measurement, mask, "{name} shot {shot}");
        assert_eq!(second_measurement, !mask, "{name} shot {shot}");
        assert!(first_measurement ^ second_measurement);
        assert_eq!(bit(&answers, shot, 0), first_measurement ^ mask);

        let logical = &traces[shot]["logical_input"];
        assert_eq!(traces[shot]["shot"], shot);
        assert_eq!(logical["bit"], u8::from(mask));
        assert_eq!(logical["applied"], mask);
        assert_eq!(logical["pauli"], pauli);
        assert_eq!(logical["support"], serde_json::json!([0, 1]));
        assert!(traces[shot]["events"].as_array().unwrap().is_empty());
    }
    assert_public_has_no_private_metadata(&public);
    (public, private)
}

#[test]
fn logical_input_metadata_cli_contract() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let (x_public, x_private) = verify_support_case(root, "logical-x", "X");
    verify_support_case(root, "logical-z", "Z");

    let repeat_public = root.join("logical-x-repeat-public");
    let repeat_private = root.join("logical-x-repeat-private");
    let repeat = run(&export_args(
        &root.join("logical-x.stim"),
        "measurements_blinded",
        &repeat_public,
        &repeat_private,
        &[
            "--logical_x_qubits",
            "0,1",
            "--seed",
            "7",
            "--batch_shots",
            "17",
            "--error_trace",
        ],
    ));
    assert_success(&repeat, "repeat export");
    for file in ["manifest.json", "answers.b8", "masks.b8", "trace.jsonl"] {
        assert_eq!(
            fs::read(x_private.join(file)).unwrap(),
            fs::read(repeat_private.join(file)).unwrap(),
            "fixed seed did not reproduce {file}"
        );
    }

    let loader =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("doc/examples/load_blinded_training_data.py");
    let loader_output = Command::new("python3")
        .arg(loader)
        .args(["--public-dir", x_public.to_str().unwrap()])
        .args(["--private-dir", x_private.to_str().unwrap()])
        .args(["--observable-rec", "-2"])
        .output()
        .expect("run Python training loader");
    assert!(
        loader_output.status.success(),
        "loader failed: {}",
        String::from_utf8_lossy(&loader_output.stderr)
    );
    assert_eq!(
        String::from_utf8(loader_output.stdout).unwrap().trim(),
        "PASS training alignment shots=64"
    );

    let wrong_offset = Command::new("python3")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("doc/examples/load_blinded_training_data.py"),
        )
        .args(["--public-dir", x_public.to_str().unwrap()])
        .args(["--private-dir", x_private.to_str().unwrap()])
        .args(["--observable-rec", "-1"])
        .output()
        .expect("run loader with a wrong observable offset");
    assert!(!wrong_offset.status.success());
    assert!(String::from_utf8_lossy(&wrong_offset.stderr)
        .contains("violates answer = O_public(measurement) XOR mask"));

    let detector_public = root.join("detector-negative-public");
    let detector_private = root.join("detector-negative-private");
    let detector_failure = run(&export_args(
        &root.join("logical-x.stim"),
        "detectors",
        &detector_public,
        &detector_private,
        &["--logical_x_qubits", "0,1"],
    ));
    assert!(!detector_failure.status.success());
    assert!(!detector_public.exists());
    assert!(!detector_private.exists());

    let early_loss_circuit = root.join("early-loss.stim");
    fs::write(
        &early_loss_circuit,
        "R 0 1\nLOSS(0.1) 1\nTICK[rstim:logical_flip_point]\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-2]\n",
    )
    .unwrap();
    let loss_public = root.join("loss-negative-public");
    let loss_private = root.join("loss-negative-private");
    let loss_failure = run(&export_args(
        &early_loss_circuit,
        "measurements_blinded",
        &loss_public,
        &loss_private,
        &["--logical_x_qubits", "0"],
    ));
    assert!(!loss_failure.status.success());
    assert!(String::from_utf8_lossy(&loss_failure.stderr).contains("positive-probability noise"));
    assert!(!loss_public.exists());
    assert!(!loss_private.exists());

    println!(
        "PASS logical input metadata shots=64 masks_match=64 answers_match=64 x_support=1 z_support=1 public_leaks=0 negative_cases=2"
    );
}
