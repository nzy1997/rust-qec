# Affine Permutation Arithmetic Design

Scope: GitHub issue #135, the internal `qec-code` algebra helper for APM-CSS affine permutation blocks over `Z_P`.

## Context

The merged Table A1 manifest in `qec-code/tests/fixtures/apm/table_a1_manifest.json` records the source-grounded affine coefficients for the P=96 and P=192 APM-CSS instances. The construction contract in `qec-code/doc/apm_css.md` defines every APM block as `AffineMap { a, b, modulus }` representing `x -> a*x + b mod modulus`, valid only when `gcd(a, modulus) == 1`.

Issue #135 only needs this smallest algebraic unit. It must not build Delta/Gamma sets, CSS matrices, or public generator APIs.

## Chosen Approach

Add a private Rust module at `qec-code/src/codes/apm.rs` and include it from `qec-code/src/codes/mod.rs` with `mod apm;`. The module will expose `pub(crate)` types and methods for future code inside the crate while exporting nothing from the public crate API.

The core type is `AffinePermutation`, storing normalized `u64` values:

- `modulus`
- `slope`
- `offset`

Construction validates a positive modulus and unit slope modulo the modulus. It normalizes `slope` and `offset` into canonical residues.

## Operations

- `new(modulus, slope, offset) -> Result<AffinePermutation, AffinePermutationError>` validates and normalizes parameters.
- `apply(index) -> u64` evaluates `slope * index + offset mod modulus`.
- `inverse() -> AffinePermutation` returns the inverse affine permutation, using the modular inverse of the validated slope.
- `compose_after(inner) -> Result<AffinePermutation, AffinePermutationError>` returns `self(inner(x))` and reports a modulus mismatch explicitly.
- `is_unit_slope() -> bool` supports direct internal checks and test assertions.

Arithmetic uses standard-library integer operations. Products and sums are evaluated in `u128`, which is wide enough for `u64 * u64 + u64`, before reducing modulo the `u64` modulus.

## Errors

Use a local internal error enum with `Display`:

- `InvalidModulus` for `P=0`.
- `NonUnitSlope { slope, modulus }` for invalid affine multipliers.
- `ModulusMismatch { lhs, rhs }` for composition across different `Z_P`.

The non-unit error text names both the slope and modulus, satisfying the negative control for `P=96,a=2,b=1`.

## Testing

Add private module unit tests in `qec-code/src/codes/apm.rs`, so the helper remains internal but is still testable with Cargo's unit-test harness.

The main test is named `affine_permutation_round_trips_and_composes` and uses representative Table A1 maps from the manifest for P=96 and P=192. It verifies:

- inverse application recovers sampled indices, including `0`, `1`, and `P-1`;
- composition agrees with sequential application;
- inverse maps still have unit slopes.

Add negative tests for non-unit slope errors and modulus mismatch composition errors.

## Out Of Scope

- No public re-export.
- No manifest parser changes.
- No Delta/Gamma generation.
- No `Hx`/`Hz` matrix construction.
