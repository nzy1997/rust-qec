# Surface Rotated CSS Family Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the issue #68 built-in `surface_rotated:d=<distance>` CSS family to `qec-code`.

**Architecture:** Extend the existing parser-backed built-in CSS registry in `qec-code/src/codes/built_in_css.rs`. Add `SurfaceRotated` as a parameterized family, generate canonical rotated-surface `hx`/`hz` supports from local geometry helpers, and verify the exact issue-requested `d=3`, `d=5`, and invalid-distance behavior in `qec-code/tests/code.rs`.

**Tech Stack:** Rust 2024, existing `qec-code` crate, existing `QecError` typed errors, `cargo test`.

---

## File Structure

- Modify `qec-code/src/codes/built_in_css.rs`
  - Add `BuiltInCssFamily::SurfaceRotated`.
  - Teach `parse_built_in_css_code_spec(...)` and `parse_built_in_css_family_spec(...)` to recognize `surface_rotated`.
  - Dispatch `BuiltInCssFamily::SurfaceRotated` from `built_in_css_checks(...)`.
  - Add private rotated-surface geometry helpers that return canonical non-empty `hx` and `hz` row supports.
- Modify `qec-code/tests/code.rs`
  - Extend parser coverage for `surface_rotated:d=3`.
  - Add issue #68 tests for exact `d=3` rows, `d=5` counts/weights, and `distance < 2` rejection.

No `rstim`, `rsinter`, CLI, or error enum files should change.

## Task 1: Add Parser Coverage And Selector Variant

**Files:**
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/src/codes/built_in_css.rs`

- [ ] **Step 1: Add the failing parser expectation**

In `qec-code/tests/code.rs`, update `built_in_css_code_spec_parses_fixed_and_parameterized_ids` so it includes this assertion after the existing `repetition_z:d=5` assertion:

```rust
    assert_eq!(
        parse_built_in_css_code_spec("surface_rotated:d=3"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::SurfaceRotated,
            params: BuiltInCssParams { distance: 3 },
        })
    );
```

After this edit, the full test should be:

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
}
```

- [ ] **Step 2: Run the parser test and confirm it fails**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids
```

Expected: FAIL at compile time with an error like:

```text
no variant or associated item named `SurfaceRotated` found for enum `BuiltInCssFamily`
```

- [ ] **Step 3: Add the selector variant and parser recognition**

In `qec-code/src/codes/built_in_css.rs`, replace the current `BuiltInCssFamily` enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltInCssFamily {
    RepetitionX,
    RepetitionZ,
}
```

with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltInCssFamily {
    RepetitionX,
    RepetitionZ,
    SurfaceRotated,
}
```

Then update the bare-id match in `parse_built_in_css_code_spec(...)` from:

```rust
        "repetition_x" | "repetition_z" => Err(QecError::MissingBuiltInCssParameter {
            family: input.to_owned(),
            parameter: "d".to_owned(),
        }),
```

to:

```rust
        "repetition_x" | "repetition_z" | "surface_rotated" => {
            Err(QecError::MissingBuiltInCssParameter {
                family: input.to_owned(),
                parameter: "d".to_owned(),
            })
        }
```

Finally update the family match in `parse_built_in_css_family_spec(...)` from:

```rust
    let family = match family_name {
        "repetition_x" => BuiltInCssFamily::RepetitionX,
        "repetition_z" => BuiltInCssFamily::RepetitionZ,
        _ => {
            return Err(QecError::UnknownBuiltInCssFamily {
                family: family_name.to_owned(),
            });
        }
    };
```

to:

```rust
    let family = match family_name {
        "repetition_x" => BuiltInCssFamily::RepetitionX,
        "repetition_z" => BuiltInCssFamily::RepetitionZ,
        "surface_rotated" => BuiltInCssFamily::SurfaceRotated,
        _ => {
            return Err(QecError::UnknownBuiltInCssFamily {
                family: family_name.to_owned(),
            });
        }
    };
```

Because adding an enum variant makes downstream matches non-exhaustive, also
replace the current `repetition_css_checks(...)` function:

```rust
fn repetition_css_checks(
    family: BuiltInCssFamily,
    distance: usize,
) -> Result<BuiltInCssChecks> {
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
    }
}
```

with this temporary exhaustive version:

```rust
fn repetition_css_checks(
    family: BuiltInCssFamily,
    distance: usize,
) -> Result<BuiltInCssChecks> {
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
        BuiltInCssFamily::SurfaceRotated => Err(QecError::UnknownBuiltInCssCode {
            code_id: "surface_rotated".to_owned(),
        }),
    }
}
```

This branch is deliberately a short-lived compile bridge. Task 2 replaces it
with the real family dispatch and generator.

- [ ] **Step 4: Run the parser test and confirm it passes**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids
```

Expected: PASS. The output should include:

```text
test built_in_css_code_spec_parses_fixed_and_parameterized_ids ... ok
```

- [ ] **Step 5: Commit the parser extension**

Run:

```bash
git add qec-code/tests/code.rs qec-code/src/codes/built_in_css.rs
git commit -m "feat: parse surface rotated css code specs"
```

