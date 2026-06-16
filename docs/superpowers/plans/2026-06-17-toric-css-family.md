# Toric CSS Family Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the issue #71 built-in `toric:d=<distance>` CSS family to `qec-code`.

**Architecture:** Extend the existing parser-backed built-in CSS registry in `qec-code/src/codes/built_in_css.rs`. Add `Toric` as a parameterized family, generate canonical periodic square-lattice `hx`/`hz` supports from private indexing helpers, and update catalog/list tests so the family is discoverable through `qec-code code css list`.

**Tech Stack:** Rust 2024, existing `qec-code` crate, existing `QecError` typed errors, existing Cargo integration tests.

---

## File Structure

- Modify `qec-code/src/codes/built_in_css.rs`
  - Add `BuiltInCssFamily::Toric`.
  - Add `toric:d=<distance>` to `BUILT_IN_CSS_CATALOG`.
  - Teach `parse_built_in_css_code_spec(...)` and `parse_built_in_css_family_spec(...)` to recognize `toric`.
  - Dispatch `BuiltInCssFamily::Toric` from `family_css_checks(...)`.
  - Add private toric helpers that return canonical periodic `hx` and `hz` row supports.
- Modify `qec-code/tests/code.rs`
  - Extend parser coverage for `toric:d=3` and bare `toric`.
  - Add issue #71 tests for exact `d=3`, `d=4` counts/weights, and `distance < 2` rejection.
  - Extend catalog metadata coverage for `toric:d=<distance>`.
- Modify `qec-code/tests/cli.rs`
  - Extend `qec-code code css list` coverage and the direct list snapshot for the toric catalog entry.

No fixture files should be added for this issue. If the working tree already
contains issue #69 `surface_rotated` fixture-manifest edits, preserve them and
do not add toric fixture entries in this plan.

## Fixture Manifest Boundary

Do not extend `BUILT_IN_CSS_FIXTURE_CASES` in `qec-code/tests/cli.rs` for issue
#71.

Issue #61 made the fixture manifest a representative regression sweep, not a
second registry. Adding toric there would be a broader manifest policy change
and is independent from the issue #71 registry family. If issue #69
surface-rotated fixture edits are present in the same worktree, leave those
edits untouched and still avoid adding toric fixture entries. Toric CLI export
will remain covered through the shared `built_in_css_checks(...)` path and can
be pinned in a later fixture-manifest extension if needed.

## Task 1: Add Failing Tests For Toric Registry And Catalog Behavior

**Files:**
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Extend the positive built-in CSS parser test**

In `qec-code/tests/code.rs`, update `built_in_css_code_spec_parses_fixed_and_parameterized_ids` so the full function is:

```rust
#[test]
fn built_in_css_code_spec_parses_fixed_and_parameterized_ids() {
    assert_eq!(
        parse_built_in_css_code_spec("steane"),
        Ok(BuiltInCssCodeSpec::Fixed { code_id: "steane" })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=5"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::RepetitionX,
            params: BuiltInCssParams { distance: 5 },
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_z:d=5"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::RepetitionZ,
            params: BuiltInCssParams { distance: 5 },
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("surface_rotated:d=3"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::SurfaceRotated,
            params: BuiltInCssParams { distance: 3 },
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("toric:d=3"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::Toric,
            params: BuiltInCssParams { distance: 3 },
        })
    );
}
```

- [ ] **Step 2: Extend the negative parser test for bare `toric`**

In `qec-code/tests/code.rs`, add this assertion inside `built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers`, immediately after the existing bare `surface_rotated` assertion:

```rust
    assert_eq!(
        parse_built_in_css_code_spec("toric"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "toric".to_owned(),
            parameter: "d".to_owned(),
        })
    );
```

- [ ] **Step 3: Add the exact `d=3` toric test**

In `qec-code/tests/code.rs`, insert this test after `surface_rotated_rejects_distance_below_two`:

```rust
#[test]
fn toric_d3_matches_expected_checks() {
    let checks = built_in_css_checks("toric:d=3").unwrap();

    assert_eq!(checks.code_id, "toric");
    assert_eq!(checks.num_cols, 18);
    assert_eq!(
        checks.hx,
        vec![
            vec![0, 2, 9, 15],
            vec![0, 1, 10, 16],
            vec![1, 2, 11, 17],
            vec![3, 5, 9, 12],
            vec![3, 4, 10, 13],
            vec![4, 5, 11, 14],
            vec![6, 8, 12, 15],
            vec![6, 7, 13, 16],
            vec![7, 8, 14, 17],
        ]
    );
    assert_eq!(
        checks.hz,
        vec![
            vec![0, 3, 9, 10],
            vec![1, 4, 10, 11],
            vec![2, 5, 9, 11],
            vec![3, 6, 12, 13],
            vec![4, 7, 13, 14],
            vec![5, 8, 12, 14],
            vec![0, 6, 15, 16],
            vec![1, 7, 16, 17],
            vec![2, 8, 15, 17],
        ]
    );
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

- [ ] **Step 4: Add the `d=4` toric shape and row-weight test**

In `qec-code/tests/code.rs`, insert this test immediately after `toric_d3_matches_expected_checks`:

```rust
#[test]
fn toric_d4_has_expected_counts_and_weight_four_rows() {
    let checks = built_in_css_checks("toric:d=4").unwrap();

    assert_eq!(checks.code_id, "toric");
    assert_eq!(checks.num_cols, 32);
    assert_eq!(checks.hx.len(), 16);
    assert_eq!(checks.hz.len(), 16);
    assert!(
        checks.hx.iter().all(|row| row.len() == 4),
        "all toric hx rows should have weight 4: {:?}",
        checks.hx
    );
    assert!(
        checks.hz.iter().all(|row| row.len() == 4),
        "all toric hz rows should have weight 4: {:?}",
        checks.hz
    );
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

- [ ] **Step 5: Add the invalid-distance toric test**

In `qec-code/tests/code.rs`, insert this test immediately after `toric_d4_has_expected_counts_and_weight_four_rows`:

```rust
#[test]
fn toric_family_rejects_distance_below_two() {
    assert_eq!(
        built_in_css_checks("toric:d=1"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "toric".to_owned(),
            parameter: "d".to_owned(),
            value: 1,
        })
    );
}
```

- [ ] **Step 6: Update the catalog metadata test**

In `qec-code/tests/code.rs`, update `built_in_css_catalog_lists_supported_specs` so the full function is:

```rust
#[test]
fn built_in_css_catalog_lists_supported_specs() {
    let catalog = built_in_css_catalog();
    let specs = catalog.iter().map(|entry| entry.spec).collect::<Vec<_>>();
    let unique_specs = specs.iter().copied().collect::<HashSet<_>>();

    assert_eq!(
        specs,
        vec![
            "steane",
            "bb72",
            "repetition_x:d=<distance>",
            "repetition_z:d=<distance>",
            "surface_rotated:d=<distance>",
            "toric:d=<distance>",
        ]
    );
    assert_eq!(unique_specs.len(), specs.len());
    assert!(
        catalog.iter().all(|entry| !entry.description.is_empty()),
        "all catalog entries need descriptions: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "repetition_x:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "repetition_x entry should describe the distance constraint: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "repetition_z:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "repetition_z entry should describe the distance constraint: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "surface_rotated:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "surface_rotated entry should describe the distance constraint: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "toric:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "toric entry should describe the distance constraint: {catalog:?}"
    );
}
```

- [ ] **Step 7: Update the binary CSS list test**

In `qec-code/tests/cli.rs`, add this assertion inside `code_css_list_includes_supported_built_ins`, immediately after the existing `surface_rotated:d=<distance>` assertion:

```rust
    assert!(
        stdout.contains("toric:d=<distance>"),
        "stdout was: {stdout}"
    );
```

- [ ] **Step 8: Update the direct CSS list snapshot**

In `qec-code/tests/cli.rs`, update `run_code_css_list_returns_catalog_without_newline` so the full function is:

```rust
#[test]
fn run_code_css_list_returns_catalog_without_newline() {
    let output = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::list()),
        },
    })
    .unwrap();

    let expected = "Built-in CSS codes:\n  steane                        fixed [[7,1,3]] CSS code\n  bb72                          fixed [[72,12,6]] bivariate-bicycle CSS code\n  repetition_x:d=<distance>     X-check chain, distance >= 2\n  repetition_z:d=<distance>     Z-check chain, distance >= 2\n  surface_rotated:d=<distance>  rotated surface CSS code, distance >= 2\n  toric:d=<distance>            periodic square-lattice toric CSS code, distance >= 2";
    assert_eq!(output, expected);
}
```

- [ ] **Step 9: Run the toric code-test filter and verify the expected failure**

Run:

```bash
cargo test -p qec-code --test code toric
```

Expected: FAIL at compile time because `BuiltInCssFamily::Toric` does not exist yet. The compiler should report an error containing:

```text
no variant or associated item named `Toric`
```

- [ ] **Step 10: Run the CSS list filter and verify the expected failure**

Run:

```bash
cargo test -p qec-code --test cli css_list
```

Expected: FAIL because `qec-code code css list` output does not yet contain `toric:d=<distance>` and the direct list snapshot still reflects the old catalog.

## Task 2: Implement Toric Parser, Catalog, And Matrix Generation

**Files:**
- Modify: `qec-code/src/codes/built_in_css.rs`
- Verify: `qec-code/tests/code.rs`
- Verify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Add toric to the catalog**

In `qec-code/src/codes/built_in_css.rs`, update `BUILT_IN_CSS_CATALOG` so the full constant is:

```rust
const BUILT_IN_CSS_CATALOG: &[BuiltInCssCatalogEntry] = &[
    BuiltInCssCatalogEntry {
        spec: "steane",
        description: "fixed [[7,1,3]] CSS code",
    },
    BuiltInCssCatalogEntry {
        spec: "bb72",
        description: "fixed [[72,12,6]] bivariate-bicycle CSS code",
    },
    BuiltInCssCatalogEntry {
        spec: "repetition_x:d=<distance>",
        description: "X-check chain, distance >= 2",
    },
    BuiltInCssCatalogEntry {
        spec: "repetition_z:d=<distance>",
        description: "Z-check chain, distance >= 2",
    },
    BuiltInCssCatalogEntry {
        spec: "surface_rotated:d=<distance>",
        description: "rotated surface CSS code, distance >= 2",
    },
    BuiltInCssCatalogEntry {
        spec: "toric:d=<distance>",
        description: "periodic square-lattice toric CSS code, distance >= 2",
    },
];
```

- [ ] **Step 2: Add the `Toric` family variant**

In `qec-code/src/codes/built_in_css.rs`, update `BuiltInCssFamily` so the full enum is:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltInCssFamily {
    RepetitionX,
    RepetitionZ,
    SurfaceRotated,
    Toric,
}
```

- [ ] **Step 3: Treat bare `toric` as a missing-parameter family**

In `qec-code/src/codes/built_in_css.rs`, update the bare-id match in `parse_built_in_css_code_spec(...)` so the full function is:

```rust
pub fn parse_built_in_css_code_spec(input: &str) -> Result<BuiltInCssCodeSpec> {
    if let Some((family_name, params_text)) = input.split_once(':') {
        return parse_built_in_css_family_spec(family_name, params_text);
    }

    match input {
        "steane" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "steane" }),
        "bb72" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "bb72" }),
        "repetition_x" | "repetition_z" | "surface_rotated" | "toric" => {
            Err(QecError::MissingBuiltInCssParameter {
                family: input.to_owned(),
                parameter: "d".to_owned(),
            })
        }
        _ => Err(QecError::UnknownBuiltInCssCode {
            code_id: input.to_owned(),
        }),
    }
}
```

- [ ] **Step 4: Parse `toric:d=<distance>` as a family selector**

In `qec-code/src/codes/built_in_css.rs`, update `parse_built_in_css_family_spec(...)` so the full function is:

```rust
fn parse_built_in_css_family_spec(
    family_name: &str,
    params_text: &str,
) -> Result<BuiltInCssCodeSpec> {
    let family = match family_name {
        "repetition_x" => BuiltInCssFamily::RepetitionX,
        "repetition_z" => BuiltInCssFamily::RepetitionZ,
        "surface_rotated" => BuiltInCssFamily::SurfaceRotated,
        "toric" => BuiltInCssFamily::Toric,
        _ => {
            return Err(QecError::UnknownBuiltInCssFamily {
                family: family_name.to_owned(),
            });
        }
    };

    let distance = parse_repetition_distance(family_name, params_text)?;

    Ok(BuiltInCssCodeSpec::Family {
        family,
        params: BuiltInCssParams { distance },
    })
}
```

- [ ] **Step 5: Dispatch toric family generation**

In `qec-code/src/codes/built_in_css.rs`, update `family_css_checks(...)` so the full function is:

```rust
fn family_css_checks(family: BuiltInCssFamily, distance: usize) -> Result<BuiltInCssChecks> {
    match family {
        BuiltInCssFamily::RepetitionX => {
            let hx = chain_supports("repetition_x", distance)?;
            Ok(BuiltInCssChecks {
                code_id: "repetition_x",
                num_cols: distance,
                hx,
                hz: vec![],
            })
        }
        BuiltInCssFamily::RepetitionZ => {
            let hz = chain_supports("repetition_z", distance)?;
            Ok(BuiltInCssChecks {
                code_id: "repetition_z",
                num_cols: distance,
                hx: vec![],
                hz,
            })
        }
        BuiltInCssFamily::SurfaceRotated => surface_rotated_css_checks(distance),
        BuiltInCssFamily::Toric => toric_css_checks(distance),
    }
}
```

- [ ] **Step 6: Add private toric generator helpers**

In `qec-code/src/codes/built_in_css.rs`, insert these helpers after `rotated_surface_data_index(...)`:

```rust
fn toric_css_checks(distance: usize) -> Result<BuiltInCssChecks> {
    if distance < 2 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "toric".to_owned(),
            parameter: "d".to_owned(),
            value: distance,
        });
    }

    let (hx, hz) = toric_supports(distance);

    Ok(BuiltInCssChecks {
        code_id: "toric",
        num_cols: 2 * distance * distance,
        hx,
        hz,
    })
}

