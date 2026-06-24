# Issue 180 Quantum Tanner Group Validator Design

Scope: GitHub issue #180, semantic finite-group validation for parsed
`QuantumTannerSpec` values in `qec-code`.

## Context

Issue #179 added the shared quantum Tanner fixture catalog under
`qec-code/tests/fixtures/quantum_tanner/`, including the valid `toric_d4`
`Z4 x Z4` table. Issue #178 added `qec-code::codes::quantum_tanner`, which
parses the v1 JSON shape into `QuantumTannerSpec` and performs only syntactic
and shape checks. Issue #180 is the next layer: prove that a parsed
multiplication table is a finite group before any Cayley-complex or CSS
construction consumes it.

The validator stays source-grounded in the qLDPC vocabulary referenced by the
catalog:

- `drafts/qLDPC/src/qldpc/objects.py` for `CayleyComplex` group expectations.
- `drafts/qLDPC/src/qldpc/codes/quantum.py` for `QTCode` consumption of group
  members.

## Approaches Considered

Recommended: add a small semantic validator beside the parser in
`qec-code/src/codes/quantum_tanner.rs`. It takes a parsed `QuantumTannerSpec`,
validates the explicit table and generator element indices, and returns a
private-field `ValidatedFiniteGroup` with identity, inverse, multiplication, and
safe generator accessors. This keeps the parser boundary from #178 intact and
keeps future construction code close to its validated input type.

Alternative: fold semantic validation into `quantum_tanner_spec_from_json_str`.
That would make every parsed fixture a proven group, but it would blur the
explicit parser-vs-validator boundary from #178 and make parser-only malformed
JSON tests harder to reason about.

Alternative: create a separate reusable finite-group module. That may become
useful later if more code families share group validation, but it is unnecessary
for this issue and risks implying broader group-generation or group-search
support that is explicitly out of scope.

## Selected Design

Add the public validator API in `qec-code/src/codes/quantum_tanner.rs`:

```rust
pub fn validate_quantum_tanner_group_table(
    spec: &QuantumTannerSpec,
) -> Result<ValidatedFiniteGroup>
```

`ValidatedFiniteGroup` owns the validated multiplication table, inverse lookup,
and validated `A`/`B` generator element indices. Its fields stay private so
callers cannot accidentally bypass validation. It exposes:

- `order() -> usize`
- `identity() -> usize`
- `multiply(left: usize, right: usize) -> Result<usize>`
- `inv(element: usize) -> Result<usize>`
- `a_generators() -> &[usize]`
- `b_generators() -> &[usize]`
- `a_generator(index: usize) -> Option<usize>`
- `b_generator(index: usize) -> Option<usize>`

The validator will not enumerate Cayley faces, check generator symmetry, compute
local-code algebra, generate CSS matrices, parse external group database
formats, or call GAP/Oscar.

## Validation Semantics

The validator checks these conditions in an explicit order:

1. `order` is positive.
2. the declared identity is in range.
3. the multiplication table has exactly `order` rows.
4. every row has exactly `order` entries.
5. every entry is `< order`; closure follows from this.
6. exactly one two-sided identity exists in the table.
7. the discovered table identity matches the spec's declared identity.
8. every element has exactly one two-sided inverse under that identity.
9. associativity holds for every triple `(a, b, c)`.
10. every `A` and `B` generator element index is `< order`.

Failures use typed `QecError` variants. Existing group-table failures continue
to use `InvalidQuantumTannerGroupTable { reason }`. Add
`InvalidQuantumTannerGeneratorIndex` for out-of-range generator element indices
and `InvalidQuantumTannerGroupElement` for safe accessor calls on invalid
runtime element arguments.

Associativity failure messages include the offending triple and both computed
values, so the negative control cannot pass just because every table cell is an
in-range index.

## Test Strategy

Use test-driven development in `qec-code/tests/code.rs` with focused tests whose
names include `quantum_tanner_group_table_validator` so the issue's verification
filter runs only this slice.

The positive coverage will include:

- a hand-built `Z2 x Z2` table, checking identity, multiplication, inverses, and
  safe generator access.
- the catalog `toric_d4` `Z4 x Z4` fixture parsed through
  `quantum_tanner_spec_from_json_str`, checking `order = 16`, identity `0`, and
  representative inverses/products.

The negative coverage will include:

- a square, in-range table with an identity and inverses but a deliberately
  broken associativity triple. The test must assert the validator reports
  associativity.
- out-of-range generator element indices.
- invalid runtime element access through `multiply` and `inv`.

Requested focused verification:

```bash
cargo test -p qec-code quantum_tanner_group_table_validator -q
```

Because this Agent Desk sandbox blocks crates.io network access, local
development and verification may use Cargo's offline mode when the cache already
contains the required crates. The exact requested command will still be run and
reported.

## Out Of Scope

This issue will not add group generation, subgroup search, conjugacy-class
enumeration, SmallGroup support, Cayley-complex face enumeration, generator
symmetry checks, CSS `Hx`/`Hz` generation, qLDPC/qTanner importers, or CLI
support.

## Self-Review

- No placeholders remain.
- The API consumes a parsed `QuantumTannerSpec` and returns a validated group
  value, matching the issue interface.
- Parser shape checks from #178 remain separate from semantic finite-group
  validation.
- The design includes the catalog `toric_d4` positive case and a square,
  in-range non-associative negative control.
- Out-of-scope search, constructor, and importer behavior is explicitly
  excluded.
