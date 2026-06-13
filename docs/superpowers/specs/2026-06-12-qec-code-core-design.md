# qec-code Core Library Design

Date: 2026-06-12
Status: Draft accepted in-session, written for review
Scope: Next milestone for the `qec-code` subpackage

## Summary

The next milestone for `qec-code` should turn it into a stronger research-oriented core library for general stabilizer codes. The main shift is architectural: move logical-operator analysis away from exhaustive Pauli enumeration and toward reusable GF(2) and symplectic linear algebra.

The user-facing goal is a small set of useful high-level APIs on `StabilizerCode`:

- `normalizer_basis()`
- `logical_basis()`
- `canonical_logical_basis()`

The implementation goal is to support those APIs with reusable algebraic internals rather than one-off search code.

## Current State

The crate already provides:

- binary linear algebra helpers in `binary.rs`
- `Pauli`
- `StabilizerCode`
- `CssCode`
- a built-in `Steane` example
- exact logical-basis extraction
- exact distance computation
- a CLI for Steane inspection

The current logical and distance routines rely on exhaustive enumeration over non-identity Paulis. This is acceptable for tiny examples, but it blocks the crate from becoming a general-purpose research core.

## Goals

This milestone should:

1. Replace exhaustive search in logical-operator analysis with symplectic linear algebra.
2. Support general stabilizer codes rather than using CSS structure as the main abstraction.
3. Support `k > 1` logical qubits in logical-basis extraction.
4. Provide a canonical logical basis with guaranteed commutation structure.
5. Keep the external API small and useful for research scripting.

## Non-Goals

This milestone should not:

1. Promise a scalable new distance algorithm.
2. Expose a large public matrix framework before the higher-level API stabilizes.
3. Prioritize adding many new code families.
4. Expand into decoding, circuits, or syndrome workflows.

`distance` may reuse new internals later, but its behavior is not the primary target of this design.

## Architecture

The crate should move toward four layers.

### 1. `gf2`

Generic GF(2) matrix primitives:

- rank
- row reduction / RREF or equivalent reduced form
- nullspace
- row-span membership
- independent-row extraction

This layer does not know about Paulis or stabilizer semantics.

### 2. `symplectic`

Binary symplectic utilities over length-`2n` rows:

- symplectic inner product
- commutation helpers
- conversions between Pauli data and symplectic rows
- helpers needed to compute normalizer structure

This layer understands the binary representation of Pauli operators, but it is still lower-level than code analysis.

### 3. `code` / `logical`

`StabilizerCode` remains the high-level entry point. Logical-analysis APIs live here and consume the lower layers.

This layer should expose:

- `normalizer_basis()`
- `logical_basis()`
- `canonical_logical_basis()`

### 4. `css` / `codes/*`

These modules remain constructor and example layers.

- `CssCode` stays as a convenient constructor.
- Built-in code families remain examples and fixtures.
- CSS should not drive the core abstraction.

## Public API

The next milestone should add a minimal, high-value API surface.

```rust
impl StabilizerCode {
    pub fn normalizer_basis(&self) -> Result<Vec<Pauli>>;
    pub fn logical_basis(&self) -> Result<LogicalBasis>;
    pub fn canonical_logical_basis(&self) -> Result<LogicalBasis>;
}
```

`LogicalBasis` should naturally support arbitrary `k`:

```rust
pub struct LogicalBasis {
    pub k: usize,
    pub logical_x: Vec<Pauli>,
    pub logical_z: Vec<Pauli>,
}
```

One additional low-level constructor should be added:

```rust
impl Pauli {
    pub fn from_symplectic_row(row: Vec<u8>) -> Result<Self>;
}
```

The `gf2` and `symplectic` modules should initially stay conservative in visibility. Prefer `pub(crate)` or narrowly exposed items until the high-level API has proven stable.

## API Semantics

### `normalizer_basis()`

Returns a generating set for the normalizer of the stabilizer group in binary symplectic form, lifted back to `Pauli` values. The return value does not need to remove stabilizer content; it is a basis for the full normalizer space, not a basis of logical representatives modulo stabilizers.

### `logical_basis()`

Returns `k` logical `X` representatives and `k` logical `Z` representatives such that:

- each operator commutes with every stabilizer
- the returned representatives are independent modulo the stabilizer space
- the result is valid for general `k > 1`

This API does not need to enforce canonical pair ordering beyond correctness of the logical representatives.

### `canonical_logical_basis()`

Returns a stronger result with guaranteed canonical pairing:

- `X_i` anticommutes with `Z_i`
- `X_i` commutes with `Z_j` for `i != j`
- all `X_i` commute with each other
- all `Z_i` commute with each other

