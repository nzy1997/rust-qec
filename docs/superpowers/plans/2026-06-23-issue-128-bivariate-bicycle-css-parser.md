# Issue 128 Bivariate-Bicycle CSS Parser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `qec-code` built-in CSS spec parsing so `bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0` returns typed `BivariateBicycleParams`.

**Architecture:** Keep parser and validation logic in `qec-code/src/codes/built_in_css.rs`, where built-in CSS family parsing already lives. Replace the single `BuiltInCssParams { distance }` shape with a family-specific enum, preserve distance-family parsing, and validate parsed `bb` parameters before returning the parse result without wiring parsed `bb:...` specs into matrix generation.

**Tech Stack:** Rust 2024, `qec-code`, existing `QecError`, existing `BivariateBicycleParams` and issue #126 validation helpers.

## Global Constraints

- Modify only `docs/superpowers/specs/2026-06-23-issue-128-bivariate-bicycle-css-parser-design.md`, `docs/superpowers/plans/2026-06-23-issue-128-bivariate-bicycle-css-parser.md`, `qec-code/src/codes/built_in_css.rs`, and `qec-code/tests/code.rs`.
- Preserve existing parser behavior for `repetition_x`, `repetition_z`, `surface_rotated`, and `toric` specs.
- `parse_built_in_css_code_spec("bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0")` must return `BuiltInCssCodeSpec::Family` with `BuiltInCssFamily::BivariateBicycle` and `BuiltInCssParams::BivariateBicycle(BivariateBicycleParams { lx: 12, ly: 6, a_terms: vec![(3, 0), (0, 1), (0, 2)], b_terms: vec![(0, 3), (1, 0), (2, 0)] })`.
- Reject missing `a`, `lx=0`, duplicate `lx`, unknown `foo=1`, malformed `a=3`, and modulo-duplicate terms such as `a=0:0|6:0` when `lx=6`.
- Reuse issue #126 positive dimension and normalized duplicate validation rules.
- Do not add catalog text, CLI help text, benchmark integration, logical observable generation, or parsed `bb:...` matrix generation.

---

## File Structure

- Modify `qec-code/tests/code.rs`: update `BuiltInCssParams` expectations for distance families and add focused bivariate-bicycle parser acceptance/rejection tests.
- Modify `qec-code/src/codes/built_in_css.rs`: add the `BuiltInCssParams` enum, add `BuiltInCssFamily::BivariateBicycle`, dispatch `bb` parsing, and keep matrix generation out of scope for parsed `bb:...`.

### Task 1: Parse Bivariate-Bicycle CSS Family Specs

**Files:**
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/src/codes/built_in_css.rs`

**Interfaces:**
- Consumes: existing `BivariateBicycleParams`, `BuiltInCssCodeSpec`, `BuiltInCssFamily`, `BuiltInCssParams`, `QecError`, and `parse_built_in_css_code_spec(...)`.
- Produces:
  - `pub enum BuiltInCssParams { Distance { distance: usize }, BivariateBicycle(BivariateBicycleParams) }`
  - `BuiltInCssFamily::BivariateBicycle`
  - `parse_built_in_css_code_spec("bb:...") -> Result<BuiltInCssCodeSpec>`

- [ ] **Step 1: Write the failing parser tests**

In `qec-code/tests/code.rs`, update the existing distance-family expectations from:

```rust
params: BuiltInCssParams { distance: 5 },
```

to:

```rust
params: BuiltInCssParams::Distance { distance: 5 },
```

and similarly update `distance: 3` expectations.

Add this assertion to `built_in_css_code_spec_parses_fixed_and_parameterized_ids` after the `toric:d=3` assertion:

```rust
assert_eq!(
    parse_built_in_css_code_spec("bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"),
    Ok(BuiltInCssCodeSpec::Family {
        family: BuiltInCssFamily::BivariateBicycle,
        params: BuiltInCssParams::BivariateBicycle(bb144_bivariate_bicycle_params()),
    })
);
```

Add this new test after `built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers`:

```rust
#[test]
fn built_in_css_code_spec_rejects_bad_bivariate_bicycle_params() {
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=12,ly=6,b=0:3|1:0|2:0"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "a".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=0,ly=6,a=3:0,b=0:3"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "bb".to_owned(),
            parameter: "lx".to_owned(),
            value: 0,
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=12,lx=6,ly=6,a=3:0,b=0:3"),
        Err(QecError::DuplicateBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "lx".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=12,ly=6,a=3:0,b=0:3,foo=1"),
        Err(QecError::UnexpectedBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "foo".to_owned(),
        })
    );
    assert!(parse_built_in_css_code_spec("bb:lx=12,ly=6,a=3,b=0:3").is_err());
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=6,ly=6,a=0:0|6:0,b=0:3"),
        Err(QecError::DuplicateBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "a_terms".to_owned(),
        })
    );
}
```

- [ ] **Step 2: Run the filtered test to verify RED**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec --offline
```

