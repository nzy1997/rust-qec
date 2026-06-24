# Issue 179 Quantum Tanner Fixture Catalog Design

Scope: GitHub issue #179, the first shared fixture catalog for future
`qec-code` quantum Tanner work.

## Context

Issue #177 added `qec-code/doc/quantum_tanner.md`, which defines the v1
construction contract without implementing a parser, validator, constructor, or
CLI. Issue #179 is the durable acceptance-data layer that later implementation
issues should consume. The fixture catalog must therefore be ordinary JSON files
and a manifest that can be checked by tests before any `QuantumTannerSpec`
parser exists.

The positive known-answer case comes from the qLDPC toric Tanner test
vocabulary: `Z_d x Z_d` with `A = {x, x^-1}`, `B = {y, y^-1}`, and repetition
seed checks gives a rotated toric code with parameters `[[d^2, 2, d]]`. For
`d = 4`, the contract in #177 fixes the no-cover physical-qubit convention at
`n = 16`.

## Selected Approach

Create `qec-code/tests/fixtures/quantum_tanner/` with:

- `manifest.json` as the catalog index.
- `toric_d4.json` as the valid known-answer input fixture.
- `invalid_non_symmetric_a.json` as the constructor-facing rejection fixture.
- `invalid_bad_table.json` as the parser/table-shape rejection fixture for the
  immediate follow-up parser issue.

The extra invalid table fixture keeps the catalog small while preventing issue
#178 from inventing a separate bad-table fixture outside the shared catalog.

## Manifest Shape

The manifest will use a simple project-local schema:

```text
schema_version: 1
manifest_id: "quantum_tanner_acceptance_v1"
contract: { issue: 177, path, construction_mode }
verifier_command: "cargo test -p qec-code quantum_tanner_fixture_catalog_has_grounded_cases -q"
entries: [...]
```

Every entry records:

- `fixture_id`
- `input_path`
- `contract_reference`
- `provenance`
- `references`
- `expected_result`
- `verifier_command`
- `consuming_issues`

Successful fixtures use:

```text
expected_result.kind = "success"
expected_result.n/k/d/check_weight
```

Rejected fixtures use:

```text
expected_result.kind = "rejection"
expected_result.reason
```

The toric fixture provenance will explicitly state that it is reference-derived
known-answer data, not copied qLDPC implementation code.

## Test Strategy

Add `quantum_tanner_fixture_catalog_has_grounded_cases` to
`qec-code/tests/code.rs`. The test will load the manifest and every listed
fixture from disk, then verify:

- manifest schema and command fields are present
- every entry has provenance, references, input path, contract reference,
  verifier command, expected result, and consuming issue numbers
- every input path exists under `qec-code/tests/fixtures/quantum_tanner/`
- `toric_d4` has `n = 16`, `k = 2`, `d = 4`, and check weight `4`
- rejected fixtures include nonempty rejection reasons
- the positive fixture has the expected contract mode, group order, generator
  counts, and repetition seed check matrices
- the non-symmetric negative fixture actually omits the inverse generator
- the bad-table negative fixture has a malformed multiplication-table shape

The negative-control behavior is covered by making validation return structured
errors when required fields are removed or a rejection reason is missing.

## Out Of Scope

This issue will not implement a `QuantumTannerSpec` parser, semantic group
validator, Cayley-complex enumerator, local-code algebra, CSS constructor, CLI
path, distance integration, qTanner importer, qLDPC importer, or external group
search tooling.

## Self-Review

- No placeholders remain.
- The fixture paths, manifest id, schema version, verifier command, and expected
  toric parameters are explicit.
- The design remains catalog-only and does not add runtime parser or constructor
  APIs.
- The catalog includes source-grounded references and future consuming issues so
  later work can reuse the same fixtures.
