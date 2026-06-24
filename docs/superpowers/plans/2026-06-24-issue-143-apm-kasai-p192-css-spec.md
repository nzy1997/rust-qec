# Issue 143 APM Kasai P=192 CSS Spec Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the Table A1 P=192 APM CSS instance as built-in CSS spec `apm_kasai:p=192` and prove the native generator matches the paper stats.

**Architecture:** Extend the existing APM Kasai registry in `qec-code/src/codes/built_in_css.rs` from the P=96-only dispatch to a two-entry dispatch for P=96 and P=192. Acceptance coverage lives in `qec-code/tests/code.rs` and `qec-code/tests/cli.rs`, using the shared APM verifier instead of committed P=192 sparse-row fixtures.

**Tech Stack:** Rust 2024, existing `qec-code` crate, existing `qec-code/tests/support/apm_verifier.rs`, existing CLI integration harness.

## Global Constraints

- Built-in spec string: `apm_kasai:p=192`.
- P=192 paper shape: `n=2304`, `mx=576`, `mz=576`, `k=1156`.
- P=192 regular structure: Hx and Hz row weights are exactly 12; Hx and Hz column weights are exactly 3.
- `qec-code code css list` must include both `apm_kasai:p=96` and `apm_kasai:p=192`.
- `qec-code code css apm_kasai:p=192 hx|hz` must emit sparse-row JSON with `num_cols = 2304`.
- Unsupported P values must fail with an error naming the unsupported value and supported values `96` and `192`.
- Do not add stochastic decoding, circuit-level simulation, or committed full P=192 sparse-row fixtures.

---

### Task 1: Add P=192 Registry Support and Acceptance Coverage

**Files:**
- Modify: `qec-code/src/codes/built_in_css.rs`
- Modify: `qec-code/tests/code.rs`
- Modify: `qec-code/tests/cli.rs`

**Interfaces:**
- Consumes: `qec_code::codes::built_in_css::built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks>`.
- Consumes: `qec_code::codes::built_in_css::built_in_css_catalog() -> &'static [BuiltInCssCatalogEntry]`.
- Consumes: `verify_apm_css_matrices(hx, hz, expectations) -> Result<ApmCssVerifierReport, String>`.
- Produces: `built_in_css_checks("apm_kasai:p=192")` returning `BuiltInCssChecks { code_id: "apm_kasai:p=192", num_cols: 2304, hx: 576 rows, hz: 576 rows }`.
- Produces: catalog entry `BuiltInCssCatalogEntry { spec: "apm_kasai:p=192", description: "fixed Table A1 P=192 APM-CSS code" }`.
- Produces: `built_in_css_checks("apm_kasai:p=128")` error with `supported: "96, 192"` and an unsupported-value message naming `128`.

- [ ] **Step 1: Write the failing P=192 acceptance test**

In `qec-code/tests/code.rs`, update the import from `qec_code::codes::built_in_css` if needed so the test can call `built_in_css_catalog`, `built_in_css_checks`, and `BuiltInCssChecks`.

Add these helpers near the existing `apm_p96_expectations` helper:

```rust
fn apm_p192_expectations() -> ApmCssVerifierExpectations {
    ApmCssVerifierExpectations {
        num_cols: Some(2304),
        mx: Some(576),
        mz: Some(576),
        row_weight_x: Some(12),
        row_weight_z: Some(12),
        column_weight_x: Some(3),
        column_weight_z: Some(3),
        k: Some(1156),
        orthogonal: Some(true),
        girth_lower_bound: Some(6),
    }
}

fn verify_apm_checks(
    checks: &BuiltInCssChecks,
    expectations: &ApmCssVerifierExpectations,
) -> std::result::Result<ApmCssVerifierReport, String> {
    verify_apm_css_matrices(
        ApmSparseMatrixView {
            name: "Hx",
            num_cols: checks.num_cols,
            rows: &checks.hx,
        },
        ApmSparseMatrixView {
            name: "Hz",
            num_cols: checks.num_cols,
            rows: &checks.hz,
        },
        expectations,
    )
}
```

