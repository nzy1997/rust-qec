# Issue 573 Family Catalog Completeness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the issue #573 completeness gate for exactly 14 requested QEC families, with 12 available callable families and 2 deferred non-callable families.

**Architecture:** Keep the canonical catalog as deterministic JSON under `qec-code/tests/fixtures/family_manifest/`. Add a focused `family_catalog` integration test that validates the fixture against production family/constructor registries and executes positive/negative cases. Add the missing `CssFamilySpec::LiftedProduct` route while preserving the existing `CssConstructionSpec::LiftedProduct` route.

**Tech Stack:** Rust 2024, serde/serde_json, qec-code integration tests, existing `construct_css` and `parse_css_construction_json` APIs.

## Global Constraints

- The requested-family IDs are exactly `directional`, `quantum_tanner`, `generalized_bicycle`, `la_cross`, `random_hgp`, `lifted_product`, `hyperbolic_5_5`, `coprime_bb`, `toric_3d`, `color_666`, `surface`, `shor_like`, `random_two_block`, and `perturbed_hgp`.
- The supported available IDs are exactly `directional`, `quantum_tanner`, `generalized_bicycle`, `la_cross`, `random_hgp`, `lifted_product`, `coprime_bb`, `toric_3d`, `color_666`, `surface`, `shor_like`, and `random_two_block`.
- The deferred IDs are exactly `hyperbolic_5_5` and `perturbed_hgp`.
- Deferred entries must remain `availability = not_applicable`, link research contracts, and expose no callable constructors or CLI aliases.
- Every available family must have normalized inputs, provenance, expected dimensions and ranks, row-weight summaries, a distance-verification class, an executable verifier, and at least one consumer.
- Every available family must have at least one positive and one negative executable case.
- Generic construction utilities and documented legacy aliases are checked through a separate `CssConstructionSpec` registry and must not become requested-family manifest entries.
- Fixture serialization and case order must be deterministic.

---

### Task 1: Add Failing Family Catalog Gate

**Files:**
- Create: `qec-code/tests/family_catalog.rs`
- Modify: `qec-code/tests/fixtures/family_manifest/README.md`

**Interfaces:**
- Consumes: existing `RequestedFamilyId::ALL`, `CssFamilySpec::callable_requested_family_ids()`, `parse_css_construction_json`, and `construct_css`.
- Produces: the four required exact integration tests and test-local validation helpers.

- [ ] **Step 1: Write the failing test file**

Create `qec-code/tests/family_catalog.rs` with typed serde structs for the v1 manifest, helpers to validate lifecycle pairs, helper functions `parse_and_validate_catalog_text`, `validate_catalog_with_registries`, `execute_positive_case`, `execute_negative_case`, and the four required tests:

```rust
#[test]
fn complete_catalog_has_12_supported_and_2_deferred_families() {
    let catalog = parse_and_validate_catalog_text(MANIFEST_TEXT).unwrap();
    assert_eq!(catalog.families.len(), 14);
    assert_eq!(available_family_ids(&catalog), SUPPORTED_FAMILY_IDS);
    assert_eq!(deferred_family_ids(&catalog), DEFERRED_FAMILY_IDS);
    assert_eq!(serde_json::to_string_pretty(&catalog).unwrap() + "\n", MANIFEST_TEXT);
}

#[test]
fn every_supported_family_has_positive_and_negative_cases() {
    let catalog = parse_and_validate_catalog_text(MANIFEST_TEXT).unwrap();
    for family in available_families(&catalog) {
        execute_positive_cases(family);
        execute_negative_cases(family);
    }
}

#[test]
fn catalog_rejects_coverage_gaps() {
    expect_catalog_rejection("duplicate family ID", duplicate_family_id, "duplicate family_id");
    expect_catalog_rejection("third deferred family", make_third_deferred, "expected exactly two deferred families");
    expect_catalog_rejection("available family without a negative case", remove_negative_case, "requires at least one negative");
    expect_catalog_rejection("planned family with callable constructor", make_planned_callable, "planned family cannot declare callable_constructor");
    expect_catalog_rejection("deferred callable constructor", make_deferred_callable, "deferred family cannot declare callable_constructor");
    expect_registry_rejection("available family without callable variant", without_surface_callable, "available family \"surface\" has no callable CssFamilySpec variant");
    expect_construction_registry_rejection("undocumented alias", with_undocumented_alias, "undocumented non-family construction alias");
}

#[test]
fn requested_and_construction_registries_are_disjoint_and_complete() {
    let catalog = parse_and_validate_catalog_text(MANIFEST_TEXT).unwrap();
    assert_requested_family_bijection(&catalog);
    assert_available_families_match_callable_variants(&catalog, CssFamilySpec::callable_requested_family_ids());
    assert_non_family_construction_registry(CssConstructionSpec::documented_non_family_construction_ids());
}
```

