use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> Value {
    assert!(output.status.success(), "{:?}", output);
    serde_json::from_slice(&output.stdout).unwrap()
}

fn export(root: &Path, name: &str, extra: &[&str]) -> Output {
    let public = root.join(format!("{name}-public"));
    let private = root.join(format!("{name}-private"));
    let circuit = root.join("circuit.stim");
    let mut args = vec![
        "dataset",
        "export",
        "--circuit",
        circuit.to_str().unwrap(),
        "--shots",
        "16",
        "--seed",
        "7",
        "--error-trace",
        "--public-out",
        public.to_str().unwrap(),
        "--private-out",
        private.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn generated_support_tracks_distance_and_auto_matches_explicit_export() {
    for (distance, support) in [("3", "1,8,15"), ("5", "1,12,23,34,45")] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let circuit = root.join("circuit.stim");
        success(run(&[
            "circuit",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z_midswap",
            "--distance",
            distance,
            "--rounds",
            "3",
            "--operation-loss-probability",
            "0.002",
            "--measurement-loss-probability",
            "0.003",
            "--out",
            circuit.to_str().unwrap(),
        ]));
        let source = fs::read_to_string(&circuit).unwrap();
        assert!(source.contains(&format!(
            "TICK[rstim:logical_flip_point] # rstim:logical_input=X:{support}"
        )));
        // The declaration must not change the physical gates or their schedule.
        let stripped = source
            .lines()
            .map(|line| line.split('#').next().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            rstim::parser::parse_lines(&source).unwrap(),
            rstim::parser::parse_lines(&stripped).unwrap()
        );
        let value = success(export(root, "auto", &[]));
        assert_eq!(value["result"]["mode"], "measurements_blinded");
        success(export(
            root,
            "explicit",
            &[
                "--mode",
                "measurements_blinded",
                "--logical-x-qubits",
                support,
            ],
        ));
        for (bundle, files) in [
            ("public", vec!["circuit.stim", "manifest.json", "shots.b8"]),
            (
                "private",
                vec!["manifest.json", "answers.b8", "masks.b8", "trace.jsonl"],
            ),
        ] {
            for file in files {
                assert_eq!(
                    fs::read(root.join(format!("auto-{bundle}/{file}"))).unwrap(),
                    fs::read(root.join(format!("explicit-{bundle}/{file}"))).unwrap(),
                    "d={distance}, {bundle}/{file}"
                );
            }
        }
        let masks = fs::read(root.join("auto-private/masks.b8")).unwrap();
        assert!(masks.contains(&0) && masks.contains(&1));
        let support: Vec<u32> = support.split(',').map(|n| n.parse().unwrap()).collect();
        for (shot, line) in fs::read_to_string(root.join("auto-private/trace.jsonl"))
            .unwrap()
            .lines()
            .enumerate()
        {
            let trace: Value = serde_json::from_str(line).unwrap();
            assert_eq!(trace["logical_input"]["support"], json!(support));
            assert_eq!(trace["logical_input"]["pauli"], "X");
            assert_eq!(trace["logical_input"]["bit"], masks[shot]);
        }
        let public: Value =
            serde_json::from_slice(&fs::read(root.join("auto-public/manifest.json")).unwrap())
                .unwrap();
        assert_eq!(public["mode"], "measurements_blinded");
        assert!(public.get("logical_input").is_none());
    }
}

#[test]
fn generated_mle_witnesses_match_the_release_scope_identities() {
    let plan_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("docs/envelope-mle-scope.json");
    let plan: Value = serde_json::from_slice(&fs::read(plan_path).unwrap()).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("witness.stim");
    let mut generated_cases = 0;
    for case in plan["required_cases"].as_array().unwrap() {
        let Some(source) = case["evidence_identity"]["source"].as_str() else {
            continue;
        };
        if !source.starts_with("rstim circuit gen ") {
            continue;
        }
        let args: Vec<_> = source.split_whitespace().skip(1).collect();
        let output = Command::new(env!("CARGO_BIN_EXE_rstim"))
            .args(args)
            .arg("--out")
            .arg(&path)
            .output()
            .unwrap();
        success(output);
        let circuit = fs::read(&path).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&circuit)),
            case["evidence_identity"]["circuit_sha256"]
                .as_str()
                .unwrap(),
            "release scope identity is stale for {}",
            case["id"]
        );
        generated_cases += 1;
    }
    assert_eq!(generated_cases, 3);
}

