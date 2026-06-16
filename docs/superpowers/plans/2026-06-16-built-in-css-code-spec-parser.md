# Built-In CSS Code Spec Parser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the issue #57 built-in CSS code-spec parser for fixed ids and parameterized repetition-family specs without adding new matrix families.

**Architecture:** Keep parsing in `qec-code/src/codes/built_in_css.rs`, next to the existing built-in CSS registry. The parser returns a typed selector that future registry dispatch can consume, while `built_in_css_checks("steane")`, `Steane::new()`, and the current CSS CLI export path remain unchanged.

**Tech Stack:** Rust 2024, `thiserror` for typed errors, existing `cargo test -p qec-code` test harness.

---

## File Structure

- Modify `qec-code/src/codes/built_in_css.rs`: add public code-spec selector types and `parse_built_in_css_code_spec`.
- Modify `qec-code/src/error.rs`: add typed `QecError` variants for malformed parameterized code specs.
- Modify `qec-code/tests/code.rs`: add issue #57 positive and negative parser tests.
- Leave `qec-code/src/cli.rs`, `qec-code/src/codes/steane.rs`, `qec-code/src/css.rs`, `rstim`, and `rsinter` unchanged.

## Task 1: Add Positive Parser API and Coverage

**Files:**
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/src/codes/built_in_css.rs`

- [ ] **Step 1: Update the `code.rs` built-in CSS import**

Replace the first import in `qec-code/tests/code.rs`:

```rust
use qec_code::codes::built_in_css::built_in_css_checks;
```

with:

```rust
use qec_code::codes::built_in_css::{
    BuiltInCssCodeSpec, BuiltInCssFamily, BuiltInCssParams, built_in_css_checks,
    parse_built_in_css_code_spec,
};
```

- [ ] **Step 2: Add the positive parser test**

Append this test after `built_in_css_registry_exposes_steane_checks` in `qec-code/tests/code.rs`:

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
}
```

- [ ] **Step 3: Run the positive parser test and confirm it fails before implementation**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids
```

Expected: FAIL at compile time because `BuiltInCssCodeSpec`, `BuiltInCssFamily`, `BuiltInCssParams`, and `parse_built_in_css_code_spec` do not exist yet.

- [ ] **Step 4: Add the public selector types and minimal positive-path parser**

In `qec-code/src/codes/built_in_css.rs`, insert this block after `BuiltInCssChecks` and before `STEANE_ROW_SUPPORTS`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltInCssCodeSpec {
    Fixed {
        code_id: &'static str,
    },
    Family {
        family: BuiltInCssFamily,
        params: BuiltInCssParams,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltInCssFamily {
    RepetitionX,
    RepetitionZ,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInCssParams {
    pub distance: usize,
}

pub fn parse_built_in_css_code_spec(input: &str) -> Result<BuiltInCssCodeSpec> {
    if let Some((family_name, params_text)) = input.split_once(':') {
        let family = match family_name {
            "repetition_x" => BuiltInCssFamily::RepetitionX,
            "repetition_z" => BuiltInCssFamily::RepetitionZ,
            _ => {
                return Err(QecError::UnknownBuiltInCssCode {
                    code_id: family_name.to_owned(),
                });
            }
        };

        let distance = params_text
            .strip_prefix("d=")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);

        return Ok(BuiltInCssCodeSpec::Family {
            family,
            params: BuiltInCssParams { distance },
        });
    }

    match input {
        "steane" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "steane" }),
        _ => Err(QecError::UnknownBuiltInCssCode {
            code_id: input.to_owned(),
        }),
    }
}
```

This intentionally covers only the positive parser path. Task 2 replaces the permissive invalid-input behavior with typed validation errors.

- [ ] **Step 5: Run the positive parser test and confirm it passes**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids
```

Expected: PASS. The test should report `1 passed`.

- [ ] **Step 6: Commit the positive parser shell**

Run:

```bash
git add qec-code/tests/code.rs qec-code/src/codes/built_in_css.rs
git commit -m "feat: add built-in css code spec selector"
```

## Task 2: Add Typed Validation Errors and Negative Coverage

**Files:**
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/src/error.rs`
- Modify: `qec-code/src/codes/built_in_css.rs`

- [ ] **Step 1: Add the negative parser test**

Append this test after `built_in_css_code_spec_parses_fixed_and_parameterized_ids` in `qec-code/tests/code.rs`:

```rust
#[test]
fn built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers() {
    assert_eq!(
        parse_built_in_css_code_spec("unknown:d=5"),
        Err(QecError::UnknownBuiltInCssFamily {
            family: "unknown".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=nope"),
        Err(QecError::InvalidBuiltInCssIntegerParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
            value: "nope".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=5,d=7"),
        Err(QecError::DuplicateBuiltInCssParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=0"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
            value: 0,
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=5,foo=1"),
        Err(QecError::UnexpectedBuiltInCssParameter {
            family: "repetition_x".to_owned(),
            parameter: "foo".to_owned(),
        })
    );
}
```

