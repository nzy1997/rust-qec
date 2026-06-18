# Issue 90 BpLsd Fixture Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add shared-BP reuse coverage, an LSD fixture manifest, Rust validation, and Python `ldpc` differential harness support for the current `BpLsdDecoder` fixture set.

**Architecture:** Keep #90 narrow: validate and compare only `rbposd/tests/fixtures/lsd/` while leaving the mature OSD/BP parity fixtures unchanged by default. Add test-local Rust manifest validation first, then extend the existing dev parity report path so OSD and LSD produce one JSON report shape, then teach the Python harness to opt into LSD fixtures. This preserves the #98 boundary for a later unified LSD/BP-option catalog.

**Tech Stack:** Rust 2024 `rbposd` crate; `serde` and `serde_json` dev dependencies already present; Python 3 `unittest`/`pytest`; existing `rbposd/scripts/parity_harness.py`; upstream `ldpc` accessed only by the harness runtime, with unit tests mocking around constructor details.

## Global Constraints

- Do not add `rsinter` runner support, DEM adapters, benchmark result rows, or benchmark spec changes.
- Do not add new LSD algorithms, new public LSD method variants, or support for `lsd_order > 1`.
- Do not expand BP methods, schedules, `bits_per_step`, or broader BP-option support.
- Do not migrate the existing `rbposd/tests/fixtures/parity/` OSD/BP fixtures into the new manifest in #90.
- Do not make performance claims from the differential harness.
- Do not change the public shape of `DecodeResult`.
- Existing OSD/BP parity harness behavior must remain unchanged unless `--include-lsd` is passed.
- Use `DecodeError::NoLsdSolution` and `DecodeError::UnsupportedLsdOrder` exactly as they exist today.

---

## File Structure

- Create `rbposd/tests/fixtures/lsd/manifest.json`
  - Owns #90 metadata for the current LSD fixture files only.
- Modify `rbposd/tests/lsd.rs`
  - Adds shared-BP reuse regression and test-local manifest validation.
- Modify `rbposd/dev/parity_schema.rs`
  - Adds a backwards-compatible decoder-family field and LSD config schema for dev parity cases.
- Modify `rbposd/dev/parity_runner.rs`
  - Runs either `BpOsdDecoder` or `BpLsdDecoder` through the same `ParityReport` shape.
- Modify `rbposd/tests/parity_dev.rs`
  - Covers default OSD compatibility and explicit LSD parity cases.
- Modify `rbposd/scripts/parity_harness.py`
  - Adds opt-in LSD manifest discovery, upstream LSD kwargs mapping, and LSD case comparison.
- Modify `rbposd/scripts/test_parity_harness.py`
  - Covers LSD manifest loading, mapping, unsupported combinations, and opt-in `build_entries`.
- Modify `rbposd/doc/ldpc_mvp_reference.md`
  - Documents the #90 LSD manifest/differential path and the #98 boundary.
- Modify `rbposd/tests/reference.rs`
  - Locks the new documentation surface.

---

### Task 1: Add LSD Manifest Validation And Shared-Reuse Regression

**Files:**
- Create: `rbposd/tests/fixtures/lsd/manifest.json`
- Modify: `rbposd/tests/lsd.rs`

**Interfaces:**
- Consumes: existing `LsdFixture`, `BpLsdDecoder`, `LsdConfig`, `DecodeError`, `ParityCheckMatrix`, `Syndrome`.
- Produces:
  - `fn lsd_fixture_dir() -> PathBuf`
  - `fn lsd_manifest_path() -> PathBuf`
  - `impl LsdFixture { fn load_from_path(path: &Path) -> Self }`
  - `struct LsdFixtureManifest`
  - `struct LsdFixtureManifestEntry`
  - `struct ValidatedLsdManifestEntry`
  - `fn validate_lsd_fixture_manifest(manifest: &LsdFixtureManifest, fixture_dir: &Path) -> Result<Vec<ValidatedLsdManifestEntry>, String>`
  - tests `bplsd_decoder_reuse_returns_valid_solutions_for_multiple_syndromes`, `bplsd_fixture_manifest_cases_decode_cleanly`, `bplsd_fixture_manifest_rejects_invalid_case_metadata`

- [ ] **Step 1: Add failing manifest and reuse tests**

In `rbposd/tests/lsd.rs`, replace the imports at the top with:

```rust
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use rbposd::{
    BpLsdDecoder, ChannelModel, Correction, DecodeError, LsdConfig, ParityCheckMatrix, Syndrome,
};
use serde::Deserialize;
```

After `struct ExpectedFixture`, add these manifest structs:

```rust
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
```

Replace `LsdFixture::load` with this implementation and add `load_from_path`:

```rust
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
```

After the `impl LsdFixture` block, add these helper declarations and tests. The helper functions are not implemented yet in this step, so the tests must fail to compile:

```rust
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

fn expected_error_code(error: &DecodeError) -> &'static str {
    match error {
        DecodeError::NoLsdSolution => "NoLsdSolution",
        DecodeError::UnsupportedLsdOrder { .. } => "UnsupportedLsdOrder",
        DecodeError::DimensionMismatch { .. } => "DimensionMismatch",
        DecodeError::InvalidProbability => "InvalidProbability",
        DecodeError::EmptyMatrix => "EmptyMatrix",
        DecodeError::InvalidColumnIndex { .. } => "InvalidColumnIndex",
        DecodeError::InvalidRowIndex { .. } => "InvalidRowIndex",
        DecodeError::SingularSystem => "SingularSystem",
        DecodeError::BpDidNotConverge => "BpDidNotConverge",
        DecodeError::NoOsdSolution => "NoOsdSolution",
    }
}

fn assert_manifest_error(manifest: &LsdFixtureManifest, needle: &str) {
    let error = validate_lsd_fixture_manifest(manifest, &lsd_fixture_dir()).unwrap_err();
    assert!(
        error.contains(needle),
        "expected manifest error containing {needle:?}, got {error:?}"
    );
}
```

Add this test after `bplsddecoder_clone_preserves_decoding_behavior_with_fresh_workspaces`:

```rust
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
```

Add these tests before `bplsddecoder_rejects_lsd_order_above_first_supported_order`:

```rust
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
                assert_eq!(
                    fixture.expected.error.as_deref(),
                    Some(expected_error_code(&error)),
                    "fixture {}",
                    fixture.id
                );
            }
            other => panic!("unsupported expected status {other:?} in {}", fixture.id),
        }
    }
}

#[test]
fn bplsd_fixture_manifest_rejects_invalid_case_metadata() {
    let valid = LsdFixtureManifest::load(&lsd_manifest_path());

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

    let mut stale_path = valid.clone();
    stale_path.fixtures[0].path = "missing_lsd_fixture.json".to_string();
    assert_manifest_error(&stale_path, "missing_lsd_fixture.json");

    let mut missing_entry = valid.clone();
    missing_entry.fixtures.pop();
    assert_manifest_error(&missing_entry, "missing manifest entry");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rbposd bplsd_decoder_reuse_returns_valid_solutions_for_multiple_syndromes
cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly
cargo test -p rbposd bplsd_fixture_manifest_rejects_invalid_case_metadata
```

Expected:

- The reuse test may pass or fail independently.
- The manifest tests fail to compile with `cannot find function validate_lsd_fixture_manifest` or fail at runtime because `manifest.json` does not exist.

- [ ] **Step 3: Implement manifest validation helpers**

In `rbposd/tests/lsd.rs`, add this helper implementation after `assert_manifest_error`:

```rust
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
```

- [ ] **Step 4: Add the LSD fixture manifest file**

Create `rbposd/tests/fixtures/lsd/manifest.json` with exactly:

```json
{
  "fixtures": [
    {
      "id": "lsd_order_one_improves_over_baseline",
      "path": "lsd_order_one_improves_over_baseline.json",
      "provenance": "Repo-owned small-matrix LSD alignment case introduced by issue #89 to exercise deterministic order-one candidate selection against the upstream ldpc-style contract.",
      "verifier": "cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly",
      "pass_condition": "BpLsdDecoder with lsd_order=1 returns the expected residual-zero order-one correction and differs from the order-zero baseline.",
      "consumes": ["#89", "#90"]
    },
    {
      "id": "lsd_small_sparse_code",
      "path": "lsd_small_sparse_code.json",
      "provenance": "Repo-owned small sparse LSD alignment case introduced by issue #89 for the first borrowed fixture set.",
      "verifier": "cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly",
      "pass_condition": "BpLsdDecoder with lsd_order=1 decodes the syndrome to residual zero without using OSD.",
      "consumes": ["#89", "#90"]
    },
    {
      "id": "lsd_unsatisfiable_case",
      "path": "lsd_unsatisfiable_case.json",
      "provenance": "Repo-owned negative LSD alignment case introduced by issue #89 to lock the NoLsdSolution failure path.",
      "verifier": "cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly",
      "pass_condition": "BpLsdDecoder with lsd_order=1 returns DecodeError::NoLsdSolution.",
      "consumes": ["#89", "#90"]
    }
  ]
}
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p rbposd bplsd_decoder_reuse_returns_valid_solutions_for_multiple_syndromes
cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly
cargo test -p rbposd bplsd_fixture_manifest_rejects_invalid_case_metadata
```

Expected: all three commands pass.

- [ ] **Step 6: Commit Task 1**

Run:

```bash
git add rbposd/tests/lsd.rs rbposd/tests/fixtures/lsd/manifest.json
git commit -m "test: add bplsd fixture manifest validation"
```

---

### Task 2: Extend Rust Parity Schema For BpLsd Cases

**Files:**
- Modify: `rbposd/dev/parity_schema.rs`
- Modify: `rbposd/dev/parity_runner.rs`
- Modify: `rbposd/tests/parity_dev.rs`

