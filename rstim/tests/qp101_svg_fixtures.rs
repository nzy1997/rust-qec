use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use rstim::parser::parse_lines;
use rstim::qp101::Qp101Document;
use serde::Deserialize;

const MIN_CASES: usize = 6;

#[derive(Debug, Clone, Deserialize)]
struct FixtureManifest {
    version: u32,
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct FixtureCase {
    id: String,
    provenance: String,
    source_kind: String,
    input_path: String,
    expected_semantic_markers: Vec<SemanticMarker>,
}

#[derive(Debug, Clone, Deserialize)]
struct SemanticMarker {
    kind: String,
    value: String,
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("qp101_svg")
}

fn manifest_path() -> PathBuf {
    fixture_dir().join("manifest.json")
}

fn load_manifest() -> FixtureManifest {
    let path = manifest_path();
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read manifest {}: {err}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("failed to parse manifest {}: {err}", path.display()))
}

fn validate_manifest(manifest: &FixtureManifest, base_dir: &Path) -> Result<(), String> {
    if manifest.version != 1 {
        return Err(format!(
            "manifest version must be 1, got {}",
            manifest.version
        ));
    }
    if manifest.cases.len() < MIN_CASES {
        return Err(format!(
            "manifest must contain at least {MIN_CASES} cases, got {}",
            manifest.cases.len()
        ));
    }

    let mut ids = BTreeSet::new();
    for case in &manifest.cases {
        validate_case(case, base_dir)?;
        if !ids.insert(case.id.as_str()) {
            return Err(format!("case {} has duplicate id", case.id));
        }
    }

    Ok(())
}

fn validate_case(case: &FixtureCase, base_dir: &Path) -> Result<(), String> {
    let case_id = case.id.as_str();
    if !is_stable_id(case_id) {
        return Err(format!("case {case_id} has invalid id"));
    }
    if case.provenance.trim().is_empty() {
        return Err(format!("case {case_id} is missing provenance"));
    }
    if case.input_path.trim().is_empty() {
        return Err(format!("case {case_id} is missing input path"));
    }
    if Path::new(&case.input_path).is_absolute() {
        return Err(format!("case {case_id} input path must be relative"));
    }
    if case.expected_semantic_markers.is_empty() {
        return Err(format!("case {case_id} has no expected semantic markers"));
    }
    for marker in &case.expected_semantic_markers {
        if marker.kind.trim().is_empty() || marker.value.trim().is_empty() {
            return Err(format!(
                "case {case_id} has an empty expected semantic marker"
            ));
        }
    }

    let input_path = base_dir.join(&case.input_path);
    if !input_path.exists() {
        return Err(format!(
            "case {case_id} input path does not exist: {}",
            input_path.display()
        ));
    }
    let text = fs::read_to_string(&input_path).map_err(|err| {
        format!(
            "case {case_id} failed to read {}: {err}",
            input_path.display()
        )
    })?;

    match case.source_kind.as_str() {
        "stim" => {
            parse_lines(&text)
                .map_err(|err| format!("case {case_id} failed to parse Stim input: {err}"))?;
        }
        "qp101_json" => {
            serde_json::from_str::<Qp101Document>(&text)
                .map_err(|err| format!("case {case_id} failed to parse QP101 JSON input: {err}"))?;
        }
        other => {
            return Err(format!(
                "case {case_id} has unsupported source_kind {other}"
            ));
        }
    }

    Ok(())
}

fn is_stable_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn assert_invalid_case_names_id(mut case: FixtureCase, mutate: impl FnOnce(&mut FixtureCase)) {
    mutate(&mut case);
    let expected_id = case.id.clone();
    let err = validate_case(&case, &fixture_dir()).expect_err("malformed case should fail");
    assert!(
        err.contains(&expected_id),
        "error should name bad case id {expected_id}, got {err}"
    );
}

#[test]
fn qp101_svg_fixture_manifest_is_valid() {
    let manifest = load_manifest();
    validate_manifest(&manifest, &fixture_dir())
        .expect("QP101 SVG fixture manifest should be valid");

    let first_case = manifest
        .cases
        .first()
        .expect("positive manifest should contain a case")
        .clone();

    assert_invalid_case_names_id(first_case.clone(), |case| {
        case.input_path = "missing-input.stim".to_string();
    });
    assert_invalid_case_names_id(first_case.clone(), |case| {
        case.expected_semantic_markers.clear();
    });
    assert_invalid_case_names_id(first_case, |case| {
        case.source_kind = "typst".to_string();
    });
}
