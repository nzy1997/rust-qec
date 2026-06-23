# APM Commutation Checks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add crate-private APM affine commutation helpers and manifest-pair validation for issue #136.

**Architecture:** Extend `qec-code/src/codes/apm.rs` beside the existing `AffinePermutation` type. Keep the helper API crate-private, return structured validation errors, and prove the P=96 Table A1 manifest pairs with module unit tests.

**Tech Stack:** Rust 2024, existing `qec-code` crate, `serde_json` in tests, Cargo unit tests.

## Global Constraints

- Keep APM helpers crate-private; do not expose a public `qec_code::codes::apm` API.
- Use the existing `AffinePermutation` type from `qec-code/src/codes/apm.rs`.
- Residual for `f(x)=a*x+b` and `g(x)=c*x+d` is `d*(a-1) == b*(c-1) mod P`, equivalently `(a*d + b - c*b - d) mod P == 0`.
- Return a modulus mismatch error when maps use different moduli.
- Validate only explicit manifest pair constraints; do not compute Delta/Gamma sets, sweep generated Gamma pairs, generate sparse matrices, or compute graph girth.
- Run `cargo test -p qec-code affine_commutation_matches_table_a1 -q`.
- Run `cargo test` before finishing and record the exact outcome.

---

## File Structure

- Modify `qec-code/src/codes/apm.rs`: add residual, boolean commutation helper, structured validator types, and module tests.
- Modify `docs/superpowers/specs/2026-06-23-apm-commutation-checks-design.md`: committed design artifact from brainstorming.
- Modify `docs/superpowers/plans/2026-06-23-apm-commutation-checks.md`: this implementation plan.

### Task 1: APM Affine Commutation Primitive And Validator

**Files:**
- Modify: `qec-code/src/codes/apm.rs`
- Test: `qec-code/src/codes/apm.rs`

**Interfaces:**
- Consumes: `AffinePermutation::new`, `AffinePermutation::apply`, and `AffinePermutationError`.
- Produces: `AffinePermutation::commutation_residual(&self, other: &Self) -> Result<u64, AffinePermutationError>`.
- Produces: `AffinePermutation::commutes_with(&self, other: &Self) -> Result<bool, AffinePermutationError>`.
- Produces: `AffineCommutationExpectation::{Commutes, DoesNotCommute}`.
- Produces: `AffineCommutationCheck::new(code_id, left_label, right_label, left, right, expected)`.
- Produces: `validate_affine_commutation_checks(checks) -> Result<(), Vec<AffineCommutationError>>`.

- [ ] **Step 1: Write the failing Table A1 test**

Add a module test named `affine_commutation_matches_table_a1` in
`qec-code/src/codes/apm.rs`. The test should load:

```rust
let manifest: serde_json::Value =
    serde_json::from_str(include_str!("../../tests/fixtures/apm/table_a1_manifest.json")).unwrap();
let p96 = manifest["entries"]
    .as_array()
    .unwrap()
    .iter()
    .find(|entry| entry["code_id"] == "apm_kasai:p=96")
    .unwrap();
```

It should build `AffineCommutationCheck` values for all
`required_commuting_pairs` over their pair-specific modulus and all
`required_noncommuting_pairs` over full `P`, assert the validator accepts them,
assert direct calls to `commutes_with` have the expected boolean result, and
assert residual commutation agrees with `lhs.apply(rhs.apply(x)) ==
rhs.apply(lhs.apply(x))` for every `x` in the pair modulus.

Add the negative control in the same test by changing one known noncommuting
check to `AffineCommutationExpectation::Commutes` and asserting the returned
`AffineCommutationError` message contains `apm_kasai:p=96`, `f0`, and `g3`.

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test -p qec-code affine_commutation_matches_table_a1 -q --offline
```

Expected: FAIL at compile time because `AffineCommutationCheck`,
`AffineCommutationExpectation`, and `commutes_with` do not exist yet.

- [ ] **Step 3: Implement the helper API**

Add these crate-private types and methods in `qec-code/src/codes/apm.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AffineCommutationExpectation {
    Commutes,
    DoesNotCommute,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AffineCommutationCheck<'a> {
    pub(crate) code_id: &'a str,
    pub(crate) left_label: &'a str,
    pub(crate) right_label: &'a str,
    pub(crate) left: AffinePermutation,
    pub(crate) right: AffinePermutation,
    pub(crate) expected: AffineCommutationExpectation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AffineCommutationError {
    ModulusMismatch {
        code_id: String,
        left_label: String,
        right_label: String,
        lhs: u64,
        rhs: u64,
    },
    UnexpectedCommutation {
        code_id: String,
        left_label: String,
        right_label: String,
        residual: u64,
    },
    UnexpectedNoncommutation {
        code_id: String,
        left_label: String,
        right_label: String,
        residual: u64,
    },
}
```

Implement `Display` for `AffineCommutationError`. Add
`AffinePermutation::commutation_residual`, `AffinePermutation::commutes_with`,
and `validate_affine_commutation_checks`.

- [ ] **Step 4: Run the focused test to verify GREEN**

Run:

```sh
cargo test -p qec-code affine_commutation_matches_table_a1 -q --offline
```

Expected: PASS.

- [ ] **Step 5: Add mismatch coverage**

Add `affine_commutation_rejects_modulus_mismatch` in
`qec-code/src/codes/apm.rs`. It should assert:

```rust
let lhs = AffinePermutation::new(96, 5, 41).unwrap();
let rhs = AffinePermutation::new(192, 71, 127).unwrap();
assert_eq!(
    lhs.commutes_with(&rhs),
    Err(AffinePermutationError::ModulusMismatch { lhs: 96, rhs: 192 })
);
let checks = [AffineCommutationCheck::new(
    "apm_kasai:p=96",
    "f0",
    "g0",
    lhs,
    rhs,
    AffineCommutationExpectation::Commutes,
)];
let errors = validate_affine_commutation_checks(&checks).unwrap_err();
assert!(matches!(errors[0], AffineCommutationError::ModulusMismatch { lhs: 96, rhs: 192, .. }));
assert!(errors[0].to_string().contains("apm_kasai:p=96"));
```

- [ ] **Step 6: Run final qec-code checks**

Run:

```sh
rustfmt --check qec-code/src/codes/apm.rs qec-code/src/codes/mod.rs
cargo test -p qec-code affine_commutation_matches_table_a1 -q --offline
cargo test -p qec-code affine_commutation -q --offline
```

Expected: PASS.

- [ ] **Step 7: Run required broad verification**

Run:

```sh
cargo test
```

Expected: PASS when the network/dependency environment can access all workspace dependencies. If the managed Agent Desk sandbox blocks crates.io access, record the exact failure and run:

```sh
cargo test --offline
```

to distinguish network setup from code failures.

- [ ] **Step 8: Commit**

Stage only the issue #136 files:

```sh
git add qec-code/src/codes/apm.rs docs/superpowers/specs/2026-06-23-apm-commutation-checks-design.md docs/superpowers/plans/2026-06-23-apm-commutation-checks.md
git commit -m "feat: add apm affine commutation checks"
```

## Self-Review

- Spec coverage: The plan adds the commutation primitive, structured validator,
  mismatch handling, Table A1 P=96 checks, direct sampled composition agreement,
  and negative validator control.
- Placeholder scan: No TBD/TODO placeholders remain.
- Type consistency: Type and method names are consistent across the task.
