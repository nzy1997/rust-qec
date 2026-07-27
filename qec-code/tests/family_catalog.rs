use std::collections::{BTreeMap, BTreeSet, HashSet};

use qec_code::QecError;
use qec_code::family_contract::{
    CssConstructionSpec, CssFamilySpec, RequestedFamilyId, construct_css,
    parse_css_construction_json, verify_css_orthogonality,
};

const MANIFEST_TEXT: &str = include_str!("fixtures/family_manifest/manifest.v1.json");
const SCHEMA_TEXT: &str = include_str!("fixtures/family_manifest/README.md");

const MANIFEST_SCHEMA_VERSION: u64 = 1;
const MANIFEST_ID: &str = "qec_family_construction_targets_v1";
const PROMOTION_GATE_ISSUE: u64 = 573;
const EXECUTABLE_VERIFIER_NAME: &str = "family_catalog_construct_css_contract_v1";
const EXECUTABLE_VERIFIER_COMMAND: &str = "cargo test -p qec-code --test family_catalog every_supported_family_has_positive_and_negative_cases -- --exact";

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

const SUPPORTED_FAMILY_IDS: &[&str] = &[
    "directional",
    "quantum_tanner",
    "generalized_bicycle",
    "la_cross",
    "random_hgp",
    "lifted_product",
    "coprime_bb",
    "toric_3d",
    "color_666",
    "surface",
    "shor_like",
    "random_two_block",
];

const DEFERRED_FAMILY_IDS: &[&str] = &["hyperbolic_5_5", "perturbed_hgp"];

