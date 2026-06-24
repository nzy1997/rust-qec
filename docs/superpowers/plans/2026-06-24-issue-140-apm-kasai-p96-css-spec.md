# Issue 140 APM Kasai P=96 CSS Spec Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the verified Table A1 P=96 APM CSS instance as built-in CSS spec `apm_kasai:p=96`.

**Architecture:** Extend the existing built-in CSS parser/catalog in `qec-code/src/codes/built_in_css.rs` with one APM Kasai family and a P=96 builder path that delegates to `qec-code/src/codes/apm.rs`. Keep unsupported P values explicit and leave P=192 out of the catalog and build path.

**Tech Stack:** Rust, existing `qec-code` crate, existing CLI integration tests in `qec-code/tests/cli.rs`.

## Global Constraints

- Built-in spec string: `apm_kasai:p=96`.
- `apm_kasai:p=192` must not be listed or accepted in this issue; it is tracked by #143.
- Unsupported P values must fail with an error naming the unsupported value and supported value P=96.
- `cargo run -p qec-code -- code css apm_kasai:p=96 hx|hz` must emit sparse-row JSON with `num_cols = 1152`.
- Do not add rsinter benchmark fixtures or decoding smoke tests.

---

### Task 1: Register APM Kasai P=96 Built-In CSS Spec

**Files:**
- Modify: `qec-code/tests/cli.rs`
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/src/error.rs`
- Modify: `qec-code/src/codes/built_in_css.rs`

**Interfaces:**
- Consumes: `qec_code::codes::built_in_css::built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks>`.
- Produces: `built_in_css_checks("apm_kasai:p=96")` returning `BuiltInCssChecks { code_id: "apm_kasai:p=96", num_cols: 1152, .. }`.
- Produces: catalog entry `BuiltInCssCatalogEntry { spec: "apm_kasai:p=96", description: "fixed Table A1 P=96 APM-CSS code" }`.

- [ ] **Step 1: Write the failing CLI regression test**

Add this test to `qec-code/tests/cli.rs` near the existing `code_css_list_includes_supported_built_ins` and sparse-row export tests:

```rust
#[test]
fn apm_kasai_css_export() {
    let list = run_qec_code(&["code", "css", "list"]);
    assert!(list.status.success());
    assert_eq!(list.stderr, b"");

    let list_stdout = String::from_utf8(list.stdout).expect("stdout should be valid utf-8");
    assert!(
        list_stdout.contains("apm_kasai:p=96"),
        "stdout was: {list_stdout}"
    );
    assert!(
        !list_stdout.contains("apm_kasai:p=192"),
        "stdout was: {list_stdout}"
    );

    for matrix in ["hx", "hz"] {
        let output = run_qec_code(&["code", "css", "apm_kasai:p=96", matrix]);
        assert!(output.status.success(), "{matrix} stderr: {}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stderr, b"");

        let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("stdout should be sparse-row JSON");
        assert_eq!(json["format"], "sparse_rows");
        assert_eq!(json["num_cols"], 1152);
        assert!(
            json["rows"].as_array().is_some_and(|rows| !rows.is_empty()),
            "rows should be non-empty: {json}"
        );
    }

    let p128 = run_qec_code(&["code", "css", "apm_kasai:p=128", "hx"]);
    assert!(!p128.status.success());
    assert_eq!(p128.stdout, b"");
    let p128_stderr = String::from_utf8(p128.stderr).expect("stderr should be valid utf-8");
    assert!(
        p128_stderr.contains("unsupported built-in CSS integer parameter p for family apm_kasai: 128"),
        "stderr was: {p128_stderr}"
    );
    assert!(p128_stderr.contains("supported: 96"), "stderr was: {p128_stderr}");

    let p192 = run_qec_code(&["code", "css", "apm_kasai:p=192", "hx"]);
    assert!(!p192.status.success());
    assert_eq!(p192.stdout, b"");
    let p192_stderr = String::from_utf8(p192.stderr).expect("stderr should be valid utf-8");
    assert!(
        p192_stderr.contains("unsupported built-in CSS integer parameter p for family apm_kasai: 192"),
        "stderr was: {p192_stderr}"
    );
    assert!(p192_stderr.contains("#143"), "stderr was: {p192_stderr}");
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p qec-code --test cli apm_kasai_css_export -q`

Expected: FAIL because the catalog does not list `apm_kasai:p=96` yet and/or the parser rejects `apm_kasai:p=96`.

- [ ] **Step 3: Add registry errors**

In `qec-code/src/error.rs`, add these variants after `OutOfRangeBuiltInCssIntegerParameter`:

```rust
    #[error(
        "unsupported built-in CSS integer parameter {parameter} for family {family}: {value} (supported: {supported}; {note})"
    )]
    UnsupportedBuiltInCssIntegerParameter {
        family: String,
        parameter: String,
        value: usize,
        supported: String,
        note: String,
    },
    #[error("failed to build built-in CSS code {code_id}: {reason}")]
    BuiltInCssBuildFailed { code_id: String, reason: String },