Add this test near `apm_kasai_p96_matches_expected_checks_and_rejects_other_p_values`:

```rust
#[test]
fn apm_p192_builds_paper_stats() {
    let catalog = built_in_css_catalog();
    assert!(
        catalog.iter().any(|entry| entry.spec == "apm_kasai:p=192"),
        "catalog should expose apm_kasai:p=192: {catalog:?}"
    );

    let checks = built_in_css_checks("apm_kasai:p=192").unwrap();
    assert_eq!(checks.code_id, "apm_kasai:p=192");
    assert_eq!(checks.num_cols, 2304);
    assert_eq!(checks.hx.len(), 576);
    assert_eq!(checks.hz.len(), 576);
    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
    assert_rows_in_range(&checks.hx, checks.num_cols);
    assert_rows_in_range(&checks.hz, checks.num_cols);

    let report = verify_apm_checks(&checks, &apm_p192_expectations()).unwrap();
    assert!(report.orthogonal);
    assert_eq!(report.num_cols, 2304);
    assert_eq!(report.mx, 576);
    assert_eq!(report.mz, 576);
    assert_eq!(report.k, 1156);
    assert_eq!(report.rank_x + report.rank_z, 1148);
    assert_eq!(
        report.x.row_weight,
        WeightStats {
            min: 12,
            average: 12.0,
            max: 12
        }
    );
    assert_eq!(
        report.z.row_weight,
        WeightStats {
            min: 12,
            average: 12.0,
            max: 12
        }
    );
    assert_eq!(
        report.x.column_weight,
        WeightStats {
            min: 3,
            average: 3.0,
            max: 3
        }
    );
    assert_eq!(
        report.z.column_weight,
        WeightStats {
            min: 3,
            average: 3.0,
            max: 3
        }
    );
    assert!(report.x.girth.meets_lower_bound(6));
    assert!(report.z.girth.meets_lower_bound(6));

    let mut mutated = apm_kasai_p192_checks_with_one_p96_coefficient();
    let err = verify_apm_checks(&mutated, &apm_p192_expectations()).unwrap_err();
    assert!(
        err.contains("expected orthogonal=true")
            || err.contains("expected k=1156")
            || err.contains("row weight")
            || err.contains("column weight"),
        "mutated P=192 coefficient should fail structural verifier, got: {err}"
    );

    let unsupported = built_in_css_checks("apm_kasai:p=128").unwrap_err();
    let message = unsupported.to_string();
    assert!(
        message.contains("unsupported built-in CSS integer parameter p for family apm_kasai: 128"),
        "{message}"
    );
    assert!(message.contains("supported: 96, 192"), "{message}");
}
```

Add a temporary local helper below the test. It can initially call `built_in_css_checks("apm_kasai:p=192").unwrap()` and then flip one support in `hz` so the RED test compiles before production support exists:

```rust
fn apm_kasai_p192_checks_with_one_p96_coefficient() -> BuiltInCssChecks {
    let mut checks = built_in_css_checks("apm_kasai:p=192").unwrap();
    let replacement = (0..checks.num_cols)
        .find(|candidate| !checks.hz[0].contains(candidate))
        .unwrap();
    checks.hz[0][0] = replacement;
    checks.hz[0].sort_unstable();
    checks
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p qec-code apm_p192_builds_paper_stats -q`

Expected: FAIL because `apm_kasai:p=192` is not listed in the catalog and is rejected by the current registry. If the temporary mutation helper causes a later assertion failure after registry support is added, replace it in Step 4 with a true coefficient-level helper.

- [ ] **Step 3: Add P=192 production registry support**

In `qec-code/src/codes/built_in_css.rs`, replace the single supported-P constant with a supported-values string and add P=192 constants:

```rust
const APM_KASAI_SUPPORTED_P_VALUES: &str = "96, 192";
const APM_KASAI_P96_CODE_ID: &str = "apm_kasai:p=96";
const APM_KASAI_P192_CODE_ID: &str = "apm_kasai:p=192";
const APM_KASAI_P96_P: u64 = 96;
const APM_KASAI_P192_P: u64 = 192;
const APM_KASAI_J: u64 = 3;
const APM_KASAI_L: u64 = 12;
const APM_KASAI_P96_F: &[(u64, u64)] = &[(5, 41), (85, 77), (73, 66), (1, 0), (1, 72), (37, 9)];
const APM_KASAI_P96_G: &[(u64, u64)] = &[(61, 15), (1, 24), (89, 62), (25, 22), (85, 93), (25, 78)];
const APM_KASAI_P192_F: &[(u64, u64)] = &[
    (71, 127),
    (97, 80),
    (67, 117),
    (163, 165),
    (25, 60),
    (187, 33),
];
const APM_KASAI_P192_G: &[(u64, u64)] = &[
    (163, 165),
    (55, 183),
    (167, 79),
    (139, 41),
    (109, 78),
    (31, 27),
];
```

Add this catalog entry after the P=96 entry:

```rust
    BuiltInCssCatalogEntry {
        spec: "apm_kasai:p=192",
        description: "fixed Table A1 P=192 APM-CSS code",
    },
```

Replace `apm_kasai_css_checks` and the P=96 manifest helper with dispatch helpers:

```rust
fn apm_kasai_css_checks(p: usize) -> Result<BuiltInCssChecks> {
    let entry = match p {
        96 => apm_kasai_manifest_entry(
            APM_KASAI_P96_CODE_ID,
            APM_KASAI_P96_P,
            APM_KASAI_P96_F,
            APM_KASAI_P96_G,
        ),
        192 => apm_kasai_manifest_entry(
            APM_KASAI_P192_CODE_ID,
            APM_KASAI_P192_P,
            APM_KASAI_P192_F,
            APM_KASAI_P192_G,
        ),
        _ => {
            return Err(QecError::UnsupportedBuiltInCssIntegerParameter {
                family: "apm_kasai".to_owned(),
                parameter: "p".to_owned(),
                value: p,
                supported: APM_KASAI_SUPPORTED_P_VALUES.to_owned(),
                note: "available Table A1 APM-CSS instances".to_owned(),
            });
        }
    };

    Ok(build_apm_css_checks(&entry).expect("pinned APM Kasai manifest must build"))
}

fn apm_kasai_manifest_entry(
    code_id: &'static str,
    p: u64,
    f_params: &[(u64, u64)],
    g_params: &[(u64, u64)],
) -> ApmCssManifestEntry {
    let affine = |slope, offset| {
        AffinePermutation::new(p, slope, offset)
            .expect("pinned APM Kasai affine maps must be permutations")
    };
    let f = f_params
        .iter()
        .map(|&(slope, offset)| affine(slope, offset))
        .collect();
    let g = g_params
        .iter()
        .map(|&(slope, offset)| affine(slope, offset))
        .collect();

    ApmCssManifestEntry::new(code_id, p, APM_KASAI_J, APM_KASAI_L, f, g)
        .expect("pinned APM Kasai manifest must satisfy invariants")
}
```

Update `built_in_css_catalog_lists_supported_specs` so the expected spec list includes `apm_kasai:p=192` immediately after `apm_kasai:p=96`, and add a description assertion that P=192 is described.

Update `apm_kasai_p96_matches_expected_checks_and_rejects_other_p_values` so the P=128 expected error uses `supported: "96, 192"` and `note: "available Table A1 APM-CSS instances"`.

- [ ] **Step 4: Replace the temporary mutation helper with a coefficient-level negative control**

