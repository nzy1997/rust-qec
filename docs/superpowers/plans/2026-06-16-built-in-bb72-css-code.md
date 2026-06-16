# Built-In BB72 CSS Code Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the fixed built-in `bb72` CSS parity-check source requested by GitHub issue #59.

**Architecture:** Keep `bb72` in the existing `qec-code` built-in CSS registry beside `steane`. Implement the bivariate-bicycle matrix construction as private helpers in `qec-code/src/codes/built_in_css.rs`; expose only `built_in_css_checks("bb72")` and the existing CLI path.

**Tech Stack:** Rust 2024, existing `qec-code` crate, `cargo test`, existing `serde_json` dependency for CLI JSON smoke assertions.

---

## File Structure

- Modify: `qec-code/tests/code.rs`
  - Add dense-conversion and row-range helpers for `bb72` validation.
  - Add issue #59 library tests for shape, row weight, canonical sparse supports, CSS orthogonality, and parameter rejection.
- Modify: `qec-code/src/codes/built_in_css.rs`
  - Parse `bb72` as a fixed built-in id.
  - Add private fixed-term bivariate-bicycle helpers.
  - Dispatch `built_in_css_checks("bb72")`.
- Modify: `qec-code/src/css.rs`
  - Let `CssCode::from_hx_hz(...)` accept redundant CSS parity-check rows by
    selecting an independent stabilizer basis after orthogonality validation.
- Modify: `qec-code/tests/cli.rs`
  - Add a low-cost CLI smoke test proving the existing `qec-code code css bb72 hx` path emits sparse-row JSON.

No `rsinter` files change in this issue. The existing `rsinter/tests/css_surface_special.rs` helper remains in place until a separate cleanup chooses to deduplicate it.

## Task 1: Add Failing Library Tests

**Files:**
- Modify: `qec-code/tests/code.rs:9-241`

- [ ] **Step 1: Add test helpers after `assert_strictly_increasing_rows`**

Insert this block after the existing `assert_strictly_increasing_rows` helper:

```rust
fn assert_rows_in_range(rows: &[Vec<usize>], num_cols: usize) {
    for row in rows {
        for &col in row {
            assert!(
                col < num_cols,
                "row contains out-of-range column {col} for width {num_cols}: {row:?}"
            );
        }
    }
}

fn dense_rows(rows: &[Vec<usize>], width: usize) -> Vec<Vec<u8>> {
    rows.iter().map(|row| dense_row(row, width)).collect()
}

fn dense_row(row: &[usize], width: usize) -> Vec<u8> {
    let mut dense = vec![0; width];
    for &col in row {
        dense[col] = 1;
    }
    dense
}
```

- [ ] **Step 2: Add the `bb72` shape and orthogonality test**

Insert this test after `built_in_css_registry_exposes_steane_checks`:

```rust
#[test]
fn bb72_has_expected_shape_and_css_orthogonality() {
    let checks = built_in_css_checks("bb72").unwrap();

    assert_eq!(checks.code_id, "bb72");
    assert_eq!(checks.num_cols, 72);
    assert_eq!(checks.hx.len(), 36);
    assert_eq!(checks.hz.len(), 36);

    for row in checks.hx.iter().chain(checks.hz.iter()) {
        assert_eq!(row.len(), 6, "row has wrong weight: {row:?}");
    }

    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
    assert_rows_in_range(&checks.hx, checks.num_cols);
    assert_rows_in_range(&checks.hz, checks.num_cols);

    CssCode::from_hx_hz(
        dense_rows(&checks.hx, checks.num_cols),
        dense_rows(&checks.hz, checks.num_cols),
    )
    .unwrap();
}
```

- [ ] **Step 3: Add the fixed-id parser rejection test**

Insert this test after `built_in_css_code_spec_parses_fixed_and_parameterized_ids`:

```rust
#[test]
fn bb72_code_spec_rejects_unexpected_parameters() {
    assert_eq!(
        parse_built_in_css_code_spec("bb72"),
        Ok(BuiltInCssCodeSpec::Fixed { code_id: "bb72" })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb72:d=3"),
        Err(QecError::UnknownBuiltInCssFamily {
            family: "bb72".to_owned(),
        })
    );
}
```

- [ ] **Step 4: Run the focused library tests and confirm they fail**

Run:

```bash
cargo test -p qec-code --test code bb72
```

Expected: FAIL. At least `bb72_has_expected_shape_and_css_orthogonality` should panic because `built_in_css_checks("bb72")` returns `UnknownBuiltInCssCode`. `bb72_code_spec_rejects_unexpected_parameters` should also fail because `parse_built_in_css_code_spec("bb72")` does not yet parse as a fixed id.

## Task 2: Implement Fixed `bb72` Registry Support

