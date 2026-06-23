# Issue 138 APM CSS Sparse Supports Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build crate-private native APM-CSS `Hx` and `Hz` sparse-row supports from a validated manifest entry and verify P=96 exactly against pinned fixtures.

**Architecture:** Extend `qec-code/src/codes/apm.rs`, which already owns affine permutation and active-row helpers. Add a small crate-private manifest-entry type, a build error type, and `build_apm_css_checks` returning `BuiltInCssChecks`; keep public built-in CSS parsing unchanged. Test exact P=96 fixture equality and a wrong-transpose negative control in the same private module.

**Tech Stack:** Rust 2024, `qec-code` crate unit tests, existing `AffinePermutation`, `build_apm_active_row_sets`, `BuiltInCssChecks`, and `SparseRowsMatrix` JSON loader.

## Global Constraints

- Keep APM CSS construction crate-private; do not expose a public `qec_code::codes::apm` API.
- Do not register CLI or catalog support for `apm_kasai:p=96`.
- Input is one validated APM manifest entry with `P`, `J`, `L`, `f_i`, and `g_i`.
- Output must be `BuiltInCssChecks`-compatible sparse supports with `num_cols = L * P`.
- For P=96, require `num_cols == 1152` and `hx.len() == hz.len() == J * P == 288`.
- Row supports must be sorted and deduplicated.
- Native P=96 output must compare exactly against `qec-code/tests/fixtures/apm/p96_hx.json` and `qec-code/tests/fixtures/apm/p96_hz.json`.
- Include an in-memory wrong inverse/transpose-orientation negative control that differs from the fixture and fails `Hx * Hz^T == 0 mod 2`.
- Use the fixture generator's construction logic as the local reference because `drafts/` reference clones are absent in this worktree.
- Run `cargo test -p qec-code apm_p96_builds_expected_hx_hz -q`.
- Run `cargo test`.

---

## File Structure

- Modify `qec-code/src/codes/apm.rs`: add `ApmCssManifestEntry`, `ApmCssBuildError`, the matrix builder helpers, and unit tests.
- Modify `docs/superpowers/specs/2026-06-24-issue-138-apm-css-sparse-supports-design.md`: committed design artifact from brainstorming.
- Modify `docs/superpowers/plans/2026-06-24-issue-138-apm-css-sparse-supports.md`: this implementation plan.

### Task 1: APM CSS Sparse Support Builder

**Files:**
- Modify: `qec-code/src/codes/apm.rs`
- Test: `qec-code/src/codes/apm.rs`

**Interfaces:**
- Consumes: `AffinePermutation`, `build_apm_active_row_sets`, and `BuiltInCssChecks`.
- Produces: `ApmCssManifestEntry`.
- Produces: `ApmCssBuildError`.
- Produces: `build_apm_css_checks(entry: &ApmCssManifestEntry) -> Result<BuiltInCssChecks, ApmCssBuildError>`.

- [ ] **Step 1: Write the failing exact fixture test**

Add these helpers and the test in `qec-code/src/codes/apm.rs` inside the existing `#[cfg(test)] mod tests`:

```rust
fn parse_p96_apm_manifest_entry(manifest: &Value) -> ApmCssManifestEntry {
    let entry = apm_entry_by_code_id(manifest, "apm_kasai:p=96");
    let p = u64_json(&entry["P"]);
    let f = (0..6)
        .map(|index| {
            let map = &entry["f"][index];
            AffinePermutation::new(p, u64_json(&map["a"]), u64_json(&map["b"])).unwrap()
        })
        .collect::<Vec<_>>();
    let g = (0..6)
        .map(|index| {
            let map = &entry["g"][index];
            AffinePermutation::new(p, u64_json(&map["c"]), u64_json(&map["d"])).unwrap()
        })
        .collect::<Vec<_>>();

    ApmCssManifestEntry::new(
        "apm_kasai:p=96",
        p,
        u64_json(&entry["J"]),
        u64_json(&entry["L"]),
        f,
        g,
    )
    .unwrap()
}

fn load_sparse_rows_fixture(input: &str) -> (usize, Vec<Vec<usize>>) {
    let matrix = crate::css::sparse_rows_matrix_from_json_str(input).unwrap();
    (matrix.num_cols(), matrix.rows().to_vec())
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|pair| pair[0] < pair[1]),
            "row is not sorted and deduplicated: {row:?}"
        );
    }
}

fn sparse_rows_are_orthogonal(hx: &[Vec<usize>], hz: &[Vec<usize>]) -> bool {
    hx.iter().all(|x_row| {
        let x_support = x_row.iter().copied().collect::<BTreeSet<_>>();
        hz.iter()
            .all(|z_row| z_row.iter().filter(|col| x_support.contains(col)).count() % 2 == 0)
    })
}

#[test]
fn apm_p96_builds_expected_hx_hz() {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/apm/table_a1_manifest.json"
    ))
    .unwrap();
    let entry = parse_p96_apm_manifest_entry(&manifest);

    let checks = build_apm_css_checks(&entry).unwrap();
    let (expected_num_cols, expected_hx) =
        load_sparse_rows_fixture(include_str!("../../tests/fixtures/apm/p96_hx.json"));
    let (_, expected_hz) =
        load_sparse_rows_fixture(include_str!("../../tests/fixtures/apm/p96_hz.json"));

    assert_eq!(checks.code_id, "apm_kasai:p=96");
    assert_eq!(checks.num_cols, expected_num_cols);
    assert_eq!(checks.num_cols, 1152);
    assert_eq!(checks.hx.len(), 288);
    assert_eq!(checks.hz.len(), 288);
    assert_canonical_sparse_rows(&checks.hx);
    assert_canonical_sparse_rows(&checks.hz);
    assert_eq!(checks.hx, expected_hx);
    assert_eq!(checks.hz, expected_hz);
    assert!(sparse_rows_are_orthogonal(&checks.hx, &checks.hz));

    let wrong_hz = build_apm_hz_rows_with_forward_blocks_for_negative_control(&entry).unwrap();
    assert_ne!(wrong_hz, expected_hz);
    assert!(!sparse_rows_are_orthogonal(&checks.hx, &wrong_hz));
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```sh
cargo test -p qec-code apm_p96_builds_expected_hx_hz -q
```

Expected: FAIL at compile time because `ApmCssManifestEntry`, `build_apm_css_checks`, and the negative-control helper do not exist yet.

- [ ] **Step 3: Add the minimal builder implementation**

Add `use super::built_in_css::BuiltInCssChecks;` near the top of `qec-code/src/codes/apm.rs`.

Add these crate-private items before `impl AffinePermutation`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApmCssManifestEntry {
    code_id: &'static str,
    p: usize,
    j: usize,
    l: usize,
    f: Vec<AffinePermutation>,
    g: Vec<AffinePermutation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApmCssBuildError {
    InvalidActiveRows(ApmActiveRowSetError),
    UnsupportedParameterWidth { parameter: &'static str, value: u64 },
    AffineFamilyLengthMismatch { family: &'static str, expected: usize, actual: usize },
    AffineMapModulusMismatch { family: &'static str, index: usize, expected: u64, actual: u64 },
}

impl ApmCssManifestEntry {
    pub(crate) fn new(
        code_id: &'static str,
        p: u64,
        j: u64,
        l: u64,
        f: Vec<AffinePermutation>,
        g: Vec<AffinePermutation>,
    ) -> Result<Self, ApmCssBuildError> {
        let p_usize = usize::try_from(p).map_err(|_| ApmCssBuildError::UnsupportedParameterWidth {
            parameter: "P",
            value: p,
        })?;
        let j_usize = usize::try_from(j).map_err(|_| ApmCssBuildError::UnsupportedParameterWidth {
            parameter: "J",
            value: j,
        })?;
        let l_usize = usize::try_from(l).map_err(|_| ApmCssBuildError::UnsupportedParameterWidth {
            parameter: "L",
            value: l,
        })?;
        build_apm_active_row_sets(j_usize, l_usize).map_err(ApmCssBuildError::InvalidActiveRows)?;
        let l2 = l_usize / 2;
        validate_apm_affine_family("f", l2, p, &f)?;
        validate_apm_affine_family("g", l2, p, &g)?;

        Ok(Self {
            code_id,
            p: p_usize,
            j: j_usize,
            l: l_usize,
            f,
            g,
        })
    }
}

pub(crate) fn build_apm_css_checks(
    entry: &ApmCssManifestEntry,
) -> Result<BuiltInCssChecks, ApmCssBuildError> {
    let hx = build_apm_hx_rows(entry)?;
    let hz = build_apm_hz_rows(entry)?;

    Ok(BuiltInCssChecks {
        code_id: entry.code_id,
        num_cols: entry.l * entry.p,
        hx,
        hz,
    })
}
```

Also add the row helpers:

```rust
fn validate_apm_affine_family(
    family: &'static str,
    expected_len: usize,
    expected_modulus: u64,
    maps: &[AffinePermutation],
) -> Result<(), ApmCssBuildError> {
    if maps.len() != expected_len {
        return Err(ApmCssBuildError::AffineFamilyLengthMismatch {
            family,
            expected: expected_len,
            actual: maps.len(),
        });
    }
    for (index, map) in maps.iter().enumerate() {
        if map.modulus != expected_modulus {
            return Err(ApmCssBuildError::AffineMapModulusMismatch {
                family,
                index,
                expected: expected_modulus,
                actual: map.modulus,
            });
        }
    }
    Ok(())
}

fn build_apm_hx_rows(entry: &ApmCssManifestEntry) -> Result<Vec<Vec<usize>>, ApmCssBuildError> {
    build_apm_rows(entry, |entry, block_row, block_col| {
        let l2 = entry.l / 2;
        let family = if block_col < l2 { &entry.f } else { &entry.g };
        &family[(block_col % l2 + l2 - block_row) % l2]
    })
}

fn build_apm_hz_rows(entry: &ApmCssManifestEntry) -> Result<Vec<Vec<usize>>, ApmCssBuildError> {
    let inverse_f = entry.f.iter().map(AffinePermutation::inverse).collect::<Vec<_>>();
    let inverse_g = entry.g.iter().map(AffinePermutation::inverse).collect::<Vec<_>>();
    build_apm_rows_with_families(entry, &inverse_g, &inverse_f, |entry, block_row, block_col| {
        let l2 = entry.l / 2;
        (block_row + l2 - block_col % l2) % l2
    })
}

#[cfg(test)]
fn build_apm_hz_rows_with_forward_blocks_for_negative_control(
    entry: &ApmCssManifestEntry,
) -> Result<Vec<Vec<usize>>, ApmCssBuildError> {
    build_apm_rows_with_families(entry, &entry.g, &entry.f, |entry, block_row, block_col| {
        let l2 = entry.l / 2;
        (block_row + l2 - block_col % l2) % l2
    })
}
```

Finish with shared construction and display code:

```rust
fn build_apm_rows<'a>(
    entry: &'a ApmCssManifestEntry,
    map_for_block: impl Fn(&'a ApmCssManifestEntry, usize, usize) -> &'a AffinePermutation,
) -> Result<Vec<Vec<usize>>, ApmCssBuildError> {
    let mut rows = Vec::with_capacity(entry.j * entry.p);
    for block_row in 0..entry.j {
        for local_row in 0..entry.p {
            let mut row = Vec::with_capacity(entry.l);
            for block_col in 0..entry.l {
                let local_col = map_for_block(entry, block_row, block_col).apply(local_row as u64);
                row.push(block_col * entry.p + local_col as usize);
            }
            row.sort_unstable();
            row.dedup();
            rows.push(row);
        }
    }
    Ok(rows)
}

fn build_apm_rows_with_families(
    entry: &ApmCssManifestEntry,
    first_half: &[AffinePermutation],
    second_half: &[AffinePermutation],
    index_for_block: impl Fn(&ApmCssManifestEntry, usize, usize) -> usize,
) -> Result<Vec<Vec<usize>>, ApmCssBuildError> {
    build_apm_rows(entry, |entry, block_row, block_col| {
        let l2 = entry.l / 2;
        let family = if block_col < l2 { first_half } else { second_half };
        &family[index_for_block(entry, block_row, block_col)]
    })
}

impl fmt::Display for ApmCssBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidActiveRows(error) => write!(formatter, "{error}"),
            Self::UnsupportedParameterWidth { parameter, value } => {
                write!(formatter, "APM parameter {parameter}={value} does not fit usize")
            }
            Self::AffineFamilyLengthMismatch { family, expected, actual } => write!(
                formatter,
                "APM affine family {family} has {actual} maps, expected {expected}"
            ),
            Self::AffineMapModulusMismatch { family, index, expected, actual } => write!(
                formatter,
                "APM affine map {family}{index} has modulus {actual}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for ApmCssBuildError {}
```

- [ ] **Step 4: Run the focused test to verify it passes**

Run:

```sh
cargo test -p qec-code apm_p96_builds_expected_hx_hz -q
```

Expected: PASS.

- [ ] **Step 5: Run broader verification**

Run:

```sh
cargo test -p qec-code -q
cargo test
```

Expected: PASS.

- [ ] **Step 6: Commit**

Stage the implementation, design, and plan files:

```sh
git add qec-code/src/codes/apm.rs \
  docs/superpowers/specs/2026-06-24-issue-138-apm-css-sparse-supports-design.md \
  docs/superpowers/plans/2026-06-24-issue-138-apm-css-sparse-supports.md
git commit -m "feat: build apm css sparse supports"
```
