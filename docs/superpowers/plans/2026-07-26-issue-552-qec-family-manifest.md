# Issue 552 QEC Family Manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a versioned, machine-readable fixture manifest for the 14 requested QEC construction families and validate its lifecycle contract.

**Architecture:** Keep the source of truth as JSON under `qec-code/tests/fixtures/family_manifest/`, document the schema in the same directory, and validate it from a test-local Rust integration test. The test uses serde enums for lifecycle values and explicit validation for issue #552's exact family set, supported/deferred split, constructor restrictions, future case capacity, and deterministic serialization.

**Tech Stack:** Rust 2024 integration tests, `serde` derives from `qec-code` dependencies, `serde_json`, checked-in JSON fixtures, Cargo workspace tests.

## Global Constraints

- Manifest path is exactly `qec-code/tests/fixtures/family_manifest/manifest.v1.json`.
- Schema documentation path is exactly `qec-code/tests/fixtures/family_manifest/README.md`.
- Test file is exactly `qec-code/tests/family_manifest.rs`.
- Manifest top-level `schema_version` is `1`.
- Manifest `manifest_id` is `qec_family_construction_targets_v1`.
- Every family entry has `schema_version = 1`.
- The manifest contains exactly these IDs in this order: `directional`, `quantum_tanner`, `generalized_bicycle`, `la_cross`, `random_hgp`, `lifted_product`, `hyperbolic_5_5`, `coprime_bb`, `toric_3d`, `color_666`, `surface`, `shor_like`, `random_two_block`, `perturbed_hgp`.
- Target disposition enum values are exactly `supported` and `deferred`.
- Runtime availability enum values are exactly `planned`, `available`, and `not_applicable`.
- The only legal disposition/availability pairs are `(supported, planned)`, `(supported, available)`, and `(deferred, not_applicable)`.
- In this issue, exactly `hyperbolic_5_5` and `perturbed_hgp` are deferred with `availability = not_applicable`.
- In this issue, the other 12 entries are supported with `availability = planned`.
- Promotion to `available` is gated by GitHub issue #573.
- Every entry has non-empty `provenance`, `verification`, and `intended_consumers` arrays.
- Supported entries declare at least one `positive` executable case and at least one `negative` executable case.
- The manifest holds at least 24 supported-family executable cases total.
- Deferred entries declare no executable cases.
- Planned and deferred entries cannot declare a non-null `callable_constructor`.
- This issue does not add family constructors, CLI commands, public runtime registries, or public runtime APIs.
- The referenced file `docs/design/2026-07-26-qec-code-family-support.md` is absent in this checkout; ground implementation in GitHub issue #552 and this plan.

---

## File Structure

- Create `qec-code/tests/family_manifest.rs`: typed schema structs/enums, manifest loading, validation helpers, deterministic serialization check, positive coverage test, and negative-control test.
- Create `qec-code/tests/fixtures/family_manifest/manifest.v1.json`: versioned JSON source of truth for the 14 QEC family targets.
- Create `qec-code/tests/fixtures/family_manifest/README.md`: schema documentation colocated with the manifest.
- Keep this plan in `docs/superpowers/plans/2026-07-26-issue-552-qec-family-manifest.md`.

### Task 1: QEC Family Manifest Fixture And Validator

**Files:**
- Create: `qec-code/tests/family_manifest.rs`
- Create: `qec-code/tests/fixtures/family_manifest/manifest.v1.json`
- Create: `qec-code/tests/fixtures/family_manifest/README.md`
- Modify: `docs/superpowers/plans/2026-07-26-issue-552-qec-family-manifest.md`

**Interfaces:**
- Consumes: checked-in JSON fixture text from `include_str!("fixtures/family_manifest/manifest.v1.json")` and README text from `include_str!("fixtures/family_manifest/README.md")`.
- Produces: test-local function `parse_and_validate_family_manifest_value(value: serde_json::Value) -> Result<FamilyManifest, String>`.
- Produces: integration test `family_manifest_covers_requested_qec_families`.
- Produces: integration test `family_manifest_rejects_invalid_entries`.

- [x] **Step 1: Write the failing typed manifest test**

Create `qec-code/tests/family_manifest.rs` with typed serde structs and
validators before creating the fixture files. The test must load the manifest
with `include_str!("fixtures/family_manifest/manifest.v1.json")`, so this step
fails until the fixture exists.

Use these exact constants and enums:

```rust
const MANIFEST_TEXT: &str = include_str!("fixtures/family_manifest/manifest.v1.json");
const SCHEMA_TEXT: &str = include_str!("fixtures/family_manifest/README.md");

const MANIFEST_SCHEMA_VERSION: u64 = 1;
const MANIFEST_ID: &str = "qec_family_construction_targets_v1";
const PROMOTION_GATE_ISSUE: u64 = 573;

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

const DEFERRED_FAMILY_IDS: &[&str] = &["hyperbolic_5_5", "perturbed_hgp"];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum FamilyDisposition {
    Supported,
    Deferred,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeAvailability {
    Planned,
    Available,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize, serde::Serialize)]
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
```

