# Issue 179 Quantum Tanner Fixture Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a shared quantum Tanner acceptance fixture catalog under `qec-code/tests/fixtures/quantum_tanner/` and a manifest-backed test that enforces provenance, contract references, outcomes, verifier commands, and consuming issues.

**Architecture:** Keep the catalog as plain JSON test fixtures so later parser and constructor issues can consume the same files before any runtime parser exists. Keep validation in `qec-code/tests/code.rs` as test-local manifest checks that read the catalog from disk and assert the known-answer toric metadata plus negative-control rejection metadata.

**Tech Stack:** Rust 2024 integration tests, `serde_json::Value`, JSON fixtures, existing `qec-code/doc/quantum_tanner.md` contract vocabulary.

## Global Constraints

- Fixture directory is exactly `qec-code/tests/fixtures/quantum_tanner/`.
- Catalog index path is exactly `qec-code/tests/fixtures/quantum_tanner/manifest.json`.
- Manifest `schema_version` is `1`.
- Manifest `manifest_id` is `quantum_tanner_acceptance_v1`.
- Manifest verifier command is exactly `cargo test -p qec-code quantum_tanner_fixture_catalog_has_grounded_cases -q`.
- Every manifest entry must have provenance, input path, contract/schema reference, expected success or rejection result, verifier command, and consuming issue numbers.
- Positive `toric_d4` must use `Z4 x Z4`, construction mode `lr_cayley_no_cover_v1`, symmetric `A = [4, 12]`, symmetric `B = [1, 3]`, repetition seed checks `[[1, 1]]`, expected `n = 16`, `k = 2`, `d = 4`, and check weight `4`.
- The toric fixture provenance must say it is a reference-derived known-answer fixture, not copied qLDPC implementation code.
- Include reference locations: `drafts/qLDPC/src/qldpc/codes/quantum.py`, `drafts/qLDPC/src/qldpc/objects.py`, `drafts/qLDPC/src/qldpc/codes/quantum_test.py`, and `https://github.com/RebKatRad/qTanner`.
- Include invalid fixture `invalid_non_symmetric_a` with expected rejection reason `NonSymmetricGeneratorSet`.
- Include invalid fixture `invalid_bad_table` with expected rejection reason `InvalidGroupTable` so issue #178 can consume a cataloged malformed-table input.
- Do not implement the parser, constructor, CLI, distance integration, qTanner importer, qLDPC importer, or external group search.
- Local git index writes are unavailable in this Agent Desk sandbox; implementation should edit and verify files, and the controller will create the final remote commit and PR through the GitHub API.

---

## File Structure

- Create `qec-code/tests/fixtures/quantum_tanner/manifest.json`: catalog metadata and entry index.
- Create `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`: valid input fixture copied from the contract's data shape and generated `Z4 x Z4` table, not from qLDPC code.
- Create `qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json`: same shape as `toric_d4` but with `a_generator_indices: [4]`.
- Create `qec-code/tests/fixtures/quantum_tanner/invalid_bad_table.json`: same shape as `toric_d4` but with one multiplication-table row shortened to make a syntactically loaded but malformed table.
- Modify `qec-code/tests/code.rs`: add manifest validation helpers and the focused test `quantum_tanner_fixture_catalog_has_grounded_cases`.
- Keep this plan in `docs/superpowers/plans/2026-06-24-issue-179-quantum-tanner-fixture-catalog.md`.

### Task 1: Quantum Tanner Fixture Catalog And Manifest Test

**Files:**
- Create: `qec-code/tests/fixtures/quantum_tanner/manifest.json`
- Create: `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`
- Create: `qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json`
- Create: `qec-code/tests/fixtures/quantum_tanner/invalid_bad_table.json`
- Modify: `qec-code/tests/code.rs`
- Modify: `docs/superpowers/plans/2026-06-24-issue-179-quantum-tanner-fixture-catalog.md`

**Interfaces:**
- Consumes: existing `serde_json::Value` import and helper functions `required_field`, `required_array_field`, `expect_len`, `expect_u64_field`, `expect_str_field`, `expect_string_array_field`, `usize_array`, `usize_matrix`, `assert_group_table_shape`, and `generators_are_symmetric` in `qec-code/tests/code.rs`.
- Produces: integration test `quantum_tanner_fixture_catalog_has_grounded_cases`.

- [x] **Step 1: Write the failing manifest test**

Add these helpers near the existing quantum Tanner contract test in `qec-code/tests/code.rs`:

```rust
const QUANTUM_TANNER_FIXTURE_DIR: &str = "tests/fixtures/quantum_tanner";
const QUANTUM_TANNER_VERIFIER_COMMAND: &str =
    "cargo test -p qec-code quantum_tanner_fixture_catalog_has_grounded_cases -q";

fn qec_code_manifest_fixture_path(rel_path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel_path)
}

fn load_quantum_tanner_fixture(path: &str) -> Value {
    let full_path = qec_code_manifest_fixture_path(path);
    let contents = std::fs::read_to_string(&full_path)
        .unwrap_or_else(|error| panic!("fixture {full_path:?} should be readable: {error}"));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("fixture {full_path:?} should be valid JSON: {error}"))
}

fn nonempty_string_field<'a>(object: &'a Value, path: &str, key: &str) -> Result<&'a str, String> {
    let field_path = format!("{path}.{key}");
    let value = required_field(object, path, key)?
        .as_str()
        .ok_or_else(|| format!("{field_path}: expected string"))?;
    if value.trim().is_empty() {
        Err(format!("{field_path}: expected nonempty string"))
    } else {
        Ok(value)
    }
}
```

