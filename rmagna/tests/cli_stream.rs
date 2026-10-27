use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn controllers(&self, body: &str, extra: Value) {
        self.put("decoder.py", &format!("import json\nfrom pathlib import Path\ntry:\n import magna\nexcept ImportError:\n pass\nelse:\n raise RuntimeError('native magna import was allowed')\ndef decode(bits, config):\n with open('calls.txt', 'a') as out:\n  out.write(json.dumps(bits)+'\\n')\n{body}\n"));
        self.put("controllers.json", &json!({"schema_version":1,"bindings":{"copy":{"import":"decoder:decode","version":"stream-v1","config":extra}}}).to_string());
        self.put("circuit.magna", "H 0\nT 0\nH 0\nX_ERROR(1/7) 0\nM(1/11) 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCH ctrl[0] 2\nI 1");
        self.put("ideal.magna", "I 0");
        self.put("recovery.magna", "I 2");
        self.put("task.json",r#"{"outputs":[2],"ideal_state":"ideal.magna","recovery":"recovery.magna","sample_records":[0],"observables":[[1,0],[1,1]]}"#);
    }
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let f = Self(std::env::temp_dir().join(format!(
            "rmagna-cli-stream-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&f.0).unwrap();
        f.put(
            "circuit.magna",
            "H 0\nT 0\nH 0\nM(1/7) 0\nI 1\nOBSERVABLE_INCLUDE(0) rec[-1]",
        );
        f.put(
            "task.json",
            r#"{"sample_records":[0],"error_observables":[0]}"#,
        );
        f
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn stream(&self, directory: &str, extra: &[&str]) -> Output {
        let mut args = vec![
            "stream",
            "circuit.magna",
            "--task",
            "task.json",
            "--shots",
            "7",
            "--seed",
            "18446744073709551615",
            "--checkpoint-dir",
            directory,
            "--json",
        ];
        args.extend_from_slice(extra);
        self.run(&args)
    }
    fn checkpoint(&self, value: &Value) -> String {
        value["checkpoint"]["latest_committed"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn value(out: Output, code: i32) -> Value {
    assert_eq!(
        out.status.code(),
        Some(code),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn exact(f: &Files, report: &Value) -> Value {
    let saved: Value =
        serde_json::from_slice(&std::fs::read(f.0.join(f.checkpoint(report))).unwrap()).unwrap();
    let mut raw: Value = serde_json::from_str(saved["checkpoint"].as_str().unwrap()).unwrap();
    for key in ["sampling_seconds", "total_seconds"] {
        raw.as_object_mut().unwrap().remove(key);
    }
    raw["meter"].as_object_mut().unwrap().remove("elapsed");
    for key in ["compilation", "query", "decoder", "support"] {
        raw["meter"][key].as_object_mut().unwrap().remove("seconds");
    }
    raw
}
#[test]
fn fresh_process_resumption_preserves_exact_rng_statistics_rows_and_logical_work() {
    for sampler in ["affine_support", "exact_marginal"] {
        for compiler in ["shared", "parity_copies"] {
            for importance in [false, true] {
                for retain in [false, true] {
                    let f = Files::new();
                    let mut config = vec![
                        "--history-sampler",
                        sampler,
                        "--compiler",
                        compiler,
                        "--checkpoint-every",
                        "2",
                    ];
                    if importance {
                        config.extend(["--noise-proposal-scale", "2"]);
                    }
                    if retain {
                        config.push("--retain-records");
                    } else {
                        config.extend(["--max-record-values", "0"]);
                    }
                    let full = value(f.stream("full", &config), 0);
                    let mut batch_args = vec![
                        "run",
                        "circuit.magna",
                        "--task",
                        "task.json",
                        "--shots",
                        "7",
                        "--seed",
                        "18446744073709551615",
                        "--history-sampler",
                        sampler,
                        "--compiler",
                        compiler,
                        "--json",
                    ];
                    if importance {
                        batch_args.extend(["--noise-proposal-scale", "2"]);
                    }
                    let batch = value(f.run(&batch_args), 0);
                    for key in [
                        "estimates",
                        "estimates_scientific",
                        "standard_errors",
                        "standard_errors_scientific",
                        "efficiency",
                        "weights",
                    ] {
                        assert_eq!(full["metrics"][key], batch[key], "batch parity: {key}");
                    }
                    let mut prefix = config.clone();
                    prefix.extend(["--stop-after", "0"]);
                    let empty = value(f.stream("empty", &prefix), 0);
                    assert_eq!(empty["status"], "paused");
                    assert_eq!(empty["completed"], 0);
                    let empty_path = f.checkpoint(&empty);
                    let mut part = config.clone();
                    part.extend(["--resume", &empty_path, "--stop-after", "3"]);
                    let prefix = value(f.stream("prefix", &part), 0);
                    assert_eq!(prefix["status"], "paused");
                    assert_eq!(prefix["completed"], 3);
                    let prefix_path = f.checkpoint(&prefix);
                    let before = std::fs::read(f.0.join(&prefix_path)).unwrap();
                    let mut rest = config.clone();
                    rest.extend(["--resume", &prefix_path]);
                    let resumed = value(f.stream("resumed", &rest), 0);
                    assert_eq!(resumed["status"], "complete");
                    assert_eq!(exact(&f, &full), exact(&f, &resumed));
                    assert_eq!(std::fs::read(f.0.join(&prefix_path)).unwrap(), before);
                    for k in [
                        "estimates",
                        "standard_errors",
                        "efficiency",
                        "weights",
                        "sampled_records",
                    ] {
                        assert_eq!(full["metrics"][k], resumed["metrics"][k], "{k}");
                    }
                    let finished = f.checkpoint(&resumed);
                    let mut again = config.clone();
                    again.extend(["--resume", &finished]);
                    let again = value(f.stream("finished", &again), 0);
                    assert_eq!(exact(&f, &again), exact(&f, &resumed));
                }
            }
        }
    }
}
#[test]
fn checkpoints_and_output_conflicts_are_rejected_before_decoder_import() {
    let f = Files::new();
    f.put("controllers.json",r#"{"schema_version":1,"bindings":{"copy":{"import":"decoder:decode","version":"v1","config":{}}}}"#);
    f.put(
        "decoder.py",
        "raise RuntimeError('decoder imported unexpectedly')\n",
    );
    std::fs::create_dir(f.0.join("occupied")).unwrap();
    let v = value(
        f.stream("occupied", &["--controllers", "controllers.json"]),
        1,
    );
    assert!(
        v["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("checkpoint directory")
    );
    f.put("output.json", "keep");
    value(
        f.stream(
            "never-created",
            &[
                "--controllers",
                "controllers.json",
                "--output",
                "output.json",
            ],
        ),
        1,
    );
    assert!(!f.0.join("never-created").exists());
    assert_eq!(
        std::fs::read_to_string(f.0.join("output.json")).unwrap(),
        "keep"
    );
    for text in ["[]", "{}", "{", "{\"schema_version\":99}"] {
        f.put("bad.json", text);
        let v = value(
            f.stream(
                "unused",
                &["--controllers", "controllers.json", "--resume", "bad.json"],
            ),
            1,
        );
        assert_eq!(v["failures"][0]["phase"], "stream_preflight");
        assert!(!f.0.join("unused").exists());
    }
}
#[test]
fn incompatible_build_config_and_loaded_content_never_advance_the_saved_prefix() {
    let f = Files::new();
    let prefix = value(f.stream("prefix", &["--stop-after", "2"]), 0);
    let path = f.checkpoint(&prefix);
    let bytes = std::fs::read(f.0.join(&path)).unwrap();
    for (key, number, directory) in [
        ("requested", 7, "numeric-count"),
        ("original_seed", u64::MAX, "numeric-seed"),
    ] {
        let mut saved: Value = serde_json::from_slice(&bytes).unwrap();
        let mut raw: Value = serde_json::from_str(saved["checkpoint"].as_str().unwrap()).unwrap();
        raw[key] = json!(number);
        saved["checkpoint"] = json!(raw.to_string());
        f.put("numeric-header.json", &saved.to_string());
        let v = value(f.stream(directory, &["--resume", "numeric-header.json"]), 1);
        assert_eq!(v["failures"][0]["phase"], "stream_preflight");
        assert!(!f.0.join(directory).exists());
    }
    let mut saved: Value = serde_json::from_slice(&bytes).unwrap();
    saved["executable_sha256"] = json!("changed");
    f.put("changed.json", &saved.to_string());
    let v = value(f.stream("build", &["--resume", "changed.json"]), 1);
    assert_eq!(v["failures"][0]["phase"], "stream_preflight");
    let v = value(
        f.stream(
            "settings",
            &["--resume", &path, "--history-sampler", "exact_marginal"],
        ),
        1,
    );
    assert_eq!(v["failures"][0]["phase"], "stream_preflight");
    f.put(
        "circuit.magna",
        "X 0\nM 0\nI 1\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    let v = value(f.stream("content", &["--resume", &path]), 1);
    assert!(
        v["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("loaded content")
    );
    assert_eq!(std::fs::read(f.0.join(&path)).unwrap(), bytes);
}
#[test]
fn numeric_export_failure_retains_completed_counts_and_exact_terminal_state() {
    let f = Files::new();
    let v = value(f.stream("numeric", &["--numeric-work-bits", "0"]), 1);
    assert_eq!(v["completed"], 7);
    assert_eq!(v["failed"], 0);
    assert_eq!(v["export_outcome"]["status"], "failed");
    let path = f.checkpoint(&v);
    let again = value(
        f.stream("retry", &["--resume", &path, "--numeric-work-bits", "0"]),
        1,
    );
    assert_eq!(exact(&f, &v), exact(&f, &again));
    assert_eq!(again["completed"], 7);
}
#[test]
fn invalid_stream_flags_are_argument_failures_and_initial_byte_failure_draws_nothing() {
    let f = Files::new();
    for option in ["--checkpoint-every", "--max-checkpoint-bytes"] {
        value(f.stream("unused", &[option, "0"]), 2);
    }
    for mode in [
        "run",
        "inspect",
        "validate",
        "prepare",
        "enumerate",
        "compare-samplers",
    ] {
        value(
            f.run(&[mode, "circuit.magna", "--stop-after", "1", "--json"]),
            2,
        );
    }
    let v = value(f.stream("tiny", &["--max-checkpoint-bytes", "100"]), 1);
    assert_eq!(v["completed"], 0);
    assert_eq!(v["not_attempted"], 7);
    assert_eq!(v["persistence_outcome"]["status"], "failed");
}

#[test]
fn python_callbacks_target_recovery_and_finished_resumes_keep_the_original_observations() {
    for sampler in ["affine_support", "exact_marginal"] {
        let f = Files::new();
        f.controllers(" return [bits[0]]", json!({}));
        let config = [
            "--controllers",
            "controllers.json",
            "--history-sampler",
            sampler,
            "--retain-records",
            "--checkpoint-every",
            "2",
        ];
        let full = value(f.stream("full", &config), 0);
        let expected = std::fs::read_to_string(f.0.join("calls.txt")).unwrap();
        assert_eq!(expected.lines().count(), 7);
        f.put("calls.txt", "");
        let mut part = config.to_vec();
        part.extend(["--stop-after", "3"]);
        let prefix = value(f.stream("prefix", &part), 0);
        let path = f.checkpoint(&prefix);
        let mut rest = config.to_vec();
        rest.extend(["--resume", &path]);
        let resumed = value(f.stream("resumed", &rest), 0);
        assert_eq!(
            std::fs::read_to_string(f.0.join("calls.txt")).unwrap(),
            expected
        );
        assert_eq!(exact(&f, &resumed), exact(&f, &full));
        assert_eq!(resumed["execution_costs"]["decoder"]["attempted"], 7);
        let path = f.checkpoint(&resumed);
        let mut again = config.to_vec();
        again.extend(["--resume", &path]);
        let finished = value(f.stream("finished", &again), 0);
        assert_eq!(exact(&f, &finished), exact(&f, &resumed));
        assert_eq!(
            std::fs::read_to_string(f.0.join("calls.txt")).unwrap(),
            expected
        );
        // A changed target or recovery is rejected before any additional callback.
        for (file, text, directory) in [
            ("ideal.magna", "X 0", "target-change"),
            ("recovery.magna", "X 2", "recovery-change"),
        ] {
            f.put(file, text);
            let failed = value(f.stream(directory, &again), 1);
            assert!(
                failed["failures"][0]["message"]
                    .as_str()
                    .unwrap()
                    .contains("loaded content")
            );
            assert_eq!(
                std::fs::read_to_string(f.0.join("calls.txt")).unwrap(),
                expected
            );
            f.put(file, if file == "ideal.magna" { "I 0" } else { "I 2" });
        }
    }
}

#[test]
fn checkpoint_publication_failure_preserves_previous_inode_and_reports_the_actual_prefix() {
    let f = Files::new();
    f.controllers(" Path('broken/checkpoint-00000000000000000001.json').write_text('unrelated file')\n return [bits[0]]",json!({}));
    let v = value(
        f.stream(
            "broken",
            &[
                "--controllers",
                "controllers.json",
                "--checkpoint-every",
                "2",
            ],
        ),
        1,
    );
    assert_eq!(v["completed"], 2);
    assert_eq!(v["not_attempted"], 5);
    assert_eq!(v["persistence_outcome"]["status"], "failed");
    let old = exact(&f, &v);
    assert_eq!(old["committed"], "0");
    assert_eq!(
        std::fs::read_to_string(f.0.join("broken/checkpoint-00000000000000000001.json")).unwrap(),
        "unrelated file"
    );
    let saved: Value = serde_json::from_str(v["unpersisted_checkpoint"].as_str().unwrap()).unwrap();
    let raw: Value = serde_json::from_str(saved["checkpoint"].as_str().unwrap()).unwrap();
    assert_eq!(raw["committed"], "2");
    assert_eq!(
        raw["random_state"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        v["random_state"].as_u64().unwrap()
    );
    assert!(
        !f.0.join("broken/.checkpoint-00000000000000000001.pending")
            .exists()
    );
}

#[test]
fn draw_callback_failure_and_cumulative_query_exhaustion_are_terminal_across_processes() {
    let f = Files::new();
    f.controllers(
        " raise RuntimeError('intentional callback failure')",
        json!({}),
    );
    let failed = value(
        f.stream("failed", &["--controllers", "controllers.json"]),
        1,
    );
    assert_eq!(failed["completed"], 0);
    assert_eq!(failed["failed"], 1);
    let path = f.checkpoint(&failed);
    let again = value(
        f.stream(
            "terminal",
            &["--controllers", "controllers.json", "--resume", &path],
        ),
        1,
    );
    assert_eq!(exact(&f, &failed), exact(&f, &again));
    assert_eq!(
        std::fs::read_to_string(f.0.join("calls.txt"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    let f = Files::new();
    let one = value(
        f.stream(
            "one",
            &["--history-sampler", "exact_marginal", "--stop-after", "1"],
        ),
        0,
    );
    let cap = one["execution_costs"]["query"]["attempted"]
        .as_u64()
        .unwrap()
        .to_string();
    assert_ne!(cap, "0");
    let config = ["--history-sampler", "exact_marginal", "--max-queries", &cap];
    let full = value(f.stream("full", &config), 1);
    let mut part = config.to_vec();
    part.extend(["--stop-after", "1"]);
    let prefix = value(f.stream("prefix", &part), 0);
    let path = f.checkpoint(&prefix);
    let mut rest = config.to_vec();
    rest.extend(["--resume", &path]);
    let resumed = value(f.stream("resumed", &rest), 1);
    assert_eq!(exact(&f, &resumed), exact(&f, &full));
    assert_eq!(resumed["failed"], 1);
    assert_eq!(
        resumed["execution_costs"]["query"]["succeeded"]
            .as_u64()
            .unwrap()
            .to_string(),
        cap
    );
}

#[test]
fn raw_declarations_and_manifest_mismatches_reject_without_callback_or_preparation() {
    let f = Files::new();
    f.controllers(" return [bits[0]]", json!({}));
    let prefix = value(
        f.stream(
            "prefix",
            &["--controllers", "controllers.json", "--stop-after", "1"],
        ),
        0,
    );
    let path = f.checkpoint(&prefix);
    let original = std::fs::read_to_string(f.0.join("controllers.json")).unwrap();
    f.put(
        "controllers.json",
        &original.replace("stream-v1", "stream-v2"),
    );
    f.put("decoder.py", "raise RuntimeError('unexpected import')\n");
    let bad = value(
        f.stream(
            "declarations",
            &["--controllers", "controllers.json", "--resume", &path],
        ),
        1,
    );
    assert_eq!(bad["failures"][0]["phase"], "stream_preflight");
    assert!(!f.0.join("declarations").exists());
    f.controllers(" return [bits[0]]", json!({}));
    std::fs::create_dir(f.0.join("other")).unwrap();
    f.put("other/controllers.json", &original);
    f.put(
        "other/decoder.py",
        "def decode(bits, config):\n raise RuntimeError('unexpected callback')\n",
    );
    let bad = value(
        f.stream(
            "manifest",
            &["--controllers", "other/controllers.json", "--resume", &path],
        ),
        1,
    );
    assert!(
        bad["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("manifest")
    );
    assert!(bad["preparation"].is_null());
    assert_eq!(
        std::fs::read_to_string(f.0.join("calls.txt"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn maximum_requested_count_has_bounded_prefix_storage_and_resume_reads_are_capped() {
    let f = Files::new();
    let empty = value(
        f.run(&[
            "stream",
            "circuit.magna",
            "--task",
            "task.json",
            "--shots",
            "18446744073709551615",
            "--seed",
            "18446744073709551615",
            "--stop-after",
            "0",
            "--max-record-values",
            "0",
            "--checkpoint-dir",
            "huge",
            "--json",
        ]),
        0,
    );
    assert_eq!(empty["requested"].as_u64(), Some(u64::MAX));
    assert_eq!(empty["completed"], 0);
    assert!(empty["sampled_records"].is_null());
    let path = f.checkpoint(&empty);
    let bad = value(
        f.run(&[
            "stream",
            "circuit.magna",
            "--task",
            "task.json",
            "--shots",
            "18446744073709551615",
            "--seed",
            "18446744073709551615",
            "--stop-after",
            "0",
            "--max-record-values",
            "0",
            "--checkpoint-dir",
            "capped",
            "--resume",
            &path,
            "--max-checkpoint-bytes",
            "100",
            "--json",
        ]),
        1,
    );
    assert_eq!(bad["failures"][0]["phase"], "stream_preflight");
    assert!(!f.0.join("capped").exists());
}

#[test]
fn zero_weight_affine_draws_remain_in_the_exact_denominator_after_resume() {
    let f = Files::new();
    f.put("circuit.magna","H 0 1 2 3\nCCZ 0 1 2\nCCZ 0 1 3\nCCZ 0 2 3\nH 0 1 2 3\nM 0 1 2 3\nOBSERVABLE_INCLUDE(0) rec[-1]");
    f.put(
        "task.json",
        r#"{"error_observables":[0],"sample_records":[0,1,2,3]}"#,
    );
    let base = [
        "stream",
        "circuit.magna",
        "--task",
        "task.json",
        "--shots",
        "64",
        "--seed",
        "23",
        "--retain-records",
        "--checkpoint-every",
        "8",
        "--json",
    ];
    let mut args = base.to_vec();
    args.extend(["--checkpoint-dir", "full"]);
    let full = value(f.run(&args), 0);
    let mut args = base.to_vec();
    args.extend(["--checkpoint-dir", "prefix", "--stop-after", "11"]);
    let prefix = value(f.run(&args), 0);
    let path = f.checkpoint(&prefix);
    let mut args = base.to_vec();
    args.extend(["--checkpoint-dir", "resumed", "--resume", &path]);
    let resumed = value(f.run(&args), 0);
    assert_eq!(exact(&f, &full), exact(&f, &resumed));
    let efficiency: Value =
        serde_json::from_str(exact(&f, &resumed)["efficiency"].as_str().unwrap()).unwrap();
    assert!(
        efficiency["zero_weights"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            > 0
    );
    assert_eq!(resumed["completed"], 64);
}

#[test]
fn export_failed_prefix_cannot_resume_with_additional_draws() {
    let f = Files::new();
    let failed = value(
        f.stream("prefix", &["--stop-after", "2", "--numeric-work-bits", "0"]),
        1,
    );
    assert_eq!(failed["completed"], 2);
    let path = f.checkpoint(&failed);
    let resumed = value(
        f.stream("resumed", &["--resume", &path, "--numeric-work-bits", "0"]),
        1,
    );
    assert_eq!(resumed["completed"], 2);
    assert_eq!(exact(&f, &failed), exact(&f, &resumed));
}

#[test]
fn extreme_input_byte_caps_preserve_bounded_reads_without_overflow_or_callbacks() {
    let f = Files::new();
    f.controllers(" return [bits[0]]", json!({}));
    let limit = usize::MAX.to_string();
    let v = value(
        f.stream(
            "max-input",
            &[
                "--controllers",
                "controllers.json",
                "--max-task-bytes",
                &limit,
                "--stop-after",
                "0",
            ],
        ),
        0,
    );
    assert_eq!(v["completed"], 0);
    assert_eq!(v["random_state"].as_u64(), Some(u64::MAX));
    assert!(!f.0.join("calls.txt").exists());
    let v = value(
        f.stream("invalid-cap", &["--max-checkpoint-bytes", &limit]),
        2,
    );
    assert_eq!(v["failures"][0]["phase"], "arguments");
    assert!(!f.0.join("invalid-cap").exists());
}

#[cfg(unix)]
#[test]
fn nonregular_resume_files_fail_before_waiting_for_a_writer_or_importing_decoders() {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let f = Files::new();
    f.controllers(" return [bits[0]]", json!({}));
    f.put(
        "decoder.py",
        "raise RuntimeError('unexpected decoder import')\n",
    );
    assert!(
        Command::new("mkfifo")
            .arg(f.0.join("checkpoint.pipe"))
            .status()
            .unwrap()
            .success()
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_rmagna"))
        .current_dir(&f.0)
        .args([
            "stream",
            "circuit.magna",
            "--task",
            "task.json",
            "--controllers",
            "controllers.json",
            "--resume",
            "checkpoint.pipe",
            "--checkpoint-dir",
            "unused",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            let out = child.wait_with_output().unwrap();
            panic!(
                "nonregular checkpoint blocked before rejection: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let v = value(child.wait_with_output().unwrap(), 1);
    assert_eq!(v["failures"][0]["phase"], "stream_preflight");
    assert!(
        v["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("not a regular file")
    );
    assert!(!f.0.join("unused").exists());
    assert!(!f.0.join("calls.txt").exists());
}
