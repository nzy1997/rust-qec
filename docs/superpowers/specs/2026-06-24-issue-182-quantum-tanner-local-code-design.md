# Issue 182 Quantum Tanner Local Code Helpers Design

Scope: GitHub issue #182, local GF(2) helper support for the `qec-code`
quantum Tanner parser and future sparse CSS constructor.

## Context

Issue #178 added `qec-code/src/codes/quantum_tanner.rs` with a narrow
serde-backed parser for the v1 quantum Tanner contract. The parser currently
accepts explicit group data, generator-index arrays, and local binary
parity-check matrices `h_a` and `h_b` from the shared catalog fixtures.

This issue adds only the local-code algebra needed by later quantum Tanner CSS
assembly. It must not enumerate Cayley-complex faces, generate global `Hx` or
`Hz`, add CLI support, compute distance, or grow a general classical-code
library.

The qLDPC reference `QTCode.get_subcodes` builds local Tanner subcodes from
duals and tensor products: one sector uses the tensor product of the two seed
codes, and the other uses the tensor product of their duals. QuantumExpanders.jl
uses the same narrow vocabulary around component parity checks, generator
matrices, dual codes, and Kronecker products for quantum Tanner local rows.

## Approaches Considered

Recommended: extend `qec-code/src/codes/quantum_tanner.rs` with local-only
types and helper functions. The parser remains the entry point for JSON shape,
while new helpers validate seed-code rows, derive generator bases from
parity-check rows with existing GF(2) utilities, verify optional supplied
generator rows, and produce deterministic local tensor rows for the future X
and Z sectors. This keeps the feature close to the quantum Tanner code and
avoids a broad algebra package.

Alternative: add public APIs to `qec-code/src/binary.rs`. That would expose
general nullspace and tensor operations beyond the immediate quantum Tanner
need, which conflicts with the issue's request for narrow v1 helpers.

Alternative: wait until global CSS generation exists and implement local algebra
inside the constructor. That would defer the requested validation and makes the
future constructor harder to test independently.

## Selected Design

Add a public helper:

```rust
pub fn quantum_tanner_local_code_tensor_dual(
    spec: &QuantumTannerSpec,
) -> Result<QuantumTannerLocalCodeTensorDual>
```

The returned `QuantumTannerLocalCodeTensorDual` contains:

- `code_a: QuantumTannerLocalBinaryCode`
- `code_b: QuantumTannerLocalBinaryCode`
- `x_sector_rows: Vec<Vec<u8>>`
- `z_sector_rows: Vec<Vec<u8>>`

`QuantumTannerLocalBinaryCode` contains the validated width, an independent
basis for the local code's dual rows, and a generator basis for the local code
itself:

```rust
pub struct QuantumTannerLocalBinaryCode {
    pub width: usize,
    pub generator_rows: Vec<Vec<u8>>,
    pub dual_rows: Vec<Vec<u8>>,
}
```

For v1 contract input, `h_a` and `h_b` are parity-check matrices. Their kernels
are the local seed codes `C_A` and `C_B`. The helper derives:

- `code_a.generator_rows = nullspace(h_a)`
- `code_b.generator_rows = nullspace(h_b)`
- `code_a.dual_rows = independent row basis of h_a`
- `code_b.dual_rows = independent row basis of h_b`

The sector rows follow the repository contract's future CSS row assembly
semantics:

- `x_sector_rows = tensor(code_a.generator_rows, code_b.generator_rows)`
- `z_sector_rows = tensor(code_a.dual_rows, code_b.dual_rows)`

The tensor helper is a GF(2) Kronecker product over row bases, with output
columns ordered by A-major then B-minor local coordinates: `(a0,b0)`,
`(a0,b1)`, ..., `(a1,b0)`, and so on.

## Optional Generator Rows

The current fixture schema only requires check matrices. To support the issue's
negative control for check/generator consistency without changing the required
fixture shape, extend `local_codes` with optional `g_a` and `g_b` fields.

If a generator field is absent, derive the generator basis from the check rows.
If it is present, validate that:

- every generator row is binary
- every generator row has the same width as the corresponding check matrix
- every check row is orthogonal to every generator row over GF(2)
- the supplied generator rows have rank `width - rank(check_rows)`

When supplied generator rows pass validation, preserve their independent row
basis as the local code generator basis. This lets external tools provide a
specific local basis later without letting Rust trust inconsistent data.

`QuantumTannerLocalCodes` will store the optional parsed generator rows as
`g_a` and `g_b`. This is an additive change to the newly introduced quantum
Tanner parser type from #178.

## Errors

Reuse `QecError::InvalidQuantumTannerLocalCodeMatrix` for local-code validation
failures. Its `matrix` field identifies `h_a`, `h_b`, `g_a`, `g_b`, `code_a`,
or `code_b`, and its `reason` field reports the concrete validation failure.

This covers non-binary entries, inconsistent matrix widths, generator/check
non-orthogonality, and generator rank mismatch without adding broad error
vocabulary.

## Test Strategy

Add the required focused test:

```rust
#[test]
fn quantum_tanner_local_code_tensor_dual_repetition_example_rejects_bad_inputs()
```

The positive branch parses `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`
and verifies the hand-computed repetition-code local tensor example:

- `h_a = [[1, 1]]` and `h_b = [[1, 1]]`
- `C_A = C_B = span([[1, 1]])`
- `dual(C_A) = dual(C_B) = span([[1, 1]])`
- `x_sector_rows = [[1, 1, 1, 1]]`
- `z_sector_rows = [[1, 1, 1, 1]]`

The negative controls mutate the toric fixture in memory:

- `h_a[0][0] = 2` must return `InvalidQuantumTannerLocalCodeMatrix`
- adding `g_a = [[1, 0]]` must return `InvalidQuantumTannerLocalCodeMatrix`
  because it is not orthogonal to `h_a = [[1, 1]]`

Verification command:

```bash
cargo test -p qec-code quantum_tanner_local_code_tensor_dual -q
```

The broader Agent Desk verification will also run `cargo test`.

## Out Of Scope

This issue will not add semantic group validation, generator symmetry checks,
Cayley face enumeration, global sparse CSS matrix generation, CLI support,
distance computation, random local-code generation, or a public classical-code
library.

## Self-Review

- No incomplete markers remain.
- The design is local to `qec-code/src/codes/quantum_tanner.rs`.
- The design reuses existing GF(2) rank, nullspace, and independent-row helpers.
- Optional generator rows are validated rather than trusted.
- The repetition-code tensor output is hand-computable and tied to the existing
  `Z4 x Z4` toric Tanner fixture.