- [ ] **Step 2: Run the new exact tests to verify RED**

Run:

```bash
cargo test -p qec-code --test family_catalog complete_catalog_has_12_supported_and_2_deferred_families -- --exact
```

Expected: FAIL because the current manifest still marks supported families `planned` and production code does not yet expose the needed registry.

- [ ] **Step 3: Update schema README text**

Update `qec-code/tests/fixtures/family_manifest/README.md` to describe the available-state fields: `normalized_inputs`, `expected`, `row_weight_summary`, `distance_verification`, `executable_verifier`, `research_contracts`, and executable positive/negative cases.

- [ ] **Step 4: Commit Task 1**

```bash
git add qec-code/tests/family_catalog.rs qec-code/tests/fixtures/family_manifest/README.md
git commit -m "test(qec-code): add family catalog completeness gate"
```

### Task 2: Add Missing Lifted-Product Family Route And Construction Registry

**Files:**
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/tests/family_contract.rs`
- Modify: `qec-code/tests/lifted_product.rs`

**Interfaces:**
- Consumes: existing `LiftedProductSpec`, `construct_lifted_product`, `parse_css_construction_json`, and `CssConstructionSpec::LiftedProduct`.
- Produces: `CssFamilySpec::LiftedProduct(LiftedProductSpec)`, complete callable-family ID list, and `CssConstructionSpec::documented_non_family_construction_ids()`.

- [ ] **Step 1: Write focused failing assertions**

Update `qec-code/tests/lifted_product.rs` so `lifted_product_c3_matches_fixture` also constructs through `CssFamilySpec::LiftedProduct` and asserts provenance source `CssFamilySpec::LiftedProduct`.

Update `qec-code/tests/family_contract.rs` so `planned_families_have_no_callable_stub` becomes `available_families_have_callable_variants` and expects the 12 supported IDs in issue order, including `RequestedFamilyId::LiftedProduct`.

- [ ] **Step 2: Run focused tests to verify RED**

Run:

```bash
cargo test -p qec-code --test lifted_product lifted_product_c3_matches_fixture -- --exact
cargo test -p qec-code --test family_contract available_families_have_callable_variants -- --exact
```

Expected: FAIL because `CssFamilySpec::LiftedProduct` and the full callable list are missing.

- [ ] **Step 3: Implement the family variant**

Modify `qec-code/src/family_contract.rs`:

```rust
pub enum CssFamilySpec {
    Directional(DirectionalCssSpec),
    QuantumTanner(QuantumTannerSpec),
    GeneralizedBicycle(GeneralizedBicycleSpec),
    LaCross(LaCrossSpec),
    RandomHgp(RandomHgpSpec),
    LiftedProduct(LiftedProductSpec),
    CoprimeBb(CoprimeBivariateBicycleSpec),
    Toric3d(Toric3dSpec),
    Color666(Color666FamilySpec),
    Surface(SurfaceFamilySpec),
    ShorLike(ShorLikeSpec),
    RandomTwoBlock(RandomTwoBlockSpec),
}
```

Route `CssConstructionSpec::Family(CssFamilySpec::LiftedProduct(spec))` through a helper that uses source `CssFamilySpec::LiftedProduct`. Keep `CssConstructionSpec::LiftedProduct(spec)` and route it through the same helper with source `CssConstructionSpec::LiftedProduct`.

Change JSON parsing for `"lifted_product"` to return `CssFamilySpec::LiftedProduct(...).into()`.

- [ ] **Step 4: Add construction registry**

Add this exact public registry shape:

```rust
pub const DOCUMENTED_NON_FAMILY_CONSTRUCTION_IDS: &[&str] = &[
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

impl CssConstructionSpec {
    pub const fn documented_non_family_construction_ids() -> &'static [&'static str] {
        DOCUMENTED_NON_FAMILY_CONSTRUCTION_IDS
    }
}
```

- [ ] **Step 5: Run focused tests to verify GREEN**

Run:

```bash
cargo test -p qec-code --test lifted_product lifted_product_c3_matches_fixture -- --exact
cargo test -p qec-code --test family_contract available_families_have_callable_variants -- --exact
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

