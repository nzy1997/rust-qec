# APM Table A1 Manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add and validate a source-grounded JSON manifest for the P=96 and P=192 APM CSS Table A1 instances.

**Architecture:** Keep the fixture as plain JSON under qec-code test fixtures and validate it from `qec-code/tests/code.rs` with `serde_json::Value`. The validator is test-local and checks exact coefficients, dimensions, pair constraints, provenance, and a negative mutation path without adding production APM generation.

**Tech Stack:** Rust 2024, qec-code, serde_json, Cargo test fixtures.

## Global Constraints

- Manifest path is exactly `qec-code/tests/fixtures/apm/table_a1_manifest.json`.
- Include entries for exactly `apm_kasai:p=96` and `apm_kasai:p=192`.
- Do not implement Delta/Gamma generation, matrix generation, or decoder benchmarks.
- Encode Table A1 distance as an upper-bound status, not an exact distance.
- Use no new dependencies beyond qec-code's existing `serde_json`.

---

## File Structure

- Create `qec-code/tests/fixtures/apm/table_a1_manifest.json`: checked-in JSON fixture with Table A1 source data and derived expectations.
- Modify `qec-code/tests/code.rs`: add test-local manifest validator, exact expected data, focused positive test, and negative mutation test.

### Task 1: APM Table A1 Manifest Fixture And Validator

**Files:**
- Modify: `qec-code/tests/code.rs`
- Create: `qec-code/tests/fixtures/apm/table_a1_manifest.json`

**Interfaces:**
- Consumes: `serde_json::Value`.
- Produces: `validate_apm_table_a1_manifest(&Value) -> std::result::Result<(), String>` and test `apm_table_a1_manifest_pins_table_a1_reference_data`.

- [x] **Step 1: Add test imports and expected data helpers**

Add `use serde_json::Value;` near the existing imports in `qec-code/tests/code.rs`.

Add these helpers after `row_weight_counts`:

```rust
#[derive(Debug, Clone, Copy)]
struct ExpectedApmEntry {
    code_id: &'static str,
    p: u64,
    n: u64,
    mx: u64,
    mz: u64,
    k: u64,
    distance_upper_bound: u64,
    rate: &'static str,
    f: [(u64, u64); 6],
    g: [(u64, u64); 6],
    column_component_modulus: u64,
    column_component_group: &'static str,
}

const EXPECTED_APM_TABLE_A1: &[ExpectedApmEntry] = &[
    ExpectedApmEntry {
        code_id: "apm_kasai:p=96",
        p: 96,
        n: 1152,
        mx: 288,
        mz: 288,
        k: 580,
        distance_upper_bound: 12,
        rate: "0.503",
        f: [(5, 41), (85, 77), (73, 66), (1, 0), (1, 72), (37, 9)],
        g: [(61, 15), (1, 24), (89, 62), (25, 22), (85, 93), (25, 78)],
        column_component_modulus: 32,
        column_component_group: "Z32",
    },
    ExpectedApmEntry {
        code_id: "apm_kasai:p=192",
        p: 192,
        n: 2304,
        mx: 576,
        mz: 576,
        k: 1156,
        distance_upper_bound: 14,
        rate: "0.502",
        f: [(71, 127), (97, 80), (67, 117), (163, 165), (25, 60), (187, 33)],
        g: [(163, 165), (55, 183), (167, 79), (139, 41), (109, 78), (31, 27)],
        column_component_modulus: 64,
        column_component_group: "Z32xZ2",
    },
];
```

- [x] **Step 2: Add validator helpers**

Add helpers that read JSON fields, compare exact expected values, verify affine coefficients, and return `String` errors. The mutation error path must include both the code id and coefficient name, for example `apm_kasai:p=96 f[0].a`.

- [x] **Step 3: Add the focused positive and negative tests**

Add:

```rust
#[test]
fn apm_table_a1_manifest_pins_table_a1_reference_data() {
    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/apm/table_a1_manifest.json")).unwrap();

    validate_apm_table_a1_manifest(&manifest).unwrap();
}

#[test]
fn apm_table_a1_manifest_rejects_mutated_affine_coefficient() {
    let mut manifest: Value =
        serde_json::from_str(include_str!("fixtures/apm/table_a1_manifest.json")).unwrap();
    manifest["entries"][0]["f"][0]["a"] = Value::from(7);

    let err = validate_apm_table_a1_manifest(&manifest).unwrap_err();
    assert!(
        err.contains("apm_kasai:p=96") && err.contains("f[0].a"),
        "error should identify the changed coefficient and code id: {err}"
    );
}
```

- [x] **Step 4: Run the focused test and verify RED**

Run:

```bash
cargo test -p qec-code apm_table_a1_manifest -q
```

Expected: FAIL because `qec-code/tests/fixtures/apm/table_a1_manifest.json` does not exist yet.

- [x] **Step 5: Create fixture directory and manifest**

Create `qec-code/tests/fixtures/apm/table_a1_manifest.json` with:

