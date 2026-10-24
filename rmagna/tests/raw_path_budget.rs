use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::clifford::CliffordGate;
use rmagna::gf2::BitVector;
use rmagna::instrument::{CompileOptions, compile_affine};
use rmagna::scalar::ScalarArithmetic;
use rmagna::unitary::{CompilerStrategy, UnitaryCircuit, UnitaryError, UnitaryGate};

#[test]
fn raw_paths_are_bounded_before_repeated_hadamards_contract_to_a_zero_rank_boundary() {
    let a = ScalarArithmetic::default();
    for width in [1, 65] {
        let circuit =
            UnitaryCircuit::new(width, vec![UnitaryGate::Clifford(CliffordGate::H(0)); 8]).unwrap();
        for strategy in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let input = BitVector::zeros(width).unwrap();
            for limit in [0, 1, 7] {
                assert!(
                    matches!(circuit.compile_with_path_limit(&input,&a,strategy,limit),Err(UnitaryError::RawPathLimit{limit:l,required:8}) if l==limit)
                );
            }
            let bounded = circuit
                .compile_with_path_limit(&input, &a, strategy, 8)
                .unwrap();
            assert_eq!(bounded.compilation().raw_path_variables, 8);
            assert_eq!(bounded.compilation().path_variables, 0);
            let unbounded = circuit.compile_with_strategy(&input, &a, strategy).unwrap();
            assert_eq!(
                bounded
                    .probability_z(&[], &BitVector::zeros(0).unwrap(), usize::MAX, &a)
                    .unwrap()
                    .value,
                unbounded
                    .probability_z(&[], &BitVector::zeros(0).unwrap(), usize::MAX, &a)
                    .unwrap()
                    .value
            );
        }
    }
    let circuit = UnitaryCircuit::new(3, vec![UnitaryGate::Clifford(CliffordGate::X(2))]).unwrap();
    assert_eq!(
        circuit
            .compile_with_path_limit(
                &BitVector::zeros(3).unwrap(),
                &a,
                CompilerStrategy::SharedCommutingLayer,
                0
            )
            .unwrap()
            .compilation()
            .raw_path_variables,
        0
    );
}
#[test]
fn phase_lowering_counts_observation_hadamards_and_keeps_interference_results() {
    let a = ScalarArithmetic::default();
    let source = UnitaryCircuit::new(
        3,
        vec![
            UnitaryGate::Clifford(CliffordGate::H(0)),
            UnitaryGate::T { qubit: 0, turns: 1 },
            UnitaryGate::Clifford(CliffordGate::H(0)),
        ],
    )
    .unwrap();
    let input = BitVector::zeros(3).unwrap();
    for (strategy, expected) in [
        (CompilerStrategy::ParityCopies, 2),
        (CompilerStrategy::SharedCommutingLayer, 4),
    ] {
        assert!(
            matches!(source.compile_with_path_limit(&input,&a,strategy,expected-1),Err(UnitaryError::RawPathLimit{required,..}) if required==expected)
        );
        let bounded = source
            .compile_with_path_limit(&input, &a, strategy, expected)
            .unwrap();
        let unbounded = source.compile_with_strategy(&input, &a, strategy).unwrap();
        assert_eq!(bounded.compilation().raw_path_variables, expected);
        for outcome in [false, true] {
            let row = BitVector::from_indices(bounded.width(), [0]).unwrap();
            let outcomes = BitVector::from_indices(1, outcome.then_some(0)).unwrap();
            assert_eq!(
                bounded
                    .probability_z(std::slice::from_ref(&row), &outcomes, 16, &a)
                    .unwrap()
                    .value,
                unbounded
                    .probability_z(&[row], &outcomes, 16, &a)
                    .unwrap()
                    .value
            );
        }
    }
}
#[test]
fn instrument_measurement_reset_dilation_uses_the_same_precontraction_limit() {
    let source = parse_circuit(
        "H 0\nH 0\nM 0\nR 0\nH 0\nH 0\nM 0",
        &ParseOptions::default(),
    )
    .unwrap();
    let a = ScalarArithmetic::default();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let options = CompileOptions {
            compiler_strategy: strategy,
            max_path_bits: 4,
            ..Default::default()
        };
        let compiled = compile_affine(&source, &options, &a).unwrap();
        assert_eq!(compiled.compilation().raw_path_variables, 4);
        let bad = compile_affine(
            &source,
            &CompileOptions {
                max_path_bits: 3,
                ..options
            },
            &a,
        )
        .err()
        .unwrap();
        assert!(bad.to_string().contains("max_path_bits = 3"));
        assert!(bad.to_string().contains("needs 4"));
    }
}