- [ ] **Step 2: Run the issue-requested parser tests and confirm they fail before validation implementation**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers
```

Expected: FAIL at compile time because the new `QecError` variants do not exist yet.

- [ ] **Step 3: Add typed code-spec error variants**

In `qec-code/src/error.rs`, insert these variants immediately after `UnknownBuiltInCssCode`:

```rust
    #[error("unknown built-in CSS family: {family}")]
    UnknownBuiltInCssFamily { family: String },
    #[error("missing built-in CSS parameter {parameter} for family {family}")]
    MissingBuiltInCssParameter {
        family: String,
        parameter: String,
    },
    #[error("duplicate built-in CSS parameter {parameter} for family {family}")]
    DuplicateBuiltInCssParameter {
        family: String,
        parameter: String,
    },
    #[error("invalid built-in CSS integer parameter {parameter} for family {family}: {value}")]
    InvalidBuiltInCssIntegerParameter {
        family: String,
        parameter: String,
        value: String,
    },
    #[error("unexpected built-in CSS parameter {parameter} for family {family}")]
    UnexpectedBuiltInCssParameter {
        family: String,
        parameter: String,
    },
    #[error("out-of-range built-in CSS integer parameter {parameter} for family {family}: {value}")]
    OutOfRangeBuiltInCssIntegerParameter {
        family: String,
        parameter: String,
        value: usize,
    },
```

After insertion, the tail of the enum should still end with `}` and `pub type Result<T> = core::result::Result<T, QecError>;`.

- [ ] **Step 4: Replace the permissive parser with validated parsing**

In `qec-code/src/codes/built_in_css.rs`, replace the Task 1 `parse_built_in_css_code_spec` function with this implementation, and add the helper functions below it:

```rust
pub fn parse_built_in_css_code_spec(input: &str) -> Result<BuiltInCssCodeSpec> {
    if let Some((family_name, params_text)) = input.split_once(':') {
        return parse_built_in_css_family_spec(family_name, params_text);
    }

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
}

fn parse_built_in_css_family_spec(
    family_name: &str,
    params_text: &str,
) -> Result<BuiltInCssCodeSpec> {
    let family = match family_name {
        "repetition_x" => BuiltInCssFamily::RepetitionX,
        "repetition_z" => BuiltInCssFamily::RepetitionZ,
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

fn parse_repetition_distance(family_name: &str, params_text: &str) -> Result<usize> {
    if params_text.is_empty() {
        return Err(QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "d".to_owned(),
        });
    }

    let mut distance = None;

    for pair in params_text.split(',') {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(QecError::UnexpectedBuiltInCssParameter {
                family: family_name.to_owned(),
                parameter: pair.to_owned(),
            });
        };

        match key {
            "d" => {
                if distance.is_some() {
                    return Err(QecError::DuplicateBuiltInCssParameter {
                        family: family_name.to_owned(),
                        parameter: "d".to_owned(),
                    });
                }

                let parsed = value.parse::<usize>().map_err(|_| {
                    QecError::InvalidBuiltInCssIntegerParameter {
                        family: family_name.to_owned(),
                        parameter: "d".to_owned(),
                        value: value.to_owned(),
                    }
                })?;

                if parsed == 0 {
                    return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
                        family: family_name.to_owned(),
                        parameter: "d".to_owned(),
                        value: parsed,
                    });
                }

                distance = Some(parsed);
            }
            _ => {
                return Err(QecError::UnexpectedBuiltInCssParameter {
                    family: family_name.to_owned(),
                    parameter: key.to_owned(),
                });
            }
        }
    }

    distance.ok_or_else(|| QecError::MissingBuiltInCssParameter {
        family: family_name.to_owned(),
        parameter: "d".to_owned(),
    })
}
```

- [ ] **Step 5: Run the issue-requested parser tests and confirm they pass**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers
```

Expected: PASS. Both parser tests should pass.

- [ ] **Step 6: Run the full `code` integration test file**

Run:

```bash
cargo test -p qec-code --test code
```

Expected: PASS. Existing stabilizer, CSS, Steane, registry, sparse-row, and parser tests should all pass.

- [ ] **Step 7: Commit the validation behavior**

Run:

```bash
git add qec-code/tests/code.rs qec-code/src/error.rs qec-code/src/codes/built_in_css.rs
git commit -m "feat: validate built-in css code specs"
```

## Task 3: Final Regression Check

**Files:**
- Verify: `qec-code`

- [ ] **Step 1: Run the exact issue verification command**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers
```

Expected: PASS. This is the command requested by GitHub issue #57.

- [ ] **Step 2: Run all `qec-code` tests**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS. This confirms the parser did not regress existing registry, CLI, sparse-row export, Steane, logical, normalizer, distance, or binary tests.

- [ ] **Step 3: Confirm the working tree only contains intended issue #57 changes**

Run:

```bash
git status --short
```

Expected: either a clean tree, or only unrelated pre-existing untracked files that were present before implementation. The known unrelated file before this plan was `docs/superpowers/plans/2026-06-16-rsinter-memory-z-parity.md`.

