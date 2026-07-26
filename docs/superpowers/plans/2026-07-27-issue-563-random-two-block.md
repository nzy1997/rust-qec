# Random Two-Block Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic random two-block finite-group-algebra CSS construction for issue #563.

**Architecture:** Implement the sampling and lifted matrix assembly in a focused `qec_code::codes::random_two_block` module that reuses `SplitMix64V1`, `bounded_index_v1`, `FiniteGroupSpec`, and regular lift operations. Adapt `family_contract.rs` only for the common JSON/Rust construction contract, so CLI commands such as `code css construct --spec random-two-block-s3.json hx` use the same path as Rust API calls.

**Tech Stack:** Rust 2024, existing `qec-code` crate, `serde`/`serde_json`, `sha2`, `thiserror`, Cargo integration tests.

## Global Constraints

- Use pure Rust only; do not add new dependencies.
- Algorithm version 1 must use `SplitMix64V1` and `bounded_index_v1` from `qec_code::regular_classical`.
- Version 1 support sampling must be partial Fisher-Yates: for each support, initialize `pool = 0..order`; for `i` in `0..weight`, draw `offset = bounded_index_v1(&mut stream, (order - i) as u64).unwrap()`, set `j = i + offset as usize`, swap `pool[i]` and `pool[j]`, then canonicalize the first `weight` selected elements in ascending order.
- Use one stream in order: sample support A first, then support B.
- Reject `support_a_weight == 0`, `support_b_weight == 0`, weights larger than group order, missing JSON seed, unknown algorithm versions, invalid group tables, and group orders above `MAX_FINITE_GROUP_ORDER`.
- Construct `FiniteGroupSpec` before any sampler step so #557's order limit and table validation run before sampling.
- Build `H_X = [L(A) | R(B)]` and `H_Z = [R(B)^T | L(A)^T]` using existing left/right regular lifts.
- Explicitly verify CSS orthogonality after matrix assembly.
- Metadata must record `group_digest`, `seed`, `support_a_weight`, `support_b_weight`, and `algorithm_version`.
- The S3 fixture must use element order `0=e, 1=r, 2=r^2, 3=s, 4=rs, 5=r^2s` and the exact table from issue #563.
- Seed 7 with weights 2/2 must select `support_a = [3, 5]`, `support_b = [0, 4]`, exact fixture `H_X`/`H_Z`, stats `n=12`, `rank_x=5`, `rank_z=5`, `k=2`, and distance 2.
- Rust API and CLI must use the common `CssFamilySpec` / `construct_css` contract.

---

### Task 1: Core Random Two-Block Constructor

**Files:**
- Create: `qec-code/src/codes/random_two_block.rs`
- Modify: `qec-code/src/codes/mod.rs`
- Modify: `qec-code/src/error.rs`
- Create: `qec-code/tests/random_two_block.rs`

**Interfaces:**
- Consumes: `qec_code::finite_group::{FiniteGroupSpec, GroupAlgebraElement, left_regular_lift, right_regular_lift}`, `qec_code::regular_classical::{SplitMix64V1, bounded_index_v1}`, `qec_code::sparse_gf2::SparseGf2Matrix`, `qec_code::QecError`.
- Produces: `qec_code::codes::random_two_block::{RANDOM_TWO_BLOCK_ALGORITHM_V1, RandomTwoBlockSpec, RandomTwoBlockCssChecks, RandomTwoBlockMetadata, random_two_block_css_checks, random_two_block_spec_from_json_str}`.

- [ ] **Step 1: Write failing direct-constructor tests**

Create `qec-code/tests/random_two_block.rs` with direct API coverage first. The file should include these helpers and assertions:

```rust
use qec_code::codes::random_two_block::{
    RANDOM_TWO_BLOCK_ALGORITHM_V1, RandomTwoBlockSpec, random_two_block_css_checks,
};
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::verify_css_orthogonality;
use qec_code::finite_group::{FiniteGroupSpec, MAX_FINITE_GROUP_ORDER};
use qec_code::QecError;

fn s3_table() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 2, 3, 4, 5],
        vec![1, 2, 0, 4, 5, 3],
        vec![2, 0, 1, 5, 3, 4],
        vec![3, 5, 4, 0, 2, 1],
        vec![4, 3, 5, 1, 0, 2],
        vec![5, 4, 3, 2, 1, 0],
    ]
}

fn s3_group() -> FiniteGroupSpec {
    FiniteGroupSpec::new(6, 0, s3_table()).unwrap()
}

fn s3_spec() -> RandomTwoBlockSpec {
    RandomTwoBlockSpec::new(s3_group(), 2, 2, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1).unwrap()
}

fn expected_hx() -> Vec<Vec<usize>> {
    vec![
        vec![3, 5, 6, 10],
        vec![4, 5, 7, 11],
        vec![3, 4, 8, 9],
        vec![0, 2, 8, 9],
        vec![1, 2, 6, 10],
        vec![0, 1, 7, 11],
    ]
}

fn expected_hz() -> Vec<Vec<usize>> {
    vec![
        vec![0, 4, 9, 11],
        vec![1, 5, 10, 11],
        vec![2, 3, 9, 10],
        vec![2, 3, 6, 8],
        vec![0, 4, 7, 8],
        vec![1, 5, 6, 7],
    ]
}

fn css_code_from_sparse(n: usize, h_x: &[Vec<usize>], h_z: &[Vec<usize>]) -> CssCode {
    let hx = SparseRowsMatrix::new(n, h_x.to_vec()).unwrap().to_dense_rows();
    let hz = SparseRowsMatrix::new(n, h_z.to_vec()).unwrap().to_dense_rows();
    CssCode::from_hx_hz(hx, hz).unwrap()
}

#[test]
fn random_two_block_s3_seed7_matches_fixture() {
    let checks = random_two_block_css_checks(&s3_spec()).unwrap();

    assert_eq!(checks.num_cols, 12);
    assert_eq!(checks.support_a, vec![3, 5]);
    assert_eq!(checks.support_b, vec![0, 4]);
    assert_eq!(checks.h_x, expected_hx());
    assert_eq!(checks.h_z, expected_hz());
    assert_eq!(checks.metadata.seed, 7);
    assert_eq!(checks.metadata.support_a_weight, 2);
    assert_eq!(checks.metadata.support_b_weight, 2);
    assert_eq!(checks.metadata.algorithm_version, RANDOM_TWO_BLOCK_ALGORITHM_V1);
    assert!(checks.metadata.group_digest.starts_with("sha256:"));
    assert_eq!(checks.metadata.group_digest.len(), "sha256:".len() + 64);

    verify_css_orthogonality(checks.num_cols, &checks.h_x, &checks.h_z).unwrap();
    let css = css_code_from_sparse(checks.num_cols, &checks.h_x, &checks.h_z);
    assert_eq!(css.code().n(), 12);
    assert_eq!(css.code().num_logical_qubits(), 2);
    let distance = compute_distance(css.code()).unwrap();
    assert_eq!(distance.distance, 2);
}

#[test]
fn random_two_block_rejects_invalid_sampling_specs() {
    assert!(matches!(
        RandomTwoBlockSpec::new(s3_group(), 7, 2, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1),
        Err(QecError::InvalidRandomTwoBlockSpec {
            option: "support_a_weight",
            ..
        })
    ));
    assert!(matches!(
        RandomTwoBlockSpec::new(s3_group(), 2, 7, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1),
        Err(QecError::InvalidRandomTwoBlockSpec {
            option: "support_b_weight",
            ..
        })
    ));
    assert!(matches!(
        RandomTwoBlockSpec::new(s3_group(), 0, 2, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1),
        Err(QecError::InvalidRandomTwoBlockSpec {
            option: "support_a_weight",
            ..
        })
    ));
    assert!(matches!(
        RandomTwoBlockSpec::new(s3_group(), 2, 0, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1),
        Err(QecError::InvalidRandomTwoBlockSpec {
            option: "support_b_weight",
            ..
        })
    ));
    assert_eq!(
        RandomTwoBlockSpec::new(s3_group(), 2, 2, 7, 2),
        Err(QecError::UnsupportedRandomTwoBlockAlgorithm {
            algorithm_version: 2,
        })
    );
    assert_eq!(
        FiniteGroupSpec::new(MAX_FINITE_GROUP_ORDER + 1, 0, Vec::new()),
        Err(QecError::GroupOrderLimitExceeded {
            order: MAX_FINITE_GROUP_ORDER + 1,
            max_order: MAX_FINITE_GROUP_ORDER,
        })
    );
    assert!(matches!(
        FiniteGroupSpec::new(6, 0, vec![vec![0, 1, 2, 3, 4, 6]; 6]),
        Err(QecError::InvalidFiniteGroupTable { .. })
    ));
}
```

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test -p qec-code --test random_two_block random_two_block_s3_seed7_matches_fixture -- --exact
```

Expected: compile failure because `codes::random_two_block` and new `QecError` variants do not exist.

- [ ] **Step 3: Add typed errors and module export**

Modify `qec-code/src/error.rs` near the regular classical errors:

```rust
    #[error("invalid random two-block spec option {option}: {reason}")]
    InvalidRandomTwoBlockSpec {
        option: &'static str,
        reason: String,
    },
    #[error("unsupported random two-block algorithm version {algorithm_version}")]
    UnsupportedRandomTwoBlockAlgorithm { algorithm_version: u32 },