const DOCUMENTED_NON_FAMILY_CONSTRUCTION_IDS: &[&str] = &[
    "hypergraph_product",
    "legacy_built_in",
    "steane",
    "bb72",
    "apm_kasai",
    "bb",
    "repetition_x",
    "repetition_z",
    "surface_rotated",
    "toric",
];

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum DistanceVerificationClass {
    ConstructorKnownExact,
    ContractMetadata,
    StructuralNotPinned,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct FamilyCatalog {
    schema_version: u64,
    manifest_id: String,
    provenance: Vec<String>,
    verification: Vec<String>,
    intended_consumers: Vec<String>,
    availability_promotion_gate: AvailabilityPromotionGate,
    families: Vec<FamilyCatalogEntry>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct AvailabilityPromotionGate {
    issue: u64,
    rule: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct FamilyCatalogEntry {
    schema_version: u64,
    family_id: String,
    disposition: FamilyDisposition,
    availability: RuntimeAvailability,
    provenance: Vec<String>,
    verification: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    research_contracts: Vec<String>,
    intended_consumers: Vec<String>,
    callable_constructor: Option<CallableConstructorRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    normalized_inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expected: Option<ExpectedStats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    row_weight_summary: Option<RowWeightSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    distance_verification: Option<DistanceVerification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    executable_verifier: Option<ExecutableVerifier>,
    executable_cases: Vec<ExecutableCase>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CallableConstructorRef {
    rust_path: String,
    construction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedStats {
    n: usize,
    m_x: usize,
    m_z: usize,
    rank_x: usize,
    rank_z: usize,
    k: usize,
    d_x: Option<usize>,
    d_z: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RowWeightSummary {
    h_x: Vec<RowWeightBucket>,
    h_z: Vec<RowWeightBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RowWeightBucket {
    weight: usize,
    count: usize,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct DistanceVerification {
    #[serde(rename = "class")]
    class_name: DistanceVerificationClass,
    description: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutableVerifier {
    name: String,
    command: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutableCase {
    case_id: String,
    case_kind: ExecutableCaseKind,
    expected_outcome: ExpectedOutcome,
    description: String,
    verification: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expected_error_contains: Option<String>,
}

fn parse_and_validate_catalog_text(text: &str) -> Result<FamilyCatalog, String> {
    let value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    parse_and_validate_catalog_value(
        value,
        CssFamilySpec::callable_requested_family_ids(),
        CssConstructionSpec::documented_non_family_construction_ids(),
    )
}

fn parse_and_validate_catalog_value(
    value: serde_json::Value,
    callable_family_ids: &[RequestedFamilyId],
    non_family_construction_ids: &[&str],
) -> Result<FamilyCatalog, String> {
    let catalog: FamilyCatalog =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    validate_catalog_with_registries(&catalog, callable_family_ids, non_family_construction_ids)?;
    Ok(catalog)
}

fn validate_catalog_with_registries(
    catalog: &FamilyCatalog,
    callable_family_ids: &[RequestedFamilyId],
    non_family_construction_ids: &[&str],
) -> Result<(), String> {
    validate_global_manifest_fields(catalog)?;
    assert_non_family_construction_registry(non_family_construction_ids)?;

    let callable_family_id_set = callable_family_ids
        .iter()
        .map(|id| id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_family_ids = HashSet::new();
    let mut supported_available = Vec::new();
    let mut deferred = Vec::new();
    let mut supported_case_count = 0usize;

    for entry in &catalog.families {
        if !seen_family_ids.insert(entry.family_id.as_str()) {
            return Err(format!("duplicate family_id {:?}", entry.family_id));
        }
        validate_entry_common_fields(entry)?;
        validate_lifecycle_pair(entry.disposition, entry.availability)?;

        match (entry.disposition, entry.availability) {
            (FamilyDisposition::Supported, RuntimeAvailability::Available) => {
                supported_available.push(entry.family_id.as_str());
                validate_available_family(entry, &callable_family_id_set)?;
                supported_case_count += entry.executable_cases.len();
            }
            (FamilyDisposition::Supported, RuntimeAvailability::Planned) => {
                if entry.callable_constructor.is_some() {
                    return Err(format!(
                        "planned family {:?} cannot declare callable_constructor",
                        entry.family_id
                    ));
                }
                return Err(format!(
                    "supported family {:?} must be availability=available for issue #573",
                    entry.family_id
                ));
            }
            (FamilyDisposition::Deferred, RuntimeAvailability::NotApplicable) => {
                deferred.push(entry.family_id.as_str());
                validate_deferred_family(entry)?;
            }
            _ => return Err("illegal disposition/availability pair".to_owned()),
        }
    }

    let actual_ids = catalog
        .families
        .iter()
        .map(|entry| entry.family_id.as_str())
        .collect::<Vec<_>>();
    if actual_ids != REQUESTED_FAMILY_IDS {
        return Err("families must match the requested family IDs in order".to_owned());
    }
    if supported_available != SUPPORTED_FAMILY_IDS {
        return Err("expected exactly 12 supported available families".to_owned());
    }
    if deferred != DEFERRED_FAMILY_IDS {
        return Err("expected exactly two deferred families".to_owned());
    }
    if supported_case_count < 24 {
        return Err("available families require at least 24 executable cases".to_owned());
    }
    Ok(())
}

fn validate_global_manifest_fields(catalog: &FamilyCatalog) -> Result<(), String> {
    if catalog.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err("unexpected manifest schema_version".to_owned());
    }
    if catalog.manifest_id != MANIFEST_ID {
        return Err("unexpected manifest_id".to_owned());
    }
    expect_nonempty_strings("manifest provenance", &catalog.provenance)?;
    expect_nonempty_strings("manifest verification", &catalog.verification)?;
    expect_nonempty_strings("manifest intended_consumers", &catalog.intended_consumers)?;
    if catalog.availability_promotion_gate.issue != PROMOTION_GATE_ISSUE {
        return Err("unexpected availability promotion gate issue".to_owned());
    }
    if catalog.availability_promotion_gate.rule.trim().is_empty() {
        return Err("availability promotion gate rule must be non-empty".to_owned());
    }
    Ok(())
}

fn validate_entry_common_fields(entry: &FamilyCatalogEntry) -> Result<(), String> {
    if entry.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "family {:?} has unexpected schema_version",
            entry.family_id
        ));
    }
    expect_nonempty_strings("family provenance", &entry.provenance)?;
    expect_nonempty_strings("family verification", &entry.verification)?;
    expect_nonempty_strings("family intended_consumers", &entry.intended_consumers)?;
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

fn validate_available_family(
    entry: &FamilyCatalogEntry,
    callable_family_id_set: &BTreeSet<&str>,
) -> Result<(), String> {
    if !SUPPORTED_FAMILY_IDS.contains(&entry.family_id.as_str()) {
        return Err(format!("family {:?} is not supported", entry.family_id));
    }
    if !callable_family_id_set.contains(entry.family_id.as_str()) {
        return Err(format!(
            "available family {:?} has no callable CssFamilySpec variant",
            entry.family_id
        ));
    }

    let callable = entry.callable_constructor.as_ref().ok_or_else(|| {
        format!(
            "available family {:?} must declare callable_constructor",
            entry.family_id
        )
    })?;
    if callable.construction != entry.family_id {
        return Err(format!(
            "available family {:?} callable constructor construction mismatch",
            entry.family_id
        ));
    }
    if !callable.rust_path.contains("CssFamilySpec::") {
        return Err(format!(
            "available family {:?} callable constructor must use CssFamilySpec",
            entry.family_id
        ));
    }
    expect_nonempty_strings("normalized_inputs", &entry.normalized_inputs)?;
    expect_available_metadata(entry)?;
    validate_supported_cases(&entry.family_id, &entry.executable_cases)?;
    Ok(())
}

fn expect_available_metadata(entry: &FamilyCatalogEntry) -> Result<(), String> {
    let expected = entry.expected.as_ref().ok_or_else(|| {
        format!(
            "available family {:?} missing expected stats",
            entry.family_id
        )
    })?;
    if expected.n == 0 {
        return Err(format!(
            "available family {:?} expected n must be positive",
            entry.family_id
        ));
    }
    let row_weight_summary = entry.row_weight_summary.as_ref().ok_or_else(|| {
        format!(
            "available family {:?} missing row_weight_summary",
            entry.family_id
        )
    })?;
    validate_row_weight_buckets(&row_weight_summary.h_x, "h_x")?;
    validate_row_weight_buckets(&row_weight_summary.h_z, "h_z")?;
    let distance_verification = entry.distance_verification.as_ref().ok_or_else(|| {
        format!(
            "available family {:?} missing distance_verification",
            entry.family_id
        )
    })?;
    if distance_verification.description.trim().is_empty() {
        return Err(format!(
            "available family {:?} has empty distance_verification",
            entry.family_id
        ));
    }
    let executable_verifier = entry.executable_verifier.as_ref().ok_or_else(|| {
        format!(
            "available family {:?} missing executable_verifier",
            entry.family_id
        )
    })?;
    if executable_verifier.name != EXECUTABLE_VERIFIER_NAME
        || executable_verifier.command != EXECUTABLE_VERIFIER_COMMAND
    {
        return Err(format!(
            "available family {:?} has invalid executable_verifier",
            entry.family_id
        ));
    }
    Ok(())
}

fn validate_deferred_family(entry: &FamilyCatalogEntry) -> Result<(), String> {
    if !DEFERRED_FAMILY_IDS.contains(&entry.family_id.as_str()) {
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
    expect_nonempty_strings("deferred research_contracts", &entry.research_contracts)?;
    Ok(())
}

fn validate_supported_cases(family_id: &str, cases: &[ExecutableCase]) -> Result<(), String> {
    let mut has_positive_success = false;
    let mut has_negative_rejection = false;
    let mut seen_case_ids = HashSet::new();

    for case in cases {
        if case.case_id.trim().is_empty() || case.description.trim().is_empty() {
            return Err(format!(
                "available family {family_id:?} has empty executable case text"
            ));
        }
        if !seen_case_ids.insert(case.case_id.as_str()) {
            return Err(format!(
                "available family {family_id:?} has duplicate case_id {:?}",
                case.case_id
            ));
        }
        expect_nonempty_strings("executable case verification", &case.verification)?;
        let request = case.request.as_ref().ok_or_else(|| {
            format!(
                "available family {family_id:?} case {:?} missing request",
                case.case_id
            )
        })?;
        if !request.is_object() {
            return Err(format!(
                "available family {family_id:?} case {:?} request must be an object",
                case.case_id
            ));
        }
        match (case.case_kind, case.expected_outcome) {
            (ExecutableCaseKind::Positive, ExpectedOutcome::Success) => {
                has_positive_success = true;
            }
            (ExecutableCaseKind::Negative, ExpectedOutcome::Rejection) => {
                if case
                    .expected_error_contains
                    .as_ref()
                    .is_none_or(|message| message.trim().is_empty())
                {
                    return Err(format!(
                        "available family {family_id:?} negative case {:?} missing expected_error_contains",
                        case.case_id
                    ));
                }
                has_negative_rejection = true;
            }
            _ => {
                return Err(format!(
                    "available family {family_id:?} has inconsistent executable case {:?}",
                    case.case_id
                ));
            }
        }
    }

    if !has_positive_success {
        return Err(format!(
            "available family {family_id:?} requires at least one positive success case"
        ));
    }
    if !has_negative_rejection {
        return Err(format!(
            "available family {family_id:?} requires at least one negative rejection case"
        ));
    }
    Ok(())
}

fn validate_row_weight_buckets(buckets: &[RowWeightBucket], matrix: &str) -> Result<(), String> {
    if buckets.is_empty() {
        return Err(format!("{matrix} row_weight_summary must be non-empty"));
    }
    let mut previous = None;
    for bucket in buckets {
        if bucket.count == 0 {
            return Err(format!(
                "{matrix} row_weight_summary count must be positive"
            ));
        }
        if let Some(previous) = previous
            && bucket.weight <= previous
        {
            return Err(format!(
                "{matrix} row_weight_summary must be sorted by unique weight"
            ));
        }
        previous = Some(bucket.weight);
    }
    Ok(())
}

fn expect_nonempty_strings(field: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return Err(format!(
            "{field} must be a non-empty array of non-empty strings"
        ));
    }
    Ok(())
}

fn available_families(catalog: &FamilyCatalog) -> Vec<&FamilyCatalogEntry> {
    catalog
        .families
        .iter()
        .filter(|entry| entry.availability == RuntimeAvailability::Available)
        .collect()
}

fn available_family_ids(catalog: &FamilyCatalog) -> Vec<&str> {
    available_families(catalog)
        .into_iter()
        .map(|entry| entry.family_id.as_str())
        .collect()
}

fn deferred_family_ids(catalog: &FamilyCatalog) -> Vec<&str> {
    catalog
        .families
        .iter()
        .filter(|entry| entry.disposition == FamilyDisposition::Deferred)
        .map(|entry| entry.family_id.as_str())
        .collect()
}

fn execute_positive_cases(entry: &FamilyCatalogEntry) {
    let expected = entry.expected.as_ref().expect("validated expected stats");
    let expected_row_weights = entry
        .row_weight_summary
        .as_ref()
        .expect("validated row weight summary");
    let callable = entry
        .callable_constructor
        .as_ref()
        .expect("validated callable constructor");

    for case in entry
        .executable_cases
        .iter()
        .filter(|case| case.case_kind == ExecutableCaseKind::Positive)
    {
        let request = case.request.as_ref().expect("validated request");
        let request_text = serde_json::to_string(request).expect("request should serialize");
        let parsed = parse_css_construction_json(&request_text)
            .unwrap_or_else(|error| panic!("{} failed to parse: {error:?}", case.case_id));
        let result = construct_css(parsed)
            .unwrap_or_else(|error| panic!("{} failed to construct: {error:?}", case.case_id));

        assert_eq!(
            result.requested_family_id.map(RequestedFamilyId::as_str),
            Some(entry.family_id.as_str()),
            "{} requested_family_id mismatch",
            case.case_id
        );
        assert_eq!(
            result.provenance.source, callable.rust_path,
            "{} provenance source mismatch",
            case.case_id
        );
        assert_eq!(
            ExpectedStats {
                n: result.stats.n,
                m_x: result.stats.m_x,
                m_z: result.stats.m_z,
                rank_x: result.stats.rank_x,
                rank_z: result.stats.rank_z,
                k: result.stats.k,
                d_x: result.stats.d_x,
                d_z: result.stats.d_z,
            },
            *expected,
            "{} stats mismatch",
            case.case_id
        );
        assert_eq!(
            row_weight_summary(&result.checks.h_x),
            expected_row_weights.h_x,
            "{} H_X row weights mismatch",
            case.case_id
        );
        assert_eq!(
            row_weight_summary(&result.checks.h_z),
            expected_row_weights.h_z,
            "{} H_Z row weights mismatch",
            case.case_id
        );
        verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z)
            .unwrap_or_else(|error| panic!("{} orthogonality failed: {error:?}", case.case_id));

        let repeated = construct_css(
            parse_css_construction_json(&request_text).expect("request should parse repeatedly"),
        )
        .expect("request should construct repeatedly");
        assert_eq!(
            serde_json::to_string(&result).unwrap(),
            serde_json::to_string(&repeated).unwrap(),
            "{} construction serialization should be deterministic",
            case.case_id
        );
    }
}

fn execute_negative_cases(entry: &FamilyCatalogEntry) {
    for case in entry
        .executable_cases
        .iter()
        .filter(|case| case.case_kind == ExecutableCaseKind::Negative)
    {
        let request = case.request.as_ref().expect("validated request");
        let request_text = serde_json::to_string(request).expect("request should serialize");
        let error = parse_css_construction_json(&request_text)
            .and_then(construct_css)
            .expect_err("negative case should reject");
        let expected = case
            .expected_error_contains
            .as_ref()
            .expect("validated expected error");
        assert_error_contains(&error, expected, &case.case_id);
    }
}

fn row_weight_summary(rows: &[Vec<usize>]) -> Vec<RowWeightBucket> {
    let mut counts = BTreeMap::new();
    for row in rows {
        *counts.entry(row.len()).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .map(|(weight, count)| RowWeightBucket { weight, count })
        .collect()
}

fn assert_error_contains(error: &QecError, expected: &str, case_id: &str) {
    let display = error.to_string();
    let debug = format!("{error:?}");
    assert!(
        display.contains(expected) || debug.contains(expected),
        "{case_id}: expected error to contain {expected:?}, got display={display:?} debug={debug:?}"
    );
}

fn assert_schema_doc_mentions_contract(schema: &str) {
    for required_term in [
        "schema_version",
        "disposition",
        "availability",
        "available",
        "not_applicable",
        "callable_constructor",
        "normalized_inputs",
        "expected",
        "row_weight_summary",
        "distance_verification",
        "executable_verifier",
        "research_contracts",
        "executable_cases",
        "issue #573",
    ] {
        assert!(
            schema.contains(required_term),
            "schema README must mention {required_term}"
        );
    }
}

fn assert_requested_family_bijection(catalog: &FamilyCatalog) {
    let manifest_ids = catalog
        .families
        .iter()
        .map(|entry| entry.family_id.as_str())
        .collect::<Vec<_>>();
    let enum_ids = RequestedFamilyId::ALL
        .iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(manifest_ids, REQUESTED_FAMILY_IDS);
    assert_eq!(enum_ids, REQUESTED_FAMILY_IDS);
    assert_eq!(manifest_ids, enum_ids);
}

fn assert_available_families_match_callable_variants(
    catalog: &FamilyCatalog,
    callable_family_ids: &[RequestedFamilyId],
) {
    let available = available_family_ids(catalog);
    let callable = callable_family_ids
        .iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(available, SUPPORTED_FAMILY_IDS);
    assert_eq!(callable, SUPPORTED_FAMILY_IDS);
    for deferred in DEFERRED_FAMILY_IDS {
        assert!(
            !callable.contains(deferred),
            "deferred family {deferred} must not be callable"
        );
    }
}

fn assert_non_family_construction_registry(ids: &[&str]) -> Result<(), String> {
    if ids != DOCUMENTED_NON_FAMILY_CONSTRUCTION_IDS {
        return Err(format!(
            "undocumented non-family construction alias registry: {ids:?}"
        ));
    }

    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(*id) {
            return Err(format!("duplicate non-family construction alias {id:?}"));
        }
        if REQUESTED_FAMILY_IDS.contains(id) {
            return Err(format!(
                "non-family construction alias {id:?} must not be a requested-family ID"
            ));
        }
    }
    Ok(())
}

fn assert_deferred_families_have_no_runtime_aliases() {
    for construction in ["hyperbolic_5_5", "hyperbolic_5_5_quotient", "perturbed_hgp"] {
        assert_eq!(
            parse_css_construction_json(&format!(
                r#"{{"schema_version":1,"construction":"{construction}"}}"#
            )),
            Err(QecError::UnknownCssConstruction {
                construction: construction.to_owned()
            }),
        );
    }
    for inline in ["hyperbolic_5_5:d=3", "perturbed_hgp:seed=1"] {
        assert!(
            CssConstructionSpec::from_inline(inline).is_err(),
            "deferred inline alias {inline:?} must not parse"
        );
    }
}

fn expect_catalog_rejection(
    description: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
    expected_error: &str,
) {
    let mut value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    mutate(&mut value);
    let error = parse_and_validate_catalog_value(
        value,
        CssFamilySpec::callable_requested_family_ids(),
        CssConstructionSpec::documented_non_family_construction_ids(),
    )
    .expect_err(description);
    assert!(error.contains(expected_error), "{description}: {error}");
}

fn expect_callable_registry_rejection(
    description: &str,
    callable_family_ids: &[RequestedFamilyId],
    expected_error: &str,
) {
    let value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    let error = parse_and_validate_catalog_value(
        value,
        callable_family_ids,
        CssConstructionSpec::documented_non_family_construction_ids(),
    )
    .expect_err(description);
    assert!(error.contains(expected_error), "{description}: {error}");
}

fn expect_construction_registry_rejection(
    description: &str,
    non_family_construction_ids: &[&str],
    expected_error: &str,
) {
    let value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    let error = parse_and_validate_catalog_value(
        value,
        CssFamilySpec::callable_requested_family_ids(),
        non_family_construction_ids,
    )
    .expect_err(description);
    assert!(error.contains(expected_error), "{description}: {error}");
}

#[test]
fn complete_catalog_has_12_supported_and_2_deferred_families() {
    let catalog = parse_and_validate_catalog_text(MANIFEST_TEXT)
        .expect("family catalog should satisfy issue #573");
    assert_schema_doc_mentions_contract(SCHEMA_TEXT);

    assert_eq!(catalog.families.len(), 14);
    assert_eq!(available_family_ids(&catalog), SUPPORTED_FAMILY_IDS);
    assert_eq!(deferred_family_ids(&catalog), DEFERRED_FAMILY_IDS);

    let serialized = serde_json::to_string_pretty(&catalog).unwrap();
    assert_eq!(
        format!("{serialized}\n"),
        MANIFEST_TEXT,
        "checked-in catalog should be canonical pretty JSON"
    );
}

#[test]
fn every_supported_family_has_positive_and_negative_cases() {
    let catalog = parse_and_validate_catalog_text(MANIFEST_TEXT)
        .expect("family catalog should satisfy issue #573");

    let mut case_count = 0usize;
    for family in available_families(&catalog) {
        execute_positive_cases(family);
        execute_negative_cases(family);
        case_count += family.executable_cases.len();
    }
    assert!(case_count >= 24);
}

#[test]
fn catalog_rejects_coverage_gaps() {
    expect_catalog_rejection(
        "missing requested-family ID",
        |value| {
            value["families"].as_array_mut().unwrap().remove(0);
        },
        "families must match",
    );
    expect_catalog_rejection(
        "duplicate requested-family ID",
        |value| {
            value["families"][1]["family_id"] = value["families"][0]["family_id"].clone();
        },
        "duplicate family_id",
    );
    expect_catalog_rejection(
        "third deferred family",
        |value| {
            value["families"][10]["disposition"] = serde_json::json!("deferred");
            value["families"][10]["availability"] = serde_json::json!("not_applicable");
            value["families"][10]["callable_constructor"] = serde_json::Value::Null;
            value["families"][10]["executable_cases"] = serde_json::json!([]);
            value["families"][10]["research_contracts"] =
                serde_json::json!(["qec-code/doc/surface_contract.md"]);
        },
        "is not deferred",
    );
    expect_catalog_rejection(
        "available family without a negative case",
        |value| {
            let cases = value["families"][0]["executable_cases"]
                .as_array_mut()
                .unwrap();
            cases.retain(|case| case["case_kind"] != "negative");
        },
        "requires at least one negative",
    );
    expect_catalog_rejection(
        "available family with unsupported distance verification class",
        |value| {
            value["families"][0]["distance_verification"]["class"] =
                serde_json::json!("external_exact_test");
        },
        "unknown variant",
    );
    expect_catalog_rejection(
        "available family with unrelated executable verifier command",
        |value| {
            value["families"][0]["executable_verifier"]["command"] =
                serde_json::json!("cargo test -p qec-code --test unrelated");
        },
        "invalid executable_verifier",
    );
    expect_catalog_rejection(
        "planned family that claims a callable constructor",
        |value| {
            value["families"][0]["availability"] = serde_json::json!("planned");
        },
        "planned family",
    );
    expect_catalog_rejection(
        "deferred family with a callable stub",
        |value| {
            value["families"][6]["callable_constructor"] = serde_json::json!({
                "rust_path": "CssFamilySpec::Hyperbolic55",
                "construction": "hyperbolic_5_5"
            });
        },
        "deferred family",
    );

    let mut missing_surface_callable = CssFamilySpec::callable_requested_family_ids().to_vec();
    missing_surface_callable.retain(|id| *id != RequestedFamilyId::Surface);
    expect_callable_registry_rejection(
        "available requested family without CssFamilySpec variant",
        &missing_surface_callable,
        "available family \"surface\" has no callable CssFamilySpec variant",
    );

    let mut undocumented_alias =
        CssConstructionSpec::documented_non_family_construction_ids().to_vec();
    undocumented_alias.push("hgp");
    expect_construction_registry_rejection(
        "undocumented utility or legacy alias",
        &undocumented_alias,
        "undocumented non-family construction alias",
    );
}

#[test]
fn requested_and_construction_registries_are_disjoint_and_complete() {
    let catalog = parse_and_validate_catalog_text(MANIFEST_TEXT)
        .expect("family catalog should satisfy issue #573");

    assert_requested_family_bijection(&catalog);
    assert_available_families_match_callable_variants(
        &catalog,
        CssFamilySpec::callable_requested_family_ids(),
    );
    assert_non_family_construction_registry(
        CssConstructionSpec::documented_non_family_construction_ids(),
    )
    .unwrap();
    assert_deferred_families_have_no_runtime_aliases();
}
