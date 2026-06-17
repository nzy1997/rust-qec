# Issue 102 Explicit CSS Observables Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `rstim`/`rsinter` general CSS memory benchmarks accept only validated selected-basis logical observables and record any-logical result semantics for `k > 1` runs.

**Architecture:** Keep the current JSON and TOML interfaces from PR #51. Add selected-basis logical validation inside `rstim::codegen::css`, where `hx`, `hz`, `basis`, and explicit observable supports are already available. Keep decoder behavior unchanged and make `rsinter` result rows explicitly report observable source, basis, count, and any-logical failure aggregation.

**Tech Stack:** Rust 2024 workspace, existing `qec-code::binary` GF(2) helpers, existing `qec-code::css::CssCode`, `rstim::codegen::css`, `rsinter` benchmark runner, `serde_json`, `toml`, `clap` CLI tests, fixture JSON/TOML.

## Global Constraints

- This work is independent of issue #101 / PR #104.
- Keep the existing CSS observable file wrapper shape: `{"format":"sparse_rows","num_cols":N,"rows":[...]}` and existing dense wrapper support.
- `basis = "x"` interprets explicit rows as X-like logical supports.
- `basis = "z"` interprets explicit rows as Z-like logical supports.
- Explicit rows must define exactly `k` independent logical classes modulo the selected-basis stabilizer span.
- Invalid explicit observables must fail before DEM generation, decoder compilation, sampling, or completed result rows.
- `rsinter` logical failure aggregation remains any-logical per shot.
- Do not add per-logical failure-rate columns in this issue.
- Do not introduce a versioned `logical_x` / `logical_z` observable schema in this issue.
- Do not add a new public `qec-code` API for this milestone; reuse `qec_code::binary::try_in_row_span` and `try_binary_rank`.
- Preserve existing legacy surface-code benchmark behavior.

---

## File Structure

- Modify `rstim/src/codegen/css.rs`: selected-basis explicit logical validation, new error variants, shared `CssCode` construction for explicit and canonical observable paths.
- Modify `rstim/tests/css_codegen.rs`: update existing explicit-observable helpers to use real logicals and add validation tests for X/Z, stabilizer rows, non-normalizer rows, rank/count mismatch, and BB72 `k = 12`.
- Modify `rstim/tests/cli_gen.rs`: CLI invalid-observable no-overwrite regression.
- Modify `rsinter/src/bench/circuit_source.rs`: add observable-source/basis/aggregation params for CSS points.
- Modify `rsinter/src/bench/runners/mod.rs`: record `logical_observable_count` alongside `num_obs` once the DEM is known.
- Modify `rsinter/tests/bench_circuit_source.rs`: canonical fallback metadata test.
- Modify `rsinter/tests/bench_run.rs`: explicit metadata assertions, BB72 explicit observable smoke, invalid observable pre-result failure.
- Create `rsinter/tests/fixtures/css/bb72_hx.json`: BB72 sparse-row X-check fixture copied from `qec-code`.
- Create `rsinter/tests/fixtures/css/bb72_hz.json`: BB72 sparse-row Z-check fixture copied from `qec-code`.
- Create `rsinter/tests/fixtures/css/bb72_logicals_x.json`: pinned 12-row X-like logical observable fixture.
- Create `rsinter/tests/fixtures/bench/minimal_bb72_css_decoder.toml`: tiny `rmatching` CSS benchmark fixture.
- Modify `rstim/doc/cli.md`: document selected-basis observable semantics.

## Task 1: Selected-Basis Logical Validation In `rstim::codegen::css`

**Files:**
- Modify: `rstim/src/codegen/css.rs`
- Modify: `rstim/tests/css_codegen.rs`

**Interfaces:**
- Consumes: `qec_code::binary::{try_binary_rank, try_in_row_span}`, `qec_code::css::CssCode`, existing `CssMemoryConfig`, existing `CssObservableSource`.
- Produces:
  - `fn css_code_from_checks(checks: &CssCheckMatrices) -> Result<CssCode, CssCodegenError>`
  - `fn resolve_observables(config: &CssMemoryConfig, css_code: &CssCode) -> Result<Vec<Vec<usize>>, CssCodegenError>`
  - `fn validate_explicit_logical_observables(rows: &[Vec<usize>], config: &CssMemoryConfig, logical_count: usize) -> Result<(), CssCodegenError>`
  - New `CssCodegenError` variants:
    - `LogicalObservableNotInNormalizer { row: usize, basis: MemoryBasis, check_matrix: &'static str, check_row: usize }`
    - `LogicalObservableIsStabilizer { row: usize, basis: MemoryBasis }`
    - `LogicalObservableCountMismatch { basis: MemoryBasis, count: usize, expected: usize }`
    - `LogicalObservableRankMismatch { basis: MemoryBasis, rank: usize, expected: usize }`

