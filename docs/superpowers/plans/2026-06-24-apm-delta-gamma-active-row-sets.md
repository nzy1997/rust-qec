# APM Delta/Gamma Active Row Sets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic crate-private Delta/Gamma active-row set construction for APM parameters and verify the `J=3,L=12,L2=6` Table A1 case.

**Architecture:** Extend `qec-code/src/codes/apm.rs` beside the existing crate-private APM affine helpers. Return sorted active sets through a small helper type, reject invalid inputs before construction, and use the existing #136 affine commutation validator for the P=96 Gamma sweep. Update the local APM contract doc so Delta is documented modulo `L2`.

**Tech Stack:** Rust 2024, `qec-code` crate unit tests, `serde_json` test fixture loading, existing `AffinePermutation` and `validate_affine_commutation_checks` helpers.

## Global Constraints

- Keep APM helpers crate-private; do not expose a public `qec_code::codes::apm` API.
- Implement `Delta = { (k-i) mod L2 | i,k in [0,J-1] }` with `L2 = L/2`.
- Implement `Gamma = { (i,j) | (i+j) mod L2 in Delta }`.
- Keep output order stable: sorted Delta values and lexicographically sorted Gamma pairs.
- Reject invalid inputs early: odd `L`, `J == 0`, `J > L/2`, or `L2 == 0`.
- The `J=4,L=6` negative control error must state `J` must be `<= L/2`.
- Use #136 only as the pairwise commutation primitive; do not duplicate the residual formula in the Delta/Gamma implementation.
- Do not build `Hx` or `Hz`.
- Run `cargo test -p qec-code apm_delta_gamma_matches_kasai_reference -q`.
- Run `cargo test`.

---

## File Structure

- Modify `qec-code/src/codes/apm.rs`: add `ApmActiveRowSets`, `ApmActiveRowSetError`, `build_apm_active_row_sets`, and module tests.
- Modify `qec-code/doc/apm_css.md`: correct the Delta definition from `mod L` to `mod L2`.
- Modify `docs/superpowers/specs/2026-06-24-apm-delta-gamma-active-row-sets-design.md`: committed design artifact from brainstorming.
- Modify `docs/superpowers/plans/2026-06-24-apm-delta-gamma-active-row-sets.md`: this implementation plan.

### Task 1: APM Delta/Gamma Active Row Sets

**Files:**
- Modify: `qec-code/src/codes/apm.rs`
- Modify: `qec-code/doc/apm_css.md`
- Test: `qec-code/src/codes/apm.rs`

**Interfaces:**
- Consumes: `AffinePermutation`, `AffineCommutationExpectation`, `AffineCommutationCheck::new`, and `validate_affine_commutation_checks`.
- Produces: `ApmActiveRowSets { delta: Vec<usize>, gamma: Vec<(usize, usize)> }`.
- Produces: `ApmActiveRowSetError`.
- Produces: `build_apm_active_row_sets(j: usize, l: usize) -> Result<ApmActiveRowSets, ApmActiveRowSetError>`.

- [ ] **Step 1: Write the failing Delta/Gamma reference test**

Add this test in `qec-code/src/codes/apm.rs` inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn apm_delta_gamma_matches_kasai_reference() {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/apm/table_a1_manifest.json"
    ))
    .unwrap();
    let p96 = apm_entry_by_code_id(&manifest, "apm_kasai:p=96");

    let active_sets = build_apm_active_row_sets(
        u64_json(&p96["J"]) as usize,
        u64_json(&p96["L"]) as usize,
    )
    .unwrap();

    assert_eq!(active_sets.delta, vec![0, 1, 2, 4, 5]);
    assert_eq!(
        active_sets.gamma,
        vec![
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 4),
            (0, 5),
            (1, 0),
            (1, 1),
            (1, 3),
            (1, 4),
            (1, 5),
            (2, 0),
            (2, 2),
            (2, 3),
            (2, 4),
            (2, 5),
            (3, 1),
            (3, 2),
            (3, 3),
            (3, 4),
            (3, 5),
            (4, 0),
            (4, 1),
            (4, 2),
            (4, 3),
            (4, 4),
            (5, 0),
            (5, 1),
            (5, 2),
            (5, 3),
            (5, 5),
        ]
    );

    let gamma_labels = active_sets
        .gamma
        .iter()
        .map(|(left, right)| (format!("f{left}"), format!("g{right}")))
        .collect::<Vec<_>>();
    let checks = gamma_labels
        .iter()
        .map(|(left_label, right_label)| {
            commutation_check_from_pair(
                p96,
                "apm_kasai:p=96",
                left_label,
                right_label,
                u64_json(&p96["P"]),
                AffineCommutationExpectation::Commutes,
            )
        })
        .collect::<Vec<_>>();
    validate_affine_commutation_checks(&checks).unwrap();

    let invalid = build_apm_active_row_sets(4, 6).unwrap_err();
    assert!(
        invalid.to_string().contains("J must be <= L/2"),
        "{}",
        invalid
    );
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```sh
cargo test -p qec-code apm_delta_gamma_matches_kasai_reference -q --offline
```

Expected: FAIL at compile time because `build_apm_active_row_sets` does not exist yet.

- [ ] **Step 3: Add the minimal active-set implementation**

Add `use std::collections::BTreeSet;` at the top of `qec-code/src/codes/apm.rs`.

