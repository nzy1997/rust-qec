# Quantum Tanner Contract Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the existing quantum-Tanner constructor into the common CSS family contract with legacy-output parity, typed error preservation, and normalized metadata.

**Architecture:** Keep the legacy `QuantumTannerSpec` parser and `quantum_tanner_css_checks` constructor as the only quantum-Tanner implementation. Add focused contract tests, then extend the shared `construction_result` metadata path with a deterministic digest and source provenance so all construction adapters benefit from the same normalization.

**Tech Stack:** Rust 2024, `qec-code`, `serde_json`, `sha2`, Cargo integration tests.

## Global Constraints

- Do not rewrite quantum-Tanner group logic.
- Do not remove or change the legacy `code css quantum-tanner` CLI behavior.
- Preserve typed errors including `InvalidQuantumTannerGeneratorSet` and `InvalidQuantumTannerGroupTable`.
- Use `qec-code/tests/fixtures/quantum_tanner/toric_d4.json` as the compatibility fixture.
- The fixture must report `n=16`, `k=2`, distance `4`, and check weight `4`.
- Run `cargo test -p qec-code --test quantum_tanner_contract quantum_tanner_toric_d4_matches_legacy_constructor -- --exact`.
- Run `cargo test -p qec-code --test quantum_tanner_contract quantum_tanner_contract_preserves_typed_errors -- --exact`.
- Run `cargo test -p qec-code quantum_tanner`.
- Run `cargo test` before PR creation.

---

### Task 1: Add Quantum Tanner Contract Tests

**Files:**
- Create: `qec-code/tests/quantum_tanner_contract.rs`

**Interfaces:**
- Consumes: `quantum_tanner_spec_from_json_str(&str) -> Result<QuantumTannerSpec>`, `quantum_tanner_css_checks(&QuantumTannerSpec) -> Result<QuantumTannerCssChecks>`, `construct_css(CssConstructionSpec) -> Result<CssConstructionResult>`, `parse_css_construction_json(&str) -> Result<CssConstructionSpec>`.
- Produces: integration tests named `quantum_tanner_toric_d4_matches_legacy_constructor` and `quantum_tanner_contract_preserves_typed_errors`.

- [ ] **Step 1: Write the failing tests**

Create `qec-code/tests/quantum_tanner_contract.rs` with this structure:

```rust
use qec_code::codes::quantum_tanner::{
    quantum_tanner_css_checks, quantum_tanner_spec_from_json_str,
};
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, CssFamilySpec, RequestedFamilyId,
    verify_css_orthogonality,
};
use qec_code::QecError;

fn canonical(mut rows: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
    for row in &mut rows {
        row.sort_unstable();
    }
    rows
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(row.windows(2).all(|window| window[0] < window[1]));
    }
}

fn quantum_tanner_request(spec_json: &str) -> String {
    format!(r#"{{"schema_version":1,"construction":"quantum_tanner","spec":{spec_json}}}"#)
}

#[test]
fn quantum_tanner_toric_d4_matches_legacy_constructor() {
    let fixture = include_str!("fixtures/quantum_tanner/toric_d4.json");
    let spec = quantum_tanner_spec_from_json_str(fixture).unwrap();
    let legacy = quantum_tanner_css_checks(&spec).unwrap();

    let common = construct_css(CssFamilySpec::QuantumTanner(spec).into()).unwrap();

    assert_eq!(common.construction_id, "quantum_tanner");
    assert_eq!(
        common.requested_family_id,
        Some(RequestedFamilyId::QuantumTanner)
    );
    assert_eq!(common.checks.h_x, canonical(legacy.hx));
    assert_eq!(common.checks.h_z, canonical(legacy.hz));
    assert_eq!(common.stats.n, 16);
    assert_eq!(common.stats.k, 2);
    assert!(common
        .checks
        .h_x
        .iter()
        .chain(common.checks.h_z.iter())
        .all(|row| row.len() == 4));
    assert_canonical_sparse_rows(&common.checks.h_x);
    assert_canonical_sparse_rows(&common.checks.h_z);
    verify_css_orthogonality(common.stats.n, &common.checks.h_x, &common.checks.h_z).unwrap();

    let hx = SparseRowsMatrix::new(common.stats.n, common.checks.h_x.clone())
        .unwrap()
        .to_dense_rows();
    let hz = SparseRowsMatrix::new(common.stats.n, common.checks.h_z.clone())
        .unwrap()
        .to_dense_rows();
    let css = CssCode::from_hx_hz(hx, hz).unwrap();
    let distance = compute_distance(css.code()).unwrap();
    assert_eq!(distance.distance, 4);
    assert_eq!(distance.witness.weight(), 4);

    assert_eq!(common.provenance.adapter, "quantum_tanner");
    assert_eq!(common.provenance.source, "CssFamilySpec::QuantumTanner");
    assert!(common
        .provenance
        .normalized_input_digest
        .starts_with("sha256:"));
    assert_eq!(common.provenance.normalized_input_digest.len(), "sha256:".len() + 64);

    let json_common =
        construct_css(parse_css_construction_json(&quantum_tanner_request(fixture)).unwrap())
            .unwrap();
    assert_eq!(
        json_common.provenance.normalized_input_digest,
        common.provenance.normalized_input_digest
    );
    assert_eq!(json_common.checks, common.checks);
}

#[test]
fn quantum_tanner_contract_preserves_typed_errors() {
    let non_symmetric = include_str!("fixtures/quantum_tanner/invalid_non_symmetric_a.json");
    let non_symmetric_spec = parse_css_construction_json(&quantum_tanner_request(non_symmetric))
        .expect("non-symmetric generator set parses before construction validation");
    assert!(matches!(
        construct_css(non_symmetric_spec),
        Err(QecError::InvalidQuantumTannerGeneratorSet { set: "A", .. })
    ));

    let bad_table = include_str!("fixtures/quantum_tanner/invalid_bad_table.json");
    assert!(matches!(
        parse_css_construction_json(&quantum_tanner_request(bad_table)),
        Err(QecError::InvalidQuantumTannerGroupTable { .. })
    ));
}
```

