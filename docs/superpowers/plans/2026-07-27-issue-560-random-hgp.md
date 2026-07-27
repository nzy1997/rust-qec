# Issue 560 Random HGP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build deterministic parameterized random-HGP CSS codes from two versioned regular-classical sampler specs.

**Architecture:** Add a focused `qec-code/src/codes/random_hgp.rs` module that owns random-HGP spec parsing, seed validation, deterministic classical sampling, and sampled metadata. Wire that module into `family_contract.rs` as `CssFamilySpec::RandomHgp`, then lower sampled matrices to the existing general HGP construction helper so Rust and CLI routes share one contract.

**Tech Stack:** Rust 2024, `serde`, `serde_json`, existing `regular_classical`, existing `SparseGf2Matrix` HGP constructor, existing `qec-code` CLI tests.

## Global Constraints

- Issue #560 fixture: `n=6`, `m=4`, column weight 2, row weight 3, seed 7, version 1 on both sides.
- Fixture output must return `n=52`, `m_x=24`, `m_z=24`, `rank_x=21`, `rank_z=21`, and `k=10`.
- Every CSS check row has weight 5 and all checks are orthogonal.
- Repeated construction is byte-for-byte deterministic.
- Metadata includes both normalized classical specifications and sampler version.
- The implementation does not claim or brute-force a general distance.
- Rust API and CLI use the common family contract.
- Reject a missing seed, an impossible degree sequence, an unknown sampler version, and a retry-exhausted classical input.
- Use the existing deterministic regular-classical sampler and the existing general HGP constructor; do not invoke an external RNG or algebra tool.

---

### Task 1: Random HGP Family Contract

