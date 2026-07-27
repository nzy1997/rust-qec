use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use crate::error::{CssMatrixReadSource, QecError};
use crate::family_contract::{
    CssCodeStats, CssConstructionResult, RequestedFamilyId, construct_css,
    parse_css_construction_json, verify_css_orthogonality,
};

const MANIFEST_REL_PATH: &str = "tests/fixtures/family_manifest/manifest.v1.json";
const EXPECTED_FAMILY_IDS: [&str; 14] = [
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
const EXPECTED_SUPPORTED_FAMILIES: usize = 12;
const EXPECTED_DEFERRED_FAMILIES: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyVerificationReport {
    pub output: String,
    pub failed: usize,
}

pub fn verify_checked_in_family_manifest() -> Result<FamilyVerificationReport, QecError> {
    let path = checked_in_manifest_path();
    let text = fs::read_to_string(&path).map_err(|error| QecError::CssMatrixReadFailed {
        path: path.display().to_string(),
        source: CssMatrixReadSource(error.to_string()),
    })?;
    Ok(verify_family_manifest_text(&text))
}

pub fn verify_family_manifest_text(text: &str) -> FamilyVerificationReport {
    match serde_json::from_str::<FamilyCatalog>(text) {
        Ok(catalog) => verify_catalog(&catalog),
        Err(error) => failure_report(format!("FAIL manifest invalid_json={error}"), 0, 0),
    }
}

fn checked_in_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(MANIFEST_REL_PATH)
}

#[derive(Debug, Deserialize)]
struct FamilyCatalog {
    families: Vec<FamilyCatalogEntry>,
}

#[derive(Debug, Deserialize)]
struct FamilyCatalogEntry {
    family_id: String,
    disposition: FamilyDisposition,
    availability: RuntimeAvailability,
    #[serde(default)]
    research_contracts: Vec<String>,
    callable_constructor: Option<CallableConstructorRef>,
    expected: Option<ExpectedStats>,
    row_weight_summary: Option<RowWeightSummary>,
    #[serde(default)]
    executable_cases: Vec<ExecutableCase>,
}

