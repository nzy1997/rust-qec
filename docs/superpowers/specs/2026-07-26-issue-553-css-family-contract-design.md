# Issue 553 CSS Family Contract Design

Date: 2026-07-26
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #553, Roadmap ID M1-02

## Summary

Issue #553 adds the production contract that sits above the existing `qec-code`
CSS constructors. The contract must preserve all legacy entry points and matrix
fixtures while giving new family work one typed, versioned route.

The selected design is a three-layer API:

1. `RequestedFamilyId` is the closed, serializable set of exactly the 14
   manifest IDs from issue #552.
2. `CssFamilySpec` contains only requested-family variants with working
   constructors in this branch.
3. `CssConstructionSpec` routes versioned requests and also contains generic
   utilities and legacy adapters that are not requested-family manifest entries.

This keeps planned roadmap families from appearing as callable stubs, and it
prevents generic HGP or legacy aliases from claiming a requested-family identity.

## Existing Context

- `qec-code/src/codes/built_in_css.rs` owns legacy inline parsing and built-in
  CSS sparse supports for Steane, repetition chains, rotated surface, toric,
  bivariate bicycle, BB72, and APM Kasai presets.
- `qec-code/src/codes/quantum_tanner.rs` owns the structured quantum Tanner
  JSON constructor.
- `qec-code/src/css.rs` owns sparse-row validation, dense conversion, CSS
  validation, and sparse-row JSON serialization.
- `qec-code/tests/fixtures/family_manifest/manifest.v1.json` is the #552 source
  of truth for the 14 requested family IDs.
- The referenced `docs/design/2026-07-26-qec-code-family-support.md` exists on
  local `master` but is absent from this worker branch's `origin/master`.

## Approaches Considered

### 1. Layered contract with adapters - selected

Add a focused construction module that parses inline and JSON requests into the
same typed spec, canonicalizes sparse checks, computes shared statistics, and
returns deterministic metadata. Existing constructors remain callable.

Benefits:

- satisfies the issue's public-layer separation
- preserves byte-for-byte legacy sparse-row JSON
- keeps planned families absent from callable APIs
- keeps generic HGP and legacy aliases outside the requested-family set
- gives future family issues one versioned route to extend

Costs:

- duplicates a small amount of mapping from legacy built-in IDs to typed
  construction IDs

### 2. Expand `BuiltInCssCodeSpec`

Add requested-family and generic utility variants to the existing built-in enum.

Benefits:

- fewer files initially

Costs:

- mixes manifest identities, legacy aliases, and generic utilities
- makes planned families more likely to become stubs
- makes structured JSON routing awkward

This is not selected.

### 3. Public manifest registry first

Promote the #552 manifest schema into production and drive construction through
manifest entries.

Benefits:

- direct link between manifest metadata and construction

Costs:

- #552 intentionally kept the manifest test-local
- planned entries would look operational before they have constructors
- unnecessary for preserving current constructors

This is not selected.

## Public Contract

### RequestedFamilyId

`RequestedFamilyId` is a public enum that serializes with `snake_case` to exactly:

```text
directional
quantum_tanner
generalized_bicycle
la_cross
random_hgp
lifted_product
hyperbolic_5_5
coprime_bb
toric_3d
color_666
surface
shor_like
random_two_block
perturbed_hgp
```

It exposes `ALL` and `as_str()` so tests and future manifest readers can compare
the closed set without ad hoc string lists. Generic utilities and legacy aliases
must never be added to this enum.

### CssFamilySpec

`CssFamilySpec` contains only requested-family constructors that work now:

- `Surface(SurfaceFamilySpec)` adapts the existing square rotated surface
  constructor and records requested-family ID `surface`.
- `QuantumTanner(QuantumTannerSpec)` adapts the existing structured quantum
  Tanner constructor and records requested-family ID `quantum_tanner`.

No variants are added for directional, generalized bicycle, La-cross, random
HGP, lifted product, coprime BB, 3-D toric, color, Shor-like, random two-block,
hyperbolic `{5,5}`, or perturbed HGP in this issue. Generic bivariate bicycle
and HGP are utilities, not requested-family entries.

### CssConstructionSpec

`CssConstructionSpec` is the routing layer. It contains:

- `Family(CssFamilySpec)` for requested-family constructors.
- `HypergraphProduct(HypergraphProductSpec)` for a generic utility.
- `LegacyBuiltIn(LegacyBuiltInCssSpec)` for documented adapters such as
  `surface_rotated:d=3`, `steane`, `bb72`, `bb:*`, `toric:*`, repetitions, and
  APM Kasai presets.

Inline CLI syntax routes through `CssConstructionSpec::from_inline`. The rule is:

- if the compact input is a documented requested-family inline form, lower to
  `Family(...)`;
- otherwise, if the compact input is an existing legacy built-in CSS code ID,
  lower to `LegacyBuiltIn(...)`;
- structured JSON must include `schema_version` and `construction`; version `1`
  is supported and every other version returns a typed unsupported-version
  error before construction.

For the current branch, the documented requested-family inline form is
`surface_rotated:d=<distance>`, which normalizes to `construction_id =
"surface_rotated"` and `requested_family_id = "surface"`. The same normalized
spec must be produced by Rust API construction, inline parsing, and JSON parsing.

### CssConstructionResult

Every successful construction returns:

- `schema_version`
- `construction_id`
- optional `requested_family_id`
- `normalized_parameters`
- canonical sparse `checks` with `h_x` and `h_z`
- `stats` with `n`, `m_x`, `m_z`, `rank_x`, `rank_z`, and `k`
- `provenance`

Sparse rows are canonicalized at the contract boundary by sorting, deduping, and
validating support indices. The shared verifier checks `H_X H_Z^T = 0` before a
result is returned. Metadata uses `BTreeMap` and deterministic vectors so
serialization is stable.

## Error Handling

New user-input failures return typed `QecError` variants. The contract must
reject:

- unsupported construction schema versions
- malformed construction JSON
- unknown construction IDs
- missing, duplicate, malformed, or out-of-range inline parameters through the
  existing typed errors where legacy parsing already owns them
- non-canonical or non-orthogonal generated checks

Unsupported JSON schema version `2` must return the unsupported-version error
and must not silently fall back to version `1`.

## CLI Contract

Existing CLI behavior remains available. Internally, `code css <CODE_ID> <MATRIX>`
and `code css export <CODE_ID> <MATRIX>` route through
`CssConstructionSpec::from_inline` before matrix serialization. The output stays
the existing `sparse_rows` JSON, so `surface_rotated:d=3` remains byte-identical
to its current fixtures.

Structured constructor JSON uses the same fields as the Rust API and is exported
with `code css construct --spec <path> hx` or
`code css construct --spec <path> hz`. That route parses the versioned JSON,
lowers it to `CssConstructionSpec`, and uses the same `construct_css` matrix
generation path as compact inline inputs.

## Testing

Add `qec-code/tests/family_contract.rs` with the five issue-required exact tests:

- `unified_family_contract_preserves_surface_d3`
- `unified_family_contract_rejects_unknown_schema`
- `inline_json_and_rust_routes_lower_to_same_spec`
- `planned_families_have_no_callable_stub`
- `generic_construction_identity_is_not_a_requested_family`

The tests verify the closed requested-family set, planned-family absence from
callable specs, generic HGP identity separation, shared orthogonality checking,
surface D3 stats, deterministic metadata serialization, and byte-for-byte legacy
fixture preservation.

Required verification remains the five exact commands from the issue plus
`cargo test`.