**Files:**
- Modify: `qec-code/src/codes/built_in_css.rs:33-157`
- Modify: `qec-code/src/css.rs:1-60`
- Test: `qec-code/tests/code.rs`

- [ ] **Step 1: Add a regression test for redundant CSS parity checks**

Insert this test after `css_code_rejects_non_orthogonal_checks` in `qec-code/tests/code.rs`:

```rust
#[test]
fn css_code_accepts_redundant_orthogonal_checks() {
    let code = CssCode::from_hx_hz(vec![vec![1, 0], vec![0, 1], vec![1, 1]], vec![])
        .unwrap();

    assert_eq!(code.code().n(), 2);
    assert_eq!(code.code().stabilizer_rank(), 2);
    assert_eq!(code.code().stabilizers().len(), 2);
    assert_eq!(code.code().num_logical_qubits(), 0);
}
```

- [ ] **Step 2: Run the redundant-check regression test and confirm it fails**

Run:

```bash
cargo test -p qec-code --test code css_code_accepts_redundant_orthogonal_checks
```

Expected: FAIL with `DependentStabilizers`. This confirms the existing CSS
constructor overconstrains redundant parity-check matrices.

- [ ] **Step 3: Teach `CssCode::from_hx_hz` to select independent stabilizer rows**

In `qec-code/src/css.rs`, add the private GF(2) import near the top:

```rust
use crate::gf2;
```

Then replace the stabilizer construction inside `CssCode::from_hx_hz(...)`.
Change this block:

```rust
        let mut stabilizers = Vec::with_capacity(hx.len() + hz.len());
        for row in hx {
            stabilizers.push(Pauli::from_xz_bits(row, vec![0; n])?);
        }
        for row in hz {
            stabilizers.push(Pauli::from_xz_bits(vec![0; n], row)?);
        }

        Ok(Self {
            code: StabilizerCode::from_stabilizers(n, stabilizers)?,
        })
```

to:

```rust
        let mut stabilizer_rows = Vec::with_capacity(hx.len() + hz.len());
        for row in hx {
            let mut symplectic_row = row;
            symplectic_row.extend(vec![0; n]);
            stabilizer_rows.push(symplectic_row);
        }
        for row in hz {
            let mut symplectic_row = vec![0; n];
            symplectic_row.extend(row);
            stabilizer_rows.push(symplectic_row);
        }

        let stabilizers = gf2::try_select_independent_rows(&stabilizer_rows)?
            .into_iter()
            .map(Pauli::from_symplectic_row)
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            code: StabilizerCode::from_stabilizers(n, stabilizers)?,
        })
```

This keeps `StabilizerCode::from_stabilizers(...)` strict for explicit
stabilizer lists while allowing `CssCode` to accept raw redundant parity-check
matrices.

- [ ] **Step 4: Run the redundant-check regression test and confirm it passes**

Run:

```bash
cargo test -p qec-code --test code css_code_accepts_redundant_orthogonal_checks
```

Expected: PASS.

- [ ] **Step 5: Add `bb72` as an accepted fixed id**

Change the bare-id match in `parse_built_in_css_code_spec` from:

```rust
    match input {
        "steane" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "steane" }),
        "repetition_x" | "repetition_z" => Err(QecError::MissingBuiltInCssParameter {
            family: input.to_owned(),
            parameter: "d".to_owned(),
        }),
        _ => Err(QecError::UnknownBuiltInCssCode {
            code_id: input.to_owned(),
        }),
    }
```

to:

```rust
    match input {
        "steane" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "steane" }),
        "bb72" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "bb72" }),
        "repetition_x" | "repetition_z" => Err(QecError::MissingBuiltInCssParameter {
            family: input.to_owned(),
            parameter: "d".to_owned(),
        }),
        _ => Err(QecError::UnknownBuiltInCssCode {
            code_id: input.to_owned(),
        }),
    }
```

- [ ] **Step 6: Add fixed `bb72` constants and private helpers**

Insert this block after `STEANE_ROW_SUPPORTS` and before `built_in_css_checks`:

```rust
const BB72_LX: usize = 6;
const BB72_LY: usize = 6;
const BB72_A_TERMS: &[(usize, usize)] = &[(3, 0), (0, 1), (0, 2)];
const BB72_B_TERMS: &[(usize, usize)] = &[(0, 3), (1, 0), (2, 0)];

fn bb72_checks() -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    bivariate_bicycle_checks(BB72_LX, BB72_LY, BB72_A_TERMS, BB72_B_TERMS)
}

fn bivariate_bicycle_checks(
    lx: usize,
    ly: usize,
    a_terms: &[(usize, usize)],
    b_terms: &[(usize, usize)],
) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let block = lx * ly;
    let index = |x: usize, y: usize| -> usize { (x % lx) * ly + (y % ly) };
    let mut hx = Vec::with_capacity(block);
    let mut hz = Vec::with_capacity(block);

    for x in 0..lx {
        for y in 0..ly {
            let mut x_row = Vec::new();
            for &(dx, dy) in a_terms {
                x_row.push(index(x + dx, y + dy));
            }
            for &(dx, dy) in b_terms {
                x_row.push(block + index(x + dx, y + dy));
            }
            x_row.sort_unstable();
            hx.push(x_row);

            let mut z_row = Vec::new();
            for &(dx, dy) in b_terms {
                z_row.push(index((x + lx - dx % lx) % lx, (y + ly - dy % ly) % ly));
            }
            for &(dx, dy) in a_terms {
                z_row.push(block + index((x + lx - dx % lx) % lx, (y + ly - dy % ly) % ly));
            }
            z_row.sort_unstable();
            hz.push(z_row);
        }
    }

    (hx, hz)
}
```

- [ ] **Step 7: Dispatch `built_in_css_checks("bb72")`**

Add this match branch after the existing `"steane"` branch and before the `_` error branch:

```rust
        "bb72" => {
            let (hx, hz) = bb72_checks();

            Ok(BuiltInCssChecks {
                code_id: "bb72",
                num_cols: 2 * BB72_LX * BB72_LY,
                hx,
                hz,
            })
        }
```

- [ ] **Step 8: Run the focused library tests and confirm they pass**

Run:

```bash
cargo test -p qec-code --test code bb72
```

Expected: PASS. The output should include both `bb72_has_expected_shape_and_css_orthogonality` and `bb72_code_spec_rejects_unexpected_parameters` as passing tests.

- [ ] **Step 9: Commit the library implementation**

Run:

```bash
git add qec-code/tests/code.rs qec-code/src/css.rs qec-code/src/codes/built_in_css.rs
git commit -m "feat: add built-in bb72 css checks"
```

Expected: commit succeeds and includes only the library tests and registry implementation.

## Task 3: Add CLI Smoke Coverage

**Files:**
- Modify: `qec-code/tests/cli.rs:119-179`
- Test: `qec-code/tests/cli.rs`

- [ ] **Step 1: Add a CLI smoke test for `bb72` sparse-row export**

Insert this test after `code_css_unknown_id_fails`:

```rust
#[test]
fn code_css_bb72_hx_prints_sparse_rows_json() {
    let output = run_qec_code(&["code", "css", "bb72", "hx"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be sparse-row JSON");
    let rows = json["rows"]
        .as_array()
        .expect("sparse-row JSON should contain rows");

    assert_eq!(json["format"], "sparse_rows");
    assert_eq!(json["num_cols"], 72);
    assert_eq!(rows.len(), 36);
    assert!(
        rows.iter()
            .all(|row| row.as_array().is_some_and(|cols| cols.len() == 6)),
        "all bb72 hx rows should have weight 6: {rows:?}"
    );
}
```

- [ ] **Step 2: Run the CLI smoke test**

Run:

```bash
cargo test -p qec-code --test cli code_css_bb72_hx_prints_sparse_rows_json
```

Expected: PASS.

- [ ] **Step 3: Commit the CLI test**

Run:

```bash
git add qec-code/tests/cli.rs
git commit -m "test: cover bb72 css cli export"
```

Expected: commit succeeds and includes only the CLI smoke test.

## Task 4: Final Verification

**Files:**
- Verify: `qec-code/src/codes/built_in_css.rs`
- Verify: `qec-code/tests/code.rs`
- Verify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Run the issue #59 focused test filter**

Run:

```bash
cargo test -p qec-code --test code bb72
```

Expected: PASS. This covers the issue-requested tests:

- `bb72_has_expected_shape_and_css_orthogonality`
- `bb72_code_spec_rejects_unexpected_parameters`

- [ ] **Step 2: Run the full `qec-code` test suite**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS.

- [ ] **Step 3: Inspect the final diff**

Run:

```bash
git status --short
git diff --stat HEAD~2..HEAD
```

Expected: only #59 implementation files and commits are present. Existing unrelated untracked plan files may still appear in `git status --short`; do not stage or modify them.

- [ ] **Step 4: Prepare the completion summary**

Report:

```text
Implemented issue #59 by adding fixed built-in `bb72` CSS checks to `qec-code`.
Verified shape, row weights, CSS orthogonality, fixed-id parser behavior, and CLI sparse-row export.
Tests run:
- cargo test -p qec-code --test code bb72
- cargo test -p qec-code --test cli code_css_bb72_hx_prints_sparse_rows_json
- cargo test -p qec-code
```
