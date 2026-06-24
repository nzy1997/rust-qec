# Issue 178 Quantum Tanner Spec Parser Design

Scope: GitHub issue #178, a `qec-code` parser for explicit quantum Tanner JSON
input.

## Context

Issue #177 added `qec-code/doc/quantum_tanner.md`, which is the source of truth
for the v1 input field names and construction-mode vocabulary. Issue #179 added
the shared fixture catalog under `qec-code/tests/fixtures/quantum_tanner/`,
including the positive `toric_d4` fixture and the malformed-table negative
fixture that this parser issue must consume.

The parser sits before the future semantic validators and constructor. It should
turn JSON into typed Rust data and reject malformed JSON/table/local-code shapes,
but it must not prove group axioms, check generator symmetry, enumerate Cayley
faces, compute local tensor/dual spaces, generate `Hx`/`Hz`, or add CLI support.

## Approaches Considered

Recommended: add a small serde-backed module at
`qec-code/src/codes/quantum_tanner.rs`. The module exposes typed structs,
`QuantumTannerConstructionMode`, and a string parser
`quantum_tanner_spec_from_json_str`. It performs only syntax and shape checks
needed to produce trustworthy typed data. This matches existing `serde_json`
patterns and keeps the parser close to future construction code.

Alternative: keep all parser code in tests until constructor work begins. This
would satisfy one fixture test but would not provide the requested public parser
value for later issues.

Alternative: introduce a reusable schema framework. This is unnecessary for the
small v1 JSON surface and would violate the issue guidance to avoid broad schema
machinery unless existing serde patterns require it.

## Selected Design

Add `qec-code/src/codes/quantum_tanner.rs` and export it from
`qec-code/src/codes/mod.rs`.

The runtime API will be:

```rust
pub fn quantum_tanner_spec_from_json_str(input: &str) -> Result<QuantumTannerSpec>
```

`QuantumTannerSpec` stores:

- `construction_mode: QuantumTannerConstructionMode`
- `base_group: ExplicitFiniteGroup`
- `a_generator_indices: Vec<usize>`
- `b_generator_indices: Vec<usize>`
- `local_codes: QuantumTannerLocalCodes`

The parser accepts the contract/catalog field names:
`construction_mode`, `base_group`, `a_generator_indices`,
`b_generator_indices`, and `local_codes`. This follows #177 and #179 even
though the issue prose also mentions older shorthand names such as `group` and
`code_a`.

`QuantumTannerConstructionMode` supports exactly
`lr_cayley_no_cover_v1`. Any other string returns a typed unsupported-mode
error. The parser ignores fixture-only fields such as `fixture_id`, preserving
catalog compatibility without making fixture metadata part of the runtime
contract.

## Parse-Time Validation Boundary

The parser validates only data-shape constraints needed before a constructor can
safely receive the spec:

- JSON must deserialize into the v1 object shape.
- `base_group.order` must be nonzero.
- `base_group.identity` must be in range.
- `base_group.multiplication_table` must have exactly `order` rows.
- every multiplication-table row must have exactly `order` entries.
- every multiplication-table entry must be `< order`.
- `local_codes.matrix_role` must be `parity_check`.
- `local_codes.field` must be `GF(2)`.
- `local_codes.h_a` and `h_b` entries must be binary.
- every `h_a` row width must equal `a_generator_indices.len()`.
- every `h_b` row width must equal `b_generator_indices.len()`.

The parser intentionally does not validate generator symmetry, duplicate
generators, generator range, identity laws, inverses, associativity,
nondegenerate faces, CSS orthogonality, or distance metadata. Those checks belong
to later semantic validation and construction issues.

## Errors

Add typed `QecError` variants scoped to the parser:

- `InvalidQuantumTannerSpecJson(String)`
- `InvalidQuantumTannerGroupTable { reason: String }`
- `UnsupportedQuantumTannerConstructionMode { mode: String }`
- `InvalidQuantumTannerLocalCodeMatrix { matrix: &'static str, reason: String }`

These are typed enough for tests and later CLI work to match causes without
parsing display text, while avoiding premature constructor error variants.

## Test Strategy

Add `quantum_tanner_spec_json_accepts_toric_d4_and_rejects_bad_table` in
`qec-code/tests/code.rs`.

The test will first be written before implementation. The positive branch reads
the committed `toric_d4` catalog fixture and verifies the parsed value exposes:

- `16` group elements
- two `A` generators
- two `B` generators
- construction mode `lr_cayley_no_cover_v1`
- local repetition seed matrices `[[1, 1]]` for both local code inputs

The negative branch reads the committed `invalid_bad_table` catalog fixture and
requires `InvalidQuantumTannerGroupTable` before any constructor API exists.

Verification command:

```bash
cargo test -p qec-code quantum_tanner_spec_json_accepts_toric_d4_and_rejects_bad_table -q
```

## Out Of Scope

This issue will not add semantic group validation, generator symmetry checks,
Cayley-complex enumeration, local-code algebra, CSS matrix generation, CLI
support, qLDPC/qTanner importers, or external group-search integration.

## Self-Review

- No placeholders remain.
- The accepted JSON names are explicitly tied to #177 and #179.
- The parser boundary rejects malformed table shape while deferring group
  algebra and constructor behavior.
- The design uses serde and small typed Rust values without a broad schema
  framework.
