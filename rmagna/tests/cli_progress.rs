use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let f = Self(std::env::temp_dir().join(format!(
            "rmagna-cli-progress-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&f.0).unwrap();
        f.put(
            "circuit.magna",
            "H 0\nT 0\nH 0\nM(1/7) 0\nI 1 2\nOBSERVABLE_INCLUDE(0) rec[-1]",
        );
        f.put(
            "task.json",
            r#"{"sample_records":[0],"error_observables":[0]}"#,
        );
        f
    }
    fn put(&self, n: &str, s: &str) {
        std::fs::write(self.0.join(n), s).unwrap();
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_rmagna"));
        c.current_dir(&self.0);
        c
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
    fn bindings(&self, s: &str) {
        self.put("decoder.py", s);
        self.put("controllers.json",&json!({"schema_version":1,"bindings":{"copy":{"import":"decoder:decode","version":"progress-v1","config":{"calls":self.0.join("calls.txt")}}}}).to_string());
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn value(out: &Output, code: i32) -> Value {
    assert_eq!(
        out.status.code(),
        Some(code),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn stderr_progress_preserves_one_final_report_and_saved_output_for_both_samplers() {
    let f = Files::new();
    for sampler in ["affine_support", "exact_marginal"] {
        let baseline = value(
            &f.run(&[
                "run",
                "circuit.magna",
                "--task",
                "task.json",
                "--shots",
                "9",
                "--seed",
                "731",
                "--history-sampler",
                sampler,
                "--json",
            ]),
            0,
        );
        for interval in ["1", "4", "9", "10", "18446744073709551615"] {
            let out = f.run(&[
                "run",
                "circuit.magna",
                "--task",
                "task.json",
                "--shots",
                "9",
                "--seed",
                "731",
                "--history-sampler",
                sampler,
                "--progress-every",
                interval,
                "--output",
                "saved.json",
                "--json",
            ]);
            let actual = value(&out, 0);
            assert_eq!(
                actual,
                serde_json::from_slice::<Value>(&std::fs::read(f.0.join("saved.json")).unwrap())
                    .unwrap()
            );
            std::fs::remove_file(f.0.join("saved.json")).unwrap();
            for k in [
                "estimates",
                "standard_errors",
                "efficiency",
                "weights",
                "sampled_records",
                "completed",
                "failed",
                "not_attempted",
            ] {
                assert_eq!(actual[k], baseline[k], "{k}");
            }
            let every = interval.parse::<u64>().unwrap();
            let expected = (1..=9)
                .filter(|k| k % every == 0 || *k == 9)
                .collect::<Vec<_>>();
            let stderr = String::from_utf8(out.stderr).unwrap();
            let lines = stderr.lines().collect::<Vec<_>>();
            assert_eq!(lines.len(), expected.len());
            for (line, n) in lines.iter().zip(expected) {
                assert!(
                    line.starts_with(&format!("[{n}/9] history_sampler={sampler} kappa=")),
                    "{line}"
                );
            }
            assert_eq!(actual["progress"]["succeeded"], lines.len());
            assert_eq!(actual["options"]["run"]["progress_every"], every);
            assert_eq!(
                actual["fingerprints"]["circuit"],
                baseline["fingerprints"]["circuit"]
            );
            assert_ne!(
                actual["fingerprints"]["options"],
                baseline["fingerprints"]["options"]
            );
        }
    }
}
#[test]
fn invalid_progress_and_existing_output_are_rejected_before_decoder_import() {
    let f = Files::new();
    f.bindings("raise RuntimeError('decoder imported')\n");
    for mode in [
        "validate",
        "inspect",
        "prepare",
        "enumerate",
        "compare-samplers",
    ] {
        let v = value(
            &f.run(&[
                mode,
                "circuit.magna",
                "--controllers",
                "controllers.json",
                "--progress-every",
                "1",
                "--json",
            ]),
            2,
        );
        assert_eq!(v["failures"][0]["phase"], "arguments");
    }
    for interval in ["0", "-1", "NaN", "1.5", "18446744073709551616"] {
        let v = value(
            &f.run(&[
                "run",
                "circuit.magna",
                "--controllers",
                "controllers.json",
                "--progress-every",
                interval,
                "--json",
            ]),
            2,
        );
        assert_eq!(v["failures"][0]["phase"], "arguments");
    }
    value(
        &f.run(&[
            "run",
            "circuit.magna",
            "--controllers",
            "controllers.json",
            "--progress-every",
            "1",
            "--progress-every",
            "2",
            "--json",
        ]),
        2,
    );
    f.put("existing.json", "keep");
    let v = value(
        &f.run(&[
            "run",
            "circuit.magna",
            "--controllers",
            "controllers.json",
            "--progress-every",
            "1",
            "--output",
            "existing.json",
            "--json",
        ]),
        1,
    );
    assert_eq!(v["failures"][0]["phase"], "output");
    assert_eq!(
        std::fs::read_to_string(f.0.join("existing.json")).unwrap(),
        "keep"
    );
}
#[test]
fn numeric_diagnostics_can_fail_while_sampling_completes_with_owned_progress_receipt() {
    let f = Files::new();
    let out = f.run(&[
        "run",
        "circuit.magna",
        "--task",
        "task.json",
        "--shots",
        "5",
        "--progress-every",
        "2",
        "--numeric-work-bits",
        "0",
        "--json",
    ]);
    let v = value(&out, 1);
    assert_eq!(v["engine_status"], "complete");
    assert_eq!(v["completed"], 5);
    assert_eq!(v["failed"], 0);
    assert_eq!(v["progress"]["succeeded"], 3);
    assert!(v["timings"]["progress_seconds"].as_f64().unwrap() >= 0.0);
    assert!(v["efficiency"].is_null());
    let text = String::from_utf8(out.stderr).unwrap();
    assert_eq!(text.lines().count(), 3);
    assert!(
        text.lines()
            .all(|l| l.contains("kappa=unavailable (") && l.contains("max_work_bits is 0"))
    );
}
#[test]
fn progress_adds_no_decoder_callbacks_and_host_stays_isolated() {
    let f = Files::new();
    f.put("circuit.magna","H 0\nT 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nM 1\nI 2\nOBSERVABLE_INCLUDE(0) rec[-1]");
    f.put("task.json", r#"{"error_observables":[0]}"#);
    f.bindings("try:\n import magna\nexcept ImportError:\n pass\nelse:\n raise RuntimeError('native import was not blocked')\ndef decode(bits, config):\n with open(config['calls'], 'a') as out:\n  out.write('called\\n')\n return [bits[0]]\n");
    for sampler in ["affine_support", "exact_marginal"] {
        let out = f.run(&[
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--controllers",
            "controllers.json",
            "--history-sampler",
            sampler,
            "--shots",
            "7",
            "--progress-every",
            "2",
            "--json",
        ]);
        let v = value(&out, 0);
        assert_eq!(v["execution_costs"]["decoder"]["attempted"], 7);
        assert_eq!(String::from_utf8(out.stderr).unwrap().lines().count(), 4);
    }
    assert_eq!(
        std::fs::read_to_string(f.0.join("calls.txt"))
            .unwrap()
            .lines()
            .count(),
        14
    );
}
#[cfg(unix)]
#[test]
fn a_closed_stderr_retains_the_committed_prefix_as_a_progress_failure() {
    use std::os::{fd::OwnedFd, unix::net::UnixStream};
    use std::process::Stdio;
    let f = Files::new();
    let (reader, writer) = UnixStream::pair().unwrap();
    drop(reader);
    let descriptor: OwnedFd = writer.into();
    let out = f
        .command()
        .args([
            "run",
            "circuit.magna",
            "--task",
            "task.json",
            "--shots",
            "7",
            "--progress-every",
            "2",
            "--json",
        ])
        .stderr(Stdio::from(descriptor))
        .output()
        .unwrap();
    let v = value(&out, 1);
    assert_eq!(v["completed"], 2);
    assert_eq!(v["failed"], 0);
    assert_eq!(v["not_attempted"], 5);
    assert_eq!(v["failures"][0]["phase"], "progress");
    assert!(v["failures"][0]["shot"].is_null());
    assert_eq!(v["progress"]["failed"], 1);
    assert_eq!(v["progress"]["last_completed"], 2);
    assert_eq!(v["sampled_records"].as_array().unwrap().len(), 2);
}