```

Modify `qec-code/src/codes/mod.rs`:

```rust
pub mod random_two_block;
```

- [ ] **Step 4: Implement core module**

Create `qec-code/src/codes/random_two_block.rs` with:

```rust
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::{QecError, Result};
use crate::family_contract::verify_css_orthogonality;
use crate::finite_group::{
    FiniteGroupSpec, GroupAlgebraElement, left_regular_lift, right_regular_lift,
};
use crate::regular_classical::{SplitMix64V1, bounded_index_v1};

pub const RANDOM_TWO_BLOCK_ALGORITHM_V1: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomTwoBlockSpec {
    pub group: FiniteGroupSpec,
    pub support_a_weight: usize,
    pub support_b_weight: usize,
    pub seed: u64,
    pub algorithm_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomTwoBlockCssChecks {
    pub num_cols: usize,
    pub h_x: Vec<Vec<usize>>,
    pub h_z: Vec<Vec<usize>>,
    pub support_a: Vec<usize>,
    pub support_b: Vec<usize>,
    pub metadata: RandomTwoBlockMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomTwoBlockMetadata {
    pub group_digest: String,
    pub seed: u64,
    pub support_a_weight: usize,
    pub support_b_weight: usize,
    pub algorithm_version: u32,
}
```

Add `RandomTwoBlockSpec::new`, `random_two_block_css_checks`, a private `sample_support_v1`, a private `verify_random_two_block_spec`, a compact `group_digest`, and `lower_hex`. `random_two_block_css_checks` must build `left_a`, `right_b`, `h_x = left_a.hconcat(&right_b)?`, `h_z = right_b.transpose()?.hconcat(&left_a.transpose()?)?`, call `verify_css_orthogonality`, and return cloned row vectors plus metadata.

- [ ] **Step 5: Run GREEN**

Run:

```bash
cargo test -p qec-code --test random_two_block random_two_block_s3_seed7_matches_fixture -- --exact
cargo test -p qec-code --test random_two_block random_two_block_rejects_invalid_sampling_specs -- --exact
```

Expected: both tests pass for the direct API.

- [ ] **Step 6: Commit Task 1**

Run:

```bash
git add qec-code/src/codes/random_two_block.rs qec-code/src/codes/mod.rs qec-code/src/error.rs qec-code/tests/random_two_block.rs
git commit -m "feat: add random two-block constructor"
```

---

### Task 2: Common Contract And CLI Integration

**Files:**
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/tests/random_two_block.rs`
- Modify: `qec-code/tests/family_contract.rs`

**Interfaces:**
- Consumes: Task 1's `RandomTwoBlockSpec`, `random_two_block_spec_from_json_str`, and `random_two_block_css_checks`.
- Produces: `CssFamilySpec::RandomTwoBlock`, JSON construction `"random_two_block"`, common-contract normalized metadata, and CLI coverage through existing `code css construct --spec` command.

- [ ] **Step 1: Expand tests to fail on common-contract gaps**

Extend `qec-code/tests/random_two_block.rs` imports:

```rust
use clap::Parser;
use qec_code::cli::Cli;
use qec_code::cli::run;
use qec_code::family_contract::{
    CssFamilySpec, RequestedFamilyId, construct_css, parse_css_construction_json,
};
use tempfile::tempdir;
```

Add helpers:

```rust
fn s3_request_json(seed_field: &str) -> String {
    format!(
        r#"{{"schema_version":1,"construction":"random_two_block","group":{{"name":"S3","element_order":"0=e,1=r,2=r^2,3=s,4=rs,5=r^2s","order":6,"identity":0,"multiplication_table":{}}},"support_a_weight":2,"support_b_weight":2,{seed_field},"algorithm_version":1}}"#,
        serde_json::to_string(&s3_table()).unwrap()
    )
}

fn run_qec_code_in_process(args: &[&str]) -> Result<String, QecError> {
    let mut argv = vec!["qec-code"];
    argv.extend(args);
    run(Cli::parse_from(argv))
}
```

Extend `random_two_block_s3_seed7_matches_fixture` after direct API assertions:

```rust
    let common = construct_css(CssFamilySpec::RandomTwoBlock(s3_spec()).into()).unwrap();
    assert_eq!(common.construction_id, "random_two_block");
    assert_eq!(common.requested_family_id, Some(RequestedFamilyId::RandomTwoBlock));
    assert_eq!(common.stats.n, 12);
    assert_eq!(common.stats.rank_x, 5);
    assert_eq!(common.stats.rank_z, 5);
    assert_eq!(common.stats.k, 2);
    assert_eq!(common.checks.h_x, expected_hx());
    assert_eq!(common.checks.h_z, expected_hz());
    assert_eq!(common.normalized_parameters["seed"], serde_json::json!(7));
    assert_eq!(common.normalized_parameters["support_a_weight"], serde_json::json!(2));
    assert_eq!(common.normalized_parameters["support_b_weight"], serde_json::json!(2));
    assert_eq!(
        common.normalized_parameters["algorithm_version"],
        serde_json::json!(RANDOM_TWO_BLOCK_ALGORITHM_V1)
    );
    assert_eq!(common.normalized_parameters["support_a"], serde_json::json!([3, 5]));
    assert_eq!(common.normalized_parameters["support_b"], serde_json::json!([0, 4]));
    assert_eq!(
        common.normalized_parameters["group_digest"],
        serde_json::json!(checks.metadata.group_digest)
    );

    let parsed = parse_css_construction_json(&s3_request_json(r#""seed":7"#)).unwrap();
    let parsed_common = construct_css(parsed).unwrap();
    assert_eq!(parsed_common.checks, common.checks);
    assert_eq!(parsed_common.normalized_parameters, common.normalized_parameters);

    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("random-two-block-s3.json");
    std::fs::write(&spec_path, s3_request_json(r#""seed":7"#)).unwrap();
    let cli_hx = run_qec_code_in_process(&[
        "code",
        "css",
        "construct",
        "--spec",
        spec_path.to_str().unwrap(),
        "hx",
    ])
    .unwrap();
    let cli_hz = run_qec_code_in_process(&[
        "code",
        "css",
        "construct",
        "--spec",
        spec_path.to_str().unwrap(),
        "hz",
    ])
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&cli_hx).unwrap()["rows"],
        serde_json::json!(expected_hx())
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&cli_hz).unwrap()["rows"],
        serde_json::json!(expected_hz())
    );
```

Extend `random_two_block_rejects_invalid_sampling_specs`:

```rust
    assert!(matches!(
        parse_css_construction_json(&s3_request_json("")),
        Err(QecError::InvalidRandomTwoBlockSpec { option: "seed", .. })
    ));
    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"random_two_block","group":{"order":257,"identity":0,"multiplication_table":[]},"support_a_weight":1,"support_b_weight":1,"seed":7,"algorithm_version":1}"#
        ),
        Err(QecError::GroupOrderLimitExceeded { .. })
    ));
```

Update `qec-code/tests/family_contract.rs` so `planned_families_have_no_callable_stub` expects:

```rust
&[
    RequestedFamilyId::Surface,
    RequestedFamilyId::QuantumTanner,
    RequestedFamilyId::RandomTwoBlock,
]
```

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test -p qec-code --test random_two_block random_two_block_s3_seed7_matches_fixture -- --exact
```

Expected: compile failure because `CssFamilySpec::RandomTwoBlock` and JSON parsing are not integrated yet.

- [ ] **Step 3: Implement common contract adapter**

Modify `qec-code/src/family_contract.rs`:

```rust
use crate::codes::random_two_block::{
    RandomTwoBlockSpec, random_two_block_css_checks, random_two_block_spec_from_json_str,
};
```

Add enum variant:

```rust
RandomTwoBlock(RandomTwoBlockSpec),
```

Include `RequestedFamilyId::RandomTwoBlock` in `callable_requested_family_ids`.

Add `construct_css` branch:

```rust
        CssConstructionSpec::Family(CssFamilySpec::RandomTwoBlock(spec)) => {
            let checks = random_two_block_css_checks(&spec)?;
            let parameters = random_two_block_normalized_parameters(&spec, &checks);
            construction_result(
                "random_two_block",
                Some(RequestedFamilyId::RandomTwoBlock),
                parameters,
                checks.num_cols,
                checks.h_x,
                checks.h_z,
                "random_two_block",
                "CssFamilySpec::RandomTwoBlock",
                None,
            )
        }
```

Add parse branch:

```rust
        "random_two_block" => Ok(CssFamilySpec::RandomTwoBlock(
            random_two_block_spec_from_json_str(input)?,
        )
        .into()),
```

Add private normalized-parameters helper:

```rust
fn random_two_block_normalized_parameters(
    spec: &RandomTwoBlockSpec,
    checks: &crate::codes::random_two_block::RandomTwoBlockCssChecks,
) -> BTreeMap<String, Value> {
    let mut group = BTreeMap::new();
    group.insert("order".to_owned(), Value::from(spec.group.order()));
    group.insert("identity".to_owned(), Value::from(spec.group.identity()));
    group.insert(
        "multiplication_table".to_owned(),
        serde_json::to_value(spec.group.multiplication_table())
            .expect("serializable random two-block group table"),
    );

    let mut parameters = BTreeMap::new();
    parameters.insert(
        "group".to_owned(),
        serde_json::to_value(group).expect("serializable random two-block group"),
    );
    parameters.insert(
        "group_digest".to_owned(),
        Value::from(checks.metadata.group_digest.clone()),
    );
    parameters.insert("seed".to_owned(), Value::from(checks.metadata.seed));
    parameters.insert(
        "support_a_weight".to_owned(),
        Value::from(checks.metadata.support_a_weight),
    );
    parameters.insert(
        "support_b_weight".to_owned(),
        Value::from(checks.metadata.support_b_weight),
    );
    parameters.insert(
        "algorithm_version".to_owned(),
        Value::from(checks.metadata.algorithm_version),
    );
    parameters.insert(
        "support_a".to_owned(),
        serde_json::to_value(&checks.support_a).expect("serializable support A"),
    );
    parameters.insert(
        "support_b".to_owned(),
        serde_json::to_value(&checks.support_b).expect("serializable support B"),
    );
    parameters
}
```

- [ ] **Step 4: Run GREEN**

Run:

```bash
cargo test -p qec-code --test random_two_block random_two_block_s3_seed7_matches_fixture -- --exact
cargo test -p qec-code --test random_two_block random_two_block_rejects_invalid_sampling_specs -- --exact
cargo test -p qec-code --test family_contract planned_families_have_no_callable_stub -- --exact
```

Expected: all pass.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
git add qec-code/src/family_contract.rs qec-code/tests/random_two_block.rs qec-code/tests/family_contract.rs
git commit -m "feat: route random two-block through css contract"
```

---

### Task 3: Verification And Cleanup

**Files:**
- Modify only files touched by Tasks 1 and 2 if verification finds issues.

**Interfaces:**
- Consumes: Completed Tasks 1 and 2.
- Produces: Rustfmt-clean implementation and full verification evidence for PR.

- [ ] **Step 1: Run focused required tests**

Run:

```bash
cargo test -p qec-code --test random_two_block random_two_block_s3_seed7_matches_fixture -- --exact
cargo test -p qec-code --test random_two_block random_two_block_rejects_invalid_sampling_specs -- --exact
```

Expected: both pass.

- [ ] **Step 2: Run crate and workspace tests**

Run:

```bash
cargo test -p qec-code
cargo test
```

Expected: both pass. If `cargo test` emits known `rmatching/tests/coverage.rs` warnings, record them as existing output noise only if the exit code is zero.

- [ ] **Step 3: Run formatting and diff checks**

Run:

```bash
cargo fmt --check
git diff --check origin/master..HEAD
```

Expected: both pass or only report pre-existing unrelated formatting drift. Fix any formatting introduced by this branch.

- [ ] **Step 4: Commit verification fixes if needed**

If Step 1-3 required changes, run:

```bash
git add qec-code/src/codes/random_two_block.rs qec-code/src/codes/mod.rs qec-code/src/error.rs qec-code/src/family_contract.rs qec-code/tests/random_two_block.rs qec-code/tests/family_contract.rs
git commit -m "fix: clean up random two-block verification"
```
