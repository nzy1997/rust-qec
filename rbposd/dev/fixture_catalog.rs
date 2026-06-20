use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FixtureCatalog {
    pub fixtures: Vec<FixtureCatalogEntry>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    BpOption,
    Lsd,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FixtureCatalogEntry {
    pub id: String,
    pub kind: FixtureKind,
    pub decoder: String,
    pub path: String,
    pub matrix_path: String,
    pub syndrome_path: String,
    pub provenance: String,
    pub verifier: String,
    pub pass_condition: String,
    pub consumes: Vec<String>,
    pub modes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixtureCatalogEntry {
    pub id: String,
    pub kind: FixtureKind,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct LsdFixtureFile {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ParityFixtureFile {
    name: String,
    config: ParityFixtureConfig,
}

#[derive(Debug, Deserialize)]
struct ParityFixtureConfig {
    bp_variant: String,
    schedule: String,
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

pub fn catalog_path() -> PathBuf {
    fixture_root().join("catalog.json")
}

pub fn load_catalog(path: &Path) -> FixtureCatalog {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

pub fn validate_catalog(
    catalog: &FixtureCatalog,
    fixture_root: &Path,
) -> Result<Vec<ValidatedFixtureCatalogEntry>, String> {
    if catalog.fixtures.is_empty() {
        return Err("fixture catalog fixtures must not be empty".to_string());
    }

    let required_lsd = fixture_group_files(&fixture_root.join("lsd"))?;
    let required_bp = required_bp_catalog_paths(&fixture_root.join("parity"))?;
    let required_paths = required_lsd
        .iter()
        .chain(required_bp.iter())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut seen_ids = BTreeSet::new();
    let mut seen_paths = BTreeSet::new();
    let mut entries_by_path = BTreeMap::new();
    let mut validated = Vec::new();

    for entry in &catalog.fixtures {
        if entry.id.trim().is_empty() {
            return Err("fixture catalog entry id must not be empty".to_string());
        }
        if entry.decoder.trim().is_empty() {
            return Err(format!(
                "fixture catalog entry {} decoder must not be empty",
                entry.id
            ));
        }
        if entry.path.trim().is_empty() {
            return Err(format!(
                "fixture catalog entry {} path must not be empty",
                entry.id
            ));
        }
        if entry.matrix_path.trim().is_empty() || !entry.matrix_path.starts_with(&entry.path) {
            return Err(format!(
                "fixture catalog entry {} matrix_path must reference its fixture path",
                entry.id
            ));
        }
        if entry.syndrome_path.trim().is_empty() || !entry.syndrome_path.starts_with(&entry.path) {
            return Err(format!(
                "fixture catalog entry {} syndrome_path must reference its fixture path",
                entry.id
            ));
        }
        if entry.provenance.trim().is_empty() {
            return Err(format!(
                "fixture catalog entry {} provenance must not be empty",
                entry.id
            ));
        }
        if entry.verifier.trim().is_empty() {
            return Err(format!(
                "fixture catalog entry {} verifier must not be empty",
                entry.id
            ));
        }
        if entry.pass_condition.trim().is_empty() {
            return Err(format!(
                "fixture catalog entry {} pass_condition must not be empty",
                entry.id
            ));
        }
        if entry.consumes.is_empty() || !entry.consumes.iter().any(|value| value == "#98") {
            return Err(format!(
                "fixture catalog entry {} must consume #98",
                entry.id
            ));
        }
        if entry.modes.is_empty() {
            return Err(format!(
                "fixture catalog entry {} modes must not be empty",
                entry.id
            ));
        }
        if !seen_ids.insert(entry.id.clone()) {
            return Err(format!("duplicate fixture catalog id {}", entry.id));
        }
        if !seen_paths.insert(entry.path.clone()) {
            return Err(format!("duplicate fixture catalog path {}", entry.path));
        }

        let full_path = fixture_root.join(&entry.path);
        if !full_path.exists() {
            return Err(format!(
                "fixture catalog entry {} points to missing fixture {}",
                entry.id, entry.path
            ));
        }

        match entry.kind {
            FixtureKind::Lsd => {
                if !entry.path.starts_with("lsd/") {
                    return Err(format!(
                        "fixture catalog entry {} must point under lsd/",
                        entry.id
                    ));
                }
                if entry.decoder != "bp_lsd" {
                    return Err(format!(
                        "fixture catalog entry {} decoder must be bp_lsd",
                        entry.id
                    ));
                }
                let fixture: LsdFixtureFile = load_fixture_file(&full_path)?;
                if fixture.id != entry.id {
                    return Err(format!(
                        "fixture catalog id {} does not match fixture id {} in {}",
                        entry.id, fixture.id, entry.path
                    ));
                }
                if !required_lsd.contains(&entry.path) {
                    return Err(format!(
                        "fixture catalog entry {} has no checked-in LSD fixture requirement",
                        entry.id
                    ));
                }
            }
            FixtureKind::BpOption => {
                if !entry.path.starts_with("parity/") {
                    return Err(format!(
                        "fixture catalog entry {} must point under parity/",
                        entry.id
                    ));
                }
                if entry.decoder != "bp_osd" {
                    return Err(format!(
                        "fixture catalog entry {} decoder must be bp_osd",
                        entry.id
                    ));
                }
                let fixture: ParityFixtureFile = load_fixture_file(&full_path)?;
                if fixture.name != entry.id {
                    return Err(format!(
                        "fixture catalog id {} does not match fixture name {} in {}",
                        entry.id, fixture.name, entry.path
                    ));
                }
                if fixture.config.bp_variant == "minimum_sum"
                    && fixture.config.schedule == "parallel"
                {
                    return Err(format!(
                        "fixture catalog entry {} must point to a non-default BP config fixture",
                        entry.id
                    ));
                }
                if !required_bp.contains(&entry.path) {
                    return Err(format!(
                        "fixture catalog entry {} has no checked-in BP-option fixture requirement",
                        entry.id
                    ));
                }
            }
        }

        entries_by_path.insert(entry.path.clone(), entry.kind);
        validated.push(ValidatedFixtureCatalogEntry {
            id: entry.id.clone(),
            kind: entry.kind,
            path: full_path,
        });
    }

    for path in &required_paths {
        if !entries_by_path.contains_key(path) {
            return Err(format!("missing fixture catalog entry for {path}"));
        }
    }
    for path in entries_by_path.keys() {
        if !required_paths.contains(path) {
            return Err(format!(
                "fixture catalog entry has no checked-in required fixture {path}"
            ));
        }
    }

    validated.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
    Ok(validated)
}

fn fixture_group_files(dir: &Path) -> Result<BTreeSet<String>, String> {
    let dir_name = dir
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("failed to read fixture directory name {}", dir.display()))?
        .to_string();
    let mut files = fs::read_dir(dir)
        .map_err(|error| format!("failed to read {}: {error}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read fixture entry: {error}"))?;
    files.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("json"));
    files.sort();

    Ok(files
        .into_iter()
        .filter_map(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(|file_name| format!("{dir_name}/{file_name}"))
        })
        .collect())
}

fn required_bp_catalog_paths(dir: &Path) -> Result<BTreeSet<String>, String> {
    let mut required = BTreeSet::new();
    let mut files = fs::read_dir(dir)
        .map_err(|error| format!("failed to read {}: {error}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read fixture entry: {error}"))?;
    files.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("json"));
    files.sort();

    for path in files {
        let fixture: ParityFixtureFile = load_fixture_file(&path)?;
        if fixture.config.bp_variant != "minimum_sum" || fixture.config.schedule != "parallel" {
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| {
                    format!("failed to read parity fixture file name {}", path.display())
                })?;
            required.insert(format!("parity/{file_name}"));
        }
    }

    Ok(required)
}

fn load_fixture_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}
