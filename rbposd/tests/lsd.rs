use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use rbposd::{
    BpLsdDecoder, ChannelModel, Correction, DecodeError, LsdConfig, ParityCheckMatrix, Syndrome,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct LsdFixture {
    id: String,
    matrix: MatrixFixture,
    channel: ChannelFixture,
    syndrome: Vec<bool>,
    lsd_order: usize,
    expected: ExpectedFixture,
}

#[derive(Debug, Deserialize)]
struct MatrixFixture {
    num_checks: usize,
    num_bits: usize,
    rows: Vec<Vec<usize>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ChannelFixture {
    Bsc { error_rate: f64 },
    BitFlipProbabilities { probabilities: Vec<f64> },
}

#[derive(Debug, Deserialize)]
struct ExpectedFixture {
    status: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    order_0_correction: Option<Vec<bool>>,
    #[serde(default)]
    order_1_correction: Option<Vec<bool>>,
}

#[derive(Debug, Clone, Deserialize)]
struct LsdFixtureManifest {
    fixtures: Vec<LsdFixtureManifestEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct LsdFixtureManifestEntry {
    id: String,
    path: String,
    provenance: String,
    verifier: String,
    pass_condition: String,
    consumes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidatedLsdManifestEntry {
    id: String,
    path: PathBuf,
}

impl LsdFixture {
    fn load(name: &str) -> Self {
        Self::load_from_path(&lsd_fixture_dir().join(name))
    }

    fn load_from_path(path: &Path) -> Self {
        let contents = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_str(&contents)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }

    fn pcm(&self) -> ParityCheckMatrix {
        ParityCheckMatrix::from_sparse_rows(
            self.matrix.num_checks,
            self.matrix.num_bits,
            self.matrix.rows.clone(),
        )
        .unwrap_or_else(|error| panic!("invalid matrix in {}: {error}", self.id))
    }

    fn channel(&self) -> ChannelModel {
        match &self.channel {
            ChannelFixture::Bsc { error_rate } => ChannelModel::Bsc {
                error_rate: *error_rate,
            },
            ChannelFixture::BitFlipProbabilities { probabilities } => {
                ChannelModel::BitFlipProbabilities(probabilities.clone())
            }
        }
    }

    fn syndrome(&self) -> Syndrome {
        Syndrome::from(self.syndrome.clone())
    }

    fn lsd_config(&self) -> LsdConfig {
        LsdConfig {
            lsd_order: self.lsd_order,
            ..LsdConfig::default()
        }
    }
}

impl LsdFixtureManifest {
    fn load(path: &Path) -> Self {
        let contents = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_str(&contents)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }
}

fn lsd_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("lsd")
}

fn lsd_manifest_path() -> PathBuf {
    lsd_fixture_dir().join("manifest.json")
}

fn assert_manifest_error(manifest: &LsdFixtureManifest, needle: &str) {
    let error = validate_lsd_fixture_manifest(manifest, &lsd_fixture_dir()).unwrap_err();
    assert!(
        error.contains(needle),
        "expected manifest error containing {needle:?}, got {error:?}"
    );
}

fn validate_lsd_fixture_manifest(
    manifest: &LsdFixtureManifest,
    fixture_dir: &Path,
) -> Result<Vec<ValidatedLsdManifestEntry>, String> {
    if manifest.fixtures.is_empty() {
        return Err("manifest fixtures must not be empty".to_string());
    }

    let mut fixture_files = fs::read_dir(fixture_dir)
        .map_err(|error| format!("failed to read {}: {error}", fixture_dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read fixture entry: {error}"))?;
    fixture_files.retain(|path| {
        path.extension().and_then(|value| value.to_str()) == Some("json")
            && path.file_name().and_then(|value| value.to_str()) != Some("manifest.json")
    });
    fixture_files.sort();

    let checked_in_paths = fixture_files
        .iter()
        .map(|path| {
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        })
        .collect::<BTreeSet<_>>();

    let mut seen_ids = BTreeSet::new();
    let mut seen_paths = BTreeSet::new();
    let mut entries_by_path = BTreeMap::new();
    let mut validated = Vec::new();

    for entry in &manifest.fixtures {
        if entry.id.trim().is_empty() {
            return Err("manifest entry id must not be empty".to_string());
        }
        if entry.path.trim().is_empty() {
            return Err(format!("manifest entry {} path must not be empty", entry.id));
        }
        if entry.provenance.trim().is_empty() {
            return Err(format!("manifest entry {} provenance must not be empty", entry.id));
        }
        if entry.verifier.trim().is_empty() {
            return Err(format!("manifest entry {} verifier must not be empty", entry.id));
        }
        if entry.pass_condition.trim().is_empty() {
            return Err(format!(
                "manifest entry {} pass_condition must not be empty",
                entry.id
            ));
        }
        if entry.consumes.is_empty() || !entry.consumes.iter().any(|value| value == "#90") {
            return Err(format!("manifest entry {} must consume #90", entry.id));
        }
        if !seen_ids.insert(entry.id.clone()) {
            return Err(format!("duplicate manifest id {}", entry.id));
        }
        if !seen_paths.insert(entry.path.clone()) {
            return Err(format!("duplicate manifest path {}", entry.path));
        }

        let full_path = fixture_dir.join(&entry.path);
        if !full_path.exists() {
            return Err(format!(
                "manifest entry {} points to missing fixture {}",
                entry.id, entry.path
            ));
        }

        let fixture = LsdFixture::load_from_path(&full_path);
        if fixture.id != entry.id {
            return Err(format!(
                "manifest id {} does not match fixture id {} in {}",
                entry.id, fixture.id, entry.path
            ));
        }

        entries_by_path.insert(entry.path.clone(), entry.id.clone());
        validated.push(ValidatedLsdManifestEntry {
            id: entry.id.clone(),
            path: full_path,
        });
    }

    for fixture_path in &checked_in_paths {
        if !entries_by_path.contains_key(fixture_path) {
            return Err(format!("missing manifest entry for {fixture_path}"));
        }
    }
    for entry_path in entries_by_path.keys() {
        if !checked_in_paths.contains(entry_path) {
            return Err(format!("manifest entry has no checked-in fixture {entry_path}"));
        }
    }

    validated.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
    Ok(validated)
}

#[test]
fn bplsddecoder_public_api_matches_reference_contract() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(!result.used_osd);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_clone_preserves_decoding_behavior_with_fresh_workspaces() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![1, 2], vec![0]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let cloned = decoder.clone();
    let syndrome = Syndrome::from(vec![true, false]);
    let first = decoder.decode(&syndrome).unwrap();
    let second = cloned.decode(&syndrome).unwrap();

    assert_eq!(second, first);
    assert_eq!(pcm.multiply(&second.correction), syndrome);
}

#[test]
fn bplsd_decoder_reuse_returns_valid_solutions_for_multiple_syndromes() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![1, 2], vec![0]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig {
            lsd_order: 1,
            ..LsdConfig::default()
        },
    )
    .unwrap();

    for syndrome in [
        Syndrome::from(vec![true, false]),
        Syndrome::from(vec![false, true]),
        Syndrome::from(vec![true, true]),
    ] {
        let result = decoder
            .decode(&syndrome)
            .unwrap_or_else(|error| panic!("failed to decode {syndrome:?}: {error}"));

        assert!(!result.used_osd);
        assert_eq!(result.residual_syndrome_weight, 0);
        assert_eq!(pcm.multiply(&result.correction), syndrome);
    }
}

#[test]
fn bplsddecoder_rejects_syndrome_length_mismatch() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm,
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let err = decoder.decode(&Syndrome::from(vec![true])).unwrap_err();

    assert_eq!(
        err,
        DecodeError::DimensionMismatch {
            what: "syndrome",
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn bplsddecoder_zero_syndrome_uses_prior_fast_path() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.9, 0.9]),
        LsdConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.bp_iterations, 0);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(result.correction, Correction::from(vec![true, true]));
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_order_zero_fallback_repairs_bp_residual_without_osd() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![1, 2], vec![0]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(!result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.bp_iterations, 30);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(
        result.correction,
        Correction::from(vec![false, true, false])
    );
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_rejects_channel_length_mismatch() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();

    let err = BpLsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2]),
        LsdConfig::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        DecodeError::DimensionMismatch {
            what: "channel probabilities",
            expected: 3,
            actual: 2,
        }
    );
}