#[test]
fn auto_falls_back_to_detectors_and_explicit_mode_can_ignore_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let circuit = root.join("circuit.stim");
    fs::write(
        &circuit,
        "R 0\nTICK[rstim:logical_flip_point]\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let result = success(export(root, "plain", &[]));
    assert_eq!(result["result"]["mode"], "detectors");
    assert!(!root.join("plain-private/masks.b8").exists());
    let result = success(export(root, "explicit-input", &["--logical-x-qubits", "0"]));
    assert_eq!(result["result"]["mode"], "measurements_blinded");

    fs::write(&circuit, "RX 0\nTICK[rstim:logical_flip_point] # rstim:logical_input=Z:0\nMX 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let result = success(export(root, "declared", &[]));
    assert_eq!(result["result"]["mode"], "measurements_blinded");
    let result = success(export(root, "detectors", &["--mode", "detectors"]));
    assert_eq!(result["result"]["mode"], "detectors");
    assert!(!root.join("detectors-private/masks.b8").exists());
    assert_eq!(
        fs::read(root.join("declared-private/answers.b8")).unwrap(),
        vec![0; 16]
    );
    let output = run(&[
        "export_decoder_dataset",
        "--circuit",
        circuit.to_str().unwrap(),
        "--shots",
        "16",
        "--seed",
        "7",
        "--public_out",
        root.join("direct-public").to_str().unwrap(),
        "--private_out",
        root.join("direct-private").to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{:?}", output);
    for (bundle, file) in [
        ("public", "shots.b8"),
        ("private", "masks.b8"),
        ("private", "answers.b8"),
    ] {
        assert_eq!(
            fs::read(root.join(format!("direct-{bundle}/{file}"))).unwrap(),
            fs::read(root.join(format!("declared-{bundle}/{file}"))).unwrap()
        );
    }
}

#[test]
fn explicit_support_overrides_a_circuit_default_after_validation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    // Both X0 and X1 flip the observable without changing any detector.
    fs::write(root.join("circuit.stim"), "R 0 1\nTICK[rstim:logical_flip_point] # rstim:logical_input=X:0\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-1] rec[-2]\n").unwrap();
    success(export(root, "override", &["--logical-x-qubits", "1"]));
    for line in fs::read_to_string(root.join("override-private/trace.jsonl"))
        .unwrap()
        .lines()
    {
        let trace: Value = serde_json::from_str(line).unwrap();
        assert_eq!(trace["logical_input"]["support"], json!([1]));
    }
    assert!(
        !export(root, "invalid-override", &["--logical-z-qubits", "1"])
            .status
            .success()
    );
    assert!(!root.join("invalid-override-public").exists());
}

#[test]
fn auto_rejects_invalid_declarations_and_unsafe_insertion_points_before_writing() {
    for (prefix, marker) in [
        (
            "R 0\n",
            "TICK[rstim:logical_flip_point] # rstim:logical_input=X",
        ),
        (
            "R 0\n",
            "TICK[rstim:logical_flip_point] # rstim:logical_input=X:",
        ),
        (
            "R 0\n",
            "TICK[rstim:logical_flip_point] # rstim:logical_input=Y:0",
        ),
        (
            "R 0\n",
            "TICK[rstim:logical_flip_point] # rstim:logical_input=X:0,0",
        ),
        (
            "R 0\n",
            "TICK[rstim:logical_flip_point] # rstim:logical_input=X:1",
        ),
        (
            "R 0\n",
            "TICK[rstim:logical_flip_point] # rstim:logical_input=Z:0",
        ),
        ("R 0\n", "TICK # rstim:logical_input=X:0"),
        (
            "R 0\n",
            "TICK[rstim:logical_flip_point] # rstim:logical_input=X:0\nTICK[rstim:logical_flip_point] # rstim:logical_input=X:0",
        ),
        (
            "R 0\nX_ERROR(0.1) 0\n",
            "TICK[rstim:logical_flip_point] # rstim:logical_input=X:0",
        ),
        (
            "R 0\n",
            "REPEAT 1 {\nTICK[rstim:logical_flip_point] # rstim:logical_input=X:0\n}",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(
            root.join("circuit.stim"),
            format!("{prefix}{marker}\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n"),
        )
        .unwrap();
        let output = export(root, "invalid", &[]);
        assert!(!output.status.success(), "accepted {prefix}{marker}");
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert!(
            matches!(
                error["error"]["code"].as_str(),
                Some("invalid_arguments" | "execution_error")
            ),
            "{error}"
        );
        assert!(!error["error"]["message"].as_str().unwrap().is_empty());
        assert!(!root.join("invalid-public").exists());
        assert!(!root.join("invalid-private").exists());
    }
}
