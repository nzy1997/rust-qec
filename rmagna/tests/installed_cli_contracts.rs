// SPDX-License-Identifier: MPL-2.0
// Typed installed-consumer equivalents of itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_installed_interface.py and test_user_interface.py.
// No pip wheel, CPython namespace or Python seed trajectory is claimed.
#[path = "support/build_artifacts.rs"]
mod build_artifacts;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

struct Installed {
    root: PathBuf,
    binary: PathBuf,
    calls: usize,
}
impl Installed {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "rmagna-installed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let source = Path::new(env!("CARGO_BIN_EXE_rmagna"));
        let binary = root.join("rmagna".to_owned() + std::env::consts::EXE_SUFFIX);
        std::fs::copy(source, &binary).unwrap();
        assert_eq!(
            Sha256::digest(std::fs::read(source).unwrap()),
            Sha256::digest(std::fs::read(&binary).unwrap())
        );
        assert!(!root.starts_with(env!("CARGO_MANIFEST_DIR")));
        let raw = include_bytes!("data/original-installed-inputs.json");
        assert_eq!(
            format!("{:x}", Sha256::digest(raw)),
            "6e8f8078f8b9e8990043d8909d7f5638f036ccc6fe7f5be518dc6b3c67ed6d9e"
        );
        let inputs: Value = serde_json::from_slice(raw).unwrap();
        let reference: Value = serde_json::from_str(include_str!(
            "../../benchmarks/magna_reproduction/reference.json"
        ))
        .unwrap();
        assert_eq!(inputs["files"].as_object().unwrap().len(), 31);
        assert_eq!(inputs["source_sha256"].as_object().unwrap().len(), 33);
        for (name, digest) in inputs["source_sha256"].as_object().unwrap() {
            assert_eq!(*digest, reference["upstream"]["files"][name]);
        }
        assert_eq!(
            inputs["upstream_commit"],
            "56847478c840395735146ef1e1ccdc3d8aedc10f"
        );
        for (name, value) in inputs["files"].as_object().unwrap() {
            let relative = name
                .strip_prefix("tests/fixtures/")
                .or_else(|| name.strip_prefix("examples/"))
                .unwrap();
            let path = root.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let text = value.as_str().unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(text.as_bytes())),
                inputs["source_sha256"][name].as_str().unwrap()
            );
            std::fs::write(path, text).unwrap();
        }
        // A local source-import fallback would fail loudly. The decoder worker
        // is embedded and imports only the explicitly selected local decoder.
        std::fs::create_dir(root.join("magna")).unwrap();
        std::fs::write(
            root.join("magna/__init__.py"),
            "raise AssertionError('unexpected source-package fallback')\n",
        )
        .unwrap();
        Self {
            root,
            binary,
            calls: 0,
        }
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.root.join(name), text).unwrap();
    }
    fn compiler(&self, artifacts: &build_artifacts::Artifacts) -> Command {
        let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
        command
            .current_dir(&self.root)
            .arg("--edition=2024")
            .arg("--extern")
            .arg(format!("rmagna={}", artifacts.library.display()));
        for directory in &artifacts.dependencies {
            command
                .arg("-L")
                .arg(format!("dependency={}", directory.display()));
        }
        command
    }
    fn command(&self, program: &Path) -> Command {
        let mut command = Command::new(program);
        command.current_dir(&self.root).env_clear();
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        command
            .env("HOME", &self.root)
            .env("TMPDIR", &self.root)
            .env("PYTHONDONTWRITEBYTECODE", "1");
        command
    }
    fn output(&mut self, args: &[&str]) -> Output {
        self.calls += 1;
        let stdout = self.root.join(format!("stdout-{}", self.calls));
        let stderr = self.root.join(format!("stderr-{}", self.calls));
        let mut child = self
            .command(&self.binary)
            .args(args)
            .stdout(Stdio::from(std::fs::File::create(&stdout).unwrap()))
            .stderr(Stdio::from(std::fs::File::create(&stderr).unwrap()))
            .spawn()
            .unwrap();
        let start = Instant::now();
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("installed command exceeded bounded smoke timeout: {args:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Output {
            status: child.wait().unwrap(),
            stdout: std::fs::read(stdout).unwrap(),
            stderr: std::fs::read(stderr).unwrap(),
        }
    }
    fn json(&mut self, args: &[&str], code: i32) -> Value {
        let output = self.output(args);
        assert_eq!(
            output.status.code(),
            Some(code),
            "{args:?}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
            panic!("{args:?}: {e}: {}", String::from_utf8_lossy(&output.stdout))
        })
    }
    fn complete(&mut self, args: &[&str], shots: u64) -> Value {
        let result = self.json(args, 0);
        assert_eq!(result["status"], "complete");
        assert_eq!(result["completed"], shots);
        assert_eq!(result["failed"], 0);
        assert_eq!(result["not_attempted"], 0);
        result
    }
}
impl Drop for Installed {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn number(value: &Value) -> f64 {
    value["value"].as_f64().unwrap()
}
fn close(found: f64, expected: f64) {
    assert!((found - expected).abs() < 1e-12, "{found} != {expected}");
}

#[test]
fn original_installed_scenarios_run_from_one_binary_without_checkout_imports() {
    let mut installed = Installed::new();
    installed_console_commands(&mut installed);
    installed_record_success(&mut installed);
    installed_tutorial(&mut installed);
    installed_nonlinear_and_decoder(&mut installed);
    installed_conditional_noise(&mut installed);
    installed_progress_and_pilot(&mut installed);
    installed_terminal_modules(&mut installed);
    installed_importance_and_low_weight(&mut installed);
    installed_portable_dk(&mut installed);
    installed_missing_decoder(&mut installed);
    assert_eq!(installed.calls, 35);
}

fn installed_console_commands(installed: &mut Installed) {
    // Console commands, output file, and same-Rust-seed reproducibility.
    for mode in ["validate", "inspect"] {
        let result = installed.json(
            &[
                mode,
                "basic/parity_feedback.magna",
                "--task",
                "basic/parity_feedback.task.json",
                "--json",
            ],
            0,
        );
        assert_eq!(
            result["status"],
            if mode == "validate" {
                "valid"
            } else {
                "complete"
            }
        );
    }
    let args = [
        "run",
        "basic/parity_feedback.magna",
        "--task",
        "basic/parity_feedback.task.json",
        "--shots",
        "8",
        "--seed",
        "19",
        "--json",
        "--output",
        "result.json",
    ];
    let first = installed.complete(&args, 8);
    assert_eq!(
        first,
        serde_json::from_slice::<Value>(
            &std::fs::read(installed.root.join("result.json")).unwrap()
        )
        .unwrap()
    );
    let repeated = installed.complete(&args[..args.len() - 2], 8);
    for key in [
        "estimates",
        "estimates_scientific",
        "sampled_records",
        "fingerprints",
        "weights",
    ] {
        assert_eq!(first[key], repeated[key], "same Rust seed: {key}");
    }
    eprintln!("installed:console validate/inspect/run;8-shot seed19 output file/replay");
}

fn installed_record_success(installed: &mut Installed) {
    // Complete original record-success example, using the Rust enumeration schema.
    installed.put(
        "record.magna",
        "X_ERROR(1/5) 0\nM(1/7) 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    installed.put("record.json", r#"{"error_observables":[0]}"#);
    let result = installed.json(
        &[
            "enumerate",
            "record.magna",
            "--task",
            "record.json",
            "--max-weight",
            "2",
            "--json",
        ],
        0,
    );
    assert_eq!(result["status"], "complete");
    close(number(&result["result"]["masses"]["error"]), 2.0 / 7.0);
    installed.complete(
        &[
            "run",
            "record.magna",
            "--task",
            "record.json",
            "--shots",
            "8",
            "--seed",
            "3",
            "--json",
        ],
        8,
    );
    eprintln!("installed:record-success exact error2/7 plus8-shot run");
}

fn installed_tutorial(installed: &mut Installed) {
    // Tutorial circuit and all actual T interference arithmetic outside source.
    let inputs: Value =
        serde_json::from_str(include_str!("data/original-installed-inputs.json")).unwrap();
    let guide = &inputs["docs"]["docs/software/getting-started.md"];
    assert_eq!(guide["python_blocks"].as_array().unwrap().len(), 2);
    installed.put("example.magna", guide["text_blocks"][0].as_str().unwrap());
    for circuit in ["example.magna", "basic/t_interference.magna"] {
        let result = installed.complete(
            &[
                "run",
                circuit,
                "--shots",
                "4",
                "--seed",
                "7",
                "--probability-preparation",
                "catalogue",
                "--json",
            ],
            4,
        );
        close(
            result["estimates"]["acceptance"].as_f64().unwrap(),
            (2.0 + 2.0f64.sqrt()) / 4.0,
        );
        assert_eq!(result["standard_errors"]["acceptance"], 0.0);
    }
    eprintln!(
        "installed:tutorial2Python examples mapped to typed API/CLI;bundled catalogue T interference"
    );
}

fn installed_nonlinear_and_decoder(installed: &mut Installed) {
    // External majority binding and the complete nonlinear selection/inverse cases.
    let decoder = [
        "--controllers",
        "basic/controllers.json",
        "--module-root",
        "basic/decoder_plugin",
    ];
    let mut args = vec![
        "run",
        "basic/majority_feedback.magna",
        "--task",
        "basic/majority_feedback.task.json",
        "--shots",
        "16",
        "--seed",
        "7",
        "--json",
    ];
    args.extend(decoder);
    let result = installed.complete(&args, 16);
    assert_eq!(result["estimates"]["acceptance"], 1.0);
    let error = result["estimates"]["error"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&error));
    for mode in ["validate", "inspect", "enumerate", "run"] {
        let mut args = vec![
            mode,
            "basic/nonlinear_postselection.magna",
            "--task",
            "basic/nonlinear_postselection.task.json",
            "--json",
        ];
        args.extend(decoder);
        if mode == "enumerate" {
            args.extend(["--max-weight", "0"]);
        }
        if mode == "run" {
            args.extend(["--shots", "32", "--seed", "7"]);
        }
        let result = installed.json(&args, 0);
        if mode == "enumerate" {
            assert_eq!(result["status"], "complete");
            assert_eq!(number(&result["result"]["masses"]["accepted"]), 0.75);
            assert_eq!(number(&result["result"]["masses"]["error"]), 0.25);
            assert_eq!(number(&result["result"]["masses"]["moments"][0]), 0.25);
            close(
                number(&result["result"]["conditional_error_within_enumerated_sector"]),
                1.0 / 3.0,
            );
        }
        if mode == "run" {
            assert_eq!(result["status"], "complete");
            assert_eq!(result["completed"], 32);
            let acceptance = result["estimates"]["acceptance"].as_f64().unwrap();
            assert!(acceptance > 0.0 && acceptance < 1.0);
            let repeated = installed.json(&args, 0);
            assert_eq!(result["estimates"], repeated["estimates"]);
        }
    }
    let nonlinear =
        std::fs::read_to_string(installed.root.join("basic/nonlinear_postselection.magna"))
            .unwrap();
    installed.put(
        "inverse.magna",
        &nonlinear.replace(
            "POSTSELECT ctrl[0]",
            "CONTROL[NOT] ctrl[0] -> ctrl[1]\nPOSTSELECT ctrl[1]",
        ),
    );
    let mut inverse = vec![
        "enumerate",
        "inverse.magna",
        "--task",
        "basic/nonlinear_postselection.task.json",
        "--max-weight",
        "0",
        "--json",
    ];
    inverse.extend(decoder);
    let result = installed.json(&inverse, 0);
    assert_eq!(number(&result["result"]["masses"]["accepted"]), 0.25);
    assert_eq!(number(&result["result"]["masses"]["error"]), 0.25);
    for bit in [0, 1] {
        installed.put(
            "bad.magna",
            &nonlinear.replace("POSTSELECT ctrl[0]", &format!("POSTSELECT({bit}) ctrl[0]")),
        );
        let mut bad = vec!["validate", "bad.magna", "--json"];
        bad.extend(decoder);
        assert_eq!(installed.json(&bad, 2)["status"], "invalid");
    }
    eprintln!(
        "installed:external majority16;nonlinear4commands,32-shot replay,exact3/4,1/4,conditional1/3;inverse1/4;two malformedselectors"
    );
}

fn installed_conditional_noise(installed: &mut Installed) {
    // Conditional noise uses the same parsed task; enumeration is an independent law.
    installed.put(
        "conditional.magna",
        "H 0\nM 0\nX_ERROR(1/3) rec[-1] 1\nM 1\nX_ERROR(1/2) rec[-1] 2",
    );
    installed.put(
        "conditional.json",
        r#"{"outputs":[2],"ideal_state":"basic/zero.magna"}"#,
    );
    installed.complete(
        &[
            "run",
            "conditional.magna",
            "--task",
            "conditional.json",
            "--shots",
            "64",
            "--seed",
            "3",
            "--json",
        ],
        64,
    );
    let result = installed.json(
        &[
            "enumerate",
            "conditional.magna",
            "--task",
            "conditional.json",
            "--max-weight",
            "2",
            "--json",
        ],
        0,
    );
    assert_eq!(number(&result["result"]["masses"]["accepted"]), 1.0);
    close(number(&result["result"]["masses"]["error"]), 1.0 / 12.0);
    eprintln!(
        "installed:conditional64-shot run;exact acceptance1,error1/12;private Native preparation counters are separate R09 scope"
    );
}

fn installed_progress_and_pilot(installed: &mut Installed) {
    installed.put("pilot.magna", "X_ERROR(1/2) 0");
    installed.put("ideal.magna", "I 0");
    let progress = installed.output(&[
        "run",
        "pilot.magna",
        "--ideal-state",
        "ideal.magna",
        "--shots",
        "32",
        "--seed",
        "13",
        "--progress-every",
        "16",
        "--json",
    ]);
    assert!(progress.status.success());
    let result: Value = serde_json::from_slice(&progress.stdout).unwrap();
    assert_eq!(result["efficiency"]["kappa"], 1.0);
    assert_eq!(
        String::from_utf8(progress.stderr)
            .unwrap()
            .matches("kappa=")
            .count(),
        2
    );
    let pilot = installed.json(
        &[
            "compare-samplers",
            "pilot.magna",
            "--ideal-state",
            "ideal.magna",
            "--pilot-shots",
            "32",
            "--seed",
            "13",
            "--json",
        ],
        0,
    );
    assert_eq!(pilot["status"], "complete");
    assert_eq!(pilot["automatic_switching"], false);
    assert!(pilot["gamma"].as_f64().unwrap() > 0.0);
    eprintln!("installed:32-shot progress,two kappa lines,and32x2 pilot,no automatic switching");
}

fn installed_terminal_modules(installed: &mut Installed) {
    installed.put("ideal.magna", "I 0");
    // Three-file terminal recovery, the original auxiliary projection and mixed target.
    let terminal = [
        "run",
        "terminal-recovery/physical.magna",
        "--task",
        "terminal-recovery/task.json",
        "--controllers",
        "terminal-recovery/controllers.json",
        "--module-root",
        "basic/decoder_plugin",
        "--shots",
        "4",
        "--seed",
        "7",
        "--json",
    ];
    let result = installed.complete(&terminal, 4);
    assert_eq!(result["estimates"]["acceptance"], 1.0);
    assert_eq!(result["estimates"]["error"], 0.0);
    let raw = installed.complete(
        &[
            "run",
            "terminal-recovery/physical.magna",
            "--ideal-state",
            "terminal-recovery/ideal.magna",
            "--json",
        ],
        1,
    );
    assert_eq!(raw["estimates"]["error"], 1.0);
    installed.put("four.magna", "I 0 1 2 3");
    installed.put("auxiliary.magna", "H 0 1 2 3 4\nCCZ 0 3 4\nCCZ 1 2 4\nH 4");
    installed.put("projected.json",r#"{"ideal_state":{"circuit":"auxiliary.magna","outputs":[0,1,2,3],"postselection":{"4":0}}}"#);
    let projected = installed.complete(
        &["run", "four.magna", "--task", "projected.json", "--json"],
        1,
    );
    close(projected["estimates"]["error"].as_f64().unwrap(), 0.9);
    installed.put("bell.magna", "H 0\nCX 0 1");
    installed.put(
        "mixed.json",
        r#"{"ideal_state":{"circuit":"bell.magna","outputs":[1]}}"#,
    );
    let mixed = installed.complete(&["run", "ideal.magna", "--task", "mixed.json", "--json"], 1);
    assert_eq!(mixed["estimates"]["error"], 0.5);
    let recovered = installed.complete(
        &[
            "run",
            "terminal-recovery/physical.magna",
            "--controllers",
            "terminal-recovery/controllers.json",
            "--module-root",
            "basic/decoder_plugin",
            "--ideal-state",
            "terminal-recovery/ideal.magna",
            "--recovery",
            "terminal-recovery/recovery.magna",
            "--shots",
            "2",
            "--json",
        ],
        2,
    );
    assert_eq!(recovered["estimates"]["error"], 0.0);
    eprintln!("installed:three-file4shots/raw/auxiliary0.9/mixed0.5/explicit recovery2shots");
}

fn installed_importance_and_low_weight(installed: &mut Installed) {
    let inputs: Value =
        serde_json::from_str(include_str!("data/original-installed-inputs.json")).unwrap();
    let sampling = &inputs["docs"]["docs/software/sampling.md"];
    assert_eq!(sampling["python_blocks"].as_array().unwrap().len(), 3);
    installed.complete(
        &[
            "run",
            "importance-sampling/physical.magna",
            "--task",
            "importance-sampling/task.json",
            "--shots",
            "1000",
            "--seed",
            "7",
            "--json",
        ],
        1000,
    );
    let importance = installed.complete(
        &[
            "run",
            "importance-sampling/physical.magna",
            "--task",
            "importance-sampling/task.json",
            "--noise-proposal-scale",
            "10",
            "--shots",
            "1000",
            "--seed",
            "7",
            "--json",
        ],
        1000,
    );
    assert_eq!(importance["sampling"]["method"], "importance");
    assert_eq!(importance["proposal_summary"]["scaled_categories"], 1);
    let ess = importance["weights"]["ess"].as_f64().unwrap();
    assert!(ess > 0.0 && ess <= 1000.0);
    for (key, expected) in [("error", 0.001), ("acceptance", 1.0)] {
        let error = importance["standard_errors"][key].as_f64().unwrap();
        assert!((importance["estimates"][key].as_f64().unwrap() - expected).abs() < 4.0 * error);
    }
    installed.complete(
        &[
            "run",
            "importance-sampling/physical.magna",
            "--task",
            "importance-sampling/task.json",
            "--sampling",
            "importance-sampling/sampling.json",
            "--shots",
            "64",
            "--seed",
            "7",
            "--json",
        ],
        64,
    );
    let guide_pilot = installed.json(
        &[
            "compare-samplers",
            "importance-sampling/physical.magna",
            "--task",
            "importance-sampling/task.json",
            "--pilot-shots",
            "256",
            "--seed",
            "7",
            "--json",
        ],
        0,
    );
    assert!(matches!(
        guide_pilot["status"].as_str(),
        Some("complete" | "inconclusive")
    ));
    assert_eq!(guide_pilot["automatic_switching"], false);
    assert_eq!(guide_pilot["pilot_shots_per_channel"], 256);
    installed.put(
        "low-weight.magna",
        "X_ERROR(1/10) 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
    );
    let low = installed.json(
        &[
            "enumerate",
            "low-weight.magna",
            "--max-weight",
            "1",
            "--json",
        ],
        0,
    );
    close(number(&low["result"]["masses"]["accepted"]), 0.9);
    assert_eq!(
        low["result"]["covered_probability"],
        json!({"numerator":"1","denominator":"1"})
    );
    eprintln!(
        "installed:importance guide3examples,1000-shot4SE checks,64-shot console;low-weight acceptance0.9 and full coverage"
    );
}

fn installed_portable_dk(installed: &mut Installed) {
    // The small original DK task and decoder are copied as data, not an environment.
    installed.complete(
        &[
            "run",
            "dk/physical.magna",
            "--task",
            "dk/task.json",
            "--controllers",
            "dk/controllers.json",
            "--shots",
            "4",
            "--seed",
            "7",
            "--output",
            "dk-result.json",
            "--json",
        ],
        4,
    );
    eprintln!("installed:portable original DK directory/local decoder/universal Rust runner4shots");
}

fn installed_missing_decoder(installed: &mut Installed) {
    let args = [
        "run",
        "basic/majority_feedback.magna",
        "--task",
        "basic/majority_feedback.task.json",
        "--shots",
        "16",
        "--seed",
        "7",
        "--json",
        "--controllers",
        "basic/controllers.json",
        "--module-root",
        "basic/decoder_plugin",
    ];
    // Missing external code is invalid input before any draw, with no checkout fallback.
    std::fs::remove_file(
        installed
            .root
            .join("basic/decoder_plugin/magna_example_decoders.py"),
    )
    .unwrap();
    for target in [
        None,
        Some("no_such_package.module:decode"),
        Some("json:no_such_function"),
    ] {
        if let Some(target) = target {
            installed.put("basic/controllers.json",&json!({"schema_version":1,"bindings":{"majority":{"import":target,"version":"1"}}}).to_string());
        }
        let invalid = installed.json(&args, 2);
        assert_eq!(invalid["status"], "invalid");
        assert!(invalid["completed"].is_null());
        assert!(
            invalid["error"]
                .as_str()
                .unwrap()
                .contains("controller process")
        );
    }
    eprintln!(
        "installed:removed decoder and two missing import/function targets rejected before draws;{} commands executed",
        installed.calls
    );
}

#[test]
fn downstream_rust_consumer_executes_both_tutorial_apis_and_embedded_catalogue() {
    let installed = Installed::new();
    let input: Value =
        serde_json::from_str(include_str!("data/original-installed-inputs.json")).unwrap();
    installed.put(
        "example.magna",
        input["docs"]["docs/software/getting-started.md"]["text_blocks"][0]
            .as_str()
            .unwrap(),
    );
    let artifacts = build_artifacts::rmagna().unwrap();
    assert!(!installed.root.starts_with(&artifacts.profile));
    let source = r#"
use rmagna::{file_api::{FileTask,FileLoadOptions,FileTaskOptions},runtime::ControllerRegistry,
    noise::NoiseSampling,task::TaskRunOptions,scalar::ScalarArithmetic,field::FieldArithmetic,
    instrument::{OriginalInstrument,OriginalOperation,OriginalGate,OriginalPauli,OriginalAssignment,OriginalLimits},
    clifford::CliffordCircuit,gf2::BitVector,unitary::CompilerStrategy};
use std::{path::Path,collections::BTreeMap};
fn main(){
 let arithmetic=ScalarArithmetic::default();
 let task=FileTask::load(Path::new("example.magna"),None,&ControllerRegistry::default(),NoiseSampling::Physical,FileTaskOptions::default(),&FileLoadOptions::default(),BTreeMap::new()).unwrap();
 let compiled=task.compile().unwrap();
 let result=compiled.run(&TaskRunOptions{shots:8,seed:7,..Default::default()}).unwrap();
 assert_eq!(result.raw_result().completed,8);
 let expected=arithmetic.scalar([2.into(),1.into(),0.into(),(-1).into()],-2).unwrap();
 assert_eq!(result.raw_result().statistics.as_ref().unwrap().accepted,FieldArithmetic::default().from_scalar(&expected).unwrap());
 let ir=OriginalInstrument::new(vec![0],vec![0],CliffordCircuit::computational(BitVector::zeros(1).unwrap()).unwrap(),vec![
  OriginalOperation{id:"h0".into(),gate:OriginalGate::H(0)},
  OriginalOperation{id:"t0".into(),gate:OriginalGate::Phase{wires:vec![0],turns:1}},
  OriginalOperation{id:"h1".into(),gate:OriginalGate::H(0)},
  OriginalOperation{id:"m0".into(),gate:OriginalGate::Measure{factors:vec![(0,OriginalPauli::Z)],inverted:false,record:"m".into()}}
 ],vec![],OriginalLimits::default(),&arithmetic).unwrap();
 let branch=ir.compile(&OriginalAssignment{true_values:vec![("m".into(),false)],..Default::default()},CompilerStrategy::ParityCopies,&arithmetic).unwrap();
 assert_eq!(branch.probability(&arithmetic).unwrap(),expected);
 let catalogue=rmagna::catalogue::builtin_phase_catalogue().unwrap();
 assert_eq!(catalogue.entries().count(),15);
 assert_eq!(rmagna::circuit::parse_circuit("I 0",&Default::default()).unwrap().format_version(),"magna-0.1");
 println!("file compile/run and original-operation tutorial exact values;15 embedded certificates;format magna-0.1");
}
"#;
    installed.put("consumer.rs", source);
    let binary = installed
        .root
        .join("consumer".to_owned() + std::env::consts::EXE_SUFFIX);
    let compile = installed
        .compiler(&artifacts)
        .arg("--crate-name=installed_consumer")
        .arg(installed.root.join("consumer.rs"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = installed.command(&binary).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8(run.stdout)
            .unwrap()
            .contains("15 embedded certificates")
    );
    // Public modules are explicit and signatures are checked positionally by
    // Rust. Neither keyword-argument reflection nor CPython reexports is implied.
    for (name, code, diagnostic) in [
        ("namespace", "use rmagna::run;", "E0432"),
        (
            "wrong_input",
            "use rmagna::file_api::{FileTask,CompiledFileTask};fn misuse(compiled:&CompiledFileTask){FileTask::load(compiled,None,&Default::default(),rmagna::noise::NoiseSampling::Physical,Default::default(),&Default::default(),Default::default());}",
            "E0308",
        ),
        ("retired_entry", "use rmagna::make_plan;", "E0432"),
    ] {
        let path = installed.root.join(format!("{name}.rs"));
        std::fs::write(&path, code).unwrap();
        let result = installed
            .compiler(&artifacts)
            .args(["--crate-type=lib", "--emit=metadata"])
            .arg(path)
            .arg("-o")
            .arg(installed.root.join(format!("{name}.rmeta")))
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains(diagnostic)
        );
    }
    std::fs::remove_file(&binary).unwrap();
    assert_eq!(
        installed.command(&binary).output().unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    eprintln!(
        "installed downstream:one compiled/executed consumer,two exact tutorial values,15 builtin certificates,3 checked negative API probes,missing executable has NotFound;shared rlibs reused,no Cargo/environment copies"
    );
}
