// SPDX-License-Identifier: MPL-2.0
// Complete original ImportanceSamplingTests.test_invalid_proposals_and_cli_config
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_importance_sampling.py.
use num_rational::BigRational as R;
use rmagna::noise::{
    NoiseSampling, SamplingDeclarationOptions, load_sampling_declaration,
    parse_sampling_declaration,
};
use rmagna::probability::ProbabilityParser;
fn importance(token: &str) -> NoiseSampling {
    parse_sampling_declaration(
        &format!(r#"{{"method":"importance","proposal_scale":{token}}}"#),
        &SamplingDeclarationOptions::default(),
    )
    .unwrap()
}
#[test]
fn original_invalid_proposals_and_cli_config() {
    for text in [
        r#"{"method":"bad"}"#,
        r#"{"method":"physical","proposal_scale":"2"}"#,
        r#"{"method":"importance","proposal_rate":"1/2"}"#,
        r#"{"method":"importance"}"#,
        r#"{"method":"importance","proposal_scale":0}"#,
        r#"{"method":"importance","proposal_scale":-1}"#,
        r#"{"method":"importance","proposal_scale":"nan"}"#,
        r#"{"method":"importance","proposal_scale":"inf"}"#,
        r#"{"method":"importance","proposal_scale":true}"#,
        r#"{"method":"importance","proposal_scale":"1/0"}"#,
    ] {
        let error = parse_sampling_declaration(text, &SamplingDeclarationOptions::default())
            .expect_err("invalid original declaration accepted");
        assert!(error.field.starts_with("sampling"));
        assert!(!error.message.is_empty());
    }
    for text in [
        "{}",
        r#"{"method":"physical"}"#,
        r#"{"method":"bad","method":"physical"}"#,
    ] {
        assert_eq!(
            parse_sampling_declaration(text, &SamplingDeclarationOptions::default()).unwrap(),
            NoiseSampling::Physical
        );
    }
    for (token, expected) in [
        ("9007199254740993", "9007199254740993"),
        (
            "1234567890123456789012345678901234567890",
            "1234567890123456789012345678901234567890",
        ),
        (r#""500/77""#, "500/77"),
        ("0.1", "1/10"),
        (" 0.1 ", "1/10"),
        (
            r#""1.0000000000000001""#,
            "10000000000000001/10000000000000000",
        ),
        ("1.0000000000000001", "1"),
        ("1e2", "100"),
        (r#""0.00000000000000001""#, "1/100000000000000000"),
    ] {
        assert_eq!(
            importance(token),
            NoiseSampling::Importance {
                proposal_scale: expected.parse::<R>().unwrap()
            },
            "{token}"
        );
    }
    assert_eq!(
        parse_sampling_declaration(
            r#"{"method":"importance","proposal_scale":"0","proposal_scale":"100"}"#,
            &SamplingDeclarationOptions::default()
        )
        .unwrap(),
        NoiseSampling::Importance {
            proposal_scale: "100".parse().unwrap()
        }
    );
    verify_original_cli();
}
#[test]
fn bounded_invalid_shapes_tokens_and_paths() {
    let options = SamplingDeclarationOptions::default();
    for text in [
        "[]",
        "null",
        "1",
        "true",
        "{",
        r#"{"method":null}"#,
        r#"{"method":"physical","extra":0}"#,
        r#"{"method":"importance","proposal_scale":{}}"#,
        r#"{"method":"importance","proposal_scale":{"$serde_json::private::Number":"100"}}"#,
        r#"{"$serde_json::private::Number":"100"}"#,
        r#"{"method":"importance","proposal_scale":[]}"#,
        r#"{"method":"importance","proposal_scale":1e999}"#,
        r#"{"method":"importance","proposal_scale":1e-999}"#,
        r#"{"method":"importance","proposal_scale":"-1/2"}"#,
        r#"{"method":"importance","proposal_scale":false}"#,
    ] {
        assert!(
            parse_sampling_declaration(text, &options).is_err(),
            "{text}"
        );
    }
    for limit in [0, usize::MAX] {
        assert!(
            parse_sampling_declaration(
                "{}",
                &SamplingDeclarationOptions {
                    max_json_bytes: limit,
                    ..options
                }
            )
            .is_err()
        );
    }
    assert!(
        parse_sampling_declaration(
            "{}",
            &SamplingDeclarationOptions {
                max_json_bytes: 1,
                ..options
            }
        )
        .is_err()
    );
    assert_eq!(
        parse_sampling_declaration(
            "{}",
            &SamplingDeclarationOptions {
                max_json_bytes: 2,
                ..options
            }
        )
        .unwrap(),
        NoiseSampling::Physical
    );
    let strict = SamplingDeclarationOptions {
        numbers: ProbabilityParser::new(4, 3).unwrap(),
        ..options
    };
    for token in ["12345", "1e4", r#""1e-4""#] {
        let text = format!(r#"{{"method":"importance","proposal_scale":{token}}}"#);
        assert!(
            parse_sampling_declaration(&text, &strict).is_err(),
            "{token}"
        );
    }
    let missing = std::env::temp_dir().join(format!(
        "rmagna-nonexistent-sampling-{}",
        std::process::id()
    ));
    assert!(!missing.exists());
    assert_eq!(
        load_sampling_declaration(&missing, &options)
            .unwrap_err()
            .field,
        "sampling.path"
    );
}
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
        let p = std::env::temp_dir().join(format!(
            "rmagna-original-sampling-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn put(&self, name: &str, bytes: impl AsRef<[u8]>) {
        std::fs::write(self.0.join(name), bytes).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "stdout={} stderr={}",
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
fn verify_original_cli() {
    let files = Files::new();
    files.put("c.magna", "X_ERROR(1/1000) 0\nM 0");
    files.put("zero.magna", "I 0");
    files.put("task.json", r#"{"outputs":[0],"ideal_state":"zero.magna"}"#);
    files.put(
        "sampling.json",
        r#"{"method":"importance","proposal_scale":"100"}"#,
    );
    for mode in ["validate", "inspect", "run"] {
        let mut args = vec![
            mode,
            "c.magna",
            "--task",
            "task.json",
            "--sampling",
            "sampling.json",
            "--json",
        ];
        if mode == "run" {
            args.extend(["--shots", "16"]);
        }
        let file = files.json(&args);
        assert_eq!(file["sampling"]["method"], "importance");
        assert_eq!(
            file["sampling"]["configuration"],
            json!({"mode":"importance","proposal_scale":{"numerator":"100","denominator":"1"}})
        );
        if mode == "run" {
            assert_eq!(file["completed"], 16);
            assert_eq!(file["status"], "complete");
            assert_eq!(file["failed"], 0);
        }
        let index = args.iter().position(|x| *x == "--sampling").unwrap();
        args[index] = "--noise-proposal-scale";
        args[index + 1] = "100";
        let scalar = files.json(&args);
        assert_eq!(file["sampling"], scalar["sampling"]);
        assert_eq!(
            file["fingerprints"]["sampling"],
            scalar["fingerprints"]["sampling"]
        );
        if mode == "run" {
            assert_eq!(file["estimates"], scalar["estimates"]);
            assert_eq!(file["standard_errors"], scalar["standard_errors"]);
        }
    }
    files.put("physical.json", "  { \"method\": \"physical\" }  ");
    for mode in ["validate", "inspect", "run"] {
        let mut args = vec![mode, "c.magna", "--task", "task.json", "--json"];
        if mode == "run" {
            args.extend(["--shots", "16"]);
        }
        let ordinary = files.json(&args);
        args.extend(["--sampling", "physical.json"]);
        let declared = files.json(&args);
        assert_eq!(declared["sampling"]["method"], "physical");
        assert_eq!(
            declared["sampling"]["configuration"],
            json!({"mode":"physical"})
        );
        assert_eq!(
            declared["fingerprints"]["sampling"],
            ordinary["fingerprints"]["sampling"]
        );
    }
}
#[test]
fn cli_conflicts_limits_and_read_failures_precede_loading_quantum_inputs() {
    let files = Files::new();
    files.put("bad.json", b"{\"method\":");
    files.put("invalid-utf8.json", [0xff, 0xfe]);
    files.put("huge.json", vec![b' '; (16 << 10) + 1]);
    files.put("physical.json", "{}");
    for name in ["missing.json", "bad.json", "invalid-utf8.json", "huge.json"] {
        let out = files.run(&[
            "validate",
            "absent-circuit.magna",
            "--controllers",
            "absent-controller.json",
            "--sampling",
            name,
            "--json",
        ]);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stderr.is_empty());
        let failure: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(failure["status"], "failed");
        assert_eq!(failure["failures"][0]["phase"], "arguments");
        let message = failure["failures"][0]["message"].as_str().unwrap();
        assert!(message.contains("--sampling"), "{message}");
        assert!(!message.contains("absent-circuit"));
        assert!(!message.contains("absent-controller"));
    }
    let directory_error =
        load_sampling_declaration(&files.0, &SamplingDeclarationOptions::default()).unwrap_err();
    assert_eq!(directory_error.field, "sampling.path");
    let out = files.run(&[
        "validate",
        "absent-circuit.magna",
        "--sampling",
        "missing.json",
        "--noise-proposal-scale",
        "100",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8(out.stdout).unwrap().contains("conflicts"));
    let out = files.run(&[
        "enumerate",
        "absent-circuit.magna",
        "--sampling",
        "physical.json",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("unavailable in enumerate")
    );
    assert!(
        load_sampling_declaration(
            &files.0.join("huge.json"),
            &SamplingDeclarationOptions::default()
        )
        .unwrap_err()
        .message
        .contains("limit")
    );
}