- [ ] **Step 1: Update existing CSS codegen tests that currently use stabilizers as observables**

In `rstim/tests/css_codegen.rs`, change `repetition_like_css_config` so the explicit observable is a logical support, not the X-check stabilizer:

```rust
fn repetition_like_css_config(rounds: usize, basis: MemoryBasis) -> CssMemoryConfig {
    CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0, 1]],
            hz: vec![],
            num_data_qubits: 2,
        },
        rounds,
        noise: NoiseParams::none(),
        basis,
        schedule: CssSchedule::Sequential,
        observables: CssObservableSource::Explicit(vec![vec![0]]),
    }
}
```

In `sequential_css_memory_z_emits_detectors_observable_and_dem`, change the explicit observable to `vec![vec![0]]`:

```rust
observables: CssObservableSource::Explicit(vec![vec![0]]),
```

In `explicit_observables_allow_redundant_orthogonal_checks`, change the explicit observable to `vec![vec![0]]`:

```rust
observables: CssObservableSource::Explicit(vec![vec![0]]),
```

In `explicit_or_canonical_prefers_explicit_observables`, change the explicit Steane support and expected output:

```rust
observables: CssObservableSource::ExplicitOrCanonical(vec![vec![0, 1, 3]]),
```

```rust
assert!(text.contains("OBSERVABLE_INCLUDE(0) rec[-7] rec[-6] rec[-4]"));
```

- [ ] **Step 2: Add failing selected-basis validation tests**

Append these tests and helpers to `rstim/tests/css_codegen.rs`:

```rust
fn steane_css_config(basis: MemoryBasis, observables: Vec<Vec<usize>>) -> CssMemoryConfig {
    let h = steane_h();
    CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: h.clone(),
            hz: h,
            num_data_qubits: 7,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::Explicit(observables),
    }
}

#[test]
fn explicit_x_logical_observables_are_validated() {
    let circuit = css_memory(steane_css_config(MemoryBasis::X, vec![vec![0, 1, 3]])).unwrap();

    assert_eq!(stats::num_observables(&circuit), 1);
    ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
}

#[test]
fn explicit_z_logical_observables_are_validated() {
    let circuit = css_memory(steane_css_config(MemoryBasis::Z, vec![vec![0, 1, 3]])).unwrap();

    assert_eq!(stats::num_observables(&circuit), 1);
    ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
}

#[test]
fn explicit_x_observable_must_commute_with_z_checks() {
    let err = css_memory(steane_css_config(MemoryBasis::X, vec![vec![0]]))
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("observable 0 is not an X logical: anticommutes with hz row 0"),
        "error was: {err}"
    );
}

#[test]
fn explicit_x_observable_must_not_be_x_stabilizer() {
    let err = css_memory(steane_css_config(MemoryBasis::X, vec![vec![0, 3, 5, 6]]))
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("observable 0 is an X stabilizer, not a logical"),
        "error was: {err}"
    );
}

#[test]
fn explicit_observable_count_must_match_k() {
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0, 1]],
            hz: vec![],
            num_data_qubits: 3,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::Explicit(vec![vec![0]]),
    };

    let err = css_memory(config).unwrap_err().to_string();

    assert!(
        err.contains("explicit X observables define 1 rows, expected k = 2"),
        "error was: {err}"
    );
}

#[test]
fn explicit_observable_rank_must_match_k() {
    let config = CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: vec![vec![0, 1]],
            hz: vec![],
            num_data_qubits: 3,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::Explicit(vec![vec![0], vec![0]]),
    };

    let err = css_memory(config).unwrap_err().to_string();

    assert!(
        err.contains("explicit X observables define rank 1, expected k = 2"),
        "error was: {err}"
    );
}

fn bb72_logicals_x() -> Vec<Vec<usize>> {
    vec![
        vec![3, 6, 12, 15, 18, 24],
        vec![4, 7, 13, 16, 19, 25],
        vec![5, 8, 14, 17, 20, 26],
        vec![0, 9, 12, 15, 21, 27],
        vec![3, 6, 9, 15, 21, 30],
        vec![4, 7, 10, 16, 22, 31],
        vec![0, 1, 3, 4, 6, 7, 9, 10, 36, 37, 39, 40],
        vec![0, 2, 3, 5, 6, 8, 9, 11, 36, 38, 39, 41],
        vec![0, 1, 2, 5, 6, 7, 8, 9, 12, 14, 37, 39, 42, 44],
        vec![2, 4, 6, 8, 13, 15, 36, 37, 38, 39, 43, 45],
        vec![2, 4, 7, 8, 9, 10, 12, 16, 36, 37, 38, 39, 42, 46],
        vec![0, 1, 4, 5, 6, 7, 8, 11, 13, 17, 36, 38, 43, 47],
    ]
}

#[test]
fn bb72_explicit_x_logicals_build_dem_with_twelve_observables() {
    let (hx, hz) = bb72_checks();
    let circuit = css_memory(CssMemoryConfig {
        checks: CssCheckMatrices {
            hx,
            hz,
            num_data_qubits: 72,
        },
        rounds: 1,
        noise: NoiseParams::none(),
        basis: MemoryBasis::X,
        schedule: CssSchedule::Greedy,
        observables: CssObservableSource::Explicit(bb72_logicals_x()),
    })
    .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();

    assert_eq!(stats::num_observables(&circuit), 12);
    assert_eq!(dem.num_observables(), 12);
}
```

