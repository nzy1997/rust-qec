# Issue 553 CSS Family Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a typed, versioned CSS construction contract that preserves existing qec-code constructors and fixtures.

**Architecture:** Add `qec-code/src/family_contract.rs` as the public contract boundary, reusing existing constructors from `codes::built_in_css` and `codes::quantum_tanner`. The CLI keeps its public inline syntax, adds a versioned JSON construction-spec route, and lowers both into `CssConstructionSpec` before matrix generation.

**Tech Stack:** Rust 2024, serde/serde_json, thiserror-backed `QecError`, existing GF(2) rank and CSS validation helpers, Cargo integration tests.

## Global Constraints

- `RequestedFamilyId` serializes to exactly the 14 normalized manifest IDs from `qec-code/tests/fixtures/family_manifest/manifest.v1.json`.
- `CssFamilySpec` exposes no callable planned-family stubs; add requested-family variants only when a working constructor exists.
- Generic utilities and legacy aliases are represented outside the requested-family set.
- `CssConstructionResult` distinguishes `construction_id` from optional `requested_family_id`.
- Every successful result returns sorted, duplicate-free sparse rows.
- Schema version `1` is supported; unsupported versions are rejected before construction.
- Existing legacy APIs in `qec-code/src/codes/built_in_css.rs` remain available.
- `surface_rotated:d=3` export stays byte-for-byte identical to current fixtures.
- The adapted distance-3 rotated surface code reports `n=9`, `m_x=4`, `m_z=4`, `rank_x=4`, `rank_z=4`, and `k=1`.
- `H_X H_Z^T = 0` is checked by a shared verifier.
- Metadata serialization is deterministic.

---

### Task 1: Contract Tests

**Files:**
- Create: `qec-code/tests/family_contract.rs`
- Modify: `docs/superpowers/plans/2026-07-26-issue-553-css-family-contract.md`

**Interfaces:**
- Consumes: future public module `qec_code::family_contract`.
- Produces: exact integration tests named by issue #553.

- [x] **Step 1: Write failing tests**

Create `qec-code/tests/family_contract.rs` that imports:

```rust
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, CssConstructionSpec, CssFamilySpec,
    HypergraphProductSpec, RequestedFamilyId, SurfaceFamilySpec, CLASSICAL_IDENTITY_2,
};
```

The tests should assert:

- `RequestedFamilyId::ALL` serializes to the 14 exact IDs.
- `construct_css(CssFamilySpec::Surface(...).into())` preserves the
  `surface_rotated:d=3` fixtures and reports the required stats.
- JSON with `schema_version: 2` returns `QecError::UnsupportedCssConstructionSchemaVersion { version: 2 }`.
- `CssConstructionSpec::from_inline("surface_rotated:d=3")`, the equivalent
  JSON spec, and the Rust API surface spec all compare equal.
- `CssFamilySpec::callable_requested_family_ids()` returns only
  `surface` and `quantum_tanner` in this issue.
- `CssConstructionSpec::HypergraphProduct(...)` returns
  `construction_id = "hypergraph_product"` and `requested_family_id = None`.

- [x] **Step 2: Verify red**

Run:

```text
cargo test -p qec-code --test family_contract unified_family_contract_preserves_surface_d3 -- --exact
```

Expected: FAIL because `qec_code::family_contract` does not exist.

- [ ] **Step 3: Commit after green**

After implementation tasks pass, include this test file in the implementation
commit.

### Task 2: Public Contract Module

**Files:**
- Create: `qec-code/src/family_contract.rs`
- Modify: `qec-code/src/lib.rs`
- Modify: `qec-code/src/error.rs`

**Interfaces:**
- Produces: `RequestedFamilyId`, `CssFamilySpec`, `CssConstructionSpec`,
  `CssConstructionResult`, `CssChecks`, `CssCodeStats`, `CssConstructionProvenance`,
  `SurfaceFamilySpec`, `HypergraphProductSpec`, `construct_css`,
  `parse_css_construction_json`, and `verify_css_orthogonality`.

- [ ] **Step 1: Implement minimal public types**

Add serde-backed public structs/enums matching the design. Use `BTreeMap<String,
serde_json::Value>` for deterministic normalized parameters. Add `impl From<CssFamilySpec>
for CssConstructionSpec`.

- [ ] **Step 2: Add typed errors**

Extend `QecError` with:

```rust
UnsupportedCssConstructionSchemaVersion { version: u64 },
InvalidCssConstructionJson(String),
UnknownCssConstruction { construction: String },
InvalidCssConstruction { construction: String, reason: String },
```

