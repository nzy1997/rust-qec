use std::collections::HashSet;

const MANIFEST_TEXT: &str = include_str!("fixtures/family_manifest/manifest.v1.json");
const SCHEMA_TEXT: &str = include_str!("fixtures/family_manifest/README.md");

const MANIFEST_SCHEMA_VERSION: u64 = 1;
const MANIFEST_ID: &str = "qec_family_construction_targets_v1";
const PROMOTION_GATE_ISSUE: u64 = 573;

const REQUESTED_FAMILY_IDS: &[&str] = &[
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
];

const DEFERRED_FAMILY_IDS: &[&str] = &["hyperbolic_5_5", "perturbed_hgp"];

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
enum FamilyDisposition {
    Supported,
    Deferred,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
enum RuntimeAvailability {
    Planned,
    Available,
    NotApplicable,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
enum ExecutableCaseKind {
    Positive,
    Negative,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ExpectedOutcome {
    Success,
    Rejection,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct FamilyManifest {
    schema_version: u64,
    manifest_id: String,
    provenance: Vec<String>,
    verification: Vec<String>,
    intended_consumers: Vec<String>,
    availability_promotion_gate: AvailabilityPromotionGate,
    families: Vec<FamilyManifestEntry>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct AvailabilityPromotionGate {
    issue: u64,
    rule: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct FamilyManifestEntry {
    schema_version: u64,
    family_id: String,
    disposition: FamilyDisposition,
    availability: RuntimeAvailability,
    provenance: Vec<String>,
    verification: Vec<String>,
    intended_consumers: Vec<String>,
    callable_constructor: Option<CallableConstructorRef>,
    executable_cases: Vec<ExecutableCase>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CallableConstructorRef {
    rust_path: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutableCase {
    case_id: String,
    case_kind: ExecutableCaseKind,
    expected_outcome: ExpectedOutcome,
    description: String,
    verification: Vec<String>,
}

fn parse_and_validate_family_manifest_text(text: &str) -> Result<FamilyManifest, String> {
    let value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    parse_and_validate_family_manifest_value(value)
}

fn parse_and_validate_family_manifest_value(
    value: serde_json::Value,
) -> Result<FamilyManifest, String> {
    let manifest = serde_json::from_value(value).map_err(|error| error.to_string())?;
    validate_family_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_family_manifest(manifest: &FamilyManifest) -> Result<(), String> {
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err("unexpected manifest schema_version".to_owned());
    }
    if manifest.manifest_id != MANIFEST_ID {
        return Err("unexpected manifest_id".to_owned());
    }
    expect_nonempty_strings("manifest provenance", &manifest.provenance)?;
    expect_nonempty_strings("manifest verification", &manifest.verification)?;
    expect_nonempty_strings("manifest intended_consumers", &manifest.intended_consumers)?;
    if manifest.availability_promotion_gate.issue != PROMOTION_GATE_ISSUE {
        return Err("unexpected availability promotion gate issue".to_owned());
    }
    if manifest.availability_promotion_gate.rule.trim().is_empty() {
        return Err("availability promotion gate rule must be non-empty".to_owned());
    }

    let mut seen_family_ids = HashSet::new();
    let mut supported_case_count = 0;
    for entry in &manifest.families {
        if !seen_family_ids.insert(entry.family_id.as_str()) {
            return Err(format!("duplicate family_id {:?}", entry.family_id));
        }
        if entry.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(format!(
                "family {:?} has unexpected schema_version",
                entry.family_id
            ));
        }
        expect_nonempty_strings("family provenance", &entry.provenance)?;
        expect_nonempty_strings("family verification", &entry.verification)?;
        expect_nonempty_strings("family intended_consumers", &entry.intended_consumers)?;
        validate_lifecycle_pair(entry.disposition, entry.availability)?;

        let is_deferred = DEFERRED_FAMILY_IDS.contains(&entry.family_id.as_str());
        match entry.disposition {
            FamilyDisposition::Supported => {
                if is_deferred {
                    return Err(format!(
                        "deferred family {:?} must be deferred",
                        entry.family_id
                    ));
                }
                if entry.availability != RuntimeAvailability::Planned {
                    return Err(format!(
                        "supported family {:?} must be planned",
                        entry.family_id
                    ));
                }
                if entry.callable_constructor.is_some() {
                    return Err(format!(
                        "planned family {:?} cannot declare callable_constructor",
                        entry.family_id
                    ));
                }
                validate_supported_cases(&entry.family_id, &entry.executable_cases)?;
                supported_case_count += entry.executable_cases.len();
            }
            FamilyDisposition::Deferred => {
                if !is_deferred {
                    return Err(format!("family {:?} is not deferred", entry.family_id));
                }
                if entry.callable_constructor.is_some() {
                    return Err(format!(
                        "deferred family {:?} cannot declare callable_constructor",
                        entry.family_id
                    ));
                }
                if !entry.executable_cases.is_empty() {
                    return Err(format!(
                        "deferred family {:?} cannot declare executable_cases",
                        entry.family_id
                    ));
                }
            }
        }
    }

    let actual_ids: Vec<&str> = manifest
        .families
        .iter()
        .map(|entry| entry.family_id.as_str())
        .collect();
    if actual_ids != REQUESTED_FAMILY_IDS {
        return Err("families must match the requested family IDs in order".to_owned());
    }
    if supported_case_count < 24 {
        return Err(
            "manifest must contain at least 24 supported-family executable cases".to_owned(),
        );
    }
    Ok(())
}

fn validate_lifecycle_pair(
    disposition: FamilyDisposition,
    availability: RuntimeAvailability,
) -> Result<(), String> {
    match (disposition, availability) {
        (FamilyDisposition::Supported, RuntimeAvailability::Planned)
        | (FamilyDisposition::Supported, RuntimeAvailability::Available)
        | (FamilyDisposition::Deferred, RuntimeAvailability::NotApplicable) => Ok(()),
        _ => Err("illegal disposition/availability pair".to_owned()),
    }
}

fn expect_nonempty_strings(field: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return Err(format!(
            "{field} must be a non-empty array of non-empty strings"
        ));
    }
    Ok(())
}

fn validate_supported_cases(family_id: &str, cases: &[ExecutableCase]) -> Result<(), String> {
    let mut has_positive_success = false;
    let mut has_negative_rejection = false;
    let mut seen_case_ids = HashSet::new();
    for case in cases {
        if case.case_id.trim().is_empty() || case.description.trim().is_empty() {
            return Err(format!(
                "supported family {family_id:?} has empty executable case text"
            ));
        }
        if !seen_case_ids.insert(case.case_id.as_str()) {
            return Err(format!(
                "supported family {family_id:?} has duplicate case_id {:?}",
                case.case_id
            ));
        }
        expect_nonempty_strings("executable case verification", &case.verification)?;
        has_positive_success |= case.case_kind == ExecutableCaseKind::Positive
            && case.expected_outcome == ExpectedOutcome::Success;
        has_negative_rejection |= case.case_kind == ExecutableCaseKind::Negative
            && case.expected_outcome == ExpectedOutcome::Rejection;
    }
    if !has_positive_success || !has_negative_rejection {
        return Err(format!(
            "supported family {family_id:?} requires positive/success and negative/rejection cases"
        ));
    }
    Ok(())
}

fn assert_schema_doc_mentions_contract(schema: &str) {
    for required_term in [
        "schema_version",
        "disposition",
        "availability",
        "supported",
        "deferred",
        "planned",
        "available",
        "not_applicable",
        "callable_constructor",
        "executable_cases",
        "issue #573",
    ] {
        assert!(
            schema.contains(required_term),
            "schema README must mention {required_term}"
        );
    }
}

fn expect_manifest_rejection(
    description: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
    expected_error: &str,
) {
    let mut value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    mutate(&mut value);
    let error = parse_and_validate_family_manifest_value(value).expect_err(description);
    assert!(error.contains(expected_error), "{description}: {error}");
}

#[test]
fn family_manifest_covers_requested_qec_families() {
    let manifest = parse_and_validate_family_manifest_text(MANIFEST_TEXT)
        .expect("family manifest should satisfy issue #552");
    assert_schema_doc_mentions_contract(SCHEMA_TEXT);

    let serialized = serde_json::to_string_pretty(&manifest).unwrap();
    assert_eq!(
        format!("{serialized}\n"),
        MANIFEST_TEXT,
        "checked-in manifest should be canonical pretty JSON"
    );
}

#[test]
fn family_manifest_rejects_invalid_entries() {
    expect_manifest_rejection(
        "duplicate family ID",
        |value| {
            value["families"][1]["family_id"] = value["families"][0]["family_id"].clone();
        },
        "duplicate family_id",
    );
    expect_manifest_rejection(
        "missing provenance",
        |value| {
            value["families"][0]
                .as_object_mut()
                .unwrap()
                .remove("provenance");
        },
        "provenance",
    );
    expect_manifest_rejection(
        "unknown disposition",
        |value| {
            value["families"][0]["disposition"] = serde_json::json!("research");
        },
        "unknown variant",
    );
    expect_manifest_rejection(
        "unknown availability",
        |value| {
            value["families"][0]["availability"] = serde_json::json!("prototype");
        },
        "unknown variant",
    );
    expect_manifest_rejection(
        "illegal disposition/availability pair",
        |value| {
            value["families"][0]["availability"] = serde_json::json!("not_applicable");
        },
        "illegal disposition/availability pair",
    );
    expect_manifest_rejection(
        "deferred callable constructor",
        |value| {
            value["families"][6]["callable_constructor"] =
                serde_json::json!({"rust_path": "qec_code::codes::hyperbolic::construct"});
        },
        "cannot declare callable_constructor",
    );
    expect_manifest_rejection(
        "planned callable constructor",
        |value| {
            value["families"][10]["callable_constructor"] =
                serde_json::json!({"rust_path": "qec_code::codes::surface::construct"});
        },
        "cannot declare callable_constructor",
    );
}