```bash
git add qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/lifted_product.rs
git commit -m "feat(qec-code): expose lifted product as requested family"
```

### Task 3: Promote Manifest To Available Executable Catalog

**Files:**
- Modify: `qec-code/tests/fixtures/family_manifest/manifest.v1.json`
- Modify: `qec-code/tests/family_catalog.rs`
- Delete: `qec-code/tests/family_manifest.rs`

**Interfaces:**
- Consumes: Task 1 validation helpers and Task 2 family/registry APIs.
- Produces: deterministic available-state manifest and executable positive/negative cases for all 12 supported families.

- [ ] **Step 1: Update the manifest fixture**

Rewrite `manifest.v1.json` in canonical pretty JSON with all 12 supported
entries marked `availability = "available"`. Each available entry must contain:

```json
{
  "callable_constructor": {
    "rust_path": "qec_code::family_contract::CssFamilySpec::<Variant>",
    "construction": "<family_id>"
  },
  "normalized_inputs": [<non-empty strings>],
  "expected": {
    "n": 0,
    "m_x": 0,
    "m_z": 0,
    "rank_x": 0,
    "rank_z": 0,
    "k": 0,
    "d_x": null,
    "d_z": null
  },
  "row_weight_summary": {
    "h_x": [{"weight": 0, "count": 0}],
    "h_z": [{"weight": 0, "count": 0}]
  },
  "distance_verification": {
    "class": "constructor_known_exact|contract_metadata|structural_not_pinned",
    "description": "<non-empty>"
  },
  "executable_verifier": {
    "name": "family_catalog_construct_css_contract_v1",
    "command": "cargo test -p qec-code --test family_catalog every_supported_family_has_positive_and_negative_cases -- --exact"
  },
  "executable_cases": [
    {"case_id": "<family>_positive_smoke", "case_kind": "positive", "expected_outcome": "success", "request": {"schema_version": 1, "construction": "<family>"}},
    {"case_id": "<family>_negative_rejection", "case_kind": "negative", "expected_outcome": "rejection", "request": {"schema_version": 1, "construction": "<family>"}, "expected_error_contains": "<typed error text>"}
  ]
}
```

Use actual expected values from the existing constructors and fixtures.

- [ ] **Step 2: Delete the obsolete planned-state test target**

Delete `qec-code/tests/family_manifest.rs` after `family_catalog.rs` covers the promoted manifest. The old target asserts supported families must remain planned, which contradicts issue #573.

- [ ] **Step 3: Run required exact tests**

Run the four issue commands:

```bash
cargo test -p qec-code --test family_catalog complete_catalog_has_12_supported_and_2_deferred_families -- --exact
cargo test -p qec-code --test family_catalog every_supported_family_has_positive_and_negative_cases -- --exact
cargo test -p qec-code --test family_catalog catalog_rejects_coverage_gaps -- --exact
cargo test -p qec-code --test family_catalog requested_and_construction_registries_are_disjoint_and_complete -- --exact
```

Expected: PASS.

- [ ] **Step 4: Commit Task 3**

```bash
git add qec-code/tests/family_catalog.rs qec-code/tests/fixtures/family_manifest/manifest.v1.json qec-code/tests/fixtures/family_manifest/README.md qec-code/tests/family_manifest.rs
git commit -m "test(qec-code): enforce complete requested family catalog"
```

### Task 4: Final Verification And PR

**Files:**
- No new files.

**Interfaces:**
- Consumes: committed Tasks 1-3.
- Produces: verified branch and pull request.

- [ ] **Step 1: Run full required verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 2: Run final code review**

Dispatch a Superpowers code-review subagent with the branch diff from merge-base to HEAD. Fix Critical and Important findings, then re-run the covering tests.

- [ ] **Step 3: Finish branch with PR option**

Use `superpowers:finishing-a-development-branch`, choose `Push and create a Pull Request`, push the worker branch, and open a PR against `master` that closes #573.