#[test]
fn bplsd_order_one_recovers_the_borrowed_small_matrix_cases() {
    for fixture_name in [
        "lsd_small_sparse_code.json",
        "lsd_order_one_improves_over_baseline.json",
    ] {
        let fixture = LsdFixture::load(fixture_name);
        assert_eq!(fixture.expected.status, "success");

        let pcm = fixture.pcm();
        let syndrome = fixture.syndrome();
        let decoder = BpLsdDecoder::new(pcm.clone(), fixture.channel(), fixture.lsd_config())
            .unwrap_or_else(|error| {
                panic!("failed to construct decoder for {}: {error}", fixture.id)
            });
        let result = decoder
            .decode(&syndrome)
            .unwrap_or_else(|error| panic!("failed to decode {}: {error}", fixture.id));

        assert!(
            !result.used_osd,
            "fixture {} unexpectedly used OSD",
            fixture.id
        );
        assert_eq!(result.residual_syndrome_weight, 0, "fixture {}", fixture.id);
        assert_eq!(
            pcm.multiply(&result.correction),
            syndrome,
            "fixture {}",
            fixture.id
        );

        if let Some(expected_order_1) = fixture.expected.order_1_correction.clone() {
            let expected_order_1 = Correction::from(expected_order_1);
            assert_eq!(
                result.correction, expected_order_1,
                "fixture {}",
                fixture.id
            );
        }

        if let Some(expected_order_0) = fixture.expected.order_0_correction.clone() {
            let order_0_decoder = BpLsdDecoder::new(
                pcm.clone(),
                fixture.channel(),
                LsdConfig {
                    lsd_order: 0,
                    ..LsdConfig::default()
                },
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to construct order-0 decoder for {}: {error}",
                    fixture.id
                )
            });
            let order_0_result = order_0_decoder.decode(&syndrome).unwrap_or_else(|error| {
                panic!("failed order-0 decode for {}: {error}", fixture.id)
            });
            let expected_order_0 = Correction::from(expected_order_0);
            assert_eq!(
                order_0_result.correction, expected_order_0,
                "fixture {}",
                fixture.id
            );
            assert_ne!(
                result.correction, order_0_result.correction,
                "fixture {} did not exercise a distinct order-1 correction",
                fixture.id
            );
        }
    }
}