**Interfaces:**
- Consumes:
  - `BpOsdDecoder::new(pcm, channel, DecoderConfig) -> Result<BpOsdDecoder, DecodeError>`
  - `BpLsdDecoder::new(pcm, channel, LsdConfig) -> Result<BpLsdDecoder, DecodeError>`
  - `ParityOutcome::from_decode_result(result: DecodeResult) -> ParityOutcome`
- Produces:
  - `pub enum DecoderSpec { BpOsd, BpLsd }`
  - `pub enum LsdMethodSpec { LocalizedStatistics }`
  - `pub struct LsdConfigSpec { pub method: LsdMethodSpec, pub lsd_order: usize }`
  - `ParityCase.decoder: DecoderSpec` with serde default `BpOsd`
  - `ParityCase.lsd_config: Option<LsdConfigSpec>`
  - `ParityCase::decode(&self) -> Result<DecodeResult, DecodeError>`

- [ ] **Step 1: Write failing parity-dev tests**

In `rbposd/tests/parity_dev.rs`, add this test after `fn osd_config()`:

```rust
fn lsd_config(order: usize) -> parity_schema::LsdConfigSpec {
    parity_schema::LsdConfigSpec {
        method: parity_schema::LsdMethodSpec::LocalizedStatistics,
        lsd_order: order,
    }
}
```

Add these tests after `parity_outcomes_use_stable_error_codes_and_partial_diagnostics_matching`:

```rust
#[test]
fn parity_case_defaults_to_bposd_decoder_for_existing_json_shape() {
    let json = r#"{
      "name": "default_decoder_case",
      "matrix": {
        "num_checks": 1,
        "num_bits": 1,
        "rows": [[0]]
      },
      "channel": {
        "kind": "bsc",
        "error_rate": 0.2
      },
      "syndrome": [true],
      "config": {
        "max_bp_iterations": 0,
        "early_stop": true,
        "bp_variant": "minimum_sum",
        "schedule": "parallel",
        "osd_variant": "osd0"
      }
    }"#;

    let case: parity_schema::ParityCase = serde_json::from_str(json).unwrap();

    assert_eq!(case.decoder, parity_schema::DecoderSpec::BpOsd);
    assert!(case.lsd_config.is_none());
}

#[test]
fn parity_runner_decodes_bplsd_cases_when_decoder_field_is_set() {
    let case = parity_schema::ParityCase {
        name: "lsd_parity_case".to_string(),
        decoder: parity_schema::DecoderSpec::BpLsd,
        matrix: parity_schema::MatrixSpec {
            num_checks: 2,
            num_bits: 3,
            rows: vec![vec![1, 2], vec![0]],
        },
        channel: parity_schema::ChannelSpec::Bsc { error_rate: 0.05 },
        syndrome: vec![true, false],
        config: osd_config(),
        lsd_config: Some(lsd_config(1)),
        expected: None,
        tags: vec!["lsd".to_string()],
    };

    let report = parity_runner::run_case(&case);

    assert_eq!(report.name, "lsd_parity_case");
    assert_eq!(report.tags, vec!["lsd"]);
    assert_eq!(report.matches_expected, None);
    match report.actual {
        parity_schema::ParityOutcome::Success {
            correction,
            diagnostics,
        } => {
            let pcm = rbposd::ParityCheckMatrix::from_sparse_rows(
                2,
                3,
                vec![vec![1, 2], vec![0]],
            )
            .unwrap();
            let syndrome = rbposd::Syndrome::from(vec![true, false]);
            assert_eq!(pcm.multiply(&rbposd::Correction::from(correction)), syndrome);
            assert_eq!(diagnostics.used_osd, Some(false));
            assert_eq!(diagnostics.residual_syndrome_weight, Some(0));
        }
        parity_schema::ParityOutcome::Error { error } => {
            panic!("expected LSD success report, got error {error}");
        }
    }
}
```

In existing struct literals in `run_case_reports_success_build_failures_and_decode_failures`, add:

```rust
        decoder: parity_schema::DecoderSpec::BpOsd,
        lsd_config: None,
```

Add those two fields to the `success_case`, `build_error_case`, and
`decode_error_case` `parity_schema::ParityCase` struct literals in that test.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rbposd parity_case_defaults_to_bposd_decoder_for_existing_json_shape
cargo test -p rbposd parity_runner_decodes_bplsd_cases_when_decoder_field_is_set
```

Expected: FAIL with missing `DecoderSpec`, `LsdConfigSpec`, `LsdMethodSpec`, `decoder`, `lsd_config`, or `ParityCase::decode`.

- [ ] **Step 3: Update parity schema imports**

In `rbposd/dev/parity_schema.rs`, replace the current multi-line `use rbposd`
import block at the top of the file with:

```rust
use rbposd::{
    BpLsdDecoder, BpOsdDecoder, BpVariant, ChannelModel, DecodeError, DecodeResult,
    DecoderConfig, LsdConfig, LsdMethod, OsdVariant, ParityCheckMatrix, Schedule, Syndrome,
};
```

- [ ] **Step 4: Add decoder and LSD config schema types**

In `rbposd/dev/parity_schema.rs`, add this after `OsdVariantSpec`:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecoderSpec {
    BpOsd,
    BpLsd,
}

impl Default for DecoderSpec {
    fn default() -> Self {
        Self::BpOsd
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LsdMethodSpec {
    LocalizedStatistics,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LsdConfigSpec {
    pub method: LsdMethodSpec,
    pub lsd_order: usize,
}

impl LsdConfigSpec {
    fn build(self) -> LsdConfig {
        LsdConfig {
            method: match self.method {
                LsdMethodSpec::LocalizedStatistics => LsdMethod::LocalizedStatistics,
            },
            lsd_order: self.lsd_order,
        }
    }
}
```