fn toric_supports(distance: usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut hx = Vec::with_capacity(distance * distance);
    let mut hz = Vec::with_capacity(distance * distance);

    for x in 0..distance {
        for y in 0..distance {
            hx.push(toric_x_check_support(distance, x, y));
            hz.push(toric_z_check_support(distance, x, y));
        }
    }

    (hx, hz)
}

fn toric_x_check_support(distance: usize, x: usize, y: usize) -> Vec<usize> {
    let prev_x = wrap_prev(x, distance);
    let prev_y = wrap_prev(y, distance);

    sorted_toric_row([
        toric_horizontal_index(distance, x, y),
        toric_horizontal_index(distance, x, prev_y),
        toric_vertical_index(distance, x, y),
        toric_vertical_index(distance, prev_x, y),
    ])
}

fn toric_z_check_support(distance: usize, x: usize, y: usize) -> Vec<usize> {
    let next_x = wrap_next(x, distance);
    let next_y = wrap_next(y, distance);

    sorted_toric_row([
        toric_horizontal_index(distance, x, y),
        toric_horizontal_index(distance, next_x, y),
        toric_vertical_index(distance, x, y),
        toric_vertical_index(distance, x, next_y),
    ])
}

fn sorted_toric_row(mut row: [usize; 4]) -> Vec<usize> {
    row.sort_unstable();
    row.to_vec()
}

