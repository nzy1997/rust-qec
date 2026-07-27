# Issue 556 Hypergraph Product CSS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the general hypergraph-product CSS constructor from two explicit classical binary parity-check matrices and expose normalized metadata through the Rust API and CLI.

**Architecture:** Reuse `CssConstructionSpec::HypergraphProduct` and `CssConstructionResult` as the public construction surface. Lower each `CssClassicalCheckSpec` to `sparse_gf2::SparseGf2Matrix`, compose HGP blocks with identity, transpose, Kronecker product, and horizontal concatenation, then hand canonical sparse rows to `construction_result` for orthogonality, ranks, and metadata.

**Tech Stack:** Rust 2024, Cargo workspace, `qec-code`, `serde_json`, `clap`, existing `sparse_gf2`, existing CSS distance helpers.

## Global Constraints

- Preserve existing `code css <CODE_ID> hx|hz`, `code css export`, `code css quantum-tanner`, and `code css construct --spec <path> hx|hz` behavior.
- Use `CssConstructionSpec::HypergraphProduct(HypergraphProductSpec)` as the Rust API for explicit classical matrices.
- Use `SparseGf2Matrix` primitives for identity, transpose, Kronecker product, and horizontal concatenation.
- Return `construction_id = "hypergraph_product"` and `requested_family_id = None`.
- Normalize `left` and `right` parameters as canonical classical sparse rows.
- Verify CSS orthogonality through the shared construction boundary.
- Keep generic HGP `stats.d_x` and `stats.d_z` as `None`; verify the issue fixture's exact distance with `compute_distance` in tests.
- Reject support `3` in a matrix declared with `num_cols = 3` as `QecError::SparseGf2SupportOutOfRange` before product construction.
- Do not add dependencies.

---

## File Structure

- Create `qec-code/tests/hypergraph_product.rs`: issue fixture, negative control, Rust API assertions, CLI `hx`/`hz`/`metadata` assertions, and exact distance verification.
- Modify `qec-code/src/family_contract.rs`: lower classical inputs to `SparseGf2Matrix`, compose HGP blocks through sparse GF(2), check final H_X/H_Z widths, and normalize metadata from canonical matrices.
- Modify `qec-code/src/cli.rs`: add a `metadata` output selector for `code css construct --spec <path> metadata` while keeping existing `hx`/`hz` matrix selectors.

## Task 1: HGP Constructor And CLI Metadata

**Files:**
- Create: `qec-code/tests/hypergraph_product.rs`
- Modify: `qec-code/src/family_contract.rs`
- Modify: `qec-code/src/cli.rs`

**Interfaces:**
- Consumes: `CssClassicalCheckSpec { num_cols, rows }`, `HypergraphProductSpec { left, right }`, `SparseGf2Matrix::{new, identity, transpose, kron, hconcat}`, `construct_css`, `parse_css_construction_json`, `verify_css_orthogonality`, `CssCode`, `compute_distance`, and CLI `run`.
- Produces: completed `construct_css(CssConstructionSpec::HypergraphProduct(...)) -> Result<CssConstructionResult>`, typed bounds errors from sparse GF(2), and `code css construct --spec <path> metadata`.

- [ ] **Step 1: Write the failing HGP fixture and negative tests**

Create `qec-code/tests/hypergraph_product.rs` with:

```rust
use std::path::PathBuf;

use qec_code::QecError;
use qec_code::cli::{run, Cli, CodeCommands, Commands, CssArgs, CssCommands, CssConstructionOutput};
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, verify_css_orthogonality, CssClassicalCheckSpec,
    CssConstructionSpec, HypergraphProductSpec,
};
use tempfile::tempdir;

fn classical_2x3() -> CssClassicalCheckSpec {
    CssClassicalCheckSpec {
        num_cols: 3,
        rows: vec![vec![0, 1], vec![1, 2]],
    }
}

fn fixture_spec() -> CssConstructionSpec {
    CssConstructionSpec::HypergraphProduct(HypergraphProductSpec {
        left: classical_2x3(),
        right: classical_2x3(),
    })
}

fn fixture_json() -> &'static str {
    r#"{"schema_version":1,"construction":"hypergraph_product","left":{"num_cols":3,"rows":[[0,1],[1,2]]},"right":{"num_cols":3,"rows":[[0,1],[1,2]]}}"#
}

fn expected_hx() -> Vec<Vec<usize>> {
    vec![
        vec![0, 3, 9],
        vec![1, 4, 9, 10],
        vec![2, 5, 10],
        vec![3, 6, 11],
        vec![4, 7, 11, 12],
        vec![5, 8, 12],
    ]
}

fn expected_hz() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 9],
        vec![1, 2, 10],
        vec![3, 4, 9, 11],
        vec![4, 5, 10, 12],
        vec![6, 7, 11],
        vec![7, 8, 12],
    ]
}

fn css_code(num_cols: usize, h_x: &[Vec<usize>], h_z: &[Vec<usize>]) -> CssCode {
    CssCode::from_hx_hz(
        SparseRowsMatrix::new(num_cols, h_x.to_vec()).unwrap().to_dense_rows(),
        SparseRowsMatrix::new(num_cols, h_z.to_vec()).unwrap().to_dense_rows(),
    )
    .unwrap()
}

fn construct_cli_output(spec_path: PathBuf, output: CssConstructionOutput) -> String {
    run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs {
                command: Some(CssCommands::Construct {
                    spec: spec_path,
                    output,
                }),
                code_id: None,
                matrix: None,
            }),
        },
    })
    .unwrap()
}

#[test]
fn hypergraph_product_matches_2x3_fixture() {
    let result = construct_css(fixture_spec()).unwrap();

    assert_eq!(result.construction_id, "hypergraph_product");
    assert_eq!(result.requested_family_id, None);
    assert_eq!(result.normalized_parameters["left"]["num_cols"], serde_json::json!(3));
    assert_eq!(
        result.normalized_parameters["left"]["rows"],
        serde_json::json!([[0, 1], [1, 2]])
    );
    assert_eq!(result.normalized_parameters["right"]["num_cols"], serde_json::json!(3));
    assert_eq!(
        result.normalized_parameters["right"]["rows"],
        serde_json::json!([[0, 1], [1, 2]])
    );
    assert_eq!(result.stats.n, 13);
    assert_eq!(result.stats.m_x, 6);
    assert_eq!(result.stats.m_z, 6);
    assert_eq!(result.stats.rank_x, 6);
    assert_eq!(result.stats.rank_z, 6);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, None);
    assert_eq!(result.stats.d_z, None);
    assert_eq!(result.checks.h_x, expected_hx());
    assert_eq!(result.checks.h_z, expected_hz());
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let distance = compute_distance(css_code(result.stats.n, &result.checks.h_x, &result.checks.h_z).code()).unwrap();
    assert_eq!(distance.distance, 3);
    assert_eq!(distance.witness.weight(), 3);

    let parsed = parse_css_construction_json(fixture_json()).unwrap();
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(parsed_result.checks, result.checks);
    assert_eq!(
        serde_json::to_string(&parsed_result.normalized_parameters).unwrap(),
        serde_json::to_string(&result.normalized_parameters).unwrap()
    );

    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("hgp.json");
    std::fs::write(&spec_path, fixture_json()).unwrap();

    let hx_json: serde_json::Value =
        serde_json::from_str(&construct_cli_output(spec_path.clone(), CssConstructionOutput::Hx)).unwrap();
    let hz_json: serde_json::Value =
        serde_json::from_str(&construct_cli_output(spec_path.clone(), CssConstructionOutput::Hz)).unwrap();
    let metadata: serde_json::Value =
        serde_json::from_str(&construct_cli_output(spec_path, CssConstructionOutput::Metadata)).unwrap();

    assert_eq!(hx_json["format"], "sparse_rows");
    assert_eq!(hx_json["num_cols"], 13);
    assert_eq!(hx_json["rows"], serde_json::json!(expected_hx()));
    assert_eq!(hz_json["format"], "sparse_rows");
    assert_eq!(hz_json["num_cols"], 13);
    assert_eq!(hz_json["rows"], serde_json::json!(expected_hz()));
    assert_eq!(metadata["construction_id"], "hypergraph_product");
    assert_eq!(metadata["requested_family_id"], serde_json::Value::Null);
    assert_eq!(metadata["stats"]["n"], 13);
    assert_eq!(metadata["stats"]["m_x"], 6);
    assert_eq!(metadata["stats"]["m_z"], 6);
    assert_eq!(metadata["stats"]["rank_x"], 6);
    assert_eq!(metadata["stats"]["rank_z"], 6);
    assert_eq!(metadata["stats"]["k"], 1);
    assert_eq!(metadata["checks"]["h_x"], serde_json::json!(expected_hx()));
    assert_eq!(metadata["checks"]["h_z"], serde_json::json!(expected_hz()));
}

#[test]
fn hypergraph_product_rejects_out_of_range_input() {
    let err = construct_css(CssConstructionSpec::HypergraphProduct(
        HypergraphProductSpec {
            left: CssClassicalCheckSpec {
                num_cols: 3,
                rows: vec![vec![0, 3]],
            },
            right: classical_2x3(),
        },
    ))
    .unwrap_err();

    assert_eq!(
        err,
        QecError::SparseGf2SupportOutOfRange {
            row: 0,
            support: 3,
            num_cols: 3,
        }
    );
}
```

- [ ] **Step 2: Run the fixture test to verify RED**

Run:

```bash
cargo test -p qec-code --test hypergraph_product hypergraph_product_matches_2x3_fixture -- --exact
```

Expected: FAIL before production changes because `CssConstructionOutput` does not exist and the CLI cannot emit metadata.

- [ ] **Step 3: Implement sparse GF(2) HGP construction**

In `qec-code/src/family_contract.rs`, add the import:

```rust
use crate::sparse_gf2::SparseGf2Matrix;
```

Replace `construct_hypergraph_product` with:

```rust
fn construct_hypergraph_product(spec: HypergraphProductSpec) -> Result<CssConstructionResult> {
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

    if h_x.num_cols() != h_z.num_cols() {
        return Err(QecError::InvalidCssConstruction {
            construction: "hypergraph_product".to_owned(),
            reason: format!(
                "H_X width {} does not match H_Z width {}",
                h_x.num_cols(),
                h_z.num_cols()
            ),
        });
    }

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

    construction_result(
        "hypergraph_product",
        None,
        parameters,
        h_x.num_cols(),
        h_x.rows().to_vec(),
        h_z.rows().to_vec(),
        "hypergraph_product",
        "CssConstructionSpec::HypergraphProduct",
        None,
    )
}

fn classical_check_matrix(spec: CssClassicalCheckSpec) -> Result<SparseGf2Matrix> {
    SparseGf2Matrix::new(spec.rows.len(), spec.num_cols, spec.rows)
}
```

- [ ] **Step 4: Add CLI metadata output selector**

In `qec-code/src/cli.rs`, add a new value enum near `CssMatrixKind`:

```rust
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CssConstructionOutput {
    Hx,
    Hz,
    Metadata,
}
```

Change the `CssCommands::Construct` field from `matrix: CssMatrixKind` to:

```rust
    Construct {
        #[arg(long)]
        spec: PathBuf,
        output: CssConstructionOutput,
    },
```

Update `run_css_args`, `run_css_construction_spec`, and construction export helpers to:

```rust
fn run_css_args(args: CssArgs) -> Result<String, QecError> {
    match args.command {
        Some(CssCommands::List) => Ok(run_css_list()),
        Some(CssCommands::Export { code_id, matrix }) => run_css(&code_id, matrix),
        Some(CssCommands::Construct { spec, output }) => {
            run_css_construction_spec(&spec, output)
        }
        Some(CssCommands::QuantumTanner { spec, matrix }) => run_css_quantum_tanner(&spec, matrix),
        None => {
            let code_id = args
                .code_id
                .expect("clap requires CODE_ID when no css subcommand is used");
            let matrix = args
                .matrix
                .expect("clap requires MATRIX when no css subcommand is used");

            run_css(&code_id, matrix)
        }
    }
}

fn run_css_construction_spec(
    path: &PathBuf,
    output: CssConstructionOutput,
) -> Result<String, QecError> {
    let input = read_css_spec_file(path)?;
    let spec = parse_css_construction_json(&input)?;
    export_css_construction_output(spec, output)
}

fn export_css_construction(
    spec: CssConstructionSpec,
    matrix: CssMatrixKind,
) -> Result<String, QecError> {
    export_css_construction_matrix(construct_css(spec)?, matrix)
}

fn export_css_construction_output(
    spec: CssConstructionSpec,
    output: CssConstructionOutput,
) -> Result<String, QecError> {
    let construction = construct_css(spec)?;
    match output {
        CssConstructionOutput::Hx => export_css_construction_matrix(construction, CssMatrixKind::Hx),
        CssConstructionOutput::Hz => export_css_construction_matrix(construction, CssMatrixKind::Hz),
        CssConstructionOutput::Metadata => Ok(serde_json::to_string(&construction)
            .expect("validated CSS construction result should always serialize")),
    }
}

fn export_css_construction_matrix(
    construction: crate::family_contract::CssConstructionResult,
    matrix: CssMatrixKind,
) -> Result<String, QecError> {
    let rows = match matrix {
        CssMatrixKind::Hx => construction.checks.h_x,
        CssMatrixKind::Hz => construction.checks.h_z,
    };

    let matrix = SparseRowsMatrix::new(construction.stats.n, rows)?;
    Ok(matrix.to_json_string())
}
```

Preserve `run_css_quantum_tanner` with the `CssMatrixKind` parameter so its CLI remains `hx|hz` only.

- [ ] **Step 5: Run the fixture test to verify GREEN**

Run:

```bash
cargo test -p qec-code --test hypergraph_product hypergraph_product_matches_2x3_fixture -- --exact
```

Expected: PASS.

- [ ] **Step 6: Run the negative control**

Run:

```bash
cargo test -p qec-code --test hypergraph_product hypergraph_product_rejects_out_of_range_input -- --exact
```

Expected: PASS.

- [ ] **Step 7: Run focused contract and CLI regression tests**

Run:

```bash
cargo test -p qec-code --test family_contract
cargo test -p qec-code --test cli run_code_css_construct_json_surface_rotated_d3_matches_inline_fixture -- --exact
cargo test -p qec-code --test cli run_code_css_construct_json_rejects_unknown_schema -- --exact
```

Expected: PASS.

- [ ] **Step 8: Commit the implementation**

Run:

```bash
git add qec-code/tests/hypergraph_product.rs qec-code/src/family_contract.rs qec-code/src/cli.rs docs/superpowers/plans/2026-07-27-issue-556-hypergraph-product-css.md
git commit -m "feat: construct hypergraph product css checks"
```
