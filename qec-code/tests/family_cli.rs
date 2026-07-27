use std::process::{Command, Output};

use qec_code::family_verifier::verify_family_manifest_text;

const MANIFEST_TEXT: &str = include_str!("fixtures/family_manifest/manifest.v1.json");

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}

fn run_qec_code(args: &[&str]) -> Output {
    Command::new(qec_code_bin())
        .args(args)
        .output()
        .expect("qec-code binary should run")
}

fn line_count(stdout: &str, prefix: &str) -> usize {
    stdout
        .lines()
        .filter(|line| line.starts_with(prefix))
        .count()
}

fn family_ids(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| {
            line.starts_with("PASS ") || line.starts_with("DEFERRED ") || line.starts_with("FAIL ")
        })
        .map(|line| line.split_whitespace().nth(1).unwrap())
        .collect()
}

fn mutate_generalized_bicycle(mutate: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    let family = value["families"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["family_id"] == "generalized_bicycle")
        .unwrap();
    mutate(family);
    format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
}

#[test]
fn verify_families_cli_reports_12_pass_and_2_deferred() {
    let output = run_qec_code(&["code", "css", "verify-families"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(line_count(&stdout, "PASS "), 12);
    assert_eq!(line_count(&stdout, "DEFERRED "), 2);
    assert_eq!(line_count(&stdout, "FAIL "), 0);
    assert!(stdout.ends_with("SUMMARY PASS supported=12 deferred=2 failed=0\n"));
    assert_eq!(
        family_ids(&stdout),
        vec![
            "directional",
            "quantum_tanner",
            "generalized_bicycle",
            "la_cross",
            "random_hgp",
            "lifted_product",
            "hyperbolic_5_5",
            "coprime_bb",
            "toric_3d",
            "color_666",
            "surface",
            "shor_like",
            "random_two_block",
            "perturbed_hgp",
        ]
    );

    for line in stdout.lines().filter(|line| line.starts_with("PASS ")) {
        for required in [
            " params=",
            " n=",
            " checks=h_x:",
            " ranks=rank_x:",
            " k=",
            " row_weights=h_x:",
            " orthogonal=true",
            " provenance=",
        ] {
            assert!(line.contains(required), "line missing {required:?}: {line}");
        }
    }
    assert!(stdout.contains(
        "DEFERRED hyperbolic_5_5 tracking_issue=#571 contract=qec-code/doc/hyperbolic_5_5_contract.md"
    ));
    assert!(stdout.contains(
        "DEFERRED perturbed_hgp tracking_issue=#572 contract=qec-code/doc/perturbed_hgp_contract.md"
    ));
}

#[test]
fn verify_families_cli_fails_on_mutated_rank() {
    let text = mutate_generalized_bicycle(|family| {
        family["expected"]["rank_x"] = serde_json::json!(5);
    });

    let report = verify_family_manifest_text(&text);

    assert_eq!(report.failed, 1);
    assert!(report
        .output
        .contains("FAIL generalized_bicycle expected rank_x=5 actual rank_x=4"));
    assert!(report
        .output
        .ends_with("SUMMARY FAIL supported=12 deferred=2 failed=1"));
}

#[test]
fn verify_families_cli_fails_when_supported_target_is_planned() {
    let text = mutate_generalized_bicycle(|family| {
        family["availability"] = serde_json::json!("planned");
    });

    let report = verify_family_manifest_text(&text);

    assert_eq!(report.failed, 1);
    assert!(report.output.contains(
        "FAIL generalized_bicycle disposition=supported availability=planned expected=available"
    ));
    assert!(report
        .output
        .ends_with("SUMMARY FAIL supported=12 deferred=2 failed=1"));
}