#[test]
fn bplsd_fixture_manifest_cases_decode_cleanly() {
    let fixture_dir = lsd_fixture_dir();
    let manifest = LsdFixtureManifest::load(&lsd_manifest_path());
    let entries = validate_lsd_fixture_manifest(&manifest, &fixture_dir).unwrap();
    let ids = entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "lsd_order_one_improves_over_baseline",
            "lsd_small_sparse_code",
            "lsd_unsatisfiable_case",
        ]
    );

    for entry in entries {
        let fixture = LsdFixture::load_from_path(&entry.path);
        let pcm = fixture.pcm();
        let syndrome = fixture.syndrome();
        let decoder = BpLsdDecoder::new(pcm.clone(), fixture.channel(), fixture.lsd_config())
            .unwrap_or_else(|error| {
                panic!("failed to construct decoder for {}: {error}", fixture.id)
            });

        match fixture.expected.status.as_str() {
            "success" => {
                let result = decoder
                    .decode(&syndrome)
                    .unwrap_or_else(|error| panic!("failed to decode {}: {error}", fixture.id));
                assert_eq!(result.residual_syndrome_weight, 0, "fixture {}", fixture.id);
                assert_eq!(
                    pcm.multiply(&result.correction),
                    syndrome,
                    "fixture {}",
                    fixture.id
                );
            }
            "error" => {
                let error = decoder.decode(&syndrome).unwrap_err();
                assert_eq!(error, DecodeError::NoLsdSolution, "fixture {}", fixture.id);
                assert_eq!(fixture.expected.error.as_deref(), Some("NoLsdSolution"));
            }
            other => panic!("unsupported expected status {other:?} in {}", fixture.id),
        }
    }
}