This should be guaranteed by the algorithm, not obtained by opportunistic search.

## Algorithm Strategy

The design intentionally separates three algebraic objects:

1. the stabilizer row space
2. the normalizer space
3. the quotient of normalizer by stabilizer

The implementation should follow that structure.

### Step 1: Build reusable GF(2) infrastructure

Refactor the current free-function style helpers in `binary.rs` into a more reusable internal matrix layer. This does not require a heavy public abstraction, but it should provide a stable internal API for:

- validation
- reduction
- rank
- nullspace
- span membership
- extraction of independent generators

### Step 2: Compute the normalizer linearly

The normalizer should be obtained from the symplectic commutation constraints, not from enumerating all Paulis. Concretely, the stabilizer rows define a linear system whose solutions are exactly the symplectic rows commuting with the stabilizer generators.

The implementation should solve that system to obtain a basis for the normalizer space.

### Step 3: Pass to logical representatives

Logical representatives should be chosen from the normalizer space modulo the stabilizer row space. The implementation should explicitly track independence modulo stabilizers rather than checking only raw independence in the ambient `2n`-dimensional space.

### Step 4: Canonicalize pairings

Canonical logical pairs should be produced by a symplectic basis-selection or symplectic reduction procedure on the logical quotient space. The algorithm should enforce the desired commutation pattern rather than hoping an arbitrary basis already has it.

## Compatibility Requirements

This milestone must preserve:

- `CssCode::from_hx_hz(...)`
- existing `Steane` construction
- existing `Pauli` behavior
- current CLI behavior unless a change is directly required by the new API

Where old APIs are currently limited by exhaustive search, behavior may broaden, but existing working cases must remain correct.

## Milestones

### Milestone 1: Internal GF(2) and symplectic primitives

Deliver:

- reusable internal matrix helpers
- symplectic row conversions
- nullspace / rank / span utilities

Validation:

- pure algebra unit tests
- edge-case validation tests

### Milestone 2: `normalizer_basis()`

Deliver:

- normalizer-basis computation on `StabilizerCode`
- removal of exhaustive search from the normalizer path

Validation:

- every returned element commutes with all stabilizers
- stabilizers lie in the returned normalizer span
- basis dimension is correct on small examples

### Milestone 3: general `logical_basis()`

Deliver:

- support for `k > 1`
- logical representatives chosen from the normalizer quotient

Validation:

- lengths match `k`
- all representatives commute with stabilizers
- representatives are independent modulo stabilizers

### Milestone 4: `canonical_logical_basis()`

Deliver:

- canonical anticommuting pairs for general stabilizer codes

Validation:

- pairwise commutation / anticommutation relations hold exactly
- existing Steane behavior remains correct

## Test Strategy

Testing should scale with the layered design.

### Algebra tests

Keep low-level tests focused on:

- empty matrices
- zero rows
- dependent rows
- rank and nullspace sanity cases
- width mismatch and invalid bit validation

### Integration tests

Add integration tests that exercise code-analysis behavior through public APIs rather than only testing internals.

At minimum, include:

1. the existing Steane example
2. one small `k = 2` stabilizer example
3. one small non-CSS stabilizer example

The second and third cases are important because this milestone is specifically about breaking the current dependence on single-logical or CSS-flavored assumptions.

### Regression policy

Run `cargo test -p qec-code` after each milestone. Prefer adding targeted integration tests over piling every assertion into one existing test file.

## Risks

### Risk 1: Overdesigning the matrix layer

If the internal GF(2) abstraction becomes too generic too early, the milestone will stall in infrastructure work. The matrix layer should exist to serve `StabilizerCode`, not to become a standalone math framework yet.

### Risk 2: Conflating logical basis with distance

Logical-basis extraction and distance are related but not equivalent. Folding distance into this milestone would expand scope and obscure whether the logical APIs themselves are well designed.

### Risk 3: Hidden CSS assumptions

Even with a general stabilizer surface API, it is easy to accidentally bake CSS expectations into the tests or basis-selection logic. This is why at least one non-CSS example must be part of acceptance.

## Success Criteria

This design is successful when:

- `logical_basis()` works for small general stabilizer codes with `k > 1`
- `canonical_logical_basis()` returns a valid canonical pairing
- the implementation no longer depends on exhaustive Pauli enumeration for logical analysis
- existing CSS and Steane constructions still work
- tests cover at least one multi-logical and one non-CSS example

## Deferred Work

The following are intentionally deferred until after this milestone:

- scalable distance algorithms
- broader public exposure of `gf2` / `symplectic` APIs
- more built-in code families
- decoder-oriented workflows
