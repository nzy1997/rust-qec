# QEC Code Steane V1 Design

Date: 2026-06-09
Status: Proposed
Scope: new in-workspace `qec-code` crate for algebraic stabilizer-code modeling,
logical-operator extraction, and exact small-code distance analysis, with
Steane code as the first shipped example

## Summary

This design adds a new workspace crate, tentatively named `qec-code`, to the
existing repository.

The crate's responsibility is code-level algebra, not circuit simulation or
decoding:

- represent qubit stabilizer codes in a general binary symplectic form
- provide convenient constructors for common structured cases such as CSS codes
- ship Steane code as the first built-in example
- compute a validated logical-operator basis
- compute exact code distance for small codes
- expose a minimal CLI for inspection and experimentation

The chosen direction is:

- keep the new work inside the current git repository as a separate workspace
  crate
- use a general qubit stabilizer-code core instead of a CSS-only model
- provide CSS and built-in code constructors as convenience layers on top of
  the same core representation
- treat Steane code as the first fully supported example, not as a special-case
  architecture driver
- make distance computation exact for small codes in v1 using a correctness-
  first search algorithm
- leave a clean integration path for future ILP-backed distance engines without
  coupling v1 to `rilpqec`

## Goals

- Add a dedicated crate for algebraic quantum-code work without expanding
  `rstim` beyond its circuit/simulator scope.
- Support a general qubit stabilizer-code abstraction suitable for future
  non-CSS stabilizer codes.
- Make Steane code available as a built-in example with a direct constructor.
- Return a validated logical operator basis, not just arbitrary commuting
  representatives.
- Compute exact distance for small codes such as Steane and return a witness
  logical operator.
- Provide a small CLI so the new crate is usable immediately during development
  and debugging.
- Keep the design compatible with later ILP-backed distance computation and
  future collaboration with `rilpqec`.

## Non-Goals

- Do not generate syndrome-extraction circuits in this crate.
- Do not integrate the crate into `rstim` circuit generation in v1.
- Do not support external file import or export formats in v1.
- Do not support qudit codes, subsystem codes, or non-Pauli generalized
  operator systems in v1.
- Do not depend on `rilpqec` in v1.
- Do not optimize distance computation for large codes in v1.
- Do not design a broad plugin-like backend framework before concrete need
  appears.

## Current State

The repository already separates related concerns across workspace crates:

- `rstim` handles circuit representation, simulation, analysis, and CLI flows
- `rmatching` and `rbposd` handle decoder implementations
- `rilpqec` handles ILP decoding from detector error models

What is missing is a crate centered on the algebraic structure of a quantum
code itself:

- stabilizer generators as first-class objects
- logical operator extraction
- exact distance analysis
- reusable code constructors independent of circuit generation

That gap currently makes it awkward to experiment with code structure in a
clean way, especially when the desired workflow is "construct a code, inspect
its algebra, and verify basic invariants" instead of "simulate a circuit" or
"decode a syndrome".

## Alternatives Considered

### 1. CSS-first crate

This option would make `Hx` and `Hz` the true core representation and derive
all other behavior from CSS assumptions.

Benefits:

- smallest v1 implementation
- Steane fits naturally
- direct path to parity-check style APIs

Costs:

- does not generalize cleanly to arbitrary stabilizer codes
- risks forcing future non-CSS work through the wrong abstraction
- makes canonical logical-operator work more awkward once the code family
  broadens

This is not the recommended option.

### 2. General symplectic core with CSS conveniences

This option uses a binary symplectic stabilizer-code model as the real core,
then layers `CssCode`/`HxHz` conveniences and built-in examples on top.

Benefits:

- matches the chosen goal of a truly general stabilizer-code abstraction
- still keeps Steane ergonomics simple
- provides a stable internal representation for logical-basis and distance
  work
- gives a clean future path toward non-CSS stabilizer codes

Costs:

- more up-front implementation than a CSS-only crate
- requires a small linear-algebra foundation before visible features land

This is the recommended option.

### 3. Fully extensible trait-heavy framework from day one

