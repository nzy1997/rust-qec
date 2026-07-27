# QEC Family Manifest Schema

`manifest.v1.json` is the versioned source of truth for the QEC construction
family targets tracked by issues #552 and #573.

## Version

The top-level `schema_version` and every entry-level `schema_version` must be
`1`. Serialization is canonicalized by `qec-code/tests/family_catalog.rs` with
`serde_json::to_string_pretty`.

## Lifecycle Fields

`disposition` is typed and must be either `supported` or `deferred`.
`availability` is typed and must be `planned`, `available`, or
`not_applicable`.

Legal pairs are exactly:

- `(supported, planned)`
- `(supported, available)`
- `(deferred, not_applicable)`

For issue #573, exactly 12 supported families are `availability=available`.
Exactly `hyperbolic_5_5` and `perturbed_hgp` remain deferred with
`availability=not_applicable`.

## Required Entry Fields

Every family entry records `provenance`, `verification`, and
`intended_consumers` as non-empty arrays. Available entries must also record
`callable_constructor`, `normalized_inputs`, `expected`, `row_weight_summary`,
`distance_verification`, and `executable_verifier`. Each available entry in
this fixture declares one `positive`/`success` case and one
`negative`/`rejection` case under `executable_cases`.

Deferred entries record `research_contracts`, keep `callable_constructor` null,
and do not declare executable cases.