#[derive(Debug, Deserialize)]
struct CallableConstructorRef {
    rust_path: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct RowWeightSummary {
    h_x: Vec<RowWeightBucket>,
    h_z: Vec<RowWeightBucket>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct RowWeightBucket {
    weight: usize,
    count: usize,
}

#[derive(Debug, Deserialize)]
struct ExecutableCase {
    case_kind: ExecutableCaseKind,
    expected_outcome: ExpectedOutcome,
    request: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FamilyDisposition {
    Supported,
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeAvailability {
    Planned,
    Available,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExecutableCaseKind {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExpectedOutcome {
    Success,
    Rejection,
}

enum EntryVerification {
    Line(String),
    Failure(String),
}

fn verify_catalog(catalog: &FamilyCatalog) -> FamilyVerificationReport {
    let supported = catalog
        .families
        .iter()
        .filter(|entry| entry.disposition == FamilyDisposition::Supported)
        .count();
    let deferred = catalog
        .families
        .iter()
        .filter(|entry| entry.disposition == FamilyDisposition::Deferred)
        .count();
    let mut lines = manifest_contract_failures(catalog, supported, deferred);
    let mut failed = lines.len();

    for entry in &catalog.families {
        match verify_entry(entry) {
            EntryVerification::Line(line) => lines.push(line),
            EntryVerification::Failure(line) => {
                failed += 1;
                lines.push(line);
            }
        }
    }

    let status = if failed == 0 { "PASS" } else { "FAIL" };
    lines.push(format!(
        "SUMMARY {status} supported={supported} deferred={deferred} failed={failed}"
    ));
    FamilyVerificationReport {
        output: lines.join("\n"),
        failed,
    }
}

fn manifest_contract_failures(
    catalog: &FamilyCatalog,
    supported: usize,
    deferred: usize,
) -> Vec<String> {
    let mut failures = Vec::new();

    for family_id in EXPECTED_FAMILY_IDS {
        let occurrences = catalog
            .families
            .iter()
            .filter(|entry| entry.family_id == family_id)
            .count();
        match occurrences {
            0 => failures.push(format!("FAIL manifest missing family_id={family_id}")),
            1 => {}
            _ => failures.push(format!("FAIL manifest duplicate family_id={family_id}")),
        }
    }

    for entry in &catalog.families {
        if !EXPECTED_FAMILY_IDS.contains(&entry.family_id.as_str()) {
            failures.push(format!(
                "FAIL manifest unexpected family_id={}",
                entry.family_id
            ));
        }
    }

    for (index, entry) in catalog.families.iter().enumerate() {
        let expected = EXPECTED_FAMILY_IDS.get(index).copied().unwrap_or("none");
        if entry.family_id != expected {
            failures.push(format!(
                "FAIL manifest family_id_order index={index} expected={expected} actual={}",
                entry.family_id
            ));
        }
    }

    if supported != EXPECTED_SUPPORTED_FAMILIES {
        failures.push(format!(
            "FAIL manifest expected supported={EXPECTED_SUPPORTED_FAMILIES} actual supported={supported}"
        ));
    }
    if deferred != EXPECTED_DEFERRED_FAMILIES {
        failures.push(format!(
            "FAIL manifest expected deferred={EXPECTED_DEFERRED_FAMILIES} actual deferred={deferred}"
        ));
    }

    failures
}

fn verify_entry(entry: &FamilyCatalogEntry) -> EntryVerification {
    match entry.disposition {
        FamilyDisposition::Supported => verify_supported_entry(entry),
        FamilyDisposition::Deferred => verify_deferred_entry(entry),
    }
}

fn verify_supported_entry(entry: &FamilyCatalogEntry) -> EntryVerification {
    if entry.availability != RuntimeAvailability::Available {
        return failure(format!(
            "FAIL {} disposition=supported availability={} expected=available",
            entry.family_id,
            availability_name(entry.availability)
        ));
    }

    let Some(case) = entry.executable_cases.iter().find(|case| {
        case.case_kind == ExecutableCaseKind::Positive
            && case.expected_outcome == ExpectedOutcome::Success
    }) else {
        return failure(format!(
            "FAIL {} missing positive success case",
            entry.family_id
        ));
    };
    let Some(request) = case.request.as_ref() else {
        return failure(format!(
            "FAIL {} positive success case missing request",
            entry.family_id
        ));
    };
    let request_text = match serde_json::to_string(request) {
        Ok(text) => text,
        Err(error) => return failure(format!("FAIL {} request_json={error}", entry.family_id)),
    };
    let result = match parse_css_construction_json(&request_text).and_then(construct_css) {
        Ok(result) => result,
        Err(error) => return failure(format!("FAIL {} construction={error}", entry.family_id)),
    };

    if let Err(error) =
        verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z)
    {
        return failure(format!("FAIL {} orthogonality={error}", entry.family_id));
    }

    if let Some(line) = verify_result_metadata(entry, &result) {
        return failure(line);
    }

    EntryVerification::Line(format_pass_line(&entry.family_id, &result))
}

fn verify_result_metadata(
    entry: &FamilyCatalogEntry,
    result: &CssConstructionResult,
) -> Option<String> {
    if result.requested_family_id.map(RequestedFamilyId::as_str) != Some(entry.family_id.as_str()) {
        return Some(format!(
            "FAIL {} expected requested_family_id={} actual requested_family_id={}",
            entry.family_id,
            entry.family_id,
            result
                .requested_family_id
                .map(RequestedFamilyId::as_str)
                .unwrap_or("none")
        ));
    }

    let Some(callable) = entry.callable_constructor.as_ref() else {
        return Some(format!(
            "FAIL {} missing callable_constructor",
            entry.family_id
        ));
    };
    if result.provenance.source != callable.rust_path {
        return Some(format!(
            "FAIL {} expected provenance={} actual provenance={}",
            entry.family_id, callable.rust_path, result.provenance.source
        ));
    }

    let Some(expected) = entry.expected.as_ref() else {
        return Some(format!("FAIL {} missing expected stats", entry.family_id));
    };
    if let Some(line) = stats_mismatch(&entry.family_id, expected, &result.stats) {
        return Some(line);
    }

    let Some(expected_weights) = entry.row_weight_summary.as_ref() else {
        return Some(format!(
            "FAIL {} missing row_weight_summary",
            entry.family_id
        ));
    };
    let actual_h_x = row_weight_summary(&result.checks.h_x);
    if actual_h_x != expected_weights.h_x {
        return Some(format!(
            "FAIL {} expected row_weights_h_x={} actual row_weights_h_x={}",
            entry.family_id,
            format_row_weights(&expected_weights.h_x),
            format_row_weights(&actual_h_x)
        ));
    }
    let actual_h_z = row_weight_summary(&result.checks.h_z);
    if actual_h_z != expected_weights.h_z {
        return Some(format!(
            "FAIL {} expected row_weights_h_z={} actual row_weights_h_z={}",
            entry.family_id,
            format_row_weights(&expected_weights.h_z),
            format_row_weights(&actual_h_z)
        ));
    }

    None
}

fn stats_mismatch(
    family_id: &str,
    expected: &ExpectedStats,
    actual: &CssCodeStats,
) -> Option<String> {
    macro_rules! compare_stat {
        ($field:ident) => {
            if expected.$field != actual.$field {
                return Some(format!(
                    "FAIL {family_id} expected {}={} actual {}={}",
                    stringify!($field),
                    expected.$field,
                    stringify!($field),
                    actual.$field
                ));
            }
        };
    }

    compare_stat!(n);
    compare_stat!(m_x);
    compare_stat!(m_z);
    compare_stat!(rank_x);
    compare_stat!(rank_z);
    compare_stat!(k);
    if expected.d_x != actual.d_x {
        return Some(format!(
            "FAIL {family_id} expected d_x={:?} actual d_x={:?}",
            expected.d_x, actual.d_x
        ));
    }
    if expected.d_z != actual.d_z {
        return Some(format!(
            "FAIL {family_id} expected d_z={:?} actual d_z={:?}",
            expected.d_z, actual.d_z
        ));
    }
    None
}

fn verify_deferred_entry(entry: &FamilyCatalogEntry) -> EntryVerification {
    if entry.availability != RuntimeAvailability::NotApplicable {
        return failure(format!(
            "FAIL {} disposition=deferred availability={} expected=not_applicable",
            entry.family_id,
            availability_name(entry.availability)
        ));
    }
    if entry.research_contracts.len() != 1 {
        return failure(format!(
            "FAIL {} expected exactly one research_contract",
            entry.family_id
        ));
    }
    let tracking_issue = match entry.family_id.as_str() {
        "hyperbolic_5_5" => 571,
        "perturbed_hgp" => 572,
        _ => return failure(format!("FAIL {} unknown deferred family", entry.family_id)),
    };
    EntryVerification::Line(format!(
        "DEFERRED {} tracking_issue=#{tracking_issue} contract={}",
        entry.family_id, entry.research_contracts[0]
    ))
}

fn format_pass_line(family_id: &str, result: &CssConstructionResult) -> String {
    let params = serde_json::to_string(&result.normalized_parameters)
        .expect("normalized CSS construction parameters should serialize");
    format!(
        "PASS {family_id} params={params} n={} checks=h_x:{},h_z:{} ranks=rank_x:{},rank_z:{} k={} row_weights=h_x:{},h_z:{} orthogonal=true provenance={}@{}",
        result.stats.n,
        result.stats.m_x,
        result.stats.m_z,
        result.stats.rank_x,
        result.stats.rank_z,
        result.stats.k,
        format_row_weights(&row_weight_summary(&result.checks.h_x)),
        format_row_weights(&row_weight_summary(&result.checks.h_z)),
        result.provenance.source,
        result.provenance.normalized_input_digest,
    )
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

fn format_row_weights(buckets: &[RowWeightBucket]) -> String {
    let values = buckets
        .iter()
        .map(|bucket| format!("w{}={}", bucket.weight, bucket.count))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(","))
}

fn availability_name(availability: RuntimeAvailability) -> &'static str {
    match availability {
        RuntimeAvailability::Planned => "planned",
        RuntimeAvailability::Available => "available",
        RuntimeAvailability::NotApplicable => "not_applicable",
    }
}

fn failure(line: String) -> EntryVerification {
    EntryVerification::Failure(line)
}

fn failure_report(line: String, supported: usize, deferred: usize) -> FamilyVerificationReport {
    FamilyVerificationReport {
        output: format!("{line}\nSUMMARY FAIL supported={supported} deferred={deferred} failed=1"),
        failed: 1,
    }
}