Add validation that checks required fields, expected outcomes, input paths, and fixture-specific invariants. The test entry point must be:

```rust
#[test]
fn quantum_tanner_fixture_catalog_has_grounded_cases() {
    let manifest =
        load_quantum_tanner_fixture("tests/fixtures/quantum_tanner/manifest.json");
    validate_quantum_tanner_catalog(&manifest).unwrap();
}
```

- [x] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p qec-code quantum_tanner_fixture_catalog_has_grounded_cases -q`

Expected: FAIL because `qec-code/tests/fixtures/quantum_tanner/manifest.json` does not exist yet, proving the test is wired to the catalog path.

- [x] **Step 3: Create the catalog fixtures**

Create `toric_d4.json` with the v1 contract fields:

```json
{
  "fixture_id": "toric_d4",
  "construction_mode": "lr_cayley_no_cover_v1",
  "base_group": {
    "name": "Z4xZ4",
    "element_order": "id = 4*x + y for (x,y) in Z4 x Z4",
    "order": 16,
    "identity": 0,
    "multiplication_table": [
      [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
    ]
  },
  "a_generator_indices": [4, 12],
  "b_generator_indices": [1, 3],
  "local_codes": {
    "matrix_role": "parity_check",
    "field": "GF(2)",
    "h_a": [[1, 1]],
    "h_b": [[1, 1]]
  }
}
```

The plan snippet shows the first multiplication-table row only to keep the plan
readable. The real `toric_d4.json` file must contain the full numeric `16 x 16`
table where row `4*x1 + y1`, column `4*x2 + y2` is
`4 * ((x1 + x2) mod 4) + ((y1 + y2) mod 4)`. This is the same table documented
in `qec-code/doc/quantum_tanner.md`. Create `invalid_non_symmetric_a.json` from
the same object but use `"fixture_id": "invalid_non_symmetric_a"` and
`"a_generator_indices": [4]`. Create `invalid_bad_table.json` from the same
object but use `"fixture_id": "invalid_bad_table"` and shorten the first
multiplication-table row to `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
14]`.

- [x] **Step 4: Create `manifest.json`**

Create a manifest with three entries:

```json
{
  "schema_version": 1,
  "manifest_id": "quantum_tanner_acceptance_v1",
  "contract": {
    "issue": 177,
    "path": "qec-code/doc/quantum_tanner.md",
    "construction_mode": "lr_cayley_no_cover_v1"
  },
  "verifier_command": "cargo test -p qec-code quantum_tanner_fixture_catalog_has_grounded_cases -q",
  "entries": [
    {
      "fixture_id": "toric_d4",
      "input_path": "qec-code/tests/fixtures/quantum_tanner/toric_d4.json",
      "contract_reference": {"issue": 177, "path": "qec-code/doc/quantum_tanner.md", "schema_version": 1},
      "provenance": {
        "kind": "reference_derived_known_answer",
        "summary": "Derived from the qLDPC toric Tanner known-answer test semantics; no qLDPC implementation code is copied.",
        "source_grounded_fields": ["construction_mode", "base_group", "a_generator_indices", "b_generator_indices", "local_codes", "expected_result"]
      },
      "references": [
        {"kind": "local", "path": "drafts/qLDPC/src/qldpc/codes/quantum.py"},
        {"kind": "local", "path": "drafts/qLDPC/src/qldpc/objects.py"},
        {"kind": "local", "path": "drafts/qLDPC/src/qldpc/codes/quantum_test.py"},
        {"kind": "external", "url": "https://github.com/qLDPCOrg/qLDPC"},
        {"kind": "external", "url": "https://github.com/RebKatRad/qTanner"}
      ],
      "expected_result": {"kind": "success", "n": 16, "k": 2, "d": 4, "check_weight": 4},
      "verifier_command": "cargo test -p qec-code quantum_tanner_fixture_catalog_has_grounded_cases -q",
      "consuming_issues": [178, 180, 181, 183, 184, 185, 186, 188]
    }
  ]
}
```

Add matching rejection entries for `invalid_non_symmetric_a` and `invalid_bad_table` with `expected_result.kind = "rejection"` and reasons `NonSymmetricGeneratorSet` and `InvalidGroupTable`.

- [x] **Step 5: Run the focused test and verify GREEN**

Run: `cargo test -p qec-code quantum_tanner_fixture_catalog_has_grounded_cases -q`

Expected: PASS with one test executed in `qec-code/tests/code.rs`.

- [x] **Step 6: Run the requested crate verification**

Run: `cargo test`

Expected: PASS for the workspace test suite.

- [x] **Step 7: Self-review and prepare handoff**

Run: `git diff --check`

Expected: no whitespace errors.

Review the diff for out-of-scope parser, constructor, CLI, or distance code. There should be none.
