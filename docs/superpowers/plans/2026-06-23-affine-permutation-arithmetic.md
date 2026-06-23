# Affine Permutation Arithmetic Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an internal `qec-code` helper for validated affine permutation arithmetic over `Z_P`.

**Architecture:** Put the helper in `qec-code/src/codes/apm.rs` and include it from `qec-code/src/codes/mod.rs` as a crate-visible internal module. Keep validation, application, inverse, composition, and tests in the same focused module until later APM generator issues need it.

**Tech Stack:** Rust 2024, standard library integer arithmetic, existing Cargo workspace.

## Global Constraints

- Keep the helper internal to `qec-code`; do not add a public crate re-export.
- Do not add dependencies.
- Validate that affine slopes are units modulo `P`.
- Non-unit slope errors must name both the slope and modulus.
- Composition must explicitly reject modulus mismatches.
- Do not compute Delta/Gamma sets or build `Hx`/`Hz` matrices.
- Verification command: `cargo test -p qec-code affine_permutation_round_trips_and_composes -q`.

---

## File Structure

- Modify `qec-code/src/codes/mod.rs`: include the internal APM helper module.
- Create `qec-code/src/codes/apm.rs`: define `AffinePermutation`, `AffinePermutationError`, modular arithmetic helpers, and unit tests.
- Add this plan and the paired design doc under `docs/superpowers/` as repository workflow artifacts.

### Task 1: Internal Affine Permutation Helper

**Files:**
- Modify: `qec-code/src/codes/mod.rs`
- Create: `qec-code/src/codes/apm.rs`
- Test: `qec-code/src/codes/apm.rs`

**Interfaces:**
- Produces: `pub(crate) struct AffinePermutation`
- Produces: `pub(crate) enum AffinePermutationError`
- Produces: `AffinePermutation::new(modulus: u64, slope: u64, offset: u64) -> Result<Self, AffinePermutationError>`
- Produces: `AffinePermutation::apply(&self, index: u64) -> u64`
- Produces: `AffinePermutation::inverse(&self) -> Self`
- Produces: `AffinePermutation::compose_after(&self, inner: &Self) -> Result<Self, AffinePermutationError>`
- Produces: `AffinePermutation::is_unit_slope(&self) -> bool`

- [x] **Step 1: Write the failing test**

Add the module declaration to `qec-code/src/codes/mod.rs`:

```rust
pub(crate) mod apm;
pub mod built_in_css;
pub mod steane;
```

Create `qec-code/src/codes/apm.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_permutation_round_trips_and_composes() {
        let cases = [
            (96, (5, 41), (25, 22)),
            (192, (71, 127), (55, 183)),
        ];

        for (modulus, outer_params, inner_params) in cases {
            let outer = AffinePermutation::new(modulus, outer_params.0, outer_params.1).unwrap();
            let inner = AffinePermutation::new(modulus, inner_params.0, inner_params.1).unwrap();
            let inverse = outer.inverse();
            assert!(inverse.is_unit_slope());

            let composed = outer.compose_after(&inner).unwrap();
            let samples = [0, 1, modulus / 3, modulus - 1];

            for index in samples {
                assert_eq!(inverse.apply(outer.apply(index)), index);
                assert_eq!(composed.apply(index), outer.apply(inner.apply(index)));
            }
        }
    }

    #[test]
    fn affine_permutation_rejects_non_unit_slope() {
        let err = AffinePermutation::new(96, 2, 1).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("slope 2"), "{message}");
        assert!(message.contains("modulo 96"), "{message}");
    }

    #[test]
    fn affine_permutation_rejects_modulus_mismatch_composition() {
        let lhs = AffinePermutation::new(96, 5, 41).unwrap();
        let rhs = AffinePermutation::new(192, 71, 127).unwrap();

        assert_eq!(
            lhs.compose_after(&rhs),
            Err(AffinePermutationError::ModulusMismatch { lhs: 96, rhs: 192 })
        );
    }
}
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p qec-code affine_permutation_round_trips_and_composes -q
```

Expected: FAIL to compile because `AffinePermutation` is not defined yet.

- [x] **Step 3: Write minimal implementation**

Replace `qec-code/src/codes/apm.rs` with:

```rust
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AffinePermutation {
    modulus: u64,
    slope: u64,
    offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AffinePermutationError {
    InvalidModulus,
    NonUnitSlope { slope: u64, modulus: u64 },
    ModulusMismatch { lhs: u64, rhs: u64 },
}

impl AffinePermutation {
    pub(crate) fn new(
        modulus: u64,
        slope: u64,
        offset: u64,
    ) -> Result<Self, AffinePermutationError> {
        if modulus == 0 {
            return Err(AffinePermutationError::InvalidModulus);
        }

        let normalized_slope = slope % modulus;
        if gcd_u64(normalized_slope, modulus) != 1 {
            return Err(AffinePermutationError::NonUnitSlope { slope, modulus });
        }

        Ok(Self {
            modulus,
            slope: normalized_slope,
            offset: offset % modulus,
        })
    }

    pub(crate) fn apply(&self, index: u64) -> u64 {
        add_mod(mul_mod(self.slope, index % self.modulus, self.modulus), self.offset, self.modulus)
    }

    pub(crate) fn inverse(&self) -> Self {
        let inverse_slope = modular_inverse(self.slope, self.modulus)
            .expect("validated affine permutation slope must have a modular inverse");
        let inverse_offset = neg_mod(mul_mod(inverse_slope, self.offset, self.modulus), self.modulus);

        Self {
            modulus: self.modulus,
            slope: inverse_slope,
            offset: inverse_offset,
        }
    }

    pub(crate) fn compose_after(
        &self,
        inner: &Self,
    ) -> Result<Self, AffinePermutationError> {
        if self.modulus != inner.modulus {
            return Err(AffinePermutationError::ModulusMismatch {
                lhs: self.modulus,
                rhs: inner.modulus,
            });
        }

        let slope = mul_mod(self.slope, inner.slope, self.modulus);
        let offset = add_mod(
            mul_mod(self.slope, inner.offset, self.modulus),
            self.offset,
            self.modulus,
        );

        Ok(Self {
            modulus: self.modulus,
            slope,
            offset,
        })
    }

    pub(crate) fn is_unit_slope(&self) -> bool {
        gcd_u64(self.slope, self.modulus) == 1
    }
}

impl fmt::Display for AffinePermutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModulus => write!(formatter, "affine permutation modulus must be positive"),
            Self::NonUnitSlope { slope, modulus } => {
                write!(formatter, "affine slope {slope} is not a unit modulo {modulus}")
            }
            Self::ModulusMismatch { lhs, rhs } => {
                write!(formatter, "affine permutation modulus mismatch: {lhs} != {rhs}")
            }
        }
    }
}

impl std::error::Error for AffinePermutationError {}

fn add_mod(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((lhs as u128 + rhs as u128) % modulus as u128) as u64
}

fn mul_mod(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((lhs as u128 * rhs as u128) % modulus as u128) as u64
}

fn neg_mod(value: u64, modulus: u64) -> u64 {
    if value == 0 {
        0
    } else {
        modulus - value
    }
}

fn gcd_u64(mut lhs: u64, mut rhs: u64) -> u64 {
    while rhs != 0 {
        let next = lhs % rhs;
        lhs = rhs;
        rhs = next;
    }
    lhs
}

fn modular_inverse(value: u64, modulus: u64) -> Option<u64> {
    let mut t = 0_i128;
    let mut next_t = 1_i128;
    let mut r = modulus as i128;
    let mut next_r = (value % modulus) as i128;

    while next_r != 0 {
        let quotient = r / next_r;

        let old_t = t;
        t = next_t;
        next_t = old_t - quotient * next_t;

        let old_r = r;
        r = next_r;
        next_r = old_r - quotient * next_r;
    }

    if r == 1 {
        Some(t.rem_euclid(modulus as i128) as u64)
    } else {
        None
    }
}
```

Keep the tests from Step 1 below the implementation.

- [x] **Step 4: Run focused test to verify it passes**

Run:

```bash
cargo test -p qec-code affine_permutation_round_trips_and_composes -q
```

Expected: PASS.

- [x] **Step 5: Run full package test**

Run:

```bash
cargo test -p qec-code -q
```

Expected: PASS.

- [x] **Step 6: Commit**

Run:

```bash
git add qec-code/src/codes/mod.rs qec-code/src/codes/apm.rs docs/superpowers/specs/2026-06-23-affine-permutation-arithmetic-design.md docs/superpowers/plans/2026-06-23-affine-permutation-arithmetic.md
git commit -m "feat: add affine permutation arithmetic"
```

## Self-Review

- Spec coverage: the task covers validation, application, inverse, composition, modulus mismatch errors, non-unit slope errors, internal-only module placement, and the requested focused verification command.
- Red-flag scan: no incomplete plan markers are intentionally left.
- Type consistency: the plan uses the same `AffinePermutation` and `AffinePermutationError` names throughout.