- [ ] **Step 3: Run CSS codegen tests and verify they fail**

Run:

```bash
cargo test -p rstim --test css_codegen -q
```

Expected: FAIL. At least the invalid observable tests should fail because current `css_memory` accepts support rows without logical validation.

- [ ] **Step 4: Import GF(2) helpers and add new error variants**

In `rstim/src/codegen/css.rs`, change the `qec-code` import:

```rust
use qec_code::binary::{try_binary_rank, try_in_row_span};
use qec_code::css::CssCode;
```

Add these variants to `CssCodegenError` after `InvalidObservable`:

```rust
    LogicalObservableNotInNormalizer {
        row: usize,
        basis: MemoryBasis,
        check_matrix: &'static str,
        check_row: usize,
    },
    LogicalObservableIsStabilizer {
        row: usize,
        basis: MemoryBasis,
    },
    LogicalObservableCountMismatch {
        basis: MemoryBasis,
        count: usize,
        expected: usize,
    },
    LogicalObservableRankMismatch {
        basis: MemoryBasis,
        rank: usize,
        expected: usize,
    },
```

Add these `Display` arms after `InvalidObservable`:

```rust
            Self::LogicalObservableNotInNormalizer {
                row,
                basis,
                check_matrix,
                check_row,
            } => write!(
                f,
                "observable {row} is not {} {} logical: anticommutes with {check_matrix} row {check_row}",
                basis_article(*basis),
                basis_label(*basis)
            ),
            Self::LogicalObservableIsStabilizer { row, basis } => write!(
                f,
                "observable {row} is {} {} stabilizer, not a logical",
                basis_article(*basis),
                basis_label(*basis)
            ),
            Self::LogicalObservableCountMismatch {
                basis,
                count,
                expected,
            } => write!(
                f,
                "explicit {} observables define {count} rows, expected k = {expected}",
                basis_label(*basis)
            ),
            Self::LogicalObservableRankMismatch {
                basis,
                rank,
                expected,
            } => write!(
                f,
                "explicit {} observables define rank {rank}, expected k = {expected}",
                basis_label(*basis)
            ),
```

Add these helpers near the other small helpers in `rstim/src/codegen/css.rs`:

```rust
fn basis_label(basis: MemoryBasis) -> &'static str {
    match basis {
        MemoryBasis::X => "X",
        MemoryBasis::Z => "Z",
    }
}

fn basis_article(basis: MemoryBasis) -> &'static str {
    match basis {
        MemoryBasis::X => "an",
        MemoryBasis::Z => "a",
    }
}
```

- [ ] **Step 5: Thread a shared `CssCode` into observable resolution**

Replace `css_memory` in `rstim/src/codegen/css.rs` with:

```rust
pub fn css_memory(config: CssMemoryConfig) -> Result<Vec<StimInstr>, CssCodegenError> {
    if config.rounds == 0 {
        return Err(CssCodegenError::InvalidRounds);
    }
    validate_supports("hx", &config.checks.hx, config.checks.num_data_qubits)?;
    validate_supports("hz", &config.checks.hz, config.checks.num_data_qubits)?;
    validate_css_orthogonality(&config.checks.hx, &config.checks.hz)?;
    let css_code = css_code_from_checks(&config.checks)?;
    let observables = resolve_observables(&config, &css_code)?;
    emit_css_memory_circuit(&config, &observables)
}
```

Add this helper below `css_memory`:

```rust
fn css_code_from_checks(checks: &CssCheckMatrices) -> Result<CssCode, CssCodegenError> {
    let hx_dense = supports_to_dense(&checks.hx, checks.num_data_qubits);
    let hz_dense = supports_to_dense(&checks.hz, checks.num_data_qubits);
    CssCode::from_hx_hz(hx_dense, hz_dense)
        .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))
}
```

- [ ] **Step 6: Validate explicit logical rows**

Replace `resolve_observables` and `canonical_observables` signatures and bodies in `rstim/src/codegen/css.rs` with:

```rust
fn resolve_observables(
    config: &CssMemoryConfig,
    css_code: &CssCode,
) -> Result<Vec<Vec<usize>>, CssCodegenError> {
    match &config.observables {
        CssObservableSource::Explicit(rows) | CssObservableSource::ExplicitOrCanonical(rows)
            if !rows.is_empty() =>
        {
            validate_observables(rows, config.checks.num_data_qubits)?;
            validate_explicit_logical_observables(
                rows,
                config,
                css_code.code().num_logical_qubits(),
            )?;
            Ok(rows.clone())
        }
        CssObservableSource::ExplicitOrCanonical(_) | CssObservableSource::CanonicalFallback => {
            canonical_observables(config, css_code)
        }
        CssObservableSource::Explicit(rows) => {
            validate_observables(rows, config.checks.num_data_qubits)?;
            Ok(rows.clone())
        }
    }
}

fn canonical_observables(
    config: &CssMemoryConfig,
    css_code: &CssCode,
) -> Result<Vec<Vec<usize>>, CssCodegenError> {
    let basis = css_code
        .code()
        .canonical_logical_basis()
        .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?;
    let logicals = match config.basis {
        MemoryBasis::X => basis.logical_x,
        MemoryBasis::Z => basis.logical_z,
    };
    let mut observables = Vec::with_capacity(logicals.len());
    for (index, logical) in logicals.iter().enumerate() {
        let support = match config.basis {
            MemoryBasis::X => {
                if logical.z_bits().iter().any(|&bit| bit != 0) {
                    return Err(CssCodegenError::MixedCanonicalLogical {
                        index,
                        basis: config.basis,
                    });
                }
                logical
                    .x_bits()
                    .iter()
                    .enumerate()
                    .filter_map(|(qubit, &bit)| (bit == 1).then_some(qubit))
                    .collect()
            }
            MemoryBasis::Z => {
                if logical.x_bits().iter().any(|&bit| bit != 0) {
                    return Err(CssCodegenError::MixedCanonicalLogical {
                        index,
                        basis: config.basis,
                    });
                }
                logical
                    .z_bits()
                    .iter()
                    .enumerate()
                    .filter_map(|(qubit, &bit)| (bit == 1).then_some(qubit))
                    .collect()
            }
        };
        observables.push(support);
    }
    validate_observables(&observables, config.checks.num_data_qubits)?;
    Ok(observables)
}
```

Then add these helpers below `canonical_observables`:

```rust
fn validate_explicit_logical_observables(
    rows: &[Vec<usize>],
    config: &CssMemoryConfig,
    logical_count: usize,
) -> Result<(), CssCodegenError> {
    if rows.len() != logical_count {
        return Err(CssCodegenError::LogicalObservableCountMismatch {
            basis: config.basis,
            count: rows.len(),
            expected: logical_count,
        });
    }

    let width = config.checks.num_data_qubits;
    let (stabilizer_rows, opposite_rows, opposite_name) = match config.basis {
        MemoryBasis::X => (&config.checks.hx, &config.checks.hz, "hz"),
        MemoryBasis::Z => (&config.checks.hz, &config.checks.hx, "hx"),
    };
    let stabilizer_dense = supports_to_dense(stabilizer_rows, width);
    let stabilizer_rank = try_binary_rank(&stabilizer_dense)
        .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?;
    let mut augmented = stabilizer_dense.clone();

    for (row_index, row) in rows.iter().enumerate() {
        let dense = support_to_dense(row, width);
        for (check_row, check) in opposite_rows.iter().enumerate() {
            if support_dot(&dense, check) != 0 {
                return Err(CssCodegenError::LogicalObservableNotInNormalizer {
                    row: row_index,
                    basis: config.basis,
                    check_matrix: opposite_name,
                    check_row,
                });
            }
        }
        if try_in_row_span(&stabilizer_dense, &dense)
            .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?
        {
            return Err(CssCodegenError::LogicalObservableIsStabilizer {
                row: row_index,
                basis: config.basis,
            });
        }
        augmented.push(dense);
    }

    let quotient_rank = try_binary_rank(&augmented)
        .map_err(|error| CssCodegenError::InvalidCss(error.to_string()))?
        .saturating_sub(stabilizer_rank);
    if quotient_rank != logical_count {
        return Err(CssCodegenError::LogicalObservableRankMismatch {
            basis: config.basis,
            rank: quotient_rank,
            expected: logical_count,
        });
    }

    Ok(())
}

fn support_to_dense(row: &[usize], width: usize) -> Vec<u8> {
    let mut dense = vec![0; width];
    for &col in row {
        dense[col] = 1;
    }
    dense
}

fn support_dot(dense: &[u8], support: &[usize]) -> u8 {
    support
        .iter()
        .fold(0, |parity, &col| parity ^ dense[col])
}
```

Replace `supports_to_dense` with this version so both dense helpers share the same behavior:

```rust
fn supports_to_dense(rows: &[Vec<usize>], width: usize) -> Vec<Vec<u8>> {
    rows.iter().map(|row| support_to_dense(row, width)).collect()
}
```