Expected: FAIL because `BuiltInCssParams::Distance`, `BuiltInCssParams::BivariateBicycle`, and `BuiltInCssFamily::BivariateBicycle` do not exist yet.

- [ ] **Step 3: Update the family parameter model**

In `qec-code/src/codes/built_in_css.rs`, replace:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInCssParams {
    pub distance: usize,
}
```

with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltInCssParams {
    Distance { distance: usize },
    BivariateBicycle(BivariateBicycleParams),
}
```

Add the bivariate-bicycle family variant:

```rust
pub enum BuiltInCssFamily {
    RepetitionX,
    RepetitionZ,
    SurfaceRotated,
    Toric,
    BivariateBicycle,
}
```

- [ ] **Step 4: Add bivariate-bicycle parser helpers**

In `qec-code/src/codes/built_in_css.rs`, update `parse_built_in_css_code_spec(...)` so the no-colon `bb` input is treated as a missing `lx` parameter:

```rust
"bb" => Err(QecError::MissingBuiltInCssParameter {
    family: input.to_owned(),
    parameter: "lx".to_owned(),
}),
```

Replace `parse_built_in_css_family_spec(...)` with this dispatch shape:

```rust
fn parse_built_in_css_family_spec(
    family_name: &str,
    params_text: &str,
) -> Result<BuiltInCssCodeSpec> {
    match family_name {
        "repetition_x" => parse_distance_family_spec(
            family_name,
            BuiltInCssFamily::RepetitionX,
            params_text,
        ),
        "repetition_z" => parse_distance_family_spec(
            family_name,
            BuiltInCssFamily::RepetitionZ,
            params_text,
        ),
        "surface_rotated" => parse_distance_family_spec(
            family_name,
            BuiltInCssFamily::SurfaceRotated,
            params_text,
        ),
        "toric" => parse_distance_family_spec(family_name, BuiltInCssFamily::Toric, params_text),
        "bb" => {
            let params = parse_bivariate_bicycle_params(family_name, params_text)?;
            Ok(BuiltInCssCodeSpec::Family {
                family: BuiltInCssFamily::BivariateBicycle,
                params: BuiltInCssParams::BivariateBicycle(params),
            })
        }
        _ => Err(QecError::UnknownBuiltInCssFamily {
            family: family_name.to_owned(),
        }),
    }
}

fn parse_distance_family_spec(
    family_name: &str,
    family: BuiltInCssFamily,
    params_text: &str,
) -> Result<BuiltInCssCodeSpec> {
    let distance = parse_repetition_distance(family_name, params_text)?;

    Ok(BuiltInCssCodeSpec::Family {
        family,
        params: BuiltInCssParams::Distance { distance },
    })
}
```

Add these helpers after `parse_repetition_distance(...)`:

```rust
fn parse_bivariate_bicycle_params(
    family_name: &str,
    params_text: &str,
) -> Result<BivariateBicycleParams> {
    if params_text.is_empty() {
        return Err(QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "lx".to_owned(),
        });
    }

    let mut lx = None;
    let mut ly = None;
    let mut a_terms = None;
    let mut b_terms = None;

    for pair in params_text.split(',') {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(QecError::UnexpectedBuiltInCssParameter {
                family: family_name.to_owned(),
                parameter: pair.to_owned(),
            });
        };

        match key {
            "lx" => parse_unique_positive_usize_param(family_name, "lx", value, &mut lx)?,
            "ly" => parse_unique_positive_usize_param(family_name, "ly", value, &mut ly)?,
            "a" => parse_unique_shift_terms_param(family_name, "a", value, &mut a_terms)?,
            "b" => parse_unique_shift_terms_param(family_name, "b", value, &mut b_terms)?,
            _ => {
                return Err(QecError::UnexpectedBuiltInCssParameter {
                    family: family_name.to_owned(),
                    parameter: key.to_owned(),
                });
            }
        }
    }

    let params = BivariateBicycleParams {
        lx: lx.ok_or_else(|| QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "lx".to_owned(),
        })?,
        ly: ly.ok_or_else(|| QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "ly".to_owned(),
        })?,
        a_terms: a_terms.ok_or_else(|| QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "a".to_owned(),
        })?,
        b_terms: b_terms.ok_or_else(|| QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "b".to_owned(),
        })?,
    };

    validate_bivariate_bicycle_params(&params)?;
    Ok(params)
}

fn parse_unique_positive_usize_param(
    family_name: &str,
    parameter: &'static str,
    value: &str,
    slot: &mut Option<usize>,
) -> Result<()> {
    if slot.is_some() {
        return Err(QecError::DuplicateBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
        });
    }

    let parsed =
        value
            .parse::<usize>()
            .map_err(|_| QecError::InvalidBuiltInCssIntegerParameter {
                family: family_name.to_owned(),
                parameter: parameter.to_owned(),
                value: value.to_owned(),
            })?;

    if parsed == 0 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
            value: parsed,
        });
    }

    *slot = Some(parsed);
    Ok(())
}

fn parse_unique_shift_terms_param(
    family_name: &str,
    parameter: &'static str,
    value: &str,
    slot: &mut Option<Vec<(usize, usize)>>,
) -> Result<()> {
    if slot.is_some() {
        return Err(QecError::DuplicateBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
        });
    }

    let terms = parse_shift_terms(family_name, parameter, value)?;
    *slot = Some(terms);
    Ok(())
}

fn parse_shift_terms(
    family_name: &str,
    parameter: &'static str,
    value: &str,
) -> Result<Vec<(usize, usize)>> {
    if value.is_empty() {
        return Err(QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
        });
    }

    value
        .split('|')
        .map(|term| parse_shift_term(family_name, parameter, term))
        .collect()
}

fn parse_shift_term(
    family_name: &str,
    parameter: &'static str,
    term: &str,
) -> Result<(usize, usize)> {
    let Some((dx, dy)) = term.split_once(':') else {
        return Err(QecError::InvalidBuiltInCssIntegerParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
            value: term.to_owned(),
        });
    };

    let dx = dx
        .parse::<usize>()
        .map_err(|_| QecError::InvalidBuiltInCssIntegerParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
            value: term.to_owned(),
        })?;
    let dy = dy
        .parse::<usize>()
        .map_err(|_| QecError::InvalidBuiltInCssIntegerParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
            value: term.to_owned(),
        })?;

    Ok((dx, dy))
}
```

- [ ] **Step 5: Keep parsed `bb:...` matrix generation out of scope**

In `built_in_css_checks(...)`, replace the family match arm with this shape:

```rust
BuiltInCssCodeSpec::Family {
    family: BuiltInCssFamily::BivariateBicycle,
    ..
} => Err(QecError::UnknownBuiltInCssCode {
    code_id: code_id.to_owned(),
}),
BuiltInCssCodeSpec::Family { family, params } => family_css_checks(family, params),
```

Replace `fn family_css_checks(family: BuiltInCssFamily, distance: usize)` with:

```rust
fn family_css_checks(family: BuiltInCssFamily, params: BuiltInCssParams) -> Result<BuiltInCssChecks> {
    let BuiltInCssParams::Distance { distance } = params else {
        unreachable!("parser produced incompatible built-in CSS family parameters");
    };

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
        BuiltInCssFamily::BivariateBicycle => {
            unreachable!("parsed bb specs are rejected before family matrix generation")
        }
    }
}
```

- [ ] **Step 6: Run the filtered test to verify GREEN**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec --offline
```

Expected: PASS with the existing distance-family parser tests and the new bivariate-bicycle parser tests.

- [ ] **Step 7: Run the requested non-offline command and final verification**

Run:

```bash
cargo test -p qec-code --test code built_in_css_code_spec
```

Expected in a network-enabled environment: PASS. If the sandbox blocks crates.io access, record the environment failure and rely on the identical `--offline` command above for parser verification.

Run:

```bash
cargo test --offline
```

Expected: PASS for the workspace test suite available from the local Cargo cache.

- [ ] **Step 8: Commit**

Run:

```bash
git add docs/superpowers/specs/2026-06-23-issue-128-bivariate-bicycle-css-parser-design.md docs/superpowers/plans/2026-06-23-issue-128-bivariate-bicycle-css-parser.md qec-code/src/codes/built_in_css.rs qec-code/tests/code.rs
git commit -m "feat: parse bivariate-bicycle css specs"
```