fn toric_horizontal_index(distance: usize, x: usize, y: usize) -> usize {
    x * distance + y
}

fn toric_vertical_index(distance: usize, x: usize, y: usize) -> usize {
    distance * distance + x * distance + y
}

fn wrap_prev(value: usize, distance: usize) -> usize {
    (value + distance - 1) % distance
}

fn wrap_next(value: usize, distance: usize) -> usize {
    (value + 1) % distance
}
```

- [ ] **Step 7: Format the qec-code package**

Run:

```bash
cargo fmt --package qec-code
```

Expected: PASS with no output.

- [ ] **Step 8: Run the focused toric tests**

Run:

```bash
cargo test -p qec-code --test code toric
```

Expected: PASS. The output should include these passing tests:

```text
toric_d3_matches_expected_checks ... ok
toric_d4_has_expected_counts_and_weight_four_rows ... ok
toric_family_rejects_distance_below_two ... ok
```

- [ ] **Step 9: Run the parser test**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids
```

Expected: PASS. This verifies `parse_built_in_css_code_spec("toric:d=3")` returns `BuiltInCssFamily::Toric`.

- [ ] **Step 10: Run the parser negative test**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers
```

Expected: PASS. This verifies bare `toric` returns `MissingBuiltInCssParameter`.

- [ ] **Step 11: Run the catalog test**

Run:

```bash
cargo test -p qec-code --test code built_in_css_catalog_lists_supported_specs
```

Expected: PASS. This verifies `built_in_css_catalog()` includes `toric:d=<distance>` exactly once and describes the distance constraint.

- [ ] **Step 12: Run the CSS list CLI tests**

Run:

```bash
cargo test -p qec-code --test cli css_list
```

Expected: PASS. This verifies both binary and direct `run(...)` list output include `toric:d=<distance>`.

- [ ] **Step 13: Inspect git state before staging**

Run:

```bash
git status --short
```

Expected in a clean issue #71 execution worktree:

```text
 M qec-code/src/codes/built_in_css.rs
 M qec-code/tests/cli.rs
 M qec-code/tests/code.rs