**Files:**
- Create: `qec-code/src/codes/random_hgp.rs`
- Modify: `qec-code/src/codes/mod.rs`
- Modify: `qec-code/src/error.rs`
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/tests/family_contract.rs`
- Create: `qec-code/tests/random_hgp.rs`

**Interfaces:**
- Consumes:
  - `qec_code::regular_classical::{REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1, RegularClassicalMatrixConfig, deterministic_regular_matrix}`
  - `qec_code::family_contract::{CssClassicalCheckSpec, HypergraphProductSpec, construct_css, parse_css_construction_json, verify_css_orthogonality}`
  - existing CLI route `code css construct --spec <path> hx|hz|metadata`
- Produces:
  - `pub mod qec_code::codes::random_hgp`
  - `pub struct RegularClassicalCodeSpec`
  - `pub struct RandomHgpSpec`
  - `pub struct RandomHgpClassicalSample`
  - `pub struct RandomHgpClassicalSamples`
  - `pub fn random_hgp_spec_from_json_str(input: &str) -> Result<RandomHgpSpec>`
  - `pub fn sample_random_hgp_classical_matrices(spec: &RandomHgpSpec) -> Result<RandomHgpClassicalSamples>`
  - `pub fn sampled_random_hgp_to_hgp_spec(samples: &RandomHgpClassicalSamples) -> HypergraphProductSpec`
  - `QecError::InvalidRandomHgpSpec { option: &'static str, reason: String }`
  - `CssFamilySpec::RandomHgp(RandomHgpSpec)`

- [ ] **Step 1: Write the failing fixture and negative tests**

Create `qec-code/tests/random_hgp.rs` with these tests. This file deliberately uses the wished-for public API before implementation.

```rust
use std::ffi::OsString;

use clap::Parser;
use qec_code::QecError;
use qec_code::cli::{Cli, run};
use qec_code::codes::random_hgp::{
    RandomHgpSpec, RegularClassicalCodeSpec, random_hgp_spec_from_json_str,
    sample_random_hgp_classical_matrices,
};
use qec_code::family_contract::{
    CssFamilySpec, RequestedFamilyId, construct_css, parse_css_construction_json,
    verify_css_orthogonality,
};
use qec_code::regular_classical::REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1;
use tempfile::tempdir;

fn regular_fixture_spec(seed: u64) -> RegularClassicalCodeSpec {
    RegularClassicalCodeSpec {
        column_count: 6,
        row_count: 4,
        column_weight: 2,
        row_weight: 3,
        seed,
        algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
        retry_limit: 16,
    }
}

fn fixture_spec() -> RandomHgpSpec {
    RandomHgpSpec::new(regular_fixture_spec(7), regular_fixture_spec(7)).unwrap()
}

fn fixture_json() -> String {
    r#"{"schema_version":1,"construction":"random_hgp","left":{"column_count":6,"row_count":4,"column_weight":2,"row_weight":3,"seed":7,"algorithm_version":1,"retry_limit":16},"right":{"column_count":6,"row_count":4,"column_weight":2,"row_weight":3,"seed":7,"algorithm_version":1,"retry_limit":16}}"#.to_owned()
}

fn fixture_json_without_left_seed() -> String {
    r#"{"schema_version":1,"construction":"random_hgp","left":{"column_count":6,"row_count":4,"column_weight":2,"row_weight":3,"algorithm_version":1,"retry_limit":16},"right":{"column_count":6,"row_count":4,"column_weight":2,"row_weight":3,"seed":7,"algorithm_version":1,"retry_limit":16}}"#.to_owned()
}

fn expected_classical_rows() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 2],
        vec![0, 3, 4],
        vec![1, 3, 5],
        vec![2, 4, 5],
    ]
}

fn run_qec_code_in_process(args: &[&str]) -> Result<String, QecError> {
    let mut argv = vec![OsString::from("qec-code")];
    argv.extend(args.iter().map(OsString::from));
    run(Cli::parse_from(argv))
}

#[test]
fn random_hgp_seed7_matches_fixture() {
    let samples = sample_random_hgp_classical_matrices(&fixture_spec()).unwrap();
    assert_eq!(samples.left.rows, expected_classical_rows());
    assert_eq!(samples.right.rows, expected_classical_rows());

    let result = construct_css(CssFamilySpec::RandomHgp(fixture_spec()).into()).unwrap();
    assert_eq!(result.construction_id, "random_hgp");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::RandomHgp));
    assert_eq!(result.stats.n, 52);
    assert_eq!(result.stats.m_x, 24);
    assert_eq!(result.stats.m_z, 24);
    assert_eq!(result.stats.rank_x, 21);
    assert_eq!(result.stats.rank_z, 21);
    assert_eq!(result.stats.k, 10);
    assert_eq!(result.stats.d_x, None);
    assert_eq!(result.stats.d_z, None);
    assert!(result.checks.h_x.iter().all(|row| row.len() == 5));
    assert!(result.checks.h_z.iter().all(|row| row.len() == 5));
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    assert_eq!(
        result.normalized_parameters["left"]["classical_spec"]["seed"],
        serde_json::json!(7)
    );
    assert_eq!(
        result.normalized_parameters["left"]["classical_spec"]["algorithm_version"],
        serde_json::json!(REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1)
    );
    assert_eq!(
        result.normalized_parameters["left"]["sampler_version"],
        serde_json::json!(REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1)
    );
    assert_eq!(
        result.normalized_parameters["left"]["rows"],
        serde_json::json!(expected_classical_rows())
    );
    assert_eq!(
        result.normalized_parameters["right"]["classical_spec"]["seed"],
        serde_json::json!(7)
    );
    assert_eq!(
        result.normalized_parameters["right"]["sampler_version"],
        serde_json::json!(REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1)
    );
    assert_eq!(
        result.normalized_parameters["right"]["rows"],
        serde_json::json!(expected_classical_rows())
    );

    let repeated = construct_css(CssFamilySpec::RandomHgp(fixture_spec()).into()).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&repeated).unwrap()
    );

    let parsed = parse_css_construction_json(&fixture_json()).unwrap();
    assert_eq!(parsed, CssFamilySpec::RandomHgp(fixture_spec()).into());
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(parsed_result.checks, result.checks);
    assert_eq!(parsed_result.normalized_parameters, result.normalized_parameters);

    let direct_spec = random_hgp_spec_from_json_str(&fixture_json()).unwrap();
    assert_eq!(direct_spec, fixture_spec());

    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("random-hgp.json");
    std::fs::write(&spec_path, fixture_json()).unwrap();
    let path = spec_path.to_str().unwrap();

    let hx_json: serde_json::Value =
        serde_json::from_str(&run_qec_code_in_process(&["code", "css", "construct", "--spec", path, "hx"]).unwrap()).unwrap();
    let hz_json: serde_json::Value =
        serde_json::from_str(&run_qec_code_in_process(&["code", "css", "construct", "--spec", path, "hz"]).unwrap()).unwrap();
    let metadata: serde_json::Value =
        serde_json::from_str(&run_qec_code_in_process(&["code", "css", "construct", "--spec", path, "metadata"]).unwrap()).unwrap();

    assert_eq!(hx_json["format"], "sparse_rows");
    assert_eq!(hx_json["num_cols"], 52);
    assert_eq!(hx_json["rows"], serde_json::json!(result.checks.h_x));
    assert_eq!(hz_json["format"], "sparse_rows");
    assert_eq!(hz_json["num_cols"], 52);
    assert_eq!(hz_json["rows"], serde_json::json!(result.checks.h_z));
    assert_eq!(metadata["construction_id"], "random_hgp");
    assert_eq!(metadata["requested_family_id"], "random_hgp");
    assert_eq!(metadata["stats"]["k"], 10);
    assert_eq!(
        metadata["normalized_parameters"]["left"]["rows"],
        serde_json::json!(expected_classical_rows())
    );
}

#[test]
fn random_hgp_rejects_unreproducible_specs() {
    assert!(matches!(
        parse_css_construction_json(&fixture_json_without_left_seed()),
        Err(QecError::InvalidRandomHgpSpec { option: "seed", .. })
    ));

    let impossible = RandomHgpSpec::new(
        RegularClassicalCodeSpec {
            column_count: 5,
            row_count: 4,
            column_weight: 2,
            row_weight: 3,
            seed: 7,
            algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
            retry_limit: 16,
        },
        regular_fixture_spec(7),
    )
    .unwrap();
    assert!(matches!(
        sample_random_hgp_classical_matrices(&impossible),
        Err(QecError::RegularClassicalMatrixStubCountMismatch {
            column_stubs: 10,
            row_stubs: 12,
        })
    ));

    let unknown_version = RandomHgpSpec::new(
        RegularClassicalCodeSpec {
            algorithm_version: 2,
            ..regular_fixture_spec(7)
        },
        regular_fixture_spec(7),
    )
    .unwrap();
    assert_eq!(
        sample_random_hgp_classical_matrices(&unknown_version),
        Err(QecError::UnsupportedRegularClassicalMatrixAlgorithm {
            algorithm_version: 2,
        })
    );

    let retry_exhausted = RandomHgpSpec::new(
        RegularClassicalCodeSpec {
            column_count: 3,
            row_count: 3,
            column_weight: 2,
            row_weight: 2,
            seed: 1,
            algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
            retry_limit: 1,
        },
        regular_fixture_spec(7),
    )
    .unwrap();
    assert!(matches!(
        sample_random_hgp_classical_matrices(&retry_exhausted),
        Err(QecError::RegularClassicalMatrixGenerationExhausted {
            retry_limit: 1,
            attempts: 1,
            ..
        })
    ));
}
```

- [ ] **Step 2: Run the fixture test to verify RED**

Run:

```bash
cargo test -p qec-code --test random_hgp random_hgp_seed7_matches_fixture -- --exact
```

Expected: FAIL at compile time with unresolved imports for `qec_code::codes::random_hgp` and `CssFamilySpec::RandomHgp`.

- [ ] **Step 3: Add the random-HGP module**

Create `qec-code/src/codes/random_hgp.rs` with the exact public API used by the tests:

```rust
use serde::Deserialize;

use crate::error::{QecError, Result};
use crate::family_contract::{CssClassicalCheckSpec, HypergraphProductSpec};
use crate::regular_classical::{RegularClassicalMatrixConfig, deterministic_regular_matrix};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RegularClassicalCodeSpec {
    pub column_count: usize,
    pub row_count: usize,
    pub column_weight: usize,
    pub row_weight: usize,
    pub seed: u64,
    pub algorithm_version: u32,
    pub retry_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomHgpSpec {
    pub left: RegularClassicalCodeSpec,
    pub right: RegularClassicalCodeSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomHgpClassicalSample {
    pub spec: RegularClassicalCodeSpec,
    pub rows: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomHgpClassicalSamples {
    pub left: RandomHgpClassicalSample,
    pub right: RandomHgpClassicalSample,
}

#[derive(Debug, Deserialize)]
struct RandomHgpSpecJson {
    left: RegularClassicalCodeSpecJson,
    right: RegularClassicalCodeSpecJson,
}

#[derive(Debug, Deserialize)]
struct RegularClassicalCodeSpecJson {
    column_count: usize,
    row_count: usize,
    column_weight: usize,
    row_weight: usize,
    seed: Option<u64>,
    algorithm_version: u32,
    retry_limit: usize,
}

impl RandomHgpSpec {
    pub fn new(left: RegularClassicalCodeSpec, right: RegularClassicalCodeSpec) -> Result<Self> {
        Ok(Self { left, right })
    }
}

pub fn random_hgp_spec_from_json_str(input: &str) -> Result<RandomHgpSpec> {
    let parsed: RandomHgpSpecJson = serde_json::from_str(input)
        .map_err(|error| QecError::InvalidCssConstructionJson(error.to_string()))?;
    RandomHgpSpec::new(
        regular_spec_from_json(parsed.left)?,
        regular_spec_from_json(parsed.right)?,
    )
}

pub fn sample_random_hgp_classical_matrices(
    spec: &RandomHgpSpec,
) -> Result<RandomHgpClassicalSamples> {
    Ok(RandomHgpClassicalSamples {
        left: sample_classical(spec.left)?,
        right: sample_classical(spec.right)?,
    })
}

pub fn sampled_random_hgp_to_hgp_spec(
    samples: &RandomHgpClassicalSamples,
) -> HypergraphProductSpec {
    HypergraphProductSpec {
        left: CssClassicalCheckSpec {
            num_cols: samples.left.spec.column_count,
            rows: samples.left.rows.clone(),
        },
        right: CssClassicalCheckSpec {
            num_cols: samples.right.spec.column_count,
            rows: samples.right.rows.clone(),
        },
    }
}

fn regular_spec_from_json(parsed: RegularClassicalCodeSpecJson) -> Result<RegularClassicalCodeSpec> {
    let seed = parsed.seed.ok_or_else(|| QecError::InvalidRandomHgpSpec {
        option: "seed",
        reason: "must be provided".to_owned(),
    })?;
    Ok(RegularClassicalCodeSpec {
        column_count: parsed.column_count,
        row_count: parsed.row_count,
        column_weight: parsed.column_weight,
        row_weight: parsed.row_weight,
        seed,
        algorithm_version: parsed.algorithm_version,
        retry_limit: parsed.retry_limit,
    })
}

fn sample_classical(spec: RegularClassicalCodeSpec) -> Result<RandomHgpClassicalSample> {
    let rows = deterministic_regular_matrix(RegularClassicalMatrixConfig {
        column_count: spec.column_count,
        row_count: spec.row_count,
        column_weight: spec.column_weight,
        row_weight: spec.row_weight,
        seed: spec.seed,
        algorithm_version: spec.algorithm_version,
        retry_limit: spec.retry_limit,
    })?;
    Ok(RandomHgpClassicalSample { spec, rows })
}
```

Modify `qec-code/src/codes/mod.rs`:

```rust
pub mod random_hgp;
```

Add the new line next to the existing `random_two_block` module export.

- [ ] **Step 4: Wire random-HGP into errors and the common family contract**

Modify `qec-code/src/error.rs` by adding this variant near the random-family errors:

```rust
#[error("invalid random HGP spec option {option}: {reason}")]
InvalidRandomHgpSpec {
    option: &'static str,
    reason: String,
},
```

Modify `qec-code/src/family_contract.rs`:

1. Import the module API:

```rust
use crate::codes::random_hgp::{
    RandomHgpClassicalSample, RandomHgpSpec, random_hgp_spec_from_json_str,
    sample_random_hgp_classical_matrices, sampled_random_hgp_to_hgp_spec,
};
```

2. Add `RandomHgp(RandomHgpSpec)` to `CssFamilySpec`.

3. Add `RequestedFamilyId::RandomHgp` to `CssFamilySpec::callable_requested_family_ids()` next to `RandomTwoBlock`.

4. Add a `construct_css` match arm:

```rust
CssConstructionSpec::Family(CssFamilySpec::RandomHgp(spec)) => {
    let samples = sample_random_hgp_classical_matrices(&spec)?;
    let hgp = sampled_random_hgp_to_hgp_spec(&samples);
    let result = construct_hypergraph_product_from_parts(
        hgp,
        "random_hgp",
        Some(RequestedFamilyId::RandomHgp),
        random_hgp_normalized_parameters(&samples),
        "random_hgp",
        "CssFamilySpec::RandomHgp",
    )?;
    Ok(result)
}
```

5. Split the existing `construct_hypergraph_product` body into a helper:

```rust
fn construct_hypergraph_product(spec: HypergraphProductSpec) -> Result<CssConstructionResult> {
    let parameters = normalized_hypergraph_product_parameters(&spec)?;
    construct_hypergraph_product_from_parts(
        spec,
        "hypergraph_product",
        None,
        parameters,
        "hypergraph_product",
        "CssConstructionSpec::HypergraphProduct",
    )
}
```

Use a helper that owns the existing HGP matrix arithmetic and accepts construction metadata:

```rust
fn construct_hypergraph_product_from_parts(
    spec: HypergraphProductSpec,
    construction_id: &'static str,
    requested_family_id: Option<RequestedFamilyId>,
    normalized_parameters: BTreeMap<String, Value>,
    adapter: &'static str,
    source: &'static str,
) -> Result<CssConstructionResult> {
    let HypergraphProductSpec {
        left: left_spec,
        right: right_spec,
    } = spec;
    let left = classical_check_matrix(left_spec)?;
    let right = classical_check_matrix(right_spec)?;

    let left_identity_rows = SparseGf2Matrix::identity(left.num_rows())?;
    let left_identity_cols = SparseGf2Matrix::identity(left.num_cols())?;
    let right_identity_rows = SparseGf2Matrix::identity(right.num_rows())?;
    let right_identity_cols = SparseGf2Matrix::identity(right.num_cols())?;
    let left_transpose = left.transpose()?;
    let right_transpose = right.transpose()?;

    let h_x = left
        .kron(&right_identity_cols)?
        .hconcat(&left_identity_rows.kron(&right_transpose)?)?;
    let h_z = left_identity_cols
        .kron(&right)?
        .hconcat(&left_transpose.kron(&right_identity_rows)?)?;
    debug_assert_eq!(h_x.num_cols(), h_z.num_cols());

    construction_result(
        construction_id,
        requested_family_id,
        normalized_parameters,
        h_x.num_cols(),
        h_x.rows().to_vec(),
        h_z.rows().to_vec(),
        adapter,
        source,
        None,
    )
}
```

6. Add normalized-parameter helpers:

```rust
fn normalized_hypergraph_product_parameters(
    spec: &HypergraphProductSpec,
) -> Result<BTreeMap<String, Value>> {
    let left = classical_check_matrix(spec.left.clone())?;
    let right = classical_check_matrix(spec.right.clone())?;
    let mut parameters = BTreeMap::new();
    parameters.insert(
        "left".to_owned(),
        serde_json::to_value(CssClassicalCheckSpec {
            num_cols: left.num_cols(),
            rows: left.rows().to_vec(),
        })
        .expect("serializable spec"),
    );
    parameters.insert(
        "right".to_owned(),
        serde_json::to_value(CssClassicalCheckSpec {
            num_cols: right.num_cols(),
            rows: right.rows().to_vec(),
        })
        .expect("serializable spec"),
    );
    Ok(parameters)
}

fn random_hgp_normalized_parameters(
    samples: &crate::codes::random_hgp::RandomHgpClassicalSamples,
) -> BTreeMap<String, Value> {
    let mut parameters = BTreeMap::new();
    parameters.insert(
        "left".to_owned(),
        random_hgp_classical_parameters(&samples.left),
    );
    parameters.insert(
        "right".to_owned(),
        random_hgp_classical_parameters(&samples.right),
    );
    parameters
}

fn random_hgp_classical_parameters(sample: &RandomHgpClassicalSample) -> Value {
    serde_json::json!({
        "classical_spec": sample.spec,
        "rows": sample.rows,
        "sampler_version": sample.spec.algorithm_version,
    })
}
```

7. Add parser support:

```rust
"random_hgp" => Ok(CssFamilySpec::RandomHgp(random_hgp_spec_from_json_str(input)?).into()),
```

- [ ] **Step 5: Update common contract tests**

Modify `qec-code/tests/family_contract.rs` expected callable IDs to include `RequestedFamilyId::RandomHgp` after `RequestedFamilyId::RandomTwoBlock`:

```rust
assert_eq!(
    CssFamilySpec::callable_requested_family_ids(),
    &[
        RequestedFamilyId::Surface,
        RequestedFamilyId::QuantumTanner,
        RequestedFamilyId::Toric3d,
        RequestedFamilyId::RandomTwoBlock,
        RequestedFamilyId::RandomHgp,
        RequestedFamilyId::Color666,
        RequestedFamilyId::ShorLike,
        RequestedFamilyId::Directional,
    ]
);
```

- [ ] **Step 6: Run the targeted tests to verify GREEN**

Run:

```bash
cargo test -p qec-code --test random_hgp random_hgp_seed7_matches_fixture -- --exact
cargo test -p qec-code --test random_hgp random_hgp_rejects_unreproducible_specs -- --exact
cargo test -p qec-code --test family_contract planned_families_have_no_callable_stub -- --exact
```

Expected: all pass.

- [ ] **Step 7: Run formatting and qec-code tests**

Run:

```bash
rustfmt --edition 2024 --check qec-code/src/codes/random_hgp.rs qec-code/src/codes/mod.rs qec-code/src/error.rs qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/random_hgp.rs
cargo test -p qec-code
```

Expected: all pass.

- [ ] **Step 8: Commit**

Commit the implementation:

```bash
git add qec-code/src/codes/random_hgp.rs qec-code/src/codes/mod.rs qec-code/src/error.rs qec-code/src/family_contract.rs qec-code/tests/family_contract.rs qec-code/tests/random_hgp.rs
git commit -m "feat: construct deterministic random hgp codes"
```

Plan complete and saved to `docs/superpowers/plans/2026-07-27-issue-560-random-hgp.md`. Two execution options:

1. Subagent-Driven (recommended) - dispatch a fresh subagent per task, review between tasks, fast iteration
2. Inline Execution - execute tasks in this session using executing-plans, batch execution with checkpoints