- [ ] **Step 3: Implement surface adapter**

`CssFamilySpec::Surface(SurfaceFamilySpec { distance })` calls the existing
`built_in_css_checks("surface_rotated:d=<distance>")`, canonicalizes rows, sets
`construction_id = "surface_rotated"`, and sets
`requested_family_id = Some(RequestedFamilyId::Surface)`.

- [ ] **Step 4: Implement quantum Tanner adapter**

`CssFamilySpec::QuantumTanner(QuantumTannerSpec)` calls
`quantum_tanner_css_checks(&spec)`, canonicalizes rows, sets
`construction_id = "quantum_tanner"`, and sets
`requested_family_id = Some(RequestedFamilyId::QuantumTanner)`.

- [ ] **Step 5: Implement legacy adapter**

`CssConstructionSpec::LegacyBuiltIn` calls existing `built_in_css_checks` and
sets `requested_family_id = None` except `surface_rotated` may be routed as the
requested surface family by `from_inline`.

- [ ] **Step 6: Implement generic HGP**

`HypergraphProductSpec` consumes two `CssClassicalCheckSpec` matrices and builds
standard HGP checks:

```text
H_X = [H1 ⊗ I(n2) | I(m1) ⊗ H2^T]
H_Z = [I(n1) ⊗ H2 | H1^T ⊗ I(m2)]
```

Return `construction_id = "hypergraph_product"` and
`requested_family_id = None`.

- [ ] **Step 7: Verify green**

Run the five issue-required exact tests and fix failures.

### Task 3: CLI Lowering

**Files:**
- Modify: `qec-code/src/cli.rs`
- Modify: `qec-code/tests/cli.rs` if existing CLI behavior needs coverage updates

**Interfaces:**
- Consumes: `CssConstructionSpec::from_inline` and `construct_css`.
- Preserves: `run_css` output contract.

- [x] **Step 1: Route CSS export through the contract**

Change `run_css` so it parses `code_id` with
`CssConstructionSpec::from_inline(code_id)`, constructs once through
`construct_css`, selects `h_x` or `h_z`, and serializes through
`SparseRowsMatrix::to_json_string()`.

- [x] **Step 2: Add structured JSON CSS construction export**

Add `code css construct --spec <path> <matrix>` so versioned JSON constructor
requests are read from disk, parsed with `parse_css_construction_json`, lowered
to `CssConstructionSpec`, and exported through the same `construct_css` matrix
generation path as inline inputs.

- [x] **Step 3: Keep distance helpers legacy-compatible**

Leave distance helper input syntax unchanged unless a focused test shows the
same lowering is needed there. The issue's byte-preservation requirement binds
CSS export output.

- [x] **Step 4: Verify fixtures**

Run:

```text
cargo test -p qec-code --test cli built_in_css_fixture_manifest_exports_match_pinned_json -- --exact
```

Expected: PASS.

### Task 4: Documentation And Final Verification

**Files:**
- Modify: `qec-code/README.md` or `qec-code/doc/*` only if a natural contract doc location exists
- Modify: `docs/superpowers/plans/2026-07-26-issue-553-css-family-contract.md`

**Interfaces:**
- Produces: documented inline/JSON routing rule.

- [ ] **Step 1: Document routing rule**

Add concise docs near existing qec-code CSS documentation explaining:

- compact inline constructors route through `CssConstructionSpec::from_inline`
- structured constructors use versioned JSON with `schema_version = 1`
- both lower to the same typed layer before matrix generation

- [ ] **Step 2: Run required issue verification**

Run:

```text
cargo test -p qec-code --test family_contract unified_family_contract_preserves_surface_d3 -- --exact
cargo test -p qec-code --test family_contract unified_family_contract_rejects_unknown_schema -- --exact
cargo test -p qec-code --test family_contract inline_json_and_rust_routes_lower_to_same_spec -- --exact
cargo test -p qec-code --test family_contract planned_families_have_no_callable_stub -- --exact
cargo test -p qec-code --test family_contract generic_construction_identity_is_not_a_requested_family -- --exact
cargo test
```

- [ ] **Step 3: Review and finish branch**

Run the Superpowers verification, review, and finishing branch flow. Choose
"Push and create a Pull Request" under the non-interactive standing policy.

## Self-Review

- Spec coverage: all issue acceptance criteria map to Tasks 1-4.
- Placeholder scan: no TBD/TODO placeholders are present.
- Type consistency: names in the plan match the intended public API.
- Scope check: implementation is one focused qec-code public contract plus CLI
  routing, not a broad implementation of all planned families.