- [ ] **Step 7: Run CSS codegen tests**

Run:

```bash
cargo test -p rstim --test css_codegen -q
```

Expected: PASS. This validates existing CSS codegen behavior plus the new logical-observable checks.

- [ ] **Step 8: Commit Task 1**

Run:

```bash
git add rstim/src/codegen/css.rs rstim/tests/css_codegen.rs
git commit -m "feat: validate explicit css logical observables"
```

Expected: commit succeeds with only those two files staged.

## Task 2: CLI Failure Path For Invalid Explicit Observables

**Files:**
- Modify: `rstim/tests/cli_gen.rs`

**Interfaces:**
- Consumes: stricter `css_memory(...)` validation from Task 1 through existing `run_css_gen(...)`.
- Produces: CLI regression coverage that invalid observable semantics fail and preserve existing output files.

- [ ] **Step 1: Add failing CLI no-overwrite test**

Append this test to `rstim/tests/cli_gen.rs`:

```rust
#[test]
fn gen_css_memory_rejects_non_logical_observable_and_preserves_out() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    let obs = dir.path().join("obs.json");
    let out = dir.path().join("out.stim");
    let steane_h = r#"{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}"#;
    std::fs::write(&hx, steane_h).unwrap();
    std::fs::write(&hz, steane_h).unwrap();
    std::fs::write(&obs, r#"{"format":"sparse_rows","num_cols":7,"rows":[[0]]}"#).unwrap();
    std::fs::write(&out, "keep me").unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "1",
            "--observables",
            obs.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("observable 0 is not an X logical"),
        "stderr: {stderr}"
    );
    assert_eq!(std::fs::read_to_string(out).unwrap(), "keep me");
}
```

- [ ] **Step 2: Run the new CLI test**

Run:

```bash
cargo test -p rstim --test cli_gen gen_css_memory_rejects_non_logical_observable_and_preserves_out -q
```

Expected after Task 1: PASS. If run before Task 1, this test should FAIL because the invalid observable is accepted.

- [ ] **Step 3: Run the full CSS CLI generator tests**

Run:

```bash
cargo test -p rstim --test cli_gen -q
```

Expected: PASS.

- [ ] **Step 4: Commit Task 2**

Run:

```bash
git add rstim/tests/cli_gen.rs
git commit -m "test: reject invalid css observables in cli"
```

Expected: commit succeeds with only `rstim/tests/cli_gen.rs` staged.

## Task 3: `rsinter` Observable Semantics Metadata

**Files:**
- Modify: `rsinter/src/bench/circuit_source.rs`
- Modify: `rsinter/src/bench/runners/mod.rs`
- Modify: `rsinter/tests/bench_circuit_source.rs`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: existing `BuiltCircuit { circuit, params, case_summary }` and runner `case_summary_with_progress(...)`.
- Produces:
  - CSS result params: `logical_observable_source`, `logical_observable_basis`, `logical_failure_aggregation`
  - Result case summary: `logical_observable_count`

- [ ] **Step 1: Add failing metadata tests**

In `rsinter/tests/bench_circuit_source.rs`, add this helper below `surface_point`:

```rust
fn css_point(observables_path: Option<&str>) -> BenchCasePoint {
    BenchCasePoint {
        input_type: "css".into(),
        code_id: Some("steane".into()),
        distance: None,
        rounds: 1,
        p: 0.0,
        basis: Some("x".into()),
        schedule: Some("greedy".into()),
        hx_path: Some("../css/steane_hx.json".into()),
        hz_path: Some("../css/steane_hz.json".into()),
        observables_path: observables_path.map(str::to_string),
        max_shots: 0,
        max_errors: 2,
        max_wall_seconds: None,
        batch_size: 4,
        decoder_params: BTreeMap::new(),
    }
}
```

Append this test to `rsinter/tests/bench_circuit_source.rs`:

```rust
#[test]
fn build_circuit_for_css_point_records_canonical_fallback_observable_metadata() {
    let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bench");
    let built = build_circuit_for_point(&css_point(None), &spec_dir).unwrap();

    assert_eq!(
        built.params["logical_observable_source"],
        serde_json::json!("canonical_fallback")
    );
    assert_eq!(
        built.params["logical_observable_basis"],
        serde_json::json!("x")
    );
    assert_eq!(
        built.params["logical_failure_aggregation"],
        serde_json::json!("any_logical")
    );
}
```

In `rsinter/tests/bench_run.rs`, extend `rust_benchmark_run_supports_css_input_type` with these assertions after the existing CSS param assertions:

```rust
    assert_eq!(
        rows[0].params["logical_observable_source"],
        serde_json::json!("explicit")
    );
    assert_eq!(
        rows[0].params["logical_observable_basis"],
        serde_json::json!("x")
    );
    assert_eq!(
        rows[0].params["logical_failure_aggregation"],
        serde_json::json!("any_logical")
    );
    assert_eq!(
        rows[0].case_summary["logical_observable_count"],
        serde_json::json!(1)
    );
```

- [ ] **Step 2: Run the metadata tests and verify they fail**

Run:

```bash
cargo test -p rsinter --test bench_circuit_source build_circuit_for_css_point_records_canonical_fallback_observable_metadata -q
cargo test -p rsinter --test bench_run rust_benchmark_run_supports_css_input_type -q
```

Expected: FAIL because the new metadata keys are not yet written.

- [ ] **Step 3: Add CSS observable params in `build_css`**

In `rsinter/src/bench/circuit_source.rs`, add this helper near `insert_max_wall_seconds`:

```rust
fn memory_basis_label(basis: MemoryBasis) -> &'static str {
    match basis {
        MemoryBasis::X => "x",
        MemoryBasis::Z => "z",
    }
}
```

In `build_css`, after parsing `basis` and `schedule`, add:

```rust
    let basis_label = memory_basis_label(basis);
    let observable_source = if point.observables_path.is_some() {
        "explicit"
    } else {
        "canonical_fallback"
    };
```

Then add these pairs to the `ParamMap::from_pairs([...])` call in `build_css`:

```rust
        (
            "logical_observable_source",
            serde_json::json!(observable_source),
        ),
        (
            "logical_observable_basis",
            serde_json::json!(basis_label),
        ),
        (
            "logical_failure_aggregation",
            serde_json::json!("any_logical"),
        ),
```

Keep the existing `("basis", serde_json::json!(basis_text))` pair unchanged so existing provenance behavior is preserved.

- [ ] **Step 4: Add `logical_observable_count` in runner case summaries**

In `rsinter/src/bench/runners/mod.rs`, update `case_summary_with_progress` to insert `logical_observable_count` after `num_obs`:

```rust
fn case_summary_with_progress(
    mut summary: CaseSummary,
    num_dets: usize,
    num_obs: usize,
    generated_shots: usize,
) -> CaseSummary {
    summary.insert("num_dets".into(), serde_json::json!(num_dets));
    summary.insert("num_obs".into(), serde_json::json!(num_obs));
    summary.insert(
        "logical_observable_count".into(),
        serde_json::json!(num_obs),
    );
    summary.insert(
        "num_shots_generated".into(),
        serde_json::json!(generated_shots),
    );
    summary
}
```

- [ ] **Step 5: Run metadata tests**

Run:

```bash
cargo test -p rsinter --test bench_circuit_source build_circuit_for_css_point_records_canonical_fallback_observable_metadata -q
cargo test -p rsinter --test bench_run rust_benchmark_run_supports_css_input_type -q
```

Expected: PASS.

- [ ] **Step 6: Run focused runner regression tests**

Run:

```bash
cargo test -p rsinter --test bench_circuit_source -q
cargo test -p rsinter --test bench_run -q
```

Expected: PASS.

- [ ] **Step 7: Commit Task 3**

Run:

```bash
git add rsinter/src/bench/circuit_source.rs rsinter/src/bench/runners/mod.rs rsinter/tests/bench_circuit_source.rs rsinter/tests/bench_run.rs
git commit -m "feat: record css observable benchmark semantics"
```

Expected: commit succeeds with only those four files staged.

## Task 4: BB72 Explicit Observable Fixtures And `rsinter` Smoke

**Files:**
- Create: `rsinter/tests/fixtures/css/bb72_hx.json`
- Create: `rsinter/tests/fixtures/css/bb72_hz.json`
- Create: `rsinter/tests/fixtures/css/bb72_logicals_x.json`
- Create: `rsinter/tests/fixtures/bench/minimal_bb72_css_decoder.toml`
- Modify: `rsinter/tests/bench_run.rs`

**Interfaces:**
- Consumes: selected-basis validation from Task 1 and metadata from Task 3.
- Produces: self-contained BB72 CSS benchmark fixture with explicit `k = 12` observables.

- [ ] **Step 1: Add BB72 CSS fixtures**

Create `rsinter/tests/fixtures/css/bb72_hx.json` with:

```json
{"format":"sparse_rows","num_cols":72,"rows":[[1,2,18,39,42,48],[2,3,19,40,43,49],[3,4,20,41,44,50],[4,5,21,36,45,51],[0,5,22,37,46,52],[0,1,23,38,47,53],[7,8,24,45,48,54],[8,9,25,46,49,55],[9,10,26,47,50,56],[10,11,27,42,51,57],[6,11,28,43,52,58],[6,7,29,44,53,59],[13,14,30,51,54,60],[14,15,31,52,55,61],[15,16,32,53,56,62],[16,17,33,48,57,63],[12,17,34,49,58,64],[12,13,35,50,59,65],[0,19,20,57,60,66],[1,20,21,58,61,67],[2,21,22,59,62,68],[3,22,23,54,63,69],[4,18,23,55,64,70],[5,18,19,56,65,71],[6,25,26,36,63,66],[7,26,27,37,64,67],[8,27,28,38,65,68],[9,28,29,39,60,69],[10,24,29,40,61,70],[11,24,25,41,62,71],[12,31,32,36,42,69],[13,32,33,37,43,70],[14,33,34,38,44,71],[15,34,35,39,45,66],[16,30,35,40,46,67],[17,30,31,41,47,68]]}
```

Create `rsinter/tests/fixtures/css/bb72_hz.json` with:

```json
{"format":"sparse_rows","num_cols":72,"rows":[[3,24,30,40,41,54],[4,25,31,36,41,55],[5,26,32,36,37,56],[0,27,33,37,38,57],[1,28,34,38,39,58],[2,29,35,39,40,59],[0,9,30,46,47,60],[1,10,31,42,47,61],[2,11,32,42,43,62],[3,6,33,43,44,63],[4,7,34,44,45,64],[5,8,35,45,46,65],[0,6,15,52,53,66],[1,7,16,48,53,67],[2,8,17,48,49,68],[3,9,12,49,50,69],[4,10,13,50,51,70],[5,11,14,51,52,71],[6,12,21,36,58,59],[7,13,22,37,54,59],[8,14,23,38,54,55],[9,15,18,39,55,56],[10,16,19,40,56,57],[11,17,20,41,57,58],[12,18,27,42,64,65],[13,19,28,43,60,65],[14,20,29,44,60,61],[15,21,24,45,61,62],[16,22,25,46,62,63],[17,23,26,47,63,64],[18,24,33,48,70,71],[19,25,34,49,66,71],[20,26,35,50,66,67],[21,27,30,51,67,68],[22,28,31,52,68,69],[23,29,32,53,69,70]]}
```

Create `rsinter/tests/fixtures/css/bb72_logicals_x.json` with:

```json
{"format":"sparse_rows","num_cols":72,"rows":[[3,6,12,15,18,24],[4,7,13,16,19,25],[5,8,14,17,20,26],[0,9,12,15,21,27],[3,6,9,15,21,30],[4,7,10,16,22,31],[0,1,3,4,6,7,9,10,36,37,39,40],[0,2,3,5,6,8,9,11,36,38,39,41],[0,1,2,5,6,7,8,9,12,14,37,39,42,44],[2,4,6,8,13,15,36,37,38,39,43,45],[2,4,7,8,9,10,12,16,36,37,38,39,42,46],[0,1,4,5,6,7,8,11,13,17,36,38,43,47]]}
```

- [ ] **Step 2: Add BB72 benchmark fixture**

Create `rsinter/tests/fixtures/bench/minimal_bb72_css_decoder.toml` with:

```toml
name = "bb72_css_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "css"
code_id = "bb72"
hx = "../css/bb72_hx.json"
hz = "../css/bb72_hz.json"
basis = "x"
rounds = [1]
p = [0.0]
schedule = "greedy"
observables = "../css/bb72_logicals_x.json"
max_shots = 4
max_errors = 4
batch_size = 4

[plot]
title = "BB72 CSS Decoder"

[plot.x]
field = "params.p"
scale = "linear"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.code_id"]
label_template = "{runner} {params.code_id}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "linear"
label = "Logical Error Rate"
```

- [ ] **Step 3: Add failing BB72 and invalid-observable benchmark tests**

Append these tests to `rsinter/tests/bench_run.rs`:

```rust
#[test]
fn rust_benchmark_run_supports_bb72_css_explicit_observables() {
    let spec_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/minimal_bb72_css_decoder.toml");
    let text = fs::read_to_string(&spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        spec_path.parent().unwrap(),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rmatching")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].params["input_type"], serde_json::json!("css"));
    assert_eq!(rows[0].params["code_id"], serde_json::json!("bb72"));
    assert_eq!(
        rows[0].params["logical_observable_source"],
        serde_json::json!("explicit")
    );
    assert_eq!(
        rows[0].params["logical_failure_aggregation"],
        serde_json::json!("any_logical")
    );
    assert_eq!(rows[0].case_summary["num_obs"], serde_json::json!(12));
    assert_eq!(
        rows[0].case_summary["logical_observable_count"],
        serde_json::json!(12)
    );
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].error, None);
}

#[test]
fn rust_benchmark_run_rejects_invalid_css_observables_before_results() {
    let spec_dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let steane_h = r#"{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}"#;
    fs::write(spec_dir.path().join("hx.json"), steane_h).unwrap();
    fs::write(spec_dir.path().join("hz.json"), steane_h).unwrap();
    fs::write(
        spec_dir.path().join("bad_obs.json"),
        r#"{"format":"sparse_rows","num_cols":7,"rows":[[0]]}"#,
    )
    .unwrap();
    let spec_text = r#"
name = "bad_css_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "css"
code_id = "steane"
hx = "hx.json"
hz = "hz.json"
basis = "x"
rounds = [1]
p = [0.0]
schedule = "greedy"
observables = "bad_obs.json"
max_shots = 4
max_errors = 4
batch_size = 4

[plot]
title = "Bad CSS Decoder"

[plot.x]
field = "params.p"
scale = "linear"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "linear"
label = "Logical Error Rate"
"#;
    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let registry = build_default_rust_runner_registry();

    let err = run_rust_benchmark(&spec, "rust", out_dir.path(), &registry, spec_dir.path())
        .unwrap_err();

    assert!(
        err.contains("observable 0 is not an X logical"),
        "error was: {err}"
    );
    assert!(
        !out_dir.path().join("rmatching").join("test-run").exists(),
        "invalid observable run must not produce a completed result directory"
    );
}
```

