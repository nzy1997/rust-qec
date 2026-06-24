# Issue 177 Quantum Tanner Contract Design

Scope: GitHub issue #177, developer-facing documentation for the future
`qec-code` quantum Tanner construction path.

## Context

`qec-code` already owns CSS construction helpers, sparse-row JSON handling, and
developer contracts such as `qec-code/doc/apm_css.md`. Issue #177 asks for a
similar contract for a future middle-shape quantum Tanner path. The Rust crate
must not become a GAP, Oscar, or group-search system. It should consume explicit
finite-group data, generator indices, and local GF(2) code matrices, validate
that data, and emit deterministic sparse CSS matrices in later implementation
issues.

The relevant reference implementation lives in the ignored local qLDPC clone:

- `drafts/qLDPC/src/qldpc/codes/quantum.py`, especially `QTCode`,
  `QTCode.get_subgraphs`, and `QTCode.get_subcodes`.
- `drafts/qLDPC/src/qldpc/objects.py`, especially `CayleyComplex`, cover
  handling, total no-conjugacy, symmetric generator validation, and face
  semantics.
- `drafts/qLDPC/src/qldpc/codes/quantum_test.py`, especially
  `test_toric_tanner_code`, where `Z_d x Z_d` with the repetition seed code
  has parameters `[[d^2, 2, d]]`.

The public upstream qLDPC project documents a broad toolkit that also integrates
with external tools such as GAP and MAGMA. QuantumExpanders.jl uses broader
quantum Tanner vocabulary around left-right Cayley complexes, generating sets
`A` and `B`, local coordinate sets, CSS matrices, and cover-style construction
modes. The `qec-code` contract should cite those sources but keep the Rust v1
surface narrower.

## Selected Approach

Create `qec-code/doc/quantum_tanner.md` and a focused doc-backed integration
test in `qec-code/tests/code.rs`.

The contract will define one supported v1 mode:

```text
lr_cayley_no_cover_v1
```

It will explicitly list cover-mode names that are reserved but unsupported in
v1 and require future code to reject them with a typed unsupported-mode error:

```text
lr_cayley_bipartite_double_cover_v1
lr_cayley_quadripartite_cover_v1
```

This keeps no-cover, bipartite cover, and quadripartite cover semantics out of
folklore while avoiding implementation work for cover enumeration in this issue.

## Contract Content

The Markdown document will specify:

- finite-group input as a zero-based explicit multiplication table,
  rectangular `order x order`, with identity index `0`
- `A` and `B` as generator index arrays that refer to base-group element
  indices in v1
- required generator symmetry, with a named bad example that omits an inverse
- local binary code matrices over GF(2), with widths matching `|A|` and `|B|`
- deterministic construction output as two `sparse_rows` matrices compatible
  with `qec-code/src/css.rs`
- validation boundaries and typed error categories for invalid group data,
  invalid generators, invalid local codes, unsupported modes, and construction
  failures
- the explicit boundary that external tools may produce group tables and
  generator sets, but `qec-code` only consumes validated explicit data
- face canonicalization into physical-qubit ids
- a `Z4 x Z4` toric Tanner known-answer example with `n = 16`, `k = 2`, and
  expected distance `4`

For the toric `d=4` example, the document will explain that
`lr_cayley_no_cover_v1` uses the base group directly. It considers oriented face
records `(g, a, b)`, computes their four base-group vertices
`{g, a*g, g*b, a*g*b}`, canonicalizes that unordered vertex set, and assigns one
physical-qubit id per distinct canonical face. With
`|G| = 16`, `|A| = |B| = 2`, there are `64` oriented records and each square is
seen from four orientations, so there are `16` physical qubits.

## Test Strategy

Add `quantum_tanner_contract_examples_compile` to `qec-code/tests/code.rs`.
The test will:

- include `qec-code/doc/quantum_tanner.md`
- require the supported and reserved construction-mode vocabulary
- require a marked construction-mode/count explanation for `toric_d4`
- require a marked bad non-symmetric generator example
- parse a documented JSON `toric_d4` example from the Markdown
- validate the documented `Z4 x Z4` multiplication table shape
- validate generator symmetry for the good example
- canonicalize documented face records and verify `n = 16`
- verify the documented known-answer metadata `k = 2` and expected distance `4`
- parse the documented bad example and verify the listed generator set is not
  symmetric

This makes the required negative controls concrete: removing the bad generator
example or the construction-mode/count explanation will fail the test.

## Out Of Scope

This issue will not implement a parser, validator, Cayley-complex enumerator,
CLI command, group search, SmallGroup import, GAP/Oscar integration, Morgenstern
or Ramanujan workflows, or quantum Tanner matrix generation.

## Self-Review

- No placeholders remain.
- The scope is one Markdown contract plus one test.
- The v1 mode decision is explicit and unsupported cover modes are rejected by
  contract rather than silently deferred.
- The toric `n = 16` counting convention is tied to deterministic face
  canonicalization.
