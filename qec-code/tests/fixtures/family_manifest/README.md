# QEC Family Manifest Schema

`manifest.v1.json` is the versioned source of truth for the QEC construction
family targets tracked by issue #552.

## Version

The top-level `schema_version` and every entry-level `schema_version` must be
`1`. Serialization is canonicalized by `qec-code/tests/family_manifest.rs` with
`serde_json::to_string_pretty`.

## Lifecycle Fields

`disposition` is typed and must be either `supported` or `deferred`.
`availability` is typed and must be `planned`, `available`, or
`not_applicable`.

Legal pairs are exactly:

- `(supported, planned)`
- `(supported, available)`
- `(deferred, not_applicable)`

For issue #552, every supported family remains `availability=planned`. Promotion
to `available` is controlled by issue #573 after constructors and executable
fixture coverage are complete.

## Required Entry Fields

Every family entry records `provenance`, `verification`, and
`intended_consumers` as non-empty arrays. `callable_constructor` must be null
for planned and deferred entries. Supported entries may declare
`executable_cases`; each supported entry in this fixture declares one
`positive`/`success` case and one `negative`/`rejection` case.

Deferred entries do not declare executable cases.