```json
{
  "schema_version": 1,
  "manifest_id": "apm_kasai_table_a1",
  "entries": [
    {
      "code_id": "apm_kasai:p=96",
      "P": 96,
      "J": 3,
      "L": 12,
      "L2": 6,
      "f": [
        {"i": 0, "a": 5, "b": 41},
        {"i": 1, "a": 85, "b": 77},
        {"i": 2, "a": 73, "b": 66},
        {"i": 3, "a": 1, "b": 0},
        {"i": 4, "a": 1, "b": 72},
        {"i": 5, "a": 37, "b": 9}
      ],
      "g": [
        {"i": 0, "c": 61, "d": 15},
        {"i": 1, "c": 1, "d": 24},
        {"i": 2, "c": 89, "d": 62},
        {"i": 3, "c": 25, "d": 22},
        {"i": 4, "c": 85, "d": 93},
        {"i": 5, "c": 25, "d": 78}
      ],
      "expected_code_shape": {
        "n": 1152,
        "mx": 288,
        "mz": 288,
        "k": 580,
        "rate": "0.503",
        "distance": {"kind": "upper_bound", "value": 12}
      },
      "expected_weights": {
        "hx_row": 12,
        "hz_row": 12,
        "hx_column": 3,
        "hz_column": 3,
        "combined_data_qubit_degree": 6
      },
      "girth": {"kind": "lower_bound", "value": 6},
      "required_commuting_pairs": [
        {"left": "column_component:f0", "right": "column_component:f1", "modulus": 32},
        {"left": "column_component:f0", "right": "column_component:g0", "modulus": 32},
        {"left": "column_component:g0", "right": "column_component:g1", "modulus": 32}
      ],
      "required_noncommuting_pairs": [
        {"left_index": 0, "right_index": 3},
        {"left_index": 1, "right_index": 2}
      ],
      "structural_expectations": {
        "active_block_rows": 3,
        "block_columns": 12,
        "apm_maps_per_family": 6,
        "column_component_modulus": 32,
        "column_component_group_status": "abelian",
        "column_component_group": "Z32"
      },
      "provenance": {
        "paper": "arXiv:2604.16209v1",
        "table": "Table A1",
        "source_grounded_fields": ["P", "J", "L", "f", "g", "rate", "distance", "girth", "required_noncommuting_pairs"],
        "derived_fields": ["n", "mx", "mz", "expected_weights", "structural_expectations"]
      },
      "references": [
        {"kind": "paper", "url": "https://arxiv.org/abs/2604.16209", "section": "Appendix A / Table A1"},
        {"kind": "paper", "url": "https://arxiv.org/pdf/2604.16209", "section": "Appendix D.2"},
        {"kind": "local", "path": "drafts/construct_apm_css_code/README.md"},
        {"kind": "local", "path": "drafts/joint_BP_plus_PP/README.md"}
      ]
    },
    {
      "code_id": "apm_kasai:p=192",
      "P": 192,
      "J": 3,
      "L": 12,
      "L2": 6,
      "f": [
        {"i": 0, "a": 71, "b": 127},
        {"i": 1, "a": 97, "b": 80},
        {"i": 2, "a": 67, "b": 117},
        {"i": 3, "a": 163, "b": 165},
        {"i": 4, "a": 25, "b": 60},
        {"i": 5, "a": 187, "b": 33}
      ],
      "g": [
        {"i": 0, "c": 163, "d": 165},
        {"i": 1, "c": 55, "d": 183},
        {"i": 2, "c": 167, "d": 79},
        {"i": 3, "c": 139, "d": 41},
        {"i": 4, "c": 109, "d": 78},
        {"i": 5, "c": 31, "d": 27}
      ],
      "expected_code_shape": {
        "n": 2304,
        "mx": 576,
        "mz": 576,
        "k": 1156,
        "rate": "0.502",
        "distance": {"kind": "upper_bound", "value": 14}
      },
      "expected_weights": {
        "hx_row": 12,
        "hz_row": 12,
        "hx_column": 3,
        "hz_column": 3,
        "combined_data_qubit_degree": 6
      },
      "girth": {"kind": "lower_bound", "value": 6},
      "required_commuting_pairs": [
        {"left": "column_component:f0", "right": "column_component:f1", "modulus": 64},
        {"left": "column_component:f0", "right": "column_component:g0", "modulus": 64},
        {"left": "column_component:g0", "right": "column_component:g1", "modulus": 64}
      ],
      "required_noncommuting_pairs": [
        {"left_index": 0, "right_index": 3},
        {"left_index": 1, "right_index": 2}
      ],
      "structural_expectations": {
        "active_block_rows": 3,
        "block_columns": 12,
        "apm_maps_per_family": 6,
        "column_component_modulus": 64,
        "column_component_group_status": "abelian",
        "column_component_group": "Z32xZ2"
      },
      "provenance": {
        "paper": "arXiv:2604.16209v1",
        "table": "Table A1",
        "source_grounded_fields": ["P", "J", "L", "f", "g", "rate", "distance", "girth", "required_noncommuting_pairs"],
        "derived_fields": ["n", "mx", "mz", "expected_weights", "structural_expectations"]
      },
      "references": [
        {"kind": "paper", "url": "https://arxiv.org/abs/2604.16209", "section": "Appendix A / Table A1"},
        {"kind": "paper", "url": "https://arxiv.org/pdf/2604.16209", "section": "Appendix D.2"},
        {"kind": "local", "path": "drafts/construct_apm_css_code/README.md"},
        {"kind": "local", "path": "drafts/joint_BP_plus_PP/README.md"}
      ]
    }
  ]
}
```

- [x] **Step 6: Run the focused test and verify GREEN**

Run:

```bash
cargo test -p qec-code apm_table_a1_manifest -q
```

Expected: PASS. This verifies both manifest entries and the mutation negative control.

- [x] **Step 7: Run the broader required gate**

Run:

```bash
cargo test
```

Expected: PASS for the workspace.

- [x] **Step 8: Commit implementation**

Run:

```bash
git add qec-code/tests/code.rs qec-code/tests/fixtures/apm/table_a1_manifest.json docs/superpowers/plans/2026-06-23-apm-table-a1-manifest.md
git commit -m "test: add apm table a1 manifest"
```
