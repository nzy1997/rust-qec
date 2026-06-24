# Issue 165 QP101 SVG Fixture Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a manifest-driven QP101 SVG renderer acceptance fixture catalog and a focused `rstim` integration test that validates the catalog and negative controls.

**Architecture:** Keep catalog validation test-local in `rstim/tests/qp101_svg_fixtures.rs` and keep fixture data under `rstim/tests/fixtures/qp101_svg/`. The manifest is JSON, uses explicit `source_kind`, and points either to local `.stim` files or existing QP101 JSON fixtures.

**Tech Stack:** Rust 2024 integration tests, `serde::Deserialize`, `serde_json`, existing `rstim::parser::parse_lines`, existing `rstim::qp101::Qp101Document`.

## Global Constraints

- The deliverable is not the SVG renderer.
- Do not change `rstim/src/qp101.rs` or the QP101-ZY JSON format.
- Store the fixture catalog under `rstim/tests/fixtures/qp101_svg/`.
- The primary verification test must be named `qp101_svg_fixture_manifest_is_valid`.
- The manifest must include at least six cases.
- Every case must include a stable `id`, non-empty `provenance`, explicit `source_kind`, `input_path`, and at least one expected semantic marker.
- Supported `source_kind` values are exactly `stim` and `qp101_json`.
- Every input path must exist and parse through the selected source-kind parser.
- Negative controls must reject missing input paths, empty expected-marker lists, and unsupported source kinds with errors that name the bad case id.
- Expected outputs must remain semantic, not pixel-perfect.

---

### Task 1: QP101 SVG Fixture Catalog And Validator

**Files:**
- Create: `rstim/tests/qp101_svg_fixtures.rs`
- Create: `rstim/tests/fixtures/qp101_svg/manifest.json`
- Create: `rstim/tests/fixtures/qp101_svg/basic_wires_gates_tick.stim`
- Create: `rstim/tests/fixtures/qp101_svg/measurement_detector_source.stim`
- Create: `rstim/tests/fixtures/qp101_svg/observable_include_source.stim`
- Create: `rstim/tests/fixtures/qp101_svg/repeat_repeated_measurements.stim`

**Interfaces:**
- Consumes: `rstim::parser::parse_lines(&str) -> Result<Vec<StimInstr>, String>` and `serde_json::from_str::<Qp101Document>(&str)`.
- Produces: `rstim/tests/fixtures/qp101_svg/manifest.json` as the shared acceptance fixture source for later renderer issues.

- [ ] **Step 1: Write the failing integration test**

Create `rstim/tests/qp101_svg_fixtures.rs`:

```rust
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
        return Err(format!("manifest version must be 1, got {}", manifest.version));
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
            return Err(format!("case {case_id} has an empty expected semantic marker"));
        }
    }

    let input_path = base_dir.join(&case.input_path);
    if !input_path.exists() {
        return Err(format!(
            "case {case_id} input path does not exist: {}",
            input_path.display()
        ));
    }
    let text = fs::read_to_string(&input_path)
        .map_err(|err| format!("case {case_id} failed to read {}: {err}", input_path.display()))?;

    match case.source_kind.as_str() {
        "stim" => {
            parse_lines(&text)
                .map_err(|err| format!("case {case_id} failed to parse Stim input: {err}"))?;
        }
        "qp101_json" => {
            serde_json::from_str::<Qp101Document>(&text).map_err(|err| {
                format!("case {case_id} failed to parse QP101 JSON input: {err}")
            })?;
        }
        other => {
            return Err(format!("case {case_id} has unsupported source_kind {other}"));
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
    validate_manifest(&manifest, &fixture_dir()).expect("QP101 SVG fixture manifest should be valid");

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
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```sh
cargo test -p rstim --test qp101_svg_fixtures qp101_svg_fixture_manifest_is_valid -q
```

Expected: FAIL because `rstim/tests/fixtures/qp101_svg/manifest.json` does not exist.

- [ ] **Step 3: Add Stim fixture files**

Create `rstim/tests/fixtures/qp101_svg/basic_wires_gates_tick.stim`:

```stim
QUBIT_COORDS(0, 0) 0
QUBIT_COORDS(1, 0) 1
H 0
CX 0 1
TICK
M 0 1
```

Create `rstim/tests/fixtures/qp101_svg/measurement_detector_source.stim`:

```stim
QUBIT_COORDS(0, 0) 0
M 0
DETECTOR rec[-1]
```

Create `rstim/tests/fixtures/qp101_svg/observable_include_source.stim`:

```stim
QUBIT_COORDS(0, 0) 0
M 0
OBSERVABLE_INCLUDE(0) rec[-1]
```

Create `rstim/tests/fixtures/qp101_svg/repeat_repeated_measurements.stim`:

```stim
QUBIT_COORDS(0, 0) 0
REPEAT 3 {
  M 0
  TICK
}
```

- [ ] **Step 4: Add the manifest**

Create `rstim/tests/fixtures/qp101_svg/manifest.json`:

```json
{
  "version": 1,
  "cases": [
    {
      "id": "basic_wires_gates_tick",
      "provenance": "Small rstim Stim fixture created for issue #165 to cover wires, H/CX gates, tick separators, and measurement labels.",
      "source_kind": "stim",
      "input_path": "basic_wires_gates_tick.stim",
      "expected_semantic_markers": [
        { "kind": "qubit_label", "value": "q0" },
        { "kind": "qubit_label", "value": "q1" },
        { "kind": "operation_label", "value": "H" },
        { "kind": "operation_label", "value": "CX" },
        { "kind": "tick", "value": "tick" }
      ]
    },
    {
      "id": "measurement_detector_source",
      "provenance": "Small rstim Stim fixture created for issue #165 to cover measurement anchors and detector rec[-1] source resolution.",
      "source_kind": "stim",
      "input_path": "measurement_detector_source.stim",
      "expected_semantic_markers": [
        { "kind": "operation_label", "value": "M" },
        { "kind": "measurement_anchor", "value": "m1" },
        { "kind": "detector_label", "value": "D0" },
        { "kind": "detector_source", "value": "rec[-1]" }
      ]
    },
    {
      "id": "observable_include_source",
      "provenance": "Small rstim Stim fixture created for issue #165 to cover observable include labels and rec[-1] source rendering.",
      "source_kind": "stim",
      "input_path": "observable_include_source.stim",
      "expected_semantic_markers": [
        { "kind": "operation_label", "value": "M" },
        { "kind": "observable_label", "value": "L0" },
        { "kind": "observable_source", "value": "rec[-1]" }
      ]
    },
    {
      "id": "repeat_repeated_measurements",
      "provenance": "Small rstim Stim fixture created for issue #165 to cover repeat group labels and measurements inside repeated bodies.",
      "source_kind": "stim",
      "input_path": "repeat_repeated_measurements.stim",
      "expected_semantic_markers": [
        { "kind": "repeat_label", "value": "repeat x3" },
        { "kind": "operation_label", "value": "M" },
        { "kind": "measurement_anchor", "value": "m1" }
      ]
    },
    {
      "id": "noise_operation_rendering",
      "provenance": "Reuses qp101-viz/checks/noise-render.qp101.json, the existing Typst renderer noise-operation check.",
      "source_kind": "qp101_json",
      "input_path": "../../../../qp101-viz/checks/noise-render.qp101.json",
      "expected_semantic_markers": [
        { "kind": "noise_label", "value": "X_ERROR" },
        { "kind": "noise_label", "value": "DEPOLARIZE1" },
        { "kind": "noise_label", "value": "DEPOLARIZE2" }
      ]
    },
    {
      "id": "sample_shot_overlay",
      "provenance": "Reuses rstim/tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise_sample_seed7.json, the seeded sample-shot QP101 fixture already verified by qp101_fixtures.",
      "source_kind": "qp101_json",
      "input_path": "../qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise_sample_seed7.json",
      "expected_semantic_markers": [
        { "kind": "annotation_tag", "value": "sample-shot" },
        { "kind": "annotation_tag", "value": "measurement-sample" },
        { "kind": "annotation_tag", "value": "noise-sample" }
      ]
    }
  ]
}
```

- [ ] **Step 5: Run the focused test to verify it passes**

Run:

```sh
cargo test -p rstim --test qp101_svg_fixtures qp101_svg_fixture_manifest_is_valid -q
```

Expected: PASS.

- [ ] **Step 6: Run the whole new test file**

Run:

```sh
cargo test -p rstim --test qp101_svg_fixtures -q
```

Expected: PASS.

- [ ] **Step 7: Check formatting and whitespace**

Run:

```sh
cargo fmt --check -p rstim
git diff --check
```

Expected: both commands pass with no output.

- [ ] **Step 8: Commit**

Run:

```sh
git add rstim/tests/qp101_svg_fixtures.rs rstim/tests/fixtures/qp101_svg
git commit -m "test: add qp101 svg fixture catalog"
```
