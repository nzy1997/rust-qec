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

#[test]
fn native_transport_controls_resolve_fingerprint_and_report_actual_admission() {
    let f = Files::new();
    let caps = [
        ("--native-transport-max-terms", "max_terms"),
        (
            "--native-transport-max-geometry-words",
            "max_geometry_words",
        ),
        (
            "--native-transport-max-construction-work",
            "max_construction_work",
        ),
    ];
    for preparation in ["native", "native_injection"] {
        let off = f.json("run", preparation, &["--shots", "3"]);
        let prepared = f.json(
            "run",
            preparation,
            &["--shots", "3", "--native-transport", "prepared"],
        );
        assert_eq!(
            off["metrics"]["estimates"],
            prepared["metrics"]["estimates"]
        );
        assert_ne!(
            off["fingerprints"]["options"],
            prepared["fingerprints"]["options"]
        );
        for family in [
            "probability_preparation",
            "density_preparation",
            "moment_preparation",
        ] {
            assert_eq!(
                off["options"]["compile"][family]["transport"]["mode"],
                "off"
            );
            assert_eq!(
                prepared["options"]["compile"][family]["transport"]["mode"],
                "prepared"
            );
        }
        for family in ["probability", "density", "moment", "support"] {
            let old = &off["compilation"]["retained_caches"][family];
            let new = &prepared["compilation"]["retained_caches"][family];
            assert_eq!(old["native_routes"], new["native_routes"]);
            assert_eq!(
                old["native_transports"],
                json!({"prepared":0,"budget_skipped":0})
            );
            let count = if family == "support" {
                1
            } else {
                new["fixed_resources"].as_u64().unwrap()
            };
            assert_eq!(
                new["native_transports"],
                json!({"prepared":count,"budget_skipped":0})
            );
        }
        for (flag, key) in caps {
            let skipped = f.json(
                "run",
                preparation,
                &["--shots", "3", "--native-transport", "prepared", flag, "0"],
            );
            assert_eq!(off["metrics"]["estimates"], skipped["metrics"]["estimates"]);
            assert_ne!(
                prepared["fingerprints"]["options"], skipped["fingerprints"]["options"],
                "{flag}"
            );
            for family in [
                "probability_preparation",
                "density_preparation",
                "moment_preparation",
            ] {
                let expected = if key == "max_construction_work" {
                    json!("0")
                } else {
                    json!(0)
                };
                assert_eq!(
                    skipped["options"]["compile"][family]["transport"][key],
                    expected
                );
            }
            for family in ["probability", "density", "moment", "support"] {
                let old = &prepared["compilation"]["retained_caches"][family];
                let new = &skipped["compilation"]["retained_caches"][family];
                assert_eq!(old["native_routes"], new["native_routes"]);
                let count = if family == "support" {
                    1
                } else {
                    new["fixed_resources"].as_u64().unwrap()
                };
                assert_eq!(
                    new["native_transports"],
                    json!({"prepared":0,"budget_skipped":count})
                );
            }
            for preparation in ["fixed", "native"] {
                let output = f.run("validate", preparation, &[flag, "0"]);
                assert!(!output.status.success());
                let text = format!(
                    "{} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(text.contains("requires"), "{text}");
            }
        }
        let output = f.run("validate", "fixed", &["--native-transport", "prepared"]);
        assert!(!output.status.success());
        let output = f.run("validate", preparation, &["--native-transport", "unknown"]);
        assert!(!output.status.success());
    }
}

#[test]
fn native_transport_stream_identity_rejects_each_changed_setting() {
    let f = Files::new();
    let first = f.json(
        "stream",
        "native",
        &[
            "--shots",
            "4",
            "--seed",
            "71",
            "--native-transport",
            "prepared",
            "--checkpoint-dir",
            "transport-first",
            "--stop-after",
            "1",
        ],
    );
    let checkpoint = first["checkpoint"]["latest_committed"].as_str().unwrap();
    for changed in [
        vec!["--native-transport", "off"],
        vec![
            "--native-transport",
            "prepared",
            "--native-transport-max-terms",
            "257",
        ],
        vec![
            "--native-transport",
            "prepared",
            "--native-transport-max-geometry-words",
            "16777217",
        ],
        vec![
            "--native-transport",
            "prepared",
            "--native-transport-max-construction-work",
            "1099511627777",
        ],
    ] {
        let mut args = vec![
            "--shots",
            "4",
            "--seed",
            "71",
            "--checkpoint-dir",
            "transport-foreign",
            "--resume",
            checkpoint,
        ];
        args.extend(changed);
        let output = f.run("stream", "native", &args);
        assert!(!output.status.success());
        let text = format!(
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            text.contains("fingerprint") || text.contains("changed"),
            "{text}"
        );
    }
    let resumed = f.json(
        "stream",
        "native",
        &[
            "--shots",
            "4",
            "--seed",
            "71",
            "--native-transport",
            "prepared",
            "--checkpoint-dir",
            "transport-same",
            "--resume",
            checkpoint,
        ],
    );
    let full = f.json(
        "stream",
        "native",
        &[
            "--shots",
            "4",
            "--seed",
            "71",
            "--native-transport",
            "prepared",
            "--checkpoint-dir",
            "transport-full",
        ],
    );
    assert_eq!(
        resumed["metrics"]["estimates"],
        full["metrics"]["estimates"]
    );
}