Add these crate-private items above `impl AffinePermutation`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApmActiveRowSets {
    pub(crate) delta: Vec<usize>,
    pub(crate) gamma: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApmActiveRowSetError {
    OddBlockColumnCount { l: usize },
    EmptyActiveRows,
    EmptyHalfBlockColumnCount { l: usize },
    ActiveRowsExceedHalfBlockColumnCount { j: usize, l2: usize },
}

pub(crate) fn build_apm_active_row_sets(
    j: usize,
    l: usize,
) -> Result<ApmActiveRowSets, ApmActiveRowSetError> {
    if l % 2 != 0 {
        return Err(ApmActiveRowSetError::OddBlockColumnCount { l });
    }

    let l2 = l / 2;
    if l2 == 0 {
        return Err(ApmActiveRowSetError::EmptyHalfBlockColumnCount { l });
    }
    if j == 0 {
        return Err(ApmActiveRowSetError::EmptyActiveRows);
    }
    if j > l2 {
        return Err(ApmActiveRowSetError::ActiveRowsExceedHalfBlockColumnCount { j, l2 });
    }

    let mut delta_set = BTreeSet::new();
    for i in 0..j {
        for k in 0..j {
            delta_set.insert((k + l2 - i) % l2);
        }
    }
    let delta = delta_set.iter().copied().collect::<Vec<_>>();

    let mut gamma = Vec::new();
    for left in 0..l2 {
        for right in 0..l2 {
            if delta_set.contains(&((left + right) % l2)) {
                gamma.push((left, right));
            }
        }
    }

    Ok(ApmActiveRowSets { delta, gamma })
}
```

Add this `Display` implementation near the existing error displays:

```rust
impl fmt::Display for ApmActiveRowSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OddBlockColumnCount { l } => {
                write!(formatter, "APM block column count L must be even, got L={l}")
            }
            Self::EmptyActiveRows => {
                write!(formatter, "APM active row count J must be > 0")
            }
            Self::EmptyHalfBlockColumnCount { l } => {
                write!(formatter, "APM half block column count L2 must be > 0, got L={l}")
            }
            Self::ActiveRowsExceedHalfBlockColumnCount { j, l2 } => write!(
                formatter,
                "APM active row count J must be <= L/2, got J={j} and L/2={l2}"
            ),
        }
    }
}

impl std::error::Error for ApmActiveRowSetError {}
```

- [ ] **Step 4: Run the focused test to verify it passes**

Run:

```sh
cargo test -p qec-code apm_delta_gamma_matches_kasai_reference -q --offline
```

Expected: PASS.

- [ ] **Step 5: Add invalid-input branch coverage**

Add this test in `qec-code/src/codes/apm.rs`:

```rust
#[test]
fn apm_active_row_sets_reject_invalid_parameters() {
    assert_eq!(
        build_apm_active_row_sets(1, 5),
        Err(ApmActiveRowSetError::OddBlockColumnCount { l: 5 })
    );
    assert_eq!(
        build_apm_active_row_sets(1, 0),
        Err(ApmActiveRowSetError::EmptyHalfBlockColumnCount { l: 0 })
    );
    assert_eq!(
        build_apm_active_row_sets(0, 6),
        Err(ApmActiveRowSetError::EmptyActiveRows)
    );
    assert_eq!(
        build_apm_active_row_sets(4, 6),
        Err(ApmActiveRowSetError::ActiveRowsExceedHalfBlockColumnCount { j: 4, l2: 3 })
    );
}
```

- [ ] **Step 6: Correct the local contract doc**

In `qec-code/doc/apm_css.md`, replace:

```text
Delta = { (r - s) mod L | r in A, s in A }
```

with:

```text
Delta = { (r - s) mod L2 | r in A, s in A }
```

Also adjust the surrounding paragraph to say Gamma is generated from sums modulo `L2`, while the legacy manifest `required_commuting_pairs` remains a smaller pinned subset from earlier issues.

- [ ] **Step 7: Run final focused checks**

Run:

```sh
rustfmt --check qec-code/src/codes/apm.rs
cargo test -p qec-code apm_delta_gamma_matches_kasai_reference -q --offline
cargo test -p qec-code apm_active_row_sets -q --offline
cargo test -p qec-code affine_commutation_matches_table_a1 -q --offline
```

Expected: all commands pass.

- [ ] **Step 8: Run required broad verification**

Run:

```sh
cargo test
```

Expected: PASS. If it fails before compilation because the sandbox cannot fetch crates.io metadata, rerun:

```sh
cargo test --offline
```

and record both outcomes.

- [ ] **Step 9: Commit**

Run:

```sh
git add qec-code/src/codes/apm.rs qec-code/doc/apm_css.md docs/superpowers/specs/2026-06-24-apm-delta-gamma-active-row-sets-design.md docs/superpowers/plans/2026-06-24-apm-delta-gamma-active-row-sets.md
git commit -m "feat: add apm delta gamma active sets"
```

## Self-Review

- Spec coverage: The plan adds deterministic Delta/Gamma construction, exact `J=3,L=12` assertions, P=96 Gamma commutation validation through #136, invalid input handling, and the required verification commands.
- Placeholder scan: No placeholders remain.
- Type consistency: The produced helper names and error types match across task steps.