- [ ] **Step 5: Extend `ParityCase` and decoder dispatch**

In `rbposd/dev/parity_schema.rs`, replace the `ParityCase` struct and impl with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParityCase {
    pub name: String,
    #[serde(default)]
    pub decoder: DecoderSpec,
    pub matrix: MatrixSpec,
    pub channel: ChannelSpec,
    pub syndrome: Vec<bool>,
    pub config: ConfigSpec,
    #[serde(default)]
    pub lsd_config: Option<LsdConfigSpec>,
    #[serde(default)]
    pub expected: Option<ParityOutcome>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ParityCase {
    pub fn build_decoder(&self) -> Result<BpOsdDecoder, DecodeError> {
        BpOsdDecoder::new(
            self.matrix.build()?,
            self.channel.build(),
            self.config.build(),
        )
    }

    fn build_lsd_decoder(&self) -> Result<BpLsdDecoder, DecodeError> {
        let lsd_config = self
            .lsd_config
            .map(LsdConfigSpec::build)
            .unwrap_or_default();
        BpLsdDecoder::new(self.matrix.build()?, self.channel.build(), lsd_config)
    }

    pub fn decode(&self) -> Result<DecodeResult, DecodeError> {
        let syndrome = self.syndrome();
        match self.decoder {
            DecoderSpec::BpOsd => self.build_decoder()?.decode(&syndrome),
            DecoderSpec::BpLsd => self.build_lsd_decoder()?.decode(&syndrome),
        }
    }

    pub fn syndrome(&self) -> Syndrome {
        Syndrome::from(self.syndrome.clone())
    }
}
```

- [ ] **Step 6: Use `ParityCase::decode` in the runner**

In `rbposd/dev/parity_runner.rs`, replace `run_case` with:

```rust
pub fn run_case(case: &ParityCase) -> ParityReport {
    let actual = match case.decode() {
        Ok(result) => ParityOutcome::from_decode_result(result),
        Err(error) => ParityOutcome::from_decode_error(error),
    };

    let matches_expected = case
        .expected
        .as_ref()
        .map(|expected| expected.matches_actual(&actual));

    ParityReport {
        name: case.name.clone(),
        expected: case.expected.clone(),
        actual,
        matches_expected,
        tags: case.tags.clone(),
    }
}
```

- [ ] **Step 7: Run focused tests**

Run:

```bash
cargo test -p rbposd parity_case_defaults_to_bposd_decoder_for_existing_json_shape
cargo test -p rbposd parity_runner_decodes_bplsd_cases_when_decoder_field_is_set
cargo test -p rbposd run_case_reports_success_build_failures_and_decode_failures
cargo test -p rbposd checked_in_parity_fixtures_match_exact_expected_outputs
```

Expected: all commands pass.

- [ ] **Step 8: Commit Task 2**

Run:

```bash
git add rbposd/dev/parity_schema.rs rbposd/dev/parity_runner.rs rbposd/tests/parity_dev.rs
git commit -m "feat: add bplsd parity schema path"
```

---

### Task 3: Add Opt-In LSD Support To The Python Parity Harness

**Files:**
- Modify: `rbposd/scripts/parity_harness.py`
- Modify: `rbposd/scripts/test_parity_harness.py`

**Interfaces:**
- Consumes:
  - Rust parity driver JSON report from Task 2
  - `rbposd/tests/fixtures/lsd/manifest.json` from Task 1
  - existing `build_entries(repo_root, fixtures_dir, skip_generated, case_limit)` comparison structure
- Produces:
  - `DEFAULT_BP_CONFIG: dict[str, Any]`
  - `load_lsd_manifest(lsd_fixtures_dir: Path) -> dict[str, Any]`
  - `iter_lsd_fixture_cases(lsd_fixtures_dir: Path) -> list[dict[str, Any]]`
  - `map_lsd_case_to_ldpc_kwargs(case: dict[str, Any]) -> dict[str, Any]`
  - `run_python_bposd(case: dict[str, Any]) -> dict[str, Any]`
  - `run_python_bplsd(case: dict[str, Any]) -> dict[str, Any]`
  - `run_python_ldpc(case: dict[str, Any]) -> dict[str, Any]` dispatches on `case.get("decoder", "bp_osd")`
  - `build_entries(repo_root: Path, fixtures_dir: Path, skip_generated: bool, case_limit: int | None, include_lsd: bool = False, lsd_fixtures_dir: Path = Path("rbposd/tests/fixtures/lsd"))`

- [ ] **Step 1: Write failing Python tests**

In `rbposd/scripts/test_parity_harness.py`, replace the import from `parity_harness` with:

```python
from parity_harness import (
    build_entries,
    classify_mismatch,
    is_real_mismatch,
    iter_generated_cases,
    iter_lsd_fixture_cases,
    load_lsd_manifest,
    map_config_to_ldpc_kwargs,
    map_lsd_case_to_ldpc_kwargs,
    matrix_to_dense,
)
```

Add these tests after `test_map_config_to_ldpc_kwargs_rejects_unsupported_early_stop`:

```python
    def test_iter_lsd_fixture_cases_loads_manifest_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            fixture_dir = Path(tmp_dir)
            (fixture_dir / "manifest.json").write_text(
                """
{
  "fixtures": [
    {
      "id": "lsd_small_sparse_code",
      "path": "lsd_small_sparse_code.json",
      "provenance": "unit test provenance",
      "verifier": "python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd",
      "pass_condition": "unit test pass condition",
      "consumes": ["#90"]
    }
  ]
}
""",
                encoding="utf-8",
            )
            (fixture_dir / "lsd_small_sparse_code.json").write_text(
                """
{
  "id": "lsd_small_sparse_code",
  "matrix": {
    "num_checks": 2,
    "num_bits": 3,
    "rows": [[1, 2], [0]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.05
  },
  "syndrome": [true, false],
  "lsd_order": 1,
  "expected": {
    "status": "success"
  }
}
""",
                encoding="utf-8",
            )

            manifest = load_lsd_manifest(fixture_dir)
            cases = iter_lsd_fixture_cases(fixture_dir)

        self.assertEqual(manifest["fixtures"][0]["id"], "lsd_small_sparse_code")
        self.assertEqual(len(cases), 1)
        self.assertEqual(cases[0]["name"], "lsd_small_sparse_code")
        self.assertEqual(cases[0]["decoder"], "bp_lsd")
        self.assertEqual(cases[0]["lsd_config"]["method"], "localized_statistics")
        self.assertEqual(cases[0]["lsd_config"]["lsd_order"], 1)
        self.assertEqual(cases[0]["tags"], ["fixture", "lsd", "#90"])

    def test_map_lsd_case_to_ldpc_kwargs_maps_supported_lsd(self) -> None:
        case = {
            "decoder": "bp_lsd",
            "config": {
                "max_bp_iterations": 30,
                "early_stop": True,
                "bp_variant": "minimum_sum",
                "schedule": "parallel",
                "osd_variant": "osd0",
            },
            "lsd_config": {
                "method": "localized_statistics",
                "lsd_order": 1,
            },
        }

        self.assertEqual(
            map_lsd_case_to_ldpc_kwargs(case),
            {
                "max_iter": 30,
                "bp_method": "minimum_sum",
                "schedule": "parallel",
                "lsd_method": "localized_statistics",
                "lsd_order": 1,
                "input_vector_type": "syndrome",
            },
        )

    def test_map_lsd_case_to_ldpc_kwargs_rejects_unsupported_lsd_order(self) -> None:
        case = {
            "decoder": "bp_lsd",
            "config": {
                "max_bp_iterations": 30,
                "early_stop": True,
                "bp_variant": "minimum_sum",
                "schedule": "parallel",
                "osd_variant": "osd0",
            },
            "lsd_config": {
                "method": "localized_statistics",
                "lsd_order": 2,
            },
        }

        with self.assertRaisesRegex(ValueError, "Unsupported lsd_order"):
            map_lsd_case_to_ldpc_kwargs(case)

    def test_build_entries_includes_lsd_cases_only_when_requested(self) -> None:
        lsd_case = {
            "name": "lsd_case",
            "decoder": "bp_lsd",
            "matrix": {"num_checks": 1, "num_bits": 1, "rows": [[0]]},
            "channel": {"kind": "bsc", "error_rate": 0.1},
            "syndrome": [True],
            "config": {
                "max_bp_iterations": 30,
                "early_stop": True,
                "bp_variant": "minimum_sum",
                "schedule": "parallel",
                "osd_variant": "osd0",
            },
            "lsd_config": {"method": "localized_statistics", "lsd_order": 1},
            "tags": ["fixture", "lsd", "#90"],
        }
        rust_report = {
            "actual": {
                "status": "success",
                "correction": [True],
                "diagnostics": {
                    "converged": False,
                    "bp_iterations": 30,
                    "used_osd": False,
                    "residual_syndrome_weight": 0,
                },
            }
        }
        python_actual = rust_report["actual"]

        with mock.patch("parity_harness.fixture_case_paths", return_value=[]):
            with mock.patch("parity_harness.iter_generated_cases", return_value=[]):
                with mock.patch("parity_harness.iter_lsd_fixture_cases", return_value=[lsd_case]):
                    with mock.patch("parity_harness.run_rust_case", return_value=rust_report):
                        with mock.patch(
                            "parity_harness.run_python_ldpc", return_value=python_actual
                        ):
                            without_lsd = build_entries(
                                repo_root=Path("."),
                                fixtures_dir=Path("."),
                                skip_generated=True,
                                case_limit=None,
                            )
                            with_lsd = build_entries(
                                repo_root=Path("."),
                                fixtures_dir=Path("."),
                                skip_generated=True,
                                case_limit=None,
                                include_lsd=True,
                                lsd_fixtures_dir=Path("lsd"),
                            )

        self.assertEqual(without_lsd, [])
        self.assertEqual(len(with_lsd), 1)
        self.assertEqual(with_lsd[0]["name"], "lsd_case")
        self.assertEqual(with_lsd[0]["source"], "lsd_fixture")
        self.assertEqual(with_lsd[0]["mismatch_classification"], "exact_match")
```

- [ ] **Step 2: Run LSD Python tests to verify they fail**

Run:

```bash
python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd
```

Expected: FAIL with import errors for `iter_lsd_fixture_cases`, `load_lsd_manifest`, or `map_lsd_case_to_ldpc_kwargs`.

- [ ] **Step 3: Add CLI flags and default BP config**

In `rbposd/scripts/parity_harness.py`, add this constant after `REAL_MISMATCH_CLASSES`:

```python
DEFAULT_BP_CONFIG = {
    "max_bp_iterations": 30,
    "early_stop": True,
    "bp_variant": "minimum_sum",
    "schedule": "parallel",
    "osd_variant": "osd0",
}
```

In `parse_args`, add these arguments after `--fixtures-dir`:

```python
    parser.add_argument(
        "--include-lsd",
        action="store_true",
        help="Also run checked-in LSD fixtures from the LSD fixture manifest.",
    )
    parser.add_argument(
        "--lsd-fixtures-dir",
        type=Path,
        default=Path("rbposd/tests/fixtures/lsd"),
        help="Directory containing checked-in LSD fixture JSON files and manifest.json.",
    )
```

- [ ] **Step 4: Add LSD manifest and case conversion helpers**

In `rbposd/scripts/parity_harness.py`, add this code after `fixture_case_paths`:

```python
def load_lsd_manifest(lsd_fixtures_dir: Path) -> dict[str, Any]:
    manifest_path = lsd_fixtures_dir / "manifest.json"
    with manifest_path.open("r", encoding="utf-8") as infile:
        manifest = json.load(infile)
    if not isinstance(manifest.get("fixtures"), list) or not manifest["fixtures"]:
        raise ValueError(f"LSD manifest {manifest_path} must contain a non-empty fixtures list")
    return manifest


def iter_lsd_fixture_cases(lsd_fixtures_dir: Path) -> list[dict[str, Any]]:
    manifest = load_lsd_manifest(lsd_fixtures_dir)
    cases: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    for entry in manifest["fixtures"]:
        fixture_id = str(entry.get("id", ""))
        fixture_path_name = str(entry.get("path", ""))
        if not fixture_id:
            raise ValueError("LSD manifest entry id must not be empty")
        if fixture_id in seen_ids:
            raise ValueError(f"Duplicate LSD manifest id: {fixture_id}")
        seen_ids.add(fixture_id)
        if not fixture_path_name:
            raise ValueError(f"LSD manifest entry {fixture_id} path must not be empty")
        if "#90" not in entry.get("consumes", []):
            raise ValueError(f"LSD manifest entry {fixture_id} must consume #90")

        fixture_path = lsd_fixtures_dir / fixture_path_name
        fixture = load_case(fixture_path)
        if fixture.get("id") != fixture_id:
            raise ValueError(
                f"LSD manifest id {fixture_id} does not match fixture id {fixture.get('id')}"
            )

        cases.append(
            {
                "name": fixture["id"],
                "decoder": "bp_lsd",
                "matrix": fixture["matrix"],
                "channel": fixture["channel"],
                "syndrome": fixture["syndrome"],
                "config": dict(DEFAULT_BP_CONFIG),
                "lsd_config": {
                    "method": "localized_statistics",
                    "lsd_order": int(fixture["lsd_order"]),
                },
                "tags": ["fixture", "lsd", "#90"],
            }
        )
    return cases
```

- [ ] **Step 5: Add LSD kwargs mapping and split Python decoder runners**

In `rbposd/scripts/parity_harness.py`, add this helper after `map_config_to_ldpc_kwargs`:

```python
def map_lsd_case_to_ldpc_kwargs(case: dict[str, Any]) -> dict[str, Any]:
    config = case.get("config", {})
    bp_variant = config.get("bp_variant")
    if bp_variant != "minimum_sum":
        raise ValueError(f"Unsupported bp_variant for LSD: {bp_variant}")

    schedule = config.get("schedule")
    if schedule != "parallel":
        raise ValueError(f"Unsupported schedule for LSD: {schedule}")

    early_stop = config.get("early_stop")
    if early_stop is not True:
        raise ValueError(
            f"Unsupported early_stop value for LSD: {early_stop}. "
            "Python ldpc parity harness currently requires early_stop=true."
        )

    lsd_config = case.get("lsd_config", {})
    lsd_method = lsd_config.get("method")
    if lsd_method != "localized_statistics":
        raise ValueError(f"Unsupported lsd_method: {lsd_method}")

    lsd_order = int(lsd_config.get("lsd_order", -1))
    if lsd_order not in (0, 1):
        raise ValueError(f"Unsupported lsd_order: {lsd_order}")

    return {
        "max_iter": int(config["max_bp_iterations"]),
        "bp_method": "minimum_sum",
        "schedule": "parallel",
        "lsd_method": "localized_statistics",
        "lsd_order": lsd_order,
        "input_vector_type": "syndrome",
    }
```

Replace the existing `run_python_ldpc` with these three functions:

```python
def add_channel_kwargs(decoder_kwargs: dict[str, Any], channel: dict[str, Any]) -> dict[str, Any]:
    if channel["kind"] == "bsc":
        decoder_kwargs["error_rate"] = float(channel["error_rate"])
    elif channel["kind"] == "bit_flip_probabilities":
        decoder_kwargs["error_channel"] = list(channel["probabilities"])
    else:
        raise ValueError(f"UnsupportedChannel(kind={channel.get('kind')})")
    return decoder_kwargs


def residual_weight_for_correction(case: dict[str, Any], correction: list[bool]) -> int:
    syndrome_bool = [bool(bit) for bit in case["syndrome"]]
    residual = [False for _ in range(len(syndrome_bool))]
    for row_index, row in enumerate(matrix_to_dense(case["matrix"])):
        parity = False
        for bit_index, include in enumerate(row):
            if include:
                parity ^= correction[bit_index]
        residual[row_index] = parity ^ syndrome_bool[row_index]
    return sum(1 for bit in residual if bit)


def run_python_bposd(case: dict[str, Any]) -> dict[str, Any]:
    import numpy as np
    from ldpc import BpOsdDecoder

    matrix = np.array(matrix_to_dense(case["matrix"]), dtype=np.uint8)
    syndrome = np.array(case["syndrome"], dtype=np.uint8)
    try:
        decoder_kwargs = add_channel_kwargs(
            map_config_to_ldpc_kwargs(case["config"]),
            case["channel"],
        )
        decoder = BpOsdDecoder(matrix, **decoder_kwargs)
        correction_arr = decoder.decode(syndrome)
    except ValueError as error:
        return {"status": "error", "error": str(error)}
    except Exception as error:  # pragma: no cover - exercised by full harness runs
        return {"status": "error", "error": f"{type(error).__name__}: {error}"}

    correction = [bool(int(value)) for value in correction_arr.tolist()]
    residual_weight = residual_weight_for_correction(case, correction)
    converged = bool(decoder.converge)
    return {
        "status": "success",
        "correction": correction,
        "diagnostics": {
            "converged": converged,
            "bp_iterations": int(decoder.iter),
            "used_osd": not converged,
            "residual_syndrome_weight": residual_weight,
        },
    }


def run_python_bplsd(case: dict[str, Any]) -> dict[str, Any]:
    import numpy as np
    from ldpc import BpLsdDecoder

    matrix = np.array(matrix_to_dense(case["matrix"]), dtype=np.uint8)
    syndrome = np.array(case["syndrome"], dtype=np.uint8)
    try:
        decoder_kwargs = add_channel_kwargs(
            map_lsd_case_to_ldpc_kwargs(case),
            case["channel"],
        )
        decoder = BpLsdDecoder(matrix, **decoder_kwargs)
        correction_arr = decoder.decode(syndrome)
    except ValueError as error:
        return {"status": "error", "error": str(error)}
    except Exception as error:  # pragma: no cover - exercised by full harness runs
        return {"status": "error", "error": f"{type(error).__name__}: {error}"}

    correction = [bool(int(value)) for value in correction_arr.tolist()]
    residual_weight = residual_weight_for_correction(case, correction)
    return {
        "status": "success",
        "correction": correction,
        "diagnostics": {
            "converged": bool(getattr(decoder, "converge", False)),
            "bp_iterations": int(getattr(decoder, "iter", case["config"]["max_bp_iterations"])),
            "used_osd": False,
            "residual_syndrome_weight": residual_weight,
        },
    }


def run_python_ldpc(case: dict[str, Any]) -> dict[str, Any]:
    decoder = case.get("decoder", "bp_osd")
    if decoder in ("bp_osd", "bposd"):
        return run_python_bposd(case)
    if decoder in ("bp_lsd", "bplsd"):
        return run_python_bplsd(case)
    return {"status": "error", "error": f"Unsupported decoder: {decoder}"}
```

- [ ] **Step 6: Add opt-in LSD cases to `build_entries` and CLI main**

Change the `build_entries` signature to:

```python
def build_entries(
    repo_root: Path,
    fixtures_dir: Path,
    skip_generated: bool,
    case_limit: int | None,
    include_lsd: bool = False,
    lsd_fixtures_dir: Path = Path("rbposd/tests/fixtures/lsd"),
) -> list[dict[str, Any]]:
```

Inside `build_entries`, after the generated-case block and before the `case_limit` block, add:

```python
    if include_lsd:
        for lsd_case in iter_lsd_fixture_cases(lsd_fixtures_dir):
            case_items.append(
                {
                    "source": "lsd_fixture",
                    "case_path": None,
                    "case": lsd_case,
                }
            )
```

In the loop where `case_path is None`, keep the existing temporary JSON flow. This allows converted LSD cases to run through the Rust `parity_driver`.

In `main`, pass the new arguments:

```python
        include_lsd=args.include_lsd,
        lsd_fixtures_dir=args.lsd_fixtures_dir,
```

- [ ] **Step 7: Run Python LSD tests**

Run:

```bash
python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd
python3 -m pytest rbposd/scripts/test_parity_harness.py
```

Expected: both commands pass. The full test command should not require `ldpc` because existing tests mock `run_python_ldpc` or cover pure mapping helpers.

- [ ] **Step 8: Commit Task 3**

Run:

```bash
git add rbposd/scripts/parity_harness.py rbposd/scripts/test_parity_harness.py
git commit -m "feat: add lsd parity harness path"
```

---

### Task 4: Document The LSD Manifest Boundary And Run Final Verification

**Files:**
- Modify: `rbposd/doc/ldpc_mvp_reference.md`
- Modify: `rbposd/tests/reference.rs`

**Interfaces:**
- Consumes: #90 manifest and harness surfaces from Tasks 1-3.
- Produces: documentation that names `manifest.json`, the LSD differential command, and the #98 boundary.

- [ ] **Step 1: Write failing documentation-surface test**

In `rbposd/tests/reference.rs`, extend the `required` array inside `task_6_documentation_surfaces_exist` by adding:

```rust
        "LSD Fixture Manifest",
        "manifest.json",
        "python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd",
        "Existing OSD/BP parity fixtures remain outside the #90 manifest",
```

- [ ] **Step 2: Run documentation test to verify it fails**

Run:

```bash
cargo test -p rbposd task_6_documentation_surfaces_exist
```

Expected: FAIL with a missing required documentation string.

- [ ] **Step 3: Update `ldpc_mvp_reference.md`**

Append this section after the existing LSD Public API Contract section in `rbposd/doc/ldpc_mvp_reference.md`:

```markdown
## LSD Fixture Manifest

Issue #90 adds an LSD-only fixture manifest at
`rbposd/tests/fixtures/lsd/manifest.json`.

The manifest covers the current checked-in LSD fixtures:

- `lsd_small_sparse_code.json`
- `lsd_order_one_improves_over_baseline.json`
- `lsd_unsatisfiable_case.json`

Each manifest entry records:

- fixture id
- fixture path
- provenance
- verifier command
- pass condition
- consuming issue ids

Rust tests validate that each checked-in LSD fixture has exactly one manifest
entry and that malformed metadata is rejected instead of silently skipped.

The Python parity harness can opt into these LSD fixtures with:

```bash
python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd
```

The harness path converts manifest-listed LSD fixtures into the existing parity
report shape and compares supported `BpLsdDecoder` cases against upstream
`ldpc`. Unsupported LSD mappings are reported as structured errors and are not
coerced into OSD decoding.

Existing OSD/BP parity fixtures remain outside the #90 manifest. The broader
shared LSD and BP-option fixture catalog remains owned by #98.
```

- [ ] **Step 4: Run final verification**

Run:

```bash
cargo test -p rbposd bplsd_decoder_reuse_returns_valid_solutions_for_multiple_syndromes
cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly
cargo test -p rbposd bplsd_fixture_manifest_rejects_invalid_case_metadata
cargo test -p rbposd minimum_sum_decoder_reuses_one_instance_for_multiple_syndromes
cargo test -p rbposd task_6_documentation_surfaces_exist
python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd
python3 -m pytest rbposd/scripts/test_parity_harness.py
git diff --check
```

Expected: all commands pass.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
git add rbposd/doc/ldpc_mvp_reference.md rbposd/tests/reference.rs
git commit -m "docs: document bplsd fixture manifest"
```

---

## Final Completion Checklist

Run:

```bash
git status --short
git log --oneline -n 5
```

Expected:

- `git status --short` shows a clean worktree.
- The last commits include:
  - `test: add bplsd fixture manifest validation`
  - `feat: add bplsd parity schema path`
  - `feat: add lsd parity harness path`
  - `docs: document bplsd fixture manifest`

Then summarize:

- LSD manifest added and validated.
- Shared BP reuse regression added for `BpLsdDecoder`.
- Dev parity schema can run `bp_lsd` cases.
- Python harness can opt into LSD fixture comparison.
- Existing OSD/BP parity behavior remains default.