#[test]
fn bplsd_fixture_manifest_rejects_invalid_case_metadata() {
    let valid = LsdFixtureManifest::load(&lsd_manifest_path());

    let empty = LsdFixtureManifest { fixtures: vec![] };
    assert_manifest_error(&empty, "must not be empty");

    let mut missing_id = valid.clone();
    missing_id.fixtures[0].id.clear();
    assert_manifest_error(&missing_id, "id");

    let mut missing_path = valid.clone();
    missing_path.fixtures[0].path.clear();
    assert_manifest_error(&missing_path, "path");

    let mut missing_provenance = valid.clone();
    missing_provenance.fixtures[0].provenance.clear();
    assert_manifest_error(&missing_provenance, "provenance");

    let mut missing_verifier = valid.clone();
    missing_verifier.fixtures[0].verifier.clear();
    assert_manifest_error(&missing_verifier, "verifier");

    let mut missing_pass_condition = valid.clone();
    missing_pass_condition.fixtures[0].pass_condition.clear();
    assert_manifest_error(&missing_pass_condition, "pass_condition");

    let mut missing_issue = valid.clone();
    missing_issue.fixtures[0].consumes.retain(|value| value != "#90");
    assert_manifest_error(&missing_issue, "#90");

    let mut duplicate_id = valid.clone();
    duplicate_id.fixtures[1].id = duplicate_id.fixtures[0].id.clone();
    assert_manifest_error(&duplicate_id, "duplicate manifest id");

    let mut duplicate_path = valid.clone();
    duplicate_path.fixtures[1].path = duplicate_path.fixtures[0].path.clone();
    assert_manifest_error(&duplicate_path, "duplicate manifest path");

    let mut stale_path = valid.clone();
    stale_path.fixtures[0].path = "missing_lsd_fixture.json".to_string();
    assert_manifest_error(&stale_path, "missing_lsd_fixture.json");

    let mut mismatched_fixture_id = valid.clone();
    mismatched_fixture_id.fixtures[0].id = "mismatched_fixture_id".to_string();
    assert_manifest_error(&mismatched_fixture_id, "does not match fixture id");

    let mut missing_entry = valid.clone();
    missing_entry.fixtures.pop();
    assert_manifest_error(&missing_entry, "missing manifest entry");
}

#[test]
fn bplsd_returns_a_decoder_error_for_an_unsatisfiable_case() {
    let fixture = LsdFixture::load("lsd_unsatisfiable_case.json");
    assert_eq!(fixture.expected.status, "error");
    assert_eq!(fixture.expected.error.as_deref(), Some("NoLsdSolution"));

    let pcm = fixture.pcm();
    let syndrome = fixture.syndrome();
    let decoder = BpLsdDecoder::new(pcm, fixture.channel(), fixture.lsd_config())
        .unwrap_or_else(|error| panic!("failed to construct decoder for {}: {error}", fixture.id));

    let error = decoder.decode(&syndrome).unwrap_err();

    assert_eq!(error, DecodeError::NoLsdSolution);
}

#[test]
fn bplsddecoder_rejects_lsd_order_above_first_supported_order() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let config = LsdConfig {
        lsd_order: 2,
        ..LsdConfig::default()
    };

    let err = BpLsdDecoder::new(pcm, ChannelModel::Bsc { error_rate: 0.05 }, config).unwrap_err();

    assert_eq!(err, DecodeError::UnsupportedLsdOrder { order: 2 });
}