```

- [ ] **Step 4: Register parser/catalog/build support**

In `qec-code/src/codes/built_in_css.rs`, import the existing APM builder types:

```rust
use super::apm::{AffinePermutation, ApmCssBuildError, ApmCssManifestEntry, build_apm_css_checks};
```

Add `ApmKasai` to `BuiltInCssFamily` and `ApmKasai { p: usize }` to `BuiltInCssParams`.

Add this catalog entry after `bb72`:

```rust
    BuiltInCssCatalogEntry {
        spec: "apm_kasai:p=96",
        description: "fixed Table A1 P=96 APM-CSS code",
    },
```

Teach `parse_built_in_css_code_spec` that bare `apm_kasai` is missing parameter `p`, and teach `parse_built_in_css_family_spec` to call `parse_apm_kasai_params`.

Add parser/build helpers using the Table A1 P=96 constants:

```rust
const APM_KASAI_SUPPORTED_P: usize = 96;
const APM_KASAI_P96_CODE_ID: &str = "apm_kasai:p=96";
const APM_KASAI_P96_J: u64 = 3;
const APM_KASAI_P96_L: u64 = 12;
const APM_KASAI_P96_F: &[(u64, u64)] = &[(5, 41), (85, 77), (73, 66), (1, 0), (1, 72), (37, 9)];
const APM_KASAI_P96_G: &[(u64, u64)] = &[(61, 15), (1, 24), (89, 62), (25, 22), (85, 93), (25, 78)];

fn parse_apm_kasai_params(family_name: &str, params_text: &str) -> Result<usize> {
    if params_text.is_empty() {
        return Err(QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "p".to_owned(),
        });
    }

    let mut p = None;
    for pair in params_text.split(',') {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(QecError::UnexpectedBuiltInCssParameter {
                family: family_name.to_owned(),
                parameter: pair.to_owned(),
            });
        };

        match key {
            "p" => {
                if p.is_some() {
                    return Err(QecError::DuplicateBuiltInCssParameter {
                        family: family_name.to_owned(),
                        parameter: "p".to_owned(),
                    });
                }
                p = Some(value.parse::<usize>().map_err(|_| {
                    QecError::InvalidBuiltInCssIntegerParameter {
                        family: family_name.to_owned(),
                        parameter: "p".to_owned(),
                        value: value.to_owned(),
                    }
                })?);
            }
            _ => {
                return Err(QecError::UnexpectedBuiltInCssParameter {
                    family: family_name.to_owned(),
                    parameter: key.to_owned(),
                });
            }
        }
    }

    p.ok_or_else(|| QecError::MissingBuiltInCssParameter {
        family: family_name.to_owned(),
        parameter: "p".to_owned(),
    })
}