This option would define a broad set of traits and backend interfaces before
the first concrete implementation is complete.

Benefits:

- maximal future flexibility in theory
- explicit seams for alternative backends

Costs:

- likely over-design for a first crate
- pushes speculative abstractions ahead of working code
- makes it easier to ship empty interfaces instead of validated behavior

This is not the recommended first step.

## Decision Summary

The new crate should be added to the current workspace and built around a
general binary symplectic stabilizer-code representation.

The architecture should be layered:

1. binary linear algebra
2. Pauli/symplectic operators
3. general stabilizer-code core
4. CSS convenience construction
5. built-in codes such as Steane
6. logical-operator extraction
7. exact small-code distance analysis
8. minimal inspection CLI

This keeps v1 focused while preserving a path to later growth.

## Recommended Architecture

### Workspace placement

Add a new crate, tentatively named `qec-code`, as another member of the
existing top-level Cargo workspace.

This placement is deliberate:

- it keeps development close to `rstim` and `rilpqec`
- it avoids prematurely splitting versioning and CI into a second repository
- it still preserves a clear ownership boundary at the crate level

The crate should not depend on `rstim` for its core algebraic types. The code
model needs to stand on its own.

### Module structure

The initial module layout should remain small and purpose-specific:

- `binary`: GF(2) vectors, matrices, rank/elimination helpers
- `pauli`: binary symplectic Pauli representation, weight, commutation checks
- `code`: core `StabilizerCode` type and validation
- `css`: CSS-specific constructors and checks
- `codes::steane`: built-in Steane constructor and metadata
- `logical`: logical-basis extraction
- `distance`: exact small-code distance computation
- `cli`: command-line front-end

The important design rule is that `codes::steane` and `css` are consumers of
the same core code model, not alternate cores.

### Core data model

The central type should be a general-purpose `StabilizerCode`.

Representative fields:

- `n: usize`: number of physical qubits
- `stabilizers: Vec<Pauli>`: an independent generating set in binary
  symplectic form
- cached derived facts as needed, such as stabilizer rank or a normalizer
  basis

The constructor must validate the algebraic invariants that define a valid
stabilizer code:

- each row has width consistent with `n`
- all stabilizer generators commute pairwise
- the generator list is linearly independent after mod-2 reduction

Primary construction paths should be:

- `StabilizerCode::from_symplectic_rows(...)`
- `CssCode::from_hx_hz(...)`
- `Steane::new()`

`Steane::new()` should be the ergonomic front door for the first built-in code,
but it must lower into the same underlying `StabilizerCode` representation used
everywhere else.

### CSS convenience layer

Although the core is general, v1 should make CSS entry straightforward.

`CssCode` should accept `Hx` and `Hz` matrices, validate the CSS commutation
condition, and then lower to the general symplectic code model. This lets v1
remain pleasant for structured codes while keeping the actual foundations
general enough for later non-CSS work.

The CSS layer is a constructor and validation convenience, not the main
semantic center of the crate.

## Logical Operator Design

The logical-operator API should return a verified basis, not ad hoc commuting
examples.

Representative result shape:

- `k: usize`
- `logical_x: Vec<Pauli>`
- `logical_z: Vec<Pauli>`

The returned operators must satisfy:

- each logical operator commutes with every stabilizer
- `logical_x[i]` anticommutes with `logical_z[i]`
- `logical_x[i]` commutes with `logical_z[j]` for `i != j`
- operators are nontrivial modulo the stabilizer span

The implementation should proceed in algebraic stages:

1. compute the stabilizer span and rank
2. compute the normalizer of the stabilizer group
3. work in the quotient space normalizer / stabilizer
4. organize a basis into symplectically paired logical X/Z operators

The exact internal algorithm may look like a symplectic Gram-Schmidt style
cleanup over quotient representatives. What matters for the design is the
observable contract: the crate returns a validated logical basis with stable
pairing semantics.

For Steane, this yields one logical X operator and one logical Z operator. The
same API must continue to work when later examples have `k > 1`.