- [ ] **Step 2: Run the first exact test to verify it fails**

Run: `cargo test -p qec-code --test quantum_tanner_contract quantum_tanner_toric_d4_matches_legacy_constructor -- --exact`

Expected: FAIL to compile because `CssConstructionProvenance` does not yet expose `source` or `normalized_input_digest`.

- [ ] **Step 3: Run the error-preservation exact test to verify the test file compiles as far as possible**

Run: `cargo test -p qec-code --test quantum_tanner_contract quantum_tanner_contract_preserves_typed_errors -- --exact`

Expected: the same compile failure from missing provenance fields before implementation.

### Task 2: Add Common Metadata Digest And Source Provenance

**Files:**
- Modify: `qec-code/Cargo.toml`
- Modify: `qec-code/src/family_contract.rs`

**Interfaces:**
- Consumes: existing `construction_result(...)` helper and `CssConstructionProvenance`.
- Produces: `CssConstructionProvenance { adapter, source, normalized_input_digest }` with `sha256:<64 lowercase hex chars>`.

- [ ] **Step 1: Add the digest dependency**

In `qec-code/Cargo.toml`, add `sha2 = "0.10"` under `[dependencies]`.

- [ ] **Step 2: Import SHA-256 helpers**

At the top of `qec-code/src/family_contract.rs`, add:

```rust
use sha2::{Digest, Sha256};
```

- [ ] **Step 3: Extend provenance metadata**

Change `CssConstructionProvenance` to:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CssConstructionProvenance {
    pub adapter: String,
    pub source: String,
    pub normalized_input_digest: String,
}
```

- [ ] **Step 4: Add source arguments to construction result calls**

Extend `construction_result` with a `source: impl Into<String>` argument and pass these exact source strings:

```rust
"CssFamilySpec::Surface"
"CssFamilySpec::QuantumTanner"
"CssConstructionSpec::HypergraphProduct"
"CssConstructionSpec::LegacyBuiltIn"
```

- [ ] **Step 5: Compute the normalized digest in `construction_result`**

Use a stable JSON payload and lower-hex encoder:

```rust
fn normalized_input_digest(
    construction_id: &str,
    requested_family_id: Option<RequestedFamilyId>,
    normalized_parameters: &BTreeMap<String, Value>,
) -> String {
    let payload = serde_json::json!({
        "schema_version": CSS_CONSTRUCTION_SCHEMA_VERSION,
        "construction_id": construction_id,
        "requested_family_id": requested_family_id,
        "normalized_parameters": normalized_parameters,
    });
    let json = serde_json::to_vec(&payload).expect("normalized construction input is serializable");
    format!("sha256:{}", lower_hex(&Sha256::digest(json)))
}

fn lower_hex(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
```

In `construction_result`, convert `construction_id`, `adapter`, and `source` to `String` before building the result, compute `normalized_input_digest`, and populate all provenance fields.

- [ ] **Step 6: Run exact contract tests**

Run: `cargo test -p qec-code --test quantum_tanner_contract quantum_tanner_toric_d4_matches_legacy_constructor -- --exact`

Expected: PASS.

Run: `cargo test -p qec-code --test quantum_tanner_contract quantum_tanner_contract_preserves_typed_errors -- --exact`

Expected: PASS.

- [ ] **Step 7: Run existing quantum-Tanner tests and full verification**

Run: `cargo test -p qec-code quantum_tanner`

Expected: PASS.

Run: `cargo test`

Expected: PASS.