## Task 2: Add Rotated-Surface Shape Tests And Generator

**Files:**
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/src/codes/built_in_css.rs`

- [ ] **Step 1: Add the issue #68 shape tests**

In `qec-code/tests/code.rs`, insert these helpers after `dense_row(...)`:

```rust
fn row_weight_counts(rows: &[Vec<usize>]) -> std::collections::BTreeMap<usize, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for row in rows {
        *counts.entry(row.len()).or_insert(0) += 1;
    }
    counts
}

fn assert_surface_rotated_d5_weights(rows: &[Vec<usize>]) {
    let counts = row_weight_counts(rows);
    assert_eq!(counts.get(&2), Some(&4));
    assert_eq!(counts.get(&4), Some(&8));
    assert_eq!(counts.values().sum::<usize>(), 12);
}
```

Then insert these tests after `bb72_has_expected_shape_and_css_orthogonality`:

```rust
#[test]
fn surface_rotated_d3_matches_expected_checks() {
    let checks = built_in_css_checks("surface_rotated:d=3").unwrap();

    assert_eq!(checks.code_id, "surface_rotated");
    assert_eq!(checks.num_cols, 9);
    assert_eq!(
        checks.hx,
        vec![
            vec![0, 3],
            vec![1, 2, 4, 5],
            vec![3, 4, 6, 7],
            vec![5, 8],
        ]
    );
    assert_eq!(
        checks.hz,
        vec![
            vec![1, 2],
            vec![0, 1, 3, 4],
            vec![4, 5, 7, 8],
            vec![6, 7],
        ]
    );
    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
}

#[test]
fn surface_rotated_d5_has_expected_check_counts_and_weights() {
    let checks = built_in_css_checks("surface_rotated:d=5").unwrap();

    assert_eq!(checks.code_id, "surface_rotated");
    assert_eq!(checks.num_cols, 25);
    assert_eq!(checks.hx.len(), 12);
    assert_eq!(checks.hz.len(), 12);
    assert_surface_rotated_d5_weights(&checks.hx);
    assert_surface_rotated_d5_weights(&checks.hz);
    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
    assert_rows_in_range(&checks.hx, checks.num_cols);
    assert_rows_in_range(&checks.hz, checks.num_cols);
}
```

- [ ] **Step 2: Run the new shape tests and confirm they fail**

Run:

```bash
cargo test -p qec-code --test code surface_rotated_d3_matches_expected_checks
cargo test -p qec-code --test code surface_rotated_d5_has_expected_check_counts_and_weights
```

Expected: both commands FAIL. The first failure should report:

```text
called `Result::unwrap()` on an `Err` value: UnknownBuiltInCssCode { code_id: "surface_rotated" }
```

or an equivalent unhandled-family error from the registry dispatch.

- [ ] **Step 3: Add registry dispatch for `SurfaceRotated`**

In `qec-code/src/codes/built_in_css.rs`, update `built_in_css_checks(...)` from:

```rust
pub fn built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks> {
    match parse_built_in_css_code_spec(code_id)? {
        BuiltInCssCodeSpec::Fixed { code_id } => fixed_built_in_css_checks(code_id),
        BuiltInCssCodeSpec::Family { family, params } => {
            repetition_css_checks(family, params.distance)
        }
    }
}
```

to:

```rust
pub fn built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks> {
    match parse_built_in_css_code_spec(code_id)? {
        BuiltInCssCodeSpec::Fixed { code_id } => fixed_built_in_css_checks(code_id),
        BuiltInCssCodeSpec::Family { family, params } => {
            family_css_checks(family, params.distance)
        }
    }
}
```

Then replace the current temporary `repetition_css_checks(...)` function:

```rust
fn repetition_css_checks(
    family: BuiltInCssFamily,
    distance: usize,
) -> Result<BuiltInCssChecks> {
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
        BuiltInCssFamily::SurfaceRotated => Err(QecError::UnknownBuiltInCssCode {
            code_id: "surface_rotated".to_owned(),
        }),
    }
}
```

with:

```rust
fn family_css_checks(
    family: BuiltInCssFamily,
    distance: usize,
) -> Result<BuiltInCssChecks> {
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
    }
}
```

- [ ] **Step 4: Add the rotated-surface generator helpers**

In `qec-code/src/codes/built_in_css.rs`, insert this code after `chain_supports(...)`:

```rust
fn surface_rotated_css_checks(distance: usize) -> Result<BuiltInCssChecks> {
    if distance < 2 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "surface_rotated".to_owned(),
            parameter: "d".to_owned(),
            value: distance,
        });
    }

    let (hx, hz) = rotated_surface_supports(distance);

    Ok(BuiltInCssChecks {
        code_id: "surface_rotated",
        num_cols: distance * distance,
        hx,
        hz,
    })
}

