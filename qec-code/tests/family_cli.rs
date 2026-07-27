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
    mutate_family("generalized_bicycle", mutate)
}

fn mutate_family(family_id: &str, mutate: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    let family = value["families"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["family_id"] == family_id)
        .unwrap();
    mutate(family);
    format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
}

fn mutate_manifest(mutate: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    mutate(&mut value);
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
    assert!(
        report
            .output
            .contains("FAIL generalized_bicycle expected rank_x=5 actual rank_x=4")
    );
    assert!(
        report
            .output
            .ends_with("SUMMARY FAIL supported=12 deferred=2 failed=1")
    );
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
    assert!(
        report
            .output
            .ends_with("SUMMARY FAIL supported=12 deferred=2 failed=1")
    );
}

#[test]
fn verify_families_cli_fails_when_deferred_family_is_missing() {
    let mut value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    value["families"]
        .as_array_mut()
        .unwrap()
        .retain(|entry| entry["family_id"] != "perturbed_hgp");
    let text = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());

    let report = verify_family_manifest_text(&text);

    assert!(report.failed > 0);
    assert!(
        report
            .output
            .contains("FAIL manifest missing family_id=perturbed_hgp")
    );
    assert!(
        report
            .output
            .contains("SUMMARY FAIL supported=12 deferred=1")
    );
}

#[test]
fn verify_families_cli_fails_when_family_is_duplicated() {
    let mut value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    let duplicate = value["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["family_id"] == "directional")
        .unwrap()
        .clone();
    value["families"].as_array_mut().unwrap().push(duplicate);
    let text = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());

    let report = verify_family_manifest_text(&text);

    assert!(report.failed > 0);
    assert!(
        report
            .output
            .contains("FAIL manifest duplicate family_id=directional")
    );
    assert!(
        report
            .output
            .contains("SUMMARY FAIL supported=13 deferred=2")
    );
}

#[test]
fn verify_families_cli_fails_on_mutated_deferred_contract() {
    let text = mutate_family("hyperbolic_5_5", |family| {
        family["research_contracts"] =
            serde_json::json!(["qec-code/doc/wrong_hyperbolic_contract.md"]);
    });

    let report = verify_family_manifest_text(&text);

    assert_eq!(report.failed, 1);
    assert!(report.output.contains(
        "FAIL hyperbolic_5_5 expected contract=qec-code/doc/hyperbolic_5_5_contract.md actual contract=qec-code/doc/wrong_hyperbolic_contract.md"
    ));
    assert!(
        report
            .output
            .ends_with("SUMMARY FAIL supported=12 deferred=2 failed=1")
    );
}

#[test]
fn verify_families_cli_reports_catalog_and_metadata_failure_boundaries() {
    let cases = [
        (
            "invalid JSON",
            "{".to_owned(),
            "FAIL manifest invalid_json=",
        ),
        (
            "unexpected deferred family",
            mutate_manifest(|manifest| {
                let mut unknown = manifest["families"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|entry| entry["family_id"] == "hyperbolic_5_5")
                    .unwrap()
                    .clone();
                unknown["family_id"] = serde_json::json!("unknown_deferred");
                unknown["research_contracts"] =
                    serde_json::json!(["qec-code/doc/unknown_deferred_contract.md"]);
                manifest["families"].as_array_mut().unwrap().push(unknown);
            }),
            "FAIL unknown_deferred unknown deferred family",
        ),
        (
            "supported not_applicable availability",
            mutate_generalized_bicycle(|family| {
                family["availability"] = serde_json::json!("not_applicable");
            }),
            "FAIL generalized_bicycle disposition=supported availability=not_applicable expected=available",
        ),
        (
            "missing positive success case",
            mutate_generalized_bicycle(|family| {
                family["executable_cases"] = serde_json::json!([]);
            }),
            "FAIL generalized_bicycle missing positive success case",
        ),
        (
            "positive success case missing request",
            mutate_generalized_bicycle(|family| {
                family["executable_cases"][0]["request"] = serde_json::Value::Null;
            }),
            "FAIL generalized_bicycle positive success case missing request",
        ),
        (
            "construction rejection",
            mutate_generalized_bicycle(|family| {
                family["executable_cases"][0]["request"]["order"] = serde_json::json!(0);
            }),
            "FAIL generalized_bicycle construction=invalid CSS construction generalized_bicycle: order must be nonzero",
        ),
        (
            "requested family id mismatch",
            mutate_generalized_bicycle(|family| {
                family["family_id"] = serde_json::json!("generalized_bicycle_alias");
            }),
            "FAIL generalized_bicycle_alias expected requested_family_id=generalized_bicycle_alias actual requested_family_id=generalized_bicycle",
        ),
        (
            "missing callable constructor",
            mutate_generalized_bicycle(|family| {
                family["callable_constructor"] = serde_json::Value::Null;
            }),
            "FAIL generalized_bicycle missing callable_constructor",
        ),
        (
            "provenance mismatch",
            mutate_generalized_bicycle(|family| {
                family["callable_constructor"]["rust_path"] = serde_json::json!("Wrong::Path");
            }),
            "FAIL generalized_bicycle expected provenance=Wrong::Path actual provenance=CssFamilySpec::GeneralizedBicycle",
        ),
        (
            "missing expected stats",
            mutate_generalized_bicycle(|family| {
                family["expected"] = serde_json::Value::Null;
            }),
            "FAIL generalized_bicycle missing expected stats",
        ),
        (
            "missing row weight summary",
            mutate_generalized_bicycle(|family| {
                family["row_weight_summary"] = serde_json::Value::Null;
            }),
            "FAIL generalized_bicycle missing row_weight_summary",
        ),
        (
            "h_x row weight mismatch",
            mutate_generalized_bicycle(|family| {
                family["row_weight_summary"]["h_x"][0]["count"] = serde_json::json!(6);
            }),
            "FAIL generalized_bicycle expected row_weights_h_x=[w4=6] actual row_weights_h_x=[w4=5]",
        ),
        (
            "h_z row weight mismatch",
            mutate_generalized_bicycle(|family| {
                family["row_weight_summary"]["h_z"][0]["count"] = serde_json::json!(6);
            }),
            "FAIL generalized_bicycle expected row_weights_h_z=[w4=6] actual row_weights_h_z=[w4=5]",
        ),
        (
            "d_x mismatch",
            mutate_generalized_bicycle(|family| {
                family["expected"]["d_x"] = serde_json::Value::Null;
            }),
            "FAIL generalized_bicycle expected d_x=None actual d_x=Some(3)",
        ),
        (
            "d_z mismatch",
            mutate_generalized_bicycle(|family| {
                family["expected"]["d_z"] = serde_json::Value::Null;
            }),
            "FAIL generalized_bicycle expected d_z=None actual d_z=Some(3)",
        ),
        (
            "deferred available availability",
            mutate_family("hyperbolic_5_5", |family| {
                family["availability"] = serde_json::json!("available");
            }),
            "FAIL hyperbolic_5_5 disposition=deferred availability=available expected=not_applicable",
        ),
        (
            "missing deferred research contract",
            mutate_family("hyperbolic_5_5", |family| {
                family["research_contracts"] = serde_json::json!([]);
            }),
            "FAIL hyperbolic_5_5 expected exactly one research_contract",
        ),
    ];

    for (label, text, expected) in cases {
        let report = verify_family_manifest_text(&text);

        assert!(report.failed > 0, "{label} should fail verification");
        assert!(
            report.output.contains(expected),
            "{label} missing {expected:?} in report:\n{}",
            report.output
        );
    }
}
