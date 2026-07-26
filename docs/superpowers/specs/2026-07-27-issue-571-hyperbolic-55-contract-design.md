# Issue 571 Hyperbolic {5,5} Contract Design

Date: 2026-07-27
Status: Accepted by non-interactive Agent Desk standing policy
Scope: GitHub issue #571, pure-Rust quotient contract for parameterized hyperbolic `{5,5}` codes

## Summary

Add an implementation-ready research contract at
`qec-code/doc/hyperbolic_5_5_contract.md` and a source-level deferred contract
test. The document will define a versioned serializable quotient input, typed
failure modes, deterministic flag-orbit cellulation, canonical vertex/edge/face
ordering, boundary validation, orientability/torsion checks, and pure-Rust
resource limits.

The issue remains documentation and guardrail work only. No constructor, CLI
route, runtime adapter, or public `hyperbolic_5_5` stub is added.

## Context

Issue #552 introduced the QEC family manifest and marked `hyperbolic_5_5` as
`deferred` with `availability=not_applicable`. Issue #565 introduced pure-Rust
binary chain complexes with checked `boundary * boundary = 0`, which this
contract can target later. The referenced design file
`docs/design/2026-07-26-qec-code-family-support.md` is not present in this
checkout, so the design is grounded in the merged family manifest, merged chain
complex module, issue text, and the cited small stellated dodecahedron paper.

The cited paper records the small stellated dodecahedron as a hyperbolic
`{5,5}` code with `[[30,8,3]]`, 12 X checks, 12 Z checks, and weight-5 checks.

## Goals

- Specify a versioned, JSON-serializable input contract for supplied
  permutation quotients.
- Compare supplied permutation quotients with subgroup/enumeration inputs and
  state when to split future implementation issues.
- Define flag-orbit reconstruction of vertices, edges, faces, edge qubits, and
  binary boundary maps.
- Require canonical ordering independent of hash-map iteration.
- Define validation for Coxeter relations, quotient transitivity, incidence,
  orientability, torsion, and nonzero boundary composition.
- Pin the small stellated dodecahedron fixture values exactly:
  `V=12`, `E=30`, `F=12`, `[[30,8,3]]`, `m_x=m_z=12`,
  `rank_x=rank_z=11`, and check weights 5.
- Include a negative quotient fixture that returns `InvalidCoxeterQuotient` and
  names the failed relation.
- Keep `hyperbolic_5_5` unavailable until the fixture reconstructs under 5
  seconds and 512 MiB in the standard test environment.

## Non-Goals

- Do not add a callable runtime constructor or stub.
- Do not implement quotient enumeration in this issue.
- Do not add general subgroup enumeration or external computer-algebra
  dependencies.
- Do not treat an explicit incidence list as a general constructor.

## Alternatives Considered

### 1. Document-only Contract plus Deferred Test

Create the research contract document and a Rust test that enforces required
sections, exact fixture fields, the negative control, and absence of callable
runtime surface. This is the recommended option because it matches the issue's
deliverable and prevents accidental support claims.

### 2. Add Rust Serde Types Now

Adding input and error enums would make future implementation easier, but it
would create public API surface before the quotient semantics are reconstructed
and tested. This conflicts with the issue's "no callable runtime stub" boundary.

### 3. Add a Fixture Catalog beside the Document

A separate JSON fixture catalog could be useful later, but this issue names the
research contract document as the deliverable. Extra fixture files would broaden
the change without making the future constructor more grounded.

## Decision

Use option 1. The PR will add:

- `qec-code/doc/hyperbolic_5_5_contract.md`
- `qec-code/tests/deferred_contracts.rs`

The contract will define the future input schema as `schema_version = 1` and
`construction = "hyperbolic_5_5_quotient"`. The first implementation issue can
consume a supplied permutation quotient. If future work needs subgroup input
without a supplied permutation action, it must split into separate quotient
enumeration and cellulation issues.

## Test Strategy

Write the deferred contract test first. The red test will fail because
`qec-code/doc/hyperbolic_5_5_contract.md` does not exist. The green step adds
the document with exact markers and required fields. The test will also scan
the `qec-code/src` tree for forbidden callable names so this issue cannot add a
runtime `hyperbolic_5_5` constructor by accident.

Verification:

```text
cargo test -p qec-code --test deferred_contracts hyperbolic_5_5_contract_is_complete_and_deferred -- --exact
cargo test -p qec-code
cargo test
```

## Self-Review

Placeholder scan: passed. Scope check: this is one contract document plus one
source-level guard test. Ambiguity check: because this is a non-interactive
Agent Desk run, design approval and spec review were accepted under the
standing policy.