#[test]
fn matched_clifford_lowerings_agree_with_pinned_native_raw_path_thresholds() {
    let document: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/raw_path_budget_native.json")).unwrap();
    assert_eq!(
        document["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(
        document["native_sha256"],
        "24ec8f3c108ca3a624ca5b0fd0b536ceb01111736f5d737643eb8f44a6d4bd77"
    );
    let vectors = document["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 6);
    for vector in vectors {
        let source =
            parse_circuit(vector["source"].as_str().unwrap(), &ParseOptions::default()).unwrap();
        let minimum = vector["minimum_successful_cap"].as_u64().unwrap() as usize;
        for strategy in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let options = CompileOptions {
                compiler_strategy: strategy,
                max_path_bits: minimum,
                ..Default::default()
            };
            let compiled = compile_affine(&source, &options, &ScalarArithmetic::default()).unwrap();
            assert_eq!(
                compiled.compilation().raw_path_variables,
                minimum,
                "{}",
                vector["case"]
            );
            for cap in vector["rejected_caps"].as_array().unwrap() {
                let cap = cap.as_u64().unwrap() as usize;
                let error = compile_affine(
                    &source,
                    &CompileOptions {
                        max_path_bits: cap,
                        ..options
                    },
                    &ScalarArithmetic::default(),
                )
                .err()
                .unwrap();
                assert!(
                    error
                        .to_string()
                        .contains(&format!("max_path_bits = {cap}"))
                );
            }
        }
    }
}

#[test]
fn file_and_cli_keep_budget_identity_target_failures_and_failed_work_accounting() {
    use rmagna::file_api::{FileLoadOptions, FileTask, FileTaskOptions};
    use rmagna::noise::NoiseSampling;
    use rmagna::runtime::{CausalStrategy, ControllerRegistry};
    use rmagna::task::{RunPhase, RunStatus, TaskRunOptions};
    use std::{
        collections::BTreeMap,
        process::Command,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    let root = std::env::temp_dir().join(format!("rmagna-path-budget-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let source = "H 0\nH 0\nH 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nI 2";
    std::fs::write(root.join("physical.magna"), source).unwrap();
    std::fs::write(root.join("task.json"), r#"{"sample_records":[0]}"#).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", move |bits: &[bool]| {
            counter.fetch_add(1, Ordering::Relaxed);
            Ok(bits.to_vec())
        })
        .unwrap();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            let load = |cap| {
                let mut options = FileTaskOptions::default();
                options.evaluation.runtime.compile.compiler_strategy = strategy;
                options.evaluation.runtime.compile.max_path_bits = cap;
                options.evaluation.runtime.strategy = causal;
                FileTask::load(
                    &root.join("physical.magna"),
                    Some(&root.join("task.json")),
                    &registry,
                    NoiseSampling::Physical,
                    options,
                    &FileLoadOptions::default(),
                    BTreeMap::new(),
                )
                .unwrap()
            };
            let run = TaskRunOptions {
                shots: 3,
                seed: 731,
                ..Default::default()
            };
            let good = load(4);
            let baseline = good.run(&run).unwrap();
            assert_eq!(baseline.raw_result().status, RunStatus::Complete);
            let before = calls.load(Ordering::Relaxed);
            let bad = load(3);
            let failed = bad.run(&run).unwrap();
            let raw = failed.raw_result();
            assert_eq!(raw.status, RunStatus::Failed);
            assert_eq!(raw.completed, 0);
            assert_eq!(raw.execution_costs.decoder.attempted, 0);
            assert!(raw.execution_costs.compilation.failed > 0);
            assert_eq!(calls.load(Ordering::Relaxed), before);
            assert!(raw.failures[0].message.contains("max_path_bits = 3"));
            let report = failed.to_json().unwrap();
            assert_eq!(report["options"]["compile"]["max_path_bits"], 3);
            for key in ["circuit", "task", "controllers", "sampling"] {
                assert_eq!(
                    failed.metadata()["fingerprints"][key],
                    baseline.metadata()["fingerprints"][key]
                );
            }
            assert_ne!(
                failed.metadata()["fingerprints"]["options"],
                baseline.metadata()["fingerprints"]["options"]
            );
        }
    }
    std::fs::write(root.join("physical.magna"), "I 2").unwrap();
    std::fs::write(root.join("ideal.magna"), "H 0\nH 0\nH 0\nH 0").unwrap();
    std::fs::write(
        root.join("task.json"),
        r#"{"outputs":[0],"ideal_state":{"circuit":"ideal.magna","outputs":[0]}}"#,
    )
    .unwrap();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let mut options = FileTaskOptions::default();
        options.evaluation.runtime.compile.max_path_bits = 3;
        options.evaluation.runtime.compile.compiler_strategy = strategy;
        let file = FileTask::load(
            &root.join("physical.magna"),
            Some(&root.join("task.json")),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            options,
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap();
        let report = file.run(&TaskRunOptions::default()).unwrap();
        let raw = report.raw_result();
        assert_eq!(raw.failures[0].phase, RunPhase::Preparation);
        assert_eq!(raw.completed, 0);
        assert_eq!(raw.execution_costs.compilation.failed, 1);
        assert_eq!(raw.execution_costs.query.attempted, 0);
        assert!(raw.failures[0].message.contains("max_path_bits = 3"));
        for (cap, code) in [("3", 1), ("4", 0)] {
            let out = Command::new(env!("CARGO_BIN_EXE_rmagna"))
                .current_dir(&root)
                .args([
                    "run",
                    "physical.magna",
                    "--task",
                    "task.json",
                    "--compiler",
                    match strategy {
                        CompilerStrategy::ParityCopies => "parity_copies",
                        CompilerStrategy::SharedCommutingLayer => "shared",
                    },
                    "--max-path-bits",
                    cap,
                    "--json",
                ])
                .output()
                .unwrap();
            assert_eq!(
                out.status.code(),
                Some(code),
                "{}",
                String::from_utf8_lossy(&out.stdout)
            );
            let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(
                value["options"]["compile"]["max_path_bits"],
                cap.parse::<u64>().unwrap()
            );
            if code == 0 {
                assert_eq!(value["completed"], 1);
            }
        }
    }
    std::fs::write(
        root.join("decoder.py"),
        "raise RuntimeError('decoder imported')\n",
    )
    .unwrap();
    std::fs::write(
        root.join("controllers.json"),
        r#"{"schema_version":1,"bindings":{"copy":{"import":"decoder:decode","version":"1"}}}"#,
    )
    .unwrap();
    for invalid in ["-1", "1.5", "18446744073709551616"] {
        let out = Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&root)
            .args([
                "run",
                "physical.magna",
                "--controllers",
                "controllers.json",
                "--max-path-bits",
                invalid,
                "--json",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["failures"][0]["phase"], "arguments");
    }
}

#[test]
fn noncommuting_phases_match_independent_exact_probabilities_at_raw_path_thresholds() {
    // H T H T H sends Z to a Bloch vector with z=1/2, hence P(0)=3/4.
    // Three physical H gates; shared lowering adds two H gates to copy its
    // selected reference observation. Other phase copies use CX gates.
    let arithmetic = ScalarArithmetic::default();
    let source = UnitaryCircuit::new(
        3,
        vec![
            UnitaryGate::Clifford(CliffordGate::H(0)),
            UnitaryGate::T { qubit: 0, turns: 1 },
            UnitaryGate::Clifford(CliffordGate::H(0)),
            UnitaryGate::T { qubit: 0, turns: 1 },
            UnitaryGate::Clifford(CliffordGate::H(0)),
        ],
    )
    .unwrap();
    let input = BitVector::zeros(3).unwrap();
    for (strategy, raw) in [
        (CompilerStrategy::ParityCopies, 3),
        (CompilerStrategy::SharedCommutingLayer, 5),
    ] {
        assert!(
            matches!(source.compile_with_path_limit(&input,&arithmetic,strategy,raw-1),Err(UnitaryError::RawPathLimit{limit,required})if limit==raw-1&&required==raw)
        );
        let bounded = source
            .compile_with_path_limit(&input, &arithmetic, strategy, raw)
            .unwrap();
        let unrestricted = source
            .compile_with_strategy(&input, &arithmetic, strategy)
            .unwrap();
        for compiled in [&bounded, &unrestricted] {
            assert_eq!(compiled.compilation().raw_path_variables, raw);
            let row = BitVector::from_indices(compiled.width(), [0]).unwrap();
            for (outcome, numerator) in [(false, 3), (true, 1)] {
                let wanted = arithmetic
                    .scalar([numerator.into(), 0.into(), 0.into(), 0.into()], -2)
                    .unwrap();
                let result = compiled
                    .probability_z(
                        std::slice::from_ref(&row),
                        &BitVector::from_indices(1, outcome.then_some(0)).unwrap(),
                        256,
                        &arithmetic,
                    )
                    .unwrap();
                assert_eq!(result.value, wanted);
            }
        }
    }
}

#[test]
fn nonzero_computational_inputs_add_no_initial_support_paths() {
    let arithmetic = ScalarArithmetic::default();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let source =
            UnitaryCircuit::new(65, vec![UnitaryGate::Clifford(CliffordGate::H(0)); 8]).unwrap();
        let input = BitVector::from_indices(65, [64]).unwrap();
        assert!(matches!(
            source.compile_with_path_limit(&input, &arithmetic, strategy, 7),
            Err(UnitaryError::RawPathLimit { required: 8, .. })
        ));
        let compiled = source
            .compile_with_path_limit(&input, &arithmetic, strategy, 8)
            .unwrap();
        assert_eq!(compiled.compilation().raw_path_variables, 8);
        assert_eq!(compiled.compilation().path_variables, 0);
        let rows = [
            BitVector::from_indices(65, [0]).unwrap(),
            BitVector::from_indices(65, [64]).unwrap(),
        ];
        assert_eq!(
            compiled
                .probability_z(
                    &rows,
                    &BitVector::from_indices(2, [1]).unwrap(),
                    16,
                    &arithmetic
                )
                .unwrap()
                .value,
            rmagna::scalar::ExactScalar::one()
        );
        let source = UnitaryCircuit::new(
            3,
            vec![
                UnitaryGate::Clifford(CliffordGate::X(1)),
                UnitaryGate::Clifford(CliffordGate::S { qubit: 2, turns: 1 }),
            ],
        )
        .unwrap();
        let compiled = source
            .compile_with_path_limit(
                &BitVector::from_indices(3, [0, 2]).unwrap(),
                &arithmetic,
                strategy,
                0,
            )
            .unwrap();
        assert_eq!(compiled.compilation().raw_path_variables, 0);
        let rows = (0..3)
            .map(|j| BitVector::from_indices(3, [j]).unwrap())
            .collect::<Vec<_>>();
        for (outcomes, wanted) in [
            (
                BitVector::from_indices(3, [0, 1, 2]).unwrap(),
                rmagna::scalar::ExactScalar::one(),
            ),
            (
                BitVector::zeros(3).unwrap(),
                rmagna::scalar::ExactScalar::zero(),
            ),
        ] {
            assert_eq!(
                compiled
                    .probability_z(&rows, &outcomes, 16, &arithmetic)
                    .unwrap()
                    .value,
                wanted
            );
        }
    }
}
