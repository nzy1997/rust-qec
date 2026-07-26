# Issue 552 QEC Family Manifest Design

Date: 2026-07-26
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #552, Roadmap ID M1-01

## Summary

Issue #552 needs one machine-readable `qec-code` source of truth for the 14
requested QEC construction families. The manifest must separate target
disposition from runtime availability so later constructor work can promote
families without exposing placeholder APIs in this issue.

This design adds a versioned JSON fixture manifest under `qec-code/tests` plus a
colocated README schema and a typed Rust integration test. The implementation is
test-local by design: it validates the manifest contract and future executable
case capacity without adding constructors, registries, CLI commands, or public
runtime APIs.

The issue references `docs/design/2026-07-26-qec-code-family-support.md`. That
file is present on local `master` but not on this worker branch's
`origin/master` base. The fixture should be grounded in the issue body and the
design reference without merging unrelated base drift into this PR.

## Goals

- Add a versioned manifest containing exactly these family IDs:
  `directional`, `quantum_tanner`, `generalized_bicycle`, `la_cross`,
  `random_hgp`, `lifted_product`, `hyperbolic_5_5`, `coprime_bb`,
  `toric_3d`, `color_666`, `surface`, `shor_like`, `random_two_block`, and
  `perturbed_hgp`.
- Use typed lifecycle fields:
  `disposition in {supported, deferred}` and
  `availability in {planned, available, not_applicable}`.
- Enforce only these legal pairs:
  `(supported, planned)`, `(supported, available)`, and
  `(deferred, not_applicable)`.
- Mark exactly `hyperbolic_5_5` and `perturbed_hgp` as deferred with
  `availability = not_applicable`.
- Mark the other 12 families as supported with `availability = planned`.
- Ensure every entry has non-empty provenance, verification, and intended
  consumer fields.
- Let supported families declare future positive and negative executable cases,
  with at least one of each for every supported family.
- Reject callable constructors on deferred entries and planned entries.
- Verify deterministic parse and serialization of the versioned schema.
- Document the schema next to the manifest.

## Non-Goals

- Do not add family constructors.
- Do not add callable constructor references for planned or deferred families.
- Do not expose a public runtime registry or placeholder runtime APIs.
- Do not promote any family to `available`; issue #573 remains the sole gate for
  availability promotion after constructors and executable cases land.
- Do not validate constructor parameters or execute cases in this issue.

## Approaches Considered

### 1. Test-local JSON fixture and typed integration test

Place `manifest.v1.json` and `README.md` under
`qec-code/tests/fixtures/family_manifest/`. Add
`qec-code/tests/family_manifest.rs` with serde-backed enums and structs, exact
family validation, negative controls, and deterministic serialization checks.

Benefits:

- matches existing qec-code fixture-manifest patterns
- keeps the source of truth machine-readable and versioned
- avoids exposing placeholder public APIs
- directly supports the requested verification commands
- makes schema documentation live next to the data file

Costs:

- validation code is test-local until a later issue chooses to promote it

This is the selected approach.

### 2. Production manifest module without constructors

Add a production module that loads the manifest and exports typed family
metadata.

Benefits:

- downstream Rust code could consume the manifest immediately

Costs:

- risks creating a runtime API surface before constructors exist
- expands compatibility obligations beyond the issue
- makes "planned" entries look more operational than they are

This is not selected for issue #552.

### 3. Documentation-only table

Add the 14-family list to Markdown without a machine-readable fixture.

Benefits:

- simplest implementation

Costs:

- fails the source-of-truth requirement
- cannot enforce duplicates, typed lifecycle values, legal pairs, or negative
  controls

This is not selected.

## Manifest Shape

The manifest file is:

```text
qec-code/tests/fixtures/family_manifest/manifest.v1.json
```

Top-level fields:

- `schema_version`: integer, currently `1`
- `manifest_id`: string, `qec_family_construction_targets_v1`
- `provenance`: non-empty array of strings
- `verification`: non-empty array of strings
- `intended_consumers`: non-empty array of strings
- `availability_promotion_gate`: object recording issue `573`
- `families`: ordered array of 14 family entries

Each family entry contains:

- `schema_version`: integer, currently `1`
- `family_id`: normalized family ID
- `disposition`: `supported` or `deferred`
- `availability`: `planned`, `available`, or `not_applicable`
- `provenance`: non-empty array of strings
- `verification`: non-empty array of strings
- `intended_consumers`: non-empty array of strings
- `callable_constructor`: null in this issue
- `executable_cases`: array of future case descriptors

Supported planned families each include two future executable case descriptors:
one `positive` case and one `negative` case. Deferred entries include no cases.

## Validation

The `family_manifest` integration test should parse the fixture into typed Rust
structs:

- `FamilyDisposition`
- `RuntimeAvailability`
- `ExecutableCaseKind`
- `FamilyManifest`
- `FamilyManifestEntry`

Validation should:

- reject duplicate family IDs
- reject missing required fields through parse or validation errors
- reject unknown enum values through typed deserialization
- reject illegal disposition/availability pairs
- reject any deferred or planned entry with a callable constructor
- require exactly the 14 family IDs in the issue body
- require exactly two deferred IDs:
  `hyperbolic_5_5` and `perturbed_hgp`
- require all other entries to be supported and planned
- require non-empty provenance, verification, and consumer fields
- require at least one positive and one negative case for each supported family
- require deferred entries to have no executable cases
- compare pretty JSON serialization against the checked-in fixture text so
  parse and serialization are deterministic and versioned
- verify the README schema file exists and documents the lifecycle fields and
  legal pair policy

## Testing

Focused verification commands:

```text
cargo test -p qec-code --test family_manifest family_manifest_covers_requested_qec_families -- --exact
cargo test -p qec-code --test family_manifest family_manifest_rejects_invalid_entries -- --exact
```

Required broader verification:

```text
cargo test
```

The first focused command should pass only when the exact 14-family set and
supported/deferred split are present. The second should exercise every negative
control from the issue body.
