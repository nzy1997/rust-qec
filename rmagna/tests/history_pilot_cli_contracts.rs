//! Original finite CLI pilot/argument contracts, pinned to
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_history_efficiency.py. No native imports or builders.

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
        let files = Self(std::env::temp_dir().join(format!(
            "rmagna-original-pilot-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&files.0).unwrap();
        files.put("physical.magna", "X_ERROR(1/2) 0\n");
        files.put("target.magna", "I 0\n");
        files.put(
            "task.json",
            &json!({"outputs":[0],"ideal_state":{"circuit":"target.magna","outputs":[0]}})
                .to_string(),
        );
        files
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
    fn forbidden_decoder(&self) {
        self.put(
            "physical.magna",
            "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[probe] det[0] -> ctrl[0]\n",
        );
        self.put("decoder.py","from pathlib import Path\nPath(__file__).with_name('imported.txt').write_text('imported')\nraise RuntimeError('forbidden decoder import')\ndef decode(bits, config):\n return [False]\n");
        self.put("controllers.json",&json!({"schema_version":1,"bindings":{"probe":{"import":"decoder:decode","version":"original-counts-v1","config":{}}}}).to_string());
    }
    fn rejected(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        let value = value(&out, 2);
        assert_eq!(value["failures"][0]["phase"], "arguments");
        assert!(!value.to_string().contains("forbidden decoder import"));
        assert!(!self.0.join("imported.txt").exists());
        value
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
fn original_eight_draw_progress_and_sixteen_draw_comparison_keep_json_streams_separate() {
    let files = Files::new();
    let out = files.run(&[
        "run",
        "physical.magna",
        "--task",
        "task.json",
        "--shots",
        "8",
        "--progress-every",
        "4",
        "--json",
    ]);
    let result = value(&out, 0);
    assert_eq!(result["status"], "complete");
    assert_eq!(result["completed"], 8);
    assert_eq!(result["history_sampler"], "affine_support");
    assert_eq!(
        result["sampling"].get("exact_marginal_records"),
        Some(&Value::Null)
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    let lines = stderr.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    for (line, completed) in lines.iter().zip([4, 8]) {
        assert!(line.starts_with(&format!(
            "[{completed}/8] history_sampler=affine_support kappa="
        )));
        assert!(line.contains("--history-sampler exact_marginal"));
    }
    let out = files.run(&[
        "compare-samplers",
        "physical.magna",
        "--task",
        "task.json",
        "--pilot-shots",
        "16",
        "--json",
    ]);
    let result = value(&out, 0);
    assert_eq!(result["status"], "complete");
    assert_eq!(result["pilot_shots_per_channel"], 16);
    assert_eq!(result["automatic_switching"], false);
    assert_eq!(result["production_started"], false);
    assert!(out.stderr.is_empty());
    for mode in ["affine_support", "exact_marginal"] {
        let pilot = &result["pilots"][mode];
        assert_eq!(pilot["history_sampler"], mode);
        assert_eq!(pilot["status"], "complete");
        assert_eq!(pilot["sampling_receipt"]["completed"], 16);
        assert_eq!(pilot["sampling"]["history_sampler"], mode);
    }
}

#[test]
fn original_invalid_counts_and_retired_modes_reject_before_decoder_import() {
    let files = Files::new();
    files.forbidden_decoder();
    for count in ["0", "-1", "true", "false", "True", "1.5"] {
        files.rejected(&[
            "run",
            "physical.magna",
            "--controllers",
            "controllers.json",
            "--progress-every",
            count,
            "--json",
        ]);
    }
    for count in [
        "0", "1", "-1", "true", "false", "True", "1.5", "None", "null",
    ] {
        files.rejected(&[
            "compare-samplers",
            "physical.magna",
            "--controllers",
            "controllers.json",
            "--pilot-shots",
            count,
            "--json",
        ]);
    }
    // A missing payload represents Python None; omitting the whole option
    // retains Rust's documented valid two-draw default.
    files.rejected(&[
        "compare-samplers",
        "physical.magna",
        "--controllers",
        "controllers.json",
        "--pilot-shots",
        "--json",
    ]);
    files.rejected(&[
        "compare-samplers",
        "physical.magna",
        "--controllers",
        "controllers.json",
        "--json",
        "--pilot-shots",
    ]);
    for retired in ["capped", "capped_affine", "born"] {
        for mode in ["inspect", "run"] {
            let rejected = files.rejected(&[
                mode,
                "physical.magna",
                "--controllers",
                "controllers.json",
                "--history-sampler",
                retired,
                "--json",
            ]);
            assert!(
                rejected
                    .to_string()
                    .contains("affine_support or exact_marginal")
            );
        }
    }
    // Canary: the same decoder really is importable/reachable with valid args.
    let canary = files.run(&[
        "run",
        "physical.magna",
        "--task",
        "task.json",
        "--controllers",
        "controllers.json",
        "--shots",
        "1",
        "--json",
    ]);
    assert!(!canary.status.success());
    assert_eq!(
        std::fs::read_to_string(files.0.join("imported.txt")).unwrap(),
        "imported"
    );
}
