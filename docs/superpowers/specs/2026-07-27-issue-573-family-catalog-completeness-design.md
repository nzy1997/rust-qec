# Issue 573 Family Catalog Completeness Design

Date: 2026-07-27
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #573, Roadmap ID M4-03

## Summary

Issue #573 promotes the #552 requested-family manifest from a planned roadmap
fixture into a hard executable catalog gate. The gate must prove that exactly
the 14 requested family IDs exist, that exactly 12 are available and 2 are
deferred, and that each available family has both a callable requested-family
constructor and positive/negative executable cases.

The selected design keeps the manifest as canonical deterministic JSON under
`qec-code/tests/fixtures/family_manifest/manifest.v1.json`, replaces the old
`family_manifest` test with a stricter `family_catalog` integration test, and
adds the missing requested-family route for `lifted_product` without removing
the existing `CssConstructionSpec::LiftedProduct` compatibility path.

The repository branch does not contain the referenced
`docs/design/2026-07-26-qec-code-family-support.md`; earlier #552/#553 specs
record the same absence. This design uses the issue #573 body plus the existing
#552 manifest, #553 family-contract design, and deferred-family contract docs as
binding local context.

## Goals

- Enforce the exact requested-family manifest:
  `directional`, `quantum_tanner`, `generalized_bicycle`, `la_cross`,
  `random_hgp`, `lifted_product`, `hyperbolic_5_5`, `coprime_bb`,
  `toric_3d`, `color_666`, `surface`, `shor_like`, `random_two_block`, and
  `perturbed_hgp`.
- Mark exactly 12 supported families `availability = available`.
- Keep exactly `hyperbolic_5_5` and `perturbed_hgp` as
  `availability = not_applicable`.
- Require every available family to declare normalized inputs, provenance,
  expected dimensions and ranks, row-weight summaries, a distance-verification
  class, an executable verifier, at least one consumer, and at least one
  positive and one negative executable case.
- Execute every available positive case through `parse_css_construction_json`
  and `construct_css`, then compare requested-family ID, expected stats,
  row-weight summaries, provenance, and deterministic serialization.
- Execute every available negative case and require a typed rejection.
- Prove a bijection between `RequestedFamilyId::ALL` and the manifest.
- Prove every available manifest family has a callable `CssFamilySpec` variant
  and neither deferred family does.
- Keep generic utilities and documented legacy aliases out of the requested
  manifest by checking them through a separate `CssConstructionSpec` registry.

## Non-Goals

- Do not implement `hyperbolic_5_5` or `perturbed_hgp` constructors.
- Do not add CLI aliases or callable stubs for deferred families.
- Do not remove existing public construction routes that already compile, such
  as `CssConstructionSpec::LiftedProduct`.
- Do not add new distance algorithms; the catalog verifier uses constructor
  metadata, orthogonality, exact rank checks, and existing known-distance fields.

## Approaches Considered

### 1. Test-owned catalog gate with production registry hooks

Keep the manifest fixture test-owned, but validate it against production
`RequestedFamilyId`, `CssFamilySpec`, and `CssConstructionSpec` registry
functions. Add only the missing production route needed to make a supported
family callable: `CssFamilySpec::LiftedProduct(LiftedProductSpec)`.

Benefits:

- smallest production API expansion required by the issue
- preserves the #552 machine-readable source of truth
- makes the exact requested-family set visible in one deterministic fixture
- lets negative controls mutate fixture values and registry inputs directly
- preserves existing generic HGP and legacy routes

Costs:

- catalog validation remains an integration-test gate instead of a runtime
  loader API

This is the selected approach.

### 2. Promote the manifest into a runtime registry

Move the manifest schema into production and load the JSON at runtime.

Benefits:

- downstream crates could inspect catalog records directly

Costs:

- expands public compatibility obligations beyond the issue
- risks conflating test completeness data with runtime construction APIs
- does not improve constructor coverage for this milestone

This is not selected.

### 3. Encode the catalog entirely in Rust tests

Delete the JSON manifest and build the catalog in test code.

Benefits:

- fewer serde schema types

Costs:

- loses deterministic fixture serialization
- makes provenance and consumer metadata less reviewable
- weakens the manifest-as-source-of-truth requirement from #552

This is not selected.

## Manifest Shape

`qec-code/tests/fixtures/family_manifest/manifest.v1.json` remains canonical
pretty JSON. The top-level manifest keeps `schema_version`, `manifest_id`,
global provenance, verification commands, intended consumers, and
`availability_promotion_gate`.

Each family entry records:

- lifecycle fields: `family_id`, `disposition`, `availability`
- metadata: `provenance`, `research_contracts`, `intended_consumers`
- constructor routing: `callable_constructor` for available families, null for
  deferred families
- catalog evidence: `normalized_inputs`, `expected`, `row_weight_summary`,
  `distance_verification`, and `executable_verifier`
- executable cases: ordered case records with `case_kind`,
  `expected_outcome`, a JSON `request`, and expected rejection text for negative
  cases

Supported available entries each use one small positive fixture and one small
negative fixture. Deferred entries link their research contracts and declare no
constructor or executable cases.

## Production Contract Changes

`CssFamilySpec` gains:

```rust
LiftedProduct(LiftedProductSpec)
```

Versioned JSON construction `"lifted_product"` lowers to that family variant.
The existing `CssConstructionSpec::LiftedProduct` route remains available for
callers that construct it directly; it continues to return the same CSS checks.
The family route records provenance source `CssFamilySpec::LiftedProduct`.

`CssFamilySpec::callable_requested_family_ids()` is reordered to match the
issue's supported-family manifest order and includes all 12 available families.

`CssConstructionSpec` exposes an exact documented non-family construction
registry. It contains generic utilities and legacy aliases that are intentionally
not requested-family manifest IDs. The catalog test checks this registry is
deterministic, disjoint from `RequestedFamilyId::ALL`, and rejects injected
undocumented aliases.

## Testing

Add `qec-code/tests/family_catalog.rs` with the four issue-required exact tests:

- `complete_catalog_has_12_supported_and_2_deferred_families`
- `every_supported_family_has_positive_and_negative_cases`
- `catalog_rejects_coverage_gaps`
- `requested_and_construction_registries_are_disjoint_and_complete`

The tests cover all listed negative controls:

- missing or duplicate requested-family ID
- a third deferred family
- an available family without a negative case
- a planned family that claims a callable constructor
- a deferred family with a callable stub
- an available family without a `CssFamilySpec` variant
- an undocumented utility or legacy alias

Required verification:

```text
cargo test -p qec-code --test family_catalog complete_catalog_has_12_supported_and_2_deferred_families -- --exact
cargo test -p qec-code --test family_catalog every_supported_family_has_positive_and_negative_cases -- --exact
cargo test -p qec-code --test family_catalog catalog_rejects_coverage_gaps -- --exact
cargo test -p qec-code --test family_catalog requested_and_construction_registries_are_disjoint_and_complete -- --exact
cargo test
```
