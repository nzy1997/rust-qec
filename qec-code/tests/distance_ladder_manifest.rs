use std::collections::{HashMap, HashSet};

use qec_code::codes::built_in_css::built_in_css_checks;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Issue225DistanceLadderCase {
    case_id: String,
    source_issue: u64,
    code_id: String,
    expected_upper_bound: u64,
    target_weight: u64,
    tier: String,
    run_mode: String,
}

fn required_issue_225_cases() -> Vec<Issue225DistanceLadderCase> {
    vec![
        Issue225DistanceLadderCase {
            case_id: "surface_rotated_d5".to_owned(),
            source_issue: 225,
            code_id: "surface_rotated:d=5".to_owned(),
            expected_upper_bound: 5,
            target_weight: 5,
            tier: "smoke".to_owned(),
            run_mode: "default_ci".to_owned(),
        },
        Issue225DistanceLadderCase {
            case_id: "surface_rotated_d9".to_owned(),
            source_issue: 225,
            code_id: "surface_rotated:d=9".to_owned(),
            expected_upper_bound: 9,
            target_weight: 9,
            tier: "full".to_owned(),
            run_mode: "ignored_full".to_owned(),
        },
        Issue225DistanceLadderCase {
            case_id: "surface_rotated_d13".to_owned(),
            source_issue: 225,
            code_id: "surface_rotated:d=13".to_owned(),
            expected_upper_bound: 13,
            target_weight: 13,
            tier: "full".to_owned(),
            run_mode: "ignored_full".to_owned(),
        },
        Issue225DistanceLadderCase {
            case_id: "toric_d5".to_owned(),
            source_issue: 225,
            code_id: "toric:d=5".to_owned(),
            expected_upper_bound: 5,
            target_weight: 5,
            tier: "smoke".to_owned(),
            run_mode: "default_ci".to_owned(),
        },
        Issue225DistanceLadderCase {
            case_id: "toric_d9".to_owned(),
            source_issue: 225,
            code_id: "toric:d=9".to_owned(),
            expected_upper_bound: 9,
            target_weight: 9,
            tier: "full".to_owned(),
            run_mode: "ignored_full".to_owned(),
        },
        Issue225DistanceLadderCase {
            case_id: "toric_d13".to_owned(),
            source_issue: 225,
            code_id: "toric:d=13".to_owned(),
            expected_upper_bound: 13,
            target_weight: 13,
            tier: "full".to_owned(),
            run_mode: "ignored_full".to_owned(),
        },
        Issue225DistanceLadderCase {
            case_id: "bb72".to_owned(),
            source_issue: 225,
            code_id: "bb72".to_owned(),
            expected_upper_bound: 6,
            target_weight: 6,
            tier: "smoke".to_owned(),
            run_mode: "default_ci".to_owned(),
        },
        Issue225DistanceLadderCase {
            case_id: "bb144".to_owned(),
            source_issue: 225,
            code_id: "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0".to_owned(),
            expected_upper_bound: 12,
            target_weight: 12,
            tier: "full".to_owned(),
            run_mode: "ignored_full".to_owned(),
        },
    ]
}

fn validate_issue_225_distance_ladder_manifest(
    manifest: &[Issue225DistanceLadderCase],
) -> Result<(), String> {
    let required_cases = required_issue_225_cases();

    let mut seen_case_ids = HashSet::new();
    for case in manifest {
        if !seen_case_ids.insert(case.case_id.clone()) {
            return Err(format!(
                "manifest contains duplicate case_id {:?}",
                case.case_id
            ));
        }

        built_in_css_checks(case.code_id.as_str())
            .map(|_| ())
            .map_err(|error| {
                format!(
                    "case {:?} has invalid code_id {:?}: {error}",
                    case.case_id, case.code_id
                )
            })?;

        if case.source_issue == 0 {
            return Err(format!(
                "case {:?} has non-positive source_issue {}",
                case.case_id, case.source_issue
            ));
        }
        if case.source_issue != 225 {
            return Err(format!(
                "case {:?} has unexpected source_issue {}",
                case.case_id, case.source_issue
            ));
        }

        if case.expected_upper_bound == 0 {
            return Err(format!(
                "case {:?} has non-positive expected_upper_bound",
                case.case_id
            ));
        }
        if case.target_weight == 0 {
            return Err(format!(
                "case {:?} has non-positive target_weight",
                case.case_id
            ));
        }
        if case.target_weight != case.expected_upper_bound {
            return Err(format!(
                "case {:?} has target_weight {} that does not equal expected_upper_bound {}",
                case.case_id, case.target_weight, case.expected_upper_bound
            ));
        }

        let expected_run_mode = match case.tier.as_str() {
            "smoke" => "default_ci",
            "full" => "ignored_full",
            _ => {
                return Err(format!(
                    "case {:?} has invalid tier {:?}",
                    case.case_id, case.tier
                ));
            }
        };
        if case.run_mode != expected_run_mode {
            return Err(format!(
                "case {:?} has run_mode {:?}, expected {:?} for tier {:?}",
                case.case_id, case.run_mode, expected_run_mode, case.tier
            ));
        }
        if case.run_mode != "default_ci" && case.run_mode != "ignored_full" {
            return Err(format!(
                "case {:?} has invalid run_mode {:?}",
                case.case_id, case.run_mode
            ));
        }

        if case.case_id == "bb144"
            && case.code_id != "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
        {
            return Err(format!(
                "case {:?} must use the exact bb144 code_id",
                case.case_id
            ));
        }
    }

    if manifest.len() != required_cases.len() {
        return Err(format!(
            "manifest has {} cases, but {} required",
            manifest.len(),
            required_cases.len()
        ));
    }

    let manifest_index: HashMap<&str, &Issue225DistanceLadderCase> = manifest
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect();

    for required in required_cases.iter() {
        let actual = manifest_index
            .get(required.case_id.as_str())
            .ok_or_else(|| {
                format!(
                    "manifest missing required case {:?}",
                    required.case_id
                )
            })?;

        if **actual != *required {
            return Err(format!(
                "manifest case {:?} does not match expected row",
                required.case_id
            ));
        }
    }

    Ok(())
}

fn issue_225_distance_ladder_manifest() -> Vec<Issue225DistanceLadderCase> {
    serde_json::from_str(include_str!("fixtures/distance/issue_225_ladder.json"))
        .expect("manifest fixture should deserialize")
}

#[test]
fn issue_225_distance_ladder_manifest_has_expected_cases() {
    let manifest = issue_225_distance_ladder_manifest();

    validate_issue_225_distance_ladder_manifest(&manifest).unwrap();
}

#[test]
fn issue_225_distance_ladder_manifest_rejects_missing_required_case() {
    let mut manifest = issue_225_distance_ladder_manifest();
    manifest.retain(|case| case.case_id != "bb144");

    let error = validate_issue_225_distance_ladder_manifest(&manifest).expect_err("expected rejection");
    assert!(error.contains("required"));
}