- [ ] **Step 4: Run new BB72 and invalid-observable tests**

Run:

```bash
cargo test -p rsinter --test bench_run rust_benchmark_run_supports_bb72_css_explicit_observables -q
cargo test -p rsinter --test bench_run rust_benchmark_run_rejects_invalid_css_observables_before_results -q
```

Expected after earlier tasks: PASS. If run before Tasks 1 and 3, these tests should fail due to missing metadata and missing logical validation.

- [ ] **Step 5: Run CSS benchmark test suite**

Run:

```bash
cargo test -p rsinter --test bench_run -q
cargo test -p rsinter --test bench_cli rsinter_bench_run_writes_artifacts_from_css_fixture_spec -q
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

Run:

```bash
git add rsinter/tests/fixtures/css/bb72_hx.json rsinter/tests/fixtures/css/bb72_hz.json rsinter/tests/fixtures/css/bb72_logicals_x.json rsinter/tests/fixtures/bench/minimal_bb72_css_decoder.toml rsinter/tests/bench_run.rs
git commit -m "test: add bb72 explicit css observable benchmark"
```

Expected: commit succeeds with only those five paths staged.

## Task 5: Document CSS Observable Semantics And Run Final Verification

**Files:**
- Modify: `rstim/doc/cli.md`

**Interfaces:**
- Consumes: selected-basis semantics implemented by Tasks 1-4.
- Produces: user-facing CLI documentation for explicit CSS observable rows.

- [ ] **Step 1: Update CLI documentation**

In `rstim/doc/cli.md`, replace the paragraph after the CSS `rstim gen` example:

```markdown
`hx.json`, `hz.json`, and observable files use the explicit JSON wrappers
accepted by `rstim::codegen::css`.
```

with:

```markdown
`hx.json`, `hz.json`, and observable files use the explicit JSON wrappers
accepted by `rstim::codegen::css`. For CSS memory generation, `--basis x`
interprets `--observables` rows as X-like logical supports, while `--basis z`
interprets them as Z-like logical supports. Explicit observable rows must define
exactly `k` independent logical classes modulo the selected-basis stabilizer
span; invalid rows fail before a circuit is written.
```

- [ ] **Step 2: Run focused final verification**

Run:

```bash
cargo test -p rstim --test css_codegen -q
cargo test -p rstim --test cli_gen gen_css_memory_ -q
cargo test -p rsinter --test bench_circuit_source -q
cargo test -p rsinter --test bench_run -q
cargo test -p rsinter --test bench_cli rsinter_bench_run_writes_artifacts_from_css_fixture_spec -q
```

Expected: all commands PASS.

- [ ] **Step 3: Run formatting check**

Run:

```bash
cargo fmt --check
```

Expected: PASS. If it fails, run `cargo fmt`, inspect the diff, then rerun `cargo fmt --check`.

- [ ] **Step 4: Inspect final diff**

Run:

```bash
git status --short
git diff --stat
```

Expected: only issue #102 files are modified. There should be no changes from issue #101 / PR #104.

- [ ] **Step 5: Commit Task 5**

Run:

```bash
git add rstim/doc/cli.md
git commit -m "docs: explain css observable semantics"
```

Expected: commit succeeds with only `rstim/doc/cli.md` staged.

## Final Acceptance Check

After all tasks are complete, run:

```bash
cargo test -p rstim --test css_codegen --test cli_gen -q
cargo test -p rsinter --test bench_circuit_source --test bench_run --test bench_cli -q
cargo fmt --check
```

Expected: all commands PASS.

Then inspect:

```bash
git log --oneline --decorate -n 8
git status --short --branch
```

Expected: the branch contains the design commit plus Task 1-5 implementation commits, and the working tree is clean.