Use serde structs with `#[serde(deny_unknown_fields)]` for the top-level
manifest, promotion gate, family entries, callable constructor reference, and
executable cases. Field order must be:

```rust
struct FamilyManifest {
    schema_version: u64,
    manifest_id: String,
    provenance: Vec<String>,
    verification: Vec<String>,
    intended_consumers: Vec<String>,
    availability_promotion_gate: AvailabilityPromotionGate,
    families: Vec<FamilyManifestEntry>,
}

struct AvailabilityPromotionGate {
    issue: u64,
    rule: String,
}

struct FamilyManifestEntry {
    schema_version: u64,
    family_id: String,
    disposition: FamilyDisposition,
    availability: RuntimeAvailability,
    provenance: Vec<String>,
    verification: Vec<String>,
    intended_consumers: Vec<String>,
    callable_constructor: Option<CallableConstructorRef>,
    executable_cases: Vec<ExecutableCase>,
}

struct CallableConstructorRef {
    rust_path: String,
}

struct ExecutableCase {
    case_id: String,
    case_kind: ExecutableCaseKind,
    expected_outcome: ExpectedOutcome,
    description: String,
    verification: Vec<String>,
}
```

Add helper behavior:

- `parse_and_validate_family_manifest_value` deserializes from `serde_json::Value` and then calls `validate_family_manifest`.
- `validate_family_manifest` checks every global constraint in this plan.
- `validate_lifecycle_pair` allows exactly the three legal pairs.
- `expect_nonempty_strings` rejects missing, empty, or whitespace-only strings in required arrays.
- `validate_supported_cases` requires at least one positive/success and one negative/rejection case for each supported entry.
- `assert_schema_doc_mentions_contract` checks the README text contains `schema_version`, `disposition`, `availability`, `supported`, `deferred`, `planned`, `available`, `not_applicable`, `callable_constructor`, `executable_cases`, and `issue #573`.

Add these exact tests:

```rust
#[test]
fn family_manifest_covers_requested_qec_families() {
    let manifest = parse_and_validate_family_manifest_text(MANIFEST_TEXT)
        .expect("family manifest should satisfy issue #552");
    assert_schema_doc_mentions_contract(SCHEMA_TEXT);

    let serialized = serde_json::to_string_pretty(&manifest).unwrap();
    assert_eq!(
        format!("{serialized}\n"),
        MANIFEST_TEXT,
        "checked-in manifest should be canonical pretty JSON"
    );
}

#[test]
fn family_manifest_rejects_invalid_entries() {
    expect_manifest_rejection(
        "duplicate family ID",
        |value| {
            value["families"][1]["family_id"] = value["families"][0]["family_id"].clone();
        },
        "duplicate family_id",
    );
    expect_manifest_rejection(
        "missing provenance",
        |value| {
            value["families"][0].as_object_mut().unwrap().remove("provenance");
        },
        "provenance",
    );
    expect_manifest_rejection(
        "unknown disposition",
        |value| {
            value["families"][0]["disposition"] = serde_json::json!("research");
        },
        "unknown variant",
    );
    expect_manifest_rejection(
        "unknown availability",
        |value| {
            value["families"][0]["availability"] = serde_json::json!("prototype");
        },
        "unknown variant",
    );
    expect_manifest_rejection(
        "illegal disposition/availability pair",
        |value| {
            value["families"][0]["availability"] = serde_json::json!("not_applicable");
        },
        "illegal disposition/availability pair",
    );
    expect_manifest_rejection(
        "deferred callable constructor",
        |value| {
            value["families"][6]["callable_constructor"] =
                serde_json::json!({"rust_path": "qec_code::codes::hyperbolic::construct"});
        },
        "cannot declare callable_constructor",
    );
    expect_manifest_rejection(
        "planned callable constructor",
        |value| {
            value["families"][10]["callable_constructor"] =
                serde_json::json!({"rust_path": "qec_code::codes::surface::construct"});
        },
        "cannot declare callable_constructor",
    );
}
```

- [x] **Step 2: Run focused test and verify RED**

Run:

```bash
cargo test -p qec-code --test family_manifest family_manifest_covers_requested_qec_families -- --exact
```

Expected: FAIL because `qec-code/tests/fixtures/family_manifest/manifest.v1.json`
and `qec-code/tests/fixtures/family_manifest/README.md` do not exist yet.

- [x] **Step 3: Create the manifest fixture**

Create `qec-code/tests/fixtures/family_manifest/manifest.v1.json` as canonical
pretty JSON with the field order defined by the Rust structs.

Top-level values:

```json
{
  "schema_version": 1,
  "manifest_id": "qec_family_construction_targets_v1",
  "provenance": [
    "GitHub issue #552, Roadmap ID M1-01, requested the 14 normalized QEC family targets.",
    "The repository design reference named in issue #552 is absent in this checkout; this manifest is grounded in the issue body."
  ],
  "verification": [
    "cargo test -p qec-code --test family_manifest family_manifest_covers_requested_qec_families -- --exact",
    "cargo test -p qec-code --test family_manifest family_manifest_rejects_invalid_entries -- --exact"
  ],
  "intended_consumers": [
    "qec-code construction-roadmap tracking",
    "future qec-code executable fixture gates",
    "issue #573 availability promotion gate"
  ],
  "availability_promotion_gate": {
    "issue": 573,
    "rule": "Supported families remain availability=planned until issue #573 verifies constructors and executable positive/negative fixture coverage before promotion to availability=available."
  },
  "families": []
}
```

Populate `families` with exactly the 14 IDs from `REQUESTED_FAMILY_IDS` in the
same order. For each supported family, use `disposition = "supported"`,
`availability = "planned"`, `callable_constructor = null`, and exactly two
`executable_cases`: one positive/success and one negative/rejection. For each
deferred family, use `disposition = "deferred"`, `availability =
"not_applicable"`, `callable_constructor = null`, and `executable_cases = []`.

Use this per-supported-family entry pattern, replacing `<id>` and text with
the exact family ID:

```json
{
  "schema_version": 1,
  "family_id": "<id>",
  "disposition": "supported",
  "availability": "planned",
  "provenance": [
    "GitHub issue #552 lists <id> as a supported QEC construction target."
  ],
  "verification": [
    "cargo test -p qec-code --test family_manifest family_manifest_covers_requested_qec_families -- --exact",
    "issue #573 must verify constructor execution before this family can become availability=available"
  ],
  "intended_consumers": [
    "qec-code family manifest readers",
    "future executable fixture cases for <id>"
  ],
  "callable_constructor": null,
  "executable_cases": [
    {
      "case_id": "<id>_positive_smoke",
      "case_kind": "positive",
      "expected_outcome": "success",
      "description": "Future small valid <id> construction case for issue #573 availability promotion.",
      "verification": [
        "planned for issue #573 constructor fixture coverage"
      ]
    },
    {
      "case_id": "<id>_negative_rejection",
      "case_kind": "negative",
      "expected_outcome": "rejection",
      "description": "Future invalid <id> construction case that must reject before availability promotion.",
      "verification": [
        "planned for issue #573 constructor fixture coverage"
      ]
    }
  ]
}
```

Use this per-deferred-family entry pattern:

```json
{
  "schema_version": 1,
  "family_id": "<id>",
  "disposition": "deferred",
  "availability": "not_applicable",
  "provenance": [
    "GitHub issue #552 explicitly defers <id> as a research target."
  ],
  "verification": [
    "cargo test -p qec-code --test family_manifest family_manifest_covers_requested_qec_families -- --exact"
  ],
  "intended_consumers": [
    "qec-code family manifest readers",
    "research-roadmap tracking for <id>"
  ],
  "callable_constructor": null,
  "executable_cases": []
}
```

- [x] **Step 4: Create the colocated schema README**

Create `qec-code/tests/fixtures/family_manifest/README.md` with:

```markdown
# QEC Family Manifest Schema

`manifest.v1.json` is the versioned source of truth for the QEC construction
family targets tracked by issue #552.

## Version

The top-level `schema_version` and every entry-level `schema_version` must be
`1`. Serialization is canonicalized by `qec-code/tests/family_manifest.rs` with
`serde_json::to_string_pretty`.

## Lifecycle Fields

`disposition` is typed and must be either `supported` or `deferred`.
`availability` is typed and must be `planned`, `available`, or
`not_applicable`.

Legal pairs are exactly:

- `(supported, planned)`
- `(supported, available)`
- `(deferred, not_applicable)`

For issue #552, every supported family remains `availability=planned`. Promotion
to `available` is controlled by issue #573 after constructors and executable
fixture coverage are complete.

## Required Entry Fields

Every family entry records `provenance`, `verification`, and
`intended_consumers` as non-empty arrays. `callable_constructor` must be null
for planned and deferred entries. Supported entries may declare
`executable_cases`; each supported entry in this fixture declares one
`positive`/`success` case and one `negative`/`rejection` case.

Deferred entries do not declare executable cases.
```

- [x] **Step 5: Run focused tests and verify GREEN**

Run:

```bash
cargo test -p qec-code --test family_manifest family_manifest_covers_requested_qec_families -- --exact
cargo test -p qec-code --test family_manifest family_manifest_rejects_invalid_entries -- --exact
```

Expected: both commands PASS.

- [ ] **Step 6: Run workspace verification**

Run:

```bash
cargo test
```

Expected: PASS for the workspace suite.

- [x] **Step 7: Self-review and commit**

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

Review the diff for out-of-scope constructors, CLI additions, or public runtime
APIs. There should be none.

Commit:

```bash
git add qec-code/tests/family_manifest.rs qec-code/tests/fixtures/family_manifest/manifest.v1.json qec-code/tests/fixtures/family_manifest/README.md docs/superpowers/plans/2026-07-26-issue-552-qec-family-manifest.md
git commit -m "test: add qec family manifest"
```