fn rotated_surface_supports(distance: usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut hx = Vec::new();
    let mut hz = Vec::new();

    for ax in 0..=distance {
        for ay in 0..=distance {
            let on_boundary_1 = ax == 0 || ax == distance;
            let on_boundary_2 = ay == 0 || ay == distance;
            let parity = (ax % 2) != (ay % 2);
            if on_boundary_1 && parity {
                continue;
            }
            if on_boundary_2 && !parity {
                continue;
            }

            let support = rotated_surface_measure_support(distance, ax, ay);
            if support.is_empty() {
                continue;
            }

            if parity {
                hx.push(support);
            } else {
                hz.push(support);
            }
        }
    }

    (hx, hz)
}

fn rotated_surface_measure_support(distance: usize, ax: usize, ay: usize) -> Vec<usize> {
    let mut support = Vec::new();
    let mx = (2 * ax) as isize;
    let my = (2 * ay) as isize;

    for (dx, dy) in [(1isize, 1isize), (1, -1), (-1, 1), (-1, -1)] {
        let x = mx + dx;
        let y = my + dy;
        if x >= 1
            && x <= (2 * distance - 1) as isize
            && y >= 1
            && y <= (2 * distance - 1) as isize
            && x % 2 == 1
            && y % 2 == 1
        {
            let qx = ((x - 1) / 2) as usize;
            let qy = ((y - 1) / 2) as usize;
            if qx < distance && qy < distance {
                support.push(rotated_surface_data_index(distance, qx, qy));
            }
        }
    }

    support.sort_unstable();
    support.dedup();
    support
}

fn rotated_surface_data_index(distance: usize, x: usize, y: usize) -> usize {
    x * distance + y
}
```

- [ ] **Step 5: Run the shape tests and confirm they pass**

Run:

```bash
cargo test -p qec-code --test code surface_rotated_d3_matches_expected_checks
cargo test -p qec-code --test code surface_rotated_d5_has_expected_check_counts_and_weights
```

Expected: both commands PASS. The output should include:

```text
test surface_rotated_d3_matches_expected_checks ... ok
test surface_rotated_d5_has_expected_check_counts_and_weights ... ok
```

- [ ] **Step 6: Run the existing family tests to catch regressions**

Run:

```bash
cargo test -p qec-code --test code repetition_x_d5_matches_chain_checks
cargo test -p qec-code --test code repetition_z_d5_matches_chain_checks
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids
```

Expected: all three commands PASS.

- [ ] **Step 7: Commit the generator**

Run:

```bash
git add qec-code/tests/code.rs qec-code/src/codes/built_in_css.rs
git commit -m "feat: add surface rotated css checks"
```

## Task 3: Add Invalid-Distance Coverage And Final Verification

**Files:**
- Modify: `qec-code/tests/code.rs`
- Verify: `qec-code`

- [ ] **Step 1: Add the invalid-distance test**

In `qec-code/tests/code.rs`, insert this test after `surface_rotated_d5_has_expected_check_counts_and_weights`:

```rust
#[test]
fn surface_rotated_rejects_distance_below_two() {
    assert_eq!(
        built_in_css_checks("surface_rotated:d=1"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "surface_rotated".to_owned(),
            parameter: "d".to_owned(),
            value: 1,
        })
    );
}
```

- [ ] **Step 2: Run the invalid-distance test and confirm it passes**

Run:

```bash
cargo test -p qec-code --test code surface_rotated_rejects_distance_below_two
```

Expected: PASS. The output should include:

```text
test surface_rotated_rejects_distance_below_two ... ok
```

- [ ] **Step 3: Run the issue #68 focused test set**

Run:

```bash
cargo test -p qec-code --test code surface_rotated_d3_matches_expected_checks
cargo test -p qec-code --test code surface_rotated_d5_has_expected_check_counts_and_weights
cargo test -p qec-code --test code surface_rotated_rejects_distance_below_two
```

Expected: all three commands PASS.

- [ ] **Step 4: Run all `qec-code` tests**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS. Existing Steane, BB72, repetition-family, parser, sparse-row, CSS, logical, distance, and CLI tests should all pass.

- [ ] **Step 5: Check the final diff and working tree**

Run:

```bash
git status --short
git diff --stat
```

Expected: only intended changes to these files should be unstaged:

```text
qec-code/src/codes/built_in_css.rs
qec-code/tests/code.rs
```

Pre-existing unrelated untracked files may still appear:

```text
docs/superpowers/plans/2026-06-16-randomized-css-distance-upper-bound.md
docs/superpowers/specs/2026-06-16-qec-code-css-list-design.md
```

- [ ] **Step 6: Commit the invalid-distance test and final verified state**

Run:

```bash
git add qec-code/tests/code.rs qec-code/src/codes/built_in_css.rs
git commit -m "test: cover surface rotated css validation"
```

- [ ] **Step 7: Prepare the completion summary**

Report:

```text
Implemented issue #68 by adding the built-in `surface_rotated:d=<distance>` CSS family to `qec-code`.
Verified exact d=3 supports, d=5 row counts and weights, and distance < 2 rejection.
Tests run:
- cargo test -p qec-code --test code surface_rotated_d3_matches_expected_checks
- cargo test -p qec-code --test code surface_rotated_d5_has_expected_check_counts_and_weights
- cargo test -p qec-code --test code surface_rotated_rejects_distance_below_two
- cargo test -p qec-code
```