fn apm_kasai_css_checks(p: usize) -> Result<BuiltInCssChecks> {
    if p != APM_KASAI_SUPPORTED_P {
        return Err(QecError::UnsupportedBuiltInCssIntegerParameter {
            family: "apm_kasai".to_owned(),
            parameter: "p".to_owned(),
            value: p,
            supported: APM_KASAI_SUPPORTED_P.to_string(),
            note: "P=192 is tracked by #143".to_owned(),
        });
    }

    let entry = apm_kasai_p96_manifest_entry()?;
    build_apm_css_checks(&entry).map_err(|error| apm_build_error(APM_KASAI_P96_CODE_ID, error))
}

fn apm_kasai_p96_manifest_entry() -> Result<ApmCssManifestEntry> {
    let affine = |slope, offset| {
        AffinePermutation::new(APM_KASAI_SUPPORTED_P as u64, slope, offset)
            .expect("pinned APM Kasai P=96 affine maps must be permutations")
    };
    let f = APM_KASAI_P96_F
        .iter()
        .map(|&(slope, offset)| affine(slope, offset))
        .collect();
    let g = APM_KASAI_P96_G
        .iter()
        .map(|&(slope, offset)| affine(slope, offset))
        .collect();

    ApmCssManifestEntry::new(
        APM_KASAI_P96_CODE_ID,
        APM_KASAI_SUPPORTED_P as u64,
        APM_KASAI_P96_J,
        APM_KASAI_P96_L,
        f,
        g,
    )
    .map_err(|error| apm_build_error(APM_KASAI_P96_CODE_ID, error))
}

fn apm_build_error(code_id: &str, error: ApmCssBuildError) -> QecError {
    QecError::BuiltInCssBuildFailed {
        code_id: code_id.to_owned(),
        reason: error.to_string(),
    }
}
```

Wire `BuiltInCssFamily::ApmKasai` in `family_css_checks`.

- [ ] **Step 5: Update existing unit test expectations**

Update `built_in_css_catalog_lists_supported_specs` in `qec-code/tests/code.rs` so the expected spec list includes `"apm_kasai:p=96"` after `"bb72"` and asserts the description mentions `"P=96"`.

Update `built_in_css_code_spec_parses_fixed_and_parameterized_ids` to assert:

```rust
    assert_eq!(
        parse_built_in_css_code_spec("apm_kasai:p=96"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::ApmKasai,
            params: BuiltInCssParams::ApmKasai { p: 96 },
        })
    );
```

Update `built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers` with:

```rust
    assert_eq!(
        parse_built_in_css_code_spec("apm_kasai"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "apm_kasai".to_owned(),
            parameter: "p".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("apm_kasai:p=nope"),
        Err(QecError::InvalidBuiltInCssIntegerParameter {
            family: "apm_kasai".to_owned(),
            parameter: "p".to_owned(),
            value: "nope".to_owned(),
        })
    );
```

Add a registry unit test asserting `built_in_css_checks("apm_kasai:p=96")` has `code_id = "apm_kasai:p=96"` and `num_cols = 1152`, while `built_in_css_checks("apm_kasai:p=128")` returns `UnsupportedBuiltInCssIntegerParameter`.

Update `run_code_css_list_returns_catalog_without_newline` in `qec-code/tests/cli.rs` to include the new catalog line with the width produced by the longest existing `bb:...` spec.

- [ ] **Step 6: Run focused tests and verify GREEN**

Run: `cargo test -p qec-code --test cli apm_kasai_css_export -q`

Expected: PASS.

Run: `cargo test -p qec-code --test code built_in_css -q`

Expected: PASS for the built-in CSS parser/catalog checks.

- [ ] **Step 7: Run required verification**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add docs/superpowers/specs/2026-06-24-issue-140-apm-kasai-p96-css-spec-design.md \
  docs/superpowers/plans/2026-06-24-issue-140-apm-kasai-p96-css-spec.md \
  qec-code/src/error.rs \
  qec-code/src/codes/built_in_css.rs \
  qec-code/tests/cli.rs \
  qec-code/tests/code.rs
git commit -m "Implement #140: register APM Kasai P=96 CSS spec"
```