## Distance Design

### Distance contract

Distance computation in v1 should be exact for small codes and should return a
certificate of what was found.

Representative result shape:

- `distance: usize`
- `witness: Pauli`
- `logical_class: XLike | ZLike | Mixed`

The witness must be verifiable as:

- commuting with all stabilizers
- not belonging to the stabilizer span
- having weight equal to the reported distance

### V1 algorithm

The v1 implementation should prioritize correctness and clarity over
performance.

The recommended algorithm is an exact search over candidate logical Paulis,
structured around the normalizer:

1. derive the normalizer
2. search for a lowest-weight Pauli in the normalizer that is not in the
   stabilizer span
3. stop at the first weight with a valid witness

The implementation may use direct weight-ordered enumeration, quotient-space
enumeration, or another exact small-code strategy, but it must remain easy to
test and reason about for cases like Steane.

This is sufficient for v1 because the target example is small and correctness
is the main requirement.

### Future ILP path

The design must leave a clean path to later ILP-backed distance computation,
but v1 should not depend on `rilpqec`.

The key reason is scope mismatch:

- `rilpqec` currently models ILP decoding from detector error models
- code distance requires a different front-end problem statement
- coupling the two crates now would distort the new crate's core abstraction

Instead, `qec-code` should keep its public distance API focused on the code
problem itself. A later phase can add an alternative engine, shared problem
lowering, or a common optimization crate once a concrete integration target is
clear.

## CLI Design

The CLI should stay intentionally small in v1 and focus on built-in codes.

Representative commands:

- `qec-code code steane summary`
- `qec-code code steane stabilizers`
- `qec-code code steane logicals`
- `qec-code code steane distance`

The CLI is for inspection, validation, and manual experimentation during
development. It should not try to solve file-format or workflow-integration
problems in the first version.

Default output should be human-readable text. Optional `--json` output is
useful if it falls out naturally from the implementation, but it is not a
required v1 deliverable.

## Testing Strategy

Testing should scale with the fact that the correctness burden is in the
algebraic foundation.

### 1. Binary and symplectic unit tests

These tests should cover:

- GF(2) row operations
- rank computation
- linear-independence checks
- symplectic inner product
- Pauli commutation and anticommutation

These are the base invariants for the entire crate.

### 2. Steane gold tests

Steane should have explicit behavior tests confirming:

- physical length is 7
- stabilizer rank is 6
- logical qubit count is 1
- returned logical X/Z operators satisfy the expected commutation relations
- exact distance is 3
- the reported witness is in the normalizer and not in the stabilizer span

The tests should validate properties, not brittle textual formatting.

### 3. CLI smoke tests

Run representative subcommands and assert that the expected fields or sections
appear in output.

This keeps the user entrypoint covered without overinvesting in CLI-specific
testing.

## Error Handling

The crate should fail explicitly on invalid algebraic inputs instead of
silently normalizing malformed data.

Representative errors include:

- inconsistent row widths
- noncommuting stabilizer generators
- linearly dependent stabilizer generator sets
- invalid CSS `Hx`/`Hz` commutation
- impossible or unsupported distance requests

These errors should be domain-specific enough that callers can understand
whether the issue is malformed input, unsupported scope, or an internal bug.

## V1 Scope Boundary

To keep the first implementation focused, v1 should stop at algebraic code
description and analysis.

Out of scope for this design:

- syndrome-extraction circuit generation
- integration with `rstim gen` or other circuit workflows
- decoder integration
- external file schemas
- large-code optimized distance algorithms
- direct `rilpqec` solver plumbing

Those items can be layered on later once the core algebraic model is stable and
validated.

## Implementation Notes

The crate name `qec-code` is tentative. The implementation plan can revisit the
final package name before code lands, but the design assumes a dedicated new
workspace member with the boundaries described above.

The most important architectural rule is this:

the code model must be the center, and everything else should lower to it.

That prevents Steane-specific ergonomics, CSS conveniences, and future solver
experiments from fragmenting the crate into competing representations.