```

If `qec-code/tests/cli.rs` already contained unrelated issue #69 edits before
execution began, do not stage the full file with `git add`. Use an isolated
issue #71 worktree or stop for user direction before committing, because a
full-file stage would mix unrelated review surfaces.

- [ ] **Step 14: Commit the toric CSS family implementation from a clean issue #71 worktree**

Run:

```bash
git add qec-code/src/codes/built_in_css.rs qec-code/tests/code.rs qec-code/tests/cli.rs
git commit -m "feat: add built-in toric css checks"
```

Expected: commit succeeds with only issue #71 implementation changes staged.

## Task 3: Final Verification

**Files:**
- Verify: `qec-code/src/codes/built_in_css.rs`
- Verify: `qec-code/tests/code.rs`
- Verify: `qec-code/tests/cli.rs`

- [ ] **Step 1: Run the issue #71 focused tests**

Run:

```bash
cargo test -p qec-code --test code toric
```

Expected: PASS. The three toric tests pass.

- [ ] **Step 2: Run the nearby parser, catalog, and list regressions**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec
cargo test -p qec-code --test code built_in_css_catalog_lists_supported_specs
cargo test -p qec-code --test cli css_list
```

Expected: PASS. Parser, catalog, and CLI list behavior remain aligned.

- [ ] **Step 3: Run all qec-code tests**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS. Existing fixed ids, repetition families, `surface_rotated`, CLI export, distance, and sparse-row tests all continue to pass.

- [ ] **Step 4: Check package formatting**

Run:

```bash
cargo fmt --check --package qec-code
```

Expected: PASS. Rustfmt reports no diff.

- [ ] **Step 5: Inspect final git state**

Run:

```bash
git status --short --branch
```

Expected: the implementation commit is present and no unintended files were modified. If pre-existing issue #69 `qec-code/tests/cli.rs` edits were present before execution, they should still be present unless they were intentionally incorporated by the user.