In `qec-code/tests/code.rs`, replace `apm_kasai_p192_checks_with_one_p96_coefficient` with a helper that builds P=192 directly from affine coefficients and changes one P=192 coefficient to the corresponding P=96 coefficient:

```rust
fn apm_kasai_p192_checks_with_one_p96_coefficient() -> BuiltInCssChecks {
    use qec_code::codes::apm::{AffinePermutation, ApmCssManifestEntry, build_apm_css_checks};

    let p = 192;
    let f = [
        (5, 127),
        (97, 80),
        (67, 117),
        (163, 165),
        (25, 60),
        (187, 33),
    ]
    .into_iter()
    .map(|(slope, offset)| AffinePermutation::new(p, slope, offset).unwrap())
    .collect::<Vec<_>>();
    let g = [
        (163, 165),
        (55, 183),
        (167, 79),
        (139, 41),
        (109, 78),
        (31, 27),
    ]
    .into_iter()
    .map(|(slope, offset)| AffinePermutation::new(p, slope, offset).unwrap())
    .collect::<Vec<_>>();

    let entry = ApmCssManifestEntry::new("apm_kasai:p=192", p, 3, 12, f, g).unwrap();
    build_apm_css_checks(&entry).unwrap()
}
```

If the `qec_code::codes::apm` module is not public to integration tests, do not make it public. Instead move this coefficient-level negative control into `qec-code/src/codes/apm.rs` where the private builder is already tested, and keep the integration test's structural mutation as an additional verifier negative control. The final implementation must still include one in-memory coefficient mutation somewhere in P=192 acceptance coverage.

- [ ] **Step 5: Update CLI coverage**

In `qec-code/tests/cli.rs`, update `apm_kasai_css_export`:

```rust
assert!(
    list_stdout.contains("apm_kasai:p=192"),
    "stdout was: {list_stdout}"
);
```

Change the export loop to cover both P values:

```rust
for (code_id, expected_num_cols) in [("apm_kasai:p=96", 1152), ("apm_kasai:p=192", 2304)] {
    for matrix in ["hx", "hz"] {
        let output = run_qec_code(&["code", "css", code_id, matrix]);
        assert!(output.status.success());
        assert_eq!(output.stderr, b"");

        let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("stdout should be sparse-row JSON");
        assert_eq!(json["format"], "sparse_rows");
        assert_eq!(json["num_cols"], expected_num_cols);
        assert!(
            json["rows"].as_array().is_some_and(|rows| !rows.is_empty()),
            "rows should be non-empty: {json}"
        );
    }
}
```

Remove the old `apm_kasai:p=192` unsupported assertion and update the P=128 supported-values assertion:

```rust
assert!(
    p128_stderr.contains("supported: 96, 192"),
    "stderr was: {p128_stderr}"
);
```

Update the formatted expected text in `code_css_list_output_matches_catalog_width` so it includes the P=192 row with the aligned width produced by the catalog.

- [ ] **Step 6: Run focused tests and fix only scoped failures**

Run:

```sh
cargo test -p qec-code apm_p192_builds_paper_stats -q
cargo test -p qec-code --test cli apm_kasai_css_export -q
cargo test -p qec-code --test code built_in_css_catalog_lists_supported_specs -q
cargo test -p qec-code --test cli code_css_list_output_matches_catalog_width -q
```

Expected: all four commands pass.

- [ ] **Step 7: Run full verification**

Run:

```sh
cargo test
```

Expected: the workspace test suite passes.

- [ ] **Step 8: Commit**

Stage the implementation files and docs:

```sh
git add docs/superpowers/specs/2026-06-24-issue-143-apm-kasai-p192-css-spec-design.md \
  docs/superpowers/plans/2026-06-24-issue-143-apm-kasai-p192-css-spec.md \
  qec-code/src/codes/built_in_css.rs \
  qec-code/tests/code.rs \
  qec-code/tests/cli.rs
git commit -m "feat: register apm kasai p192 css spec"
```
