# Issue 141 APM P=96 rsinter Fixtures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add rsinter P=96 APM-CSS Hx/Hz fixtures and a smoke test proving they match qec-code export and define a `[[1152,580]]` CSS code.

**Architecture:** Keep the fixture copy under `rsinter/tests/fixtures/css`, matching the existing BB72 fixture surface. Add a focused rsinter integration test that calls the public qec-code CLI library path in-process, compares exact CLI stdout-style JSON, parses the committed fixtures, computes rank-derived logical count, verifies orthogonality, and checks a corrupted in-memory Hz negative control.

**Tech Stack:** Rust 2024, Cargo workspace integration tests, `qec-code` dev-dependency for rsinter tests, `qec_code::cli::run`, `qec_code::css::sparse_rows_matrix_from_json_str`, `qec_code::binary::try_binary_rank`.

## Global Constraints

- Add committed `rsinter/tests/fixtures/css/apm_p96_hx.json` and `rsinter/tests/fixtures/css/apm_p96_hz.json`.
- The fixture source of truth is `qec-code code css apm_kasai:p=96 hx|hz`.
- The smoke test must be named `apm_p96_css_fixture_has_580_logicals`.
- The smoke test must prove the rsinter fixture pair exactly matches the current qec-code export or an equivalent deterministic normalized export comparison.
- The smoke test must compute `rank_x + rank_z`.
- The smoke test must assert `k = 1152 - rank_x - rank_z = 580`.
- The smoke test must assert CSS orthogonality is true.
- The smoke test must include an in-memory corrupted `Hz` fixture and assert the exact-export comparison or CSS orthogonality check fails.
- Record exact regeneration commands near the fixture test or in a small fixture README.
- Do not run stochastic BP benchmarks.
- Do not add explicit APM logical observable fixture selection in this issue.
- Run `cargo test -p rsinter apm_p96_css_fixture_has_580_logicals -q`.
- Run `cargo test`.

---

## File Structure

- Modify: `rsinter/Cargo.toml` to add `qec-code` as a dev-dependency only.
- Create: `rsinter/tests/fixtures/css/apm_p96_hx.json` generated from qec-code export.
- Create: `rsinter/tests/fixtures/css/apm_p96_hz.json` generated from qec-code export.
- Create: `rsinter/tests/fixtures/css/README.md` with exact regeneration commands.
- Create: `rsinter/tests/apm_p96_css_fixture.rs` with the fixture/export/rank/orthogonality smoke test.

### Task 1: APM P=96 rsinter Fixture Contract

**Files:**
- Modify: `rsinter/Cargo.toml`
- Create: `rsinter/tests/fixtures/css/apm_p96_hx.json`
- Create: `rsinter/tests/fixtures/css/apm_p96_hz.json`
- Create: `rsinter/tests/fixtures/css/README.md`
- Create: `rsinter/tests/apm_p96_css_fixture.rs`

**Interfaces:**
- Consumes: `qec_code::cli::run(Cli { command: Commands::Code { command: CodeCommands::Css(CssArgs::export("apm_kasai:p=96", matrix)) } }) -> Result<String, QecError>`.
- Consumes: `qec_code::css::sparse_rows_matrix_from_json_str(&str) -> Result<SparseRowsMatrix, QecError>`.
- Consumes: `qec_code::binary::try_binary_rank(&[Vec<u8>]) -> Result<usize, QecError>`.
- Produces: `rsinter/tests/fixtures/css/apm_p96_hx.json` and `apm_p96_hz.json` as CLI stdout-style JSON, including the trailing newline.
- Produces: test `apm_p96_css_fixture_has_580_logicals`.

- [ ] **Step 1: Write the failing rsinter smoke test**

Add `qec-code` as an rsinter dev-dependency in `rsinter/Cargo.toml`:

```toml
[dev-dependencies]
qec-code = { path = "../qec-code" }
tempfile = "3"
```

Create `rsinter/tests/apm_p96_css_fixture.rs`:

```rust
use qec_code::binary::try_binary_rank;
use qec_code::cli::{Cli, CodeCommands, Commands, CssArgs, CssMatrixKind, run};
use qec_code::css::{SparseRowsMatrix, sparse_rows_matrix_from_json_str};

const APM_P96_CODE_ID: &str = "apm_kasai:p=96";
const APM_P96_NUM_QUBITS: usize = 1152;
const APM_P96_LOGICALS: usize = 580;
const APM_P96_HX_JSON: &str = include_str!("fixtures/css/apm_p96_hx.json");
const APM_P96_HZ_JSON: &str = include_str!("fixtures/css/apm_p96_hz.json");

#[test]
fn apm_p96_css_fixture_has_580_logicals() {
    let hx_export = qec_code_css_stdout(CssMatrixKind::Hx);
    let hz_export = qec_code_css_stdout(CssMatrixKind::Hz);
    assert_eq!(APM_P96_HX_JSON, hx_export);
    assert_eq!(APM_P96_HZ_JSON, hz_export);

    let hx = parse_sparse_rows(APM_P96_HX_JSON);
    let hz = parse_sparse_rows(APM_P96_HZ_JSON);
    assert_eq!(hx.num_cols(), APM_P96_NUM_QUBITS);
    assert_eq!(hz.num_cols(), APM_P96_NUM_QUBITS);

    let hx_dense = hx.to_dense_rows();
    let hz_dense = hz.to_dense_rows();
    let rank_x = try_binary_rank(&hx_dense).unwrap();
    let rank_z = try_binary_rank(&hz_dense).unwrap();
    let rank_sum = rank_x + rank_z;
    let logicals = APM_P96_NUM_QUBITS
        .checked_sub(rank_sum)
        .expect("rank_x + rank_z must not exceed n");
    assert_eq!(logicals, APM_P96_LOGICALS);
    assert!(sparse_rows_are_orthogonal(&hx, &hz));

    let mut corrupted_hz_rows = hz.rows().to_vec();
    assert_eq!(corrupted_hz_rows[0][0], 69);
    corrupted_hz_rows[0][0] = 0;
    let corrupted_hz = SparseRowsMatrix::new(hz.num_cols(), corrupted_hz_rows).unwrap();

    assert_ne!(
        format!("{}\n", corrupted_hz.to_json_string()),
        hz_export,
        "changed Hz support should no longer match the qec-code export"
    );
    assert!(
        !sparse_rows_are_orthogonal(&hx, &corrupted_hz),
        "changed Hz support should also break CSS orthogonality"
    );
}

fn qec_code_css_stdout(matrix: CssMatrixKind) -> String {
    let output = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(APM_P96_CODE_ID.to_owned(), matrix)),
        },
    })
    .unwrap();
    format!("{output}\n")
}

fn parse_sparse_rows(input: &str) -> SparseRowsMatrix {
    sparse_rows_matrix_from_json_str(input).unwrap()
}

fn sparse_rows_are_orthogonal(hx: &SparseRowsMatrix, hz: &SparseRowsMatrix) -> bool {
    for x_row in hx.rows() {
        for z_row in hz.rows() {
            let overlap = x_row
                .iter()
                .filter(|x_col| z_row.contains(x_col))
                .count();
            if overlap % 2 != 0 {
                return false;
            }
        }
    }
    true
}
```

- [ ] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p rsinter apm_p96_css_fixture_has_580_logicals -q
```

Expected: FAIL because `rsinter/tests/fixtures/css/apm_p96_hx.json` and `apm_p96_hz.json` do not exist yet.

- [ ] **Step 3: Add fixture provenance README**

Create `rsinter/tests/fixtures/css/README.md`:

````markdown
# CSS Fixtures

APM P=96 fixtures are generated from the qec-code built-in CSS export:

```sh
cargo run -p qec-code -- code css apm_kasai:p=96 hx > rsinter/tests/fixtures/css/apm_p96_hx.json
cargo run -p qec-code -- code css apm_kasai:p=96 hz > rsinter/tests/fixtures/css/apm_p96_hz.json
```
````

- [ ] **Step 4: Generate the APM P=96 fixtures**

Run these commands from the repository root:

```sh
cargo run -p qec-code -- code css apm_kasai:p=96 hx > rsinter/tests/fixtures/css/apm_p96_hx.json
cargo run -p qec-code -- code css apm_kasai:p=96 hz > rsinter/tests/fixtures/css/apm_p96_hz.json
```

Expected: both files are created and contain sparse-row JSON with `num_cols = 1152`.

- [ ] **Step 5: Run focused test to verify GREEN**

Run:

```sh
cargo test -p rsinter apm_p96_css_fixture_has_580_logicals -q
```

Expected: PASS.

- [ ] **Step 6: Run full workspace verification**

Run:

```sh
cargo test
```

Expected: PASS. Existing warnings from unrelated crates may still print, but the command must exit 0.

- [ ] **Step 7: Commit**

```sh
git add rsinter/Cargo.toml \
  rsinter/tests/apm_p96_css_fixture.rs \
  rsinter/tests/fixtures/css/README.md \
  rsinter/tests/fixtures/css/apm_p96_hx.json \
  rsinter/tests/fixtures/css/apm_p96_hz.json
git commit -m "test: add apm p96 rsinter css fixtures"
```
