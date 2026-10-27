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
        let path = std::env::temp_dir().join(format!(
            "rmagna-native-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(
            path.join("circuit.magna"),
            "H 0\nT 0\nH 0\nM(1/7) 0\nI 1\nOBSERVABLE_INCLUDE(0) rec[-1]",
        )
        .unwrap();
        std::fs::write(path.join("ideal.magna"), "I 0").unwrap();
        std::fs::write(path.join("task.json"),r#"{"outputs":[1],"ideal_state":"ideal.magna","sample_records":[0],"observables":[[1,0],[1,1]]}"#).unwrap();
        Self(path)
    }
    fn run(&self, mode: &str, preparation: &str, extra: &[&str]) -> Output {
        let mut args = vec![
            mode,
            "circuit.magna",
            "--task",
            "task.json",
            "--json",
            "--probability-preparation",
            preparation,
            "--density-preparation",
            preparation,
            "--moment-preparation",
            preparation,
        ];
        args.extend_from_slice(extra);
        Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn json(&self, mode: &str, preparation: &str, extra: &[&str]) -> Value {
        let out = self.run(mode, preparation, extra);
        assert!(
            out.status.success(),
            "{} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn native_modes_report_resolved_options_actual_routes_and_reject_orphan_limits() {
    let f = Files::new();
    for mode in ["native", "native_injection"] {
        let report = f.json("run", mode, &["--shots", "3"]);
        for family in [
            "probability_preparation",
            "density_preparation",
            "moment_preparation",
        ] {
            let p = &report["options"]["compile"][family];
            assert_eq!(p["mode"], mode);
            assert_eq!(p["selection"]["max_quotient_rank"], 8);
            assert_eq!(p["selection"]["max_signature_work"], "67108864");
            assert_eq!(p["selection"]["construction"]["max_terms"], 1048576);
            assert_eq!(p["selection"]["preparation"]["max_terms"], 256);
            assert_eq!(p["injection"].is_null(), mode == "native");
        }
        let caches = &report["compilation"]["retained_caches"];
        let support = &caches["support"];
        assert_eq!(support["resource_kind"], "native_selection");
        let routes = &support["native_routes"];
        let sum: u64 = ["direct", "product", "walsh", "injected"]
            .into_iter()
            .map(|k| routes[k].as_u64().unwrap())
            .sum();
        assert_eq!(sum, 1);
        for family in ["probability", "density", "moment"] {
            let s = &caches[family];
            let sum: u64 = ["direct", "product", "walsh", "injected"]
                .into_iter()
                .map(|k| s["native_routes"][k].as_u64().unwrap())
                .sum();
            assert_eq!(sum, s["fixed_resources"].as_u64().unwrap());
        }
        let prepared = f.json("prepare", mode, &[]);
        assert_eq!(
            prepared["options"]["compile"]["probability_preparation"]["mode"],
            mode
        );
    }
    let report = f.json(
        "run",
        "native_injection",
        &["--shots", "1", "--injection-max-work", "0"],
    );
    assert_eq!(
        report["compilation"]["retained_caches"]["support"]["native_routes"]["injection_budget_skips"],
        1
    );
    for (mode, flag) in [
        ("fixed", "--native-max-terms"),
        ("native", "--injection-max-work"),
        ("native", "--fiber-search-rank"),
    ] {
        let out = f.run("validate", mode, &[flag, "0"]);
        assert!(!out.status.success());
        let text = format!(
            "{} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(text.contains("requires") && text.contains(flag), "{text}");
    }
    let fixed = f.json("validate", "fixed", &[]);
    assert_eq!(
        fixed["options"]["compile"]["probability_preparation"]["mode"],
        "fixed"
    );
}
#[test]
fn every_native_cli_bound_changes_the_resolved_fingerprint() {
    let f = Files::new();
    let baseline = f.json("validate", "native_injection", &[]);
    let flags = [
        "--native-max-quotient-rank",
        "--native-max-core-table-bytes",
        "--native-max-signature-work",
        "--native-max-transform-work",
        "--native-max-construction-terms",
        "--native-max-construction-entries",
        "--native-max-construction-dimension",
        "--native-max-construction-table-values",
        "--native-max-constraint-rows",
        "--native-max-parity-rows",
        "--native-max-verification-work",
        "--native-max-geometry-words",
        "--native-max-terms",
        "--native-max-core-dimension",
        "--native-max-core-table-values",
        "--native-max-core-work",
        "--injection-max-work",
        "--injection-max-geometry-words",
        "--injection-max-terms",
    ];
    for flag in flags {
        let report = f.json("validate", "native_injection", &[flag, "0"]);
        assert_ne!(report["options"], baseline["options"], "{flag}");
        assert_ne!(
            report["fingerprints"]["options"], baseline["fingerprints"]["options"],
            "{flag}"
        );
    }
    assert_ne!(
        f.json("validate", "native", &[])["fingerprints"]["options"],
        baseline["fingerprints"]["options"]
    );
}
#[test]
fn native_stream_resume_rejects_foreign_policy_options_and_keeps_same_policy() {
    let f = Files::new();
    let first = f.json(
        "stream",
        "native_injection",
        &[
            "--shots",
            "4",
            "--seed",
            "71",
            "--checkpoint-dir",
            "first",
            "--stop-after",
            "1",
        ],
    );
    let checkpoint = first["checkpoint"]["latest_committed"].as_str().unwrap();
    let out = f.run(
        "stream",
        "native_injection",
        &[
            "--shots",
            "4",
            "--seed",
            "71",
            "--checkpoint-dir",
            "foreign",
            "--resume",
            checkpoint,
            "--native-max-signature-work",
            "67108865",
        ],
    );
    assert!(!out.status.success());
    let text = format!(
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        text.contains("fingerprint") || text.contains("changed"),
        "{text}"
    );
    let resumed = f.json(
        "stream",
        "native_injection",
        &[
            "--shots",
            "4",
            "--seed",
            "71",
            "--checkpoint-dir",
            "same",
            "--resume",
            checkpoint,
        ],
    );
    let full = f.json(
        "stream",
        "native_injection",
        &["--shots", "4", "--seed", "71", "--checkpoint-dir", "full"],
    );
    assert_eq!(
        resumed["metrics"]["estimates"],
        full["metrics"]["estimates"]
    );
    assert_ne!(resumed["metrics"]["estimates"], json!(null));
}
