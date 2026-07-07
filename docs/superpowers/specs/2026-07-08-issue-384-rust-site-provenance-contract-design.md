# Issue 384 Rust Site Provenance Contract Design

Issue: #384 Lock provenance exposure in the Rust site contract

Date: 2026-07-08

## Context

Issues #380, #381, and #382 added canonical checked benchmark provenance to
`site/benchmark-site.json`, validated checked artifact SHA-256 entries, and
rendered provenance through `site/app.js`. Issue #384 adds a Rust source-level
contract so future site edits cannot silently remove manifest provenance fields,
checked artifact hash coverage, or the renderer hook used by the Pages build.

This Agent Desk run is non-interactive. The standing answer policy resolves the
normal Superpowers gates:

- No visual companion is needed because this is a source contract test.
- The design is approved from issue #384, parent issue #379, dependency issues
  #380, #381, and #382, merged PRs #393, #395, and #398, and the existing
  `rstim/tests/site_contract.rs` patterns.
- Use the recommended Superpowers execution option, Subagent-Driven
  Development, because the writing-plans skill marks it recommended.
- Keep the contract source-level and fast; do not run `make build-site` and do
  not reimplement Python SHA-256 digest checking in Rust.

## Approaches Considered

1. Add one focused Rust integration test named
   `checked_benchmark_provenance_is_manifest_backed` in
   `rstim/tests/site_contract.rs`. This is the chosen approach because it
   matches the issue recommendation and keeps the lock beside the existing site
   contract tests.
2. Strengthen the broad `checked_benchmark_artifacts_are_linked` test only.
   This is rejected because the issue asks for a focused regression target and
   negative controls should name provenance-specific failures directly.
3. Add Python or built-site validation instead. This is rejected because #380,
   #381, #382, and #383 already own Python and reviewer-facing built-site
   checks; #384 specifically wants the Rust site contract to lock the source
   manifest and renderer wiring.

## Design

Add `checked_benchmark_provenance_is_manifest_backed` to
`rstim/tests/site_contract.rs`. The test parses `site/benchmark-site.json`,
reads `site/app.js`, and reads `site/index.html`.

For `surface-decoder-full` and `bb-circuit-full`, the test requires:

- canonical provenance keys from #380:
  `schema_version`, `artifact_date`, `source_commit`, `commands`, `os`,
  `cpu_model`, `rust_version`, `python_version`, `dependency_versions`,
  `external_repository_commits`, `seed_policy`, `build_profile`,
  `shots_or_error_budget`, and `artifact_hashes`;
- `schema_version` equal to `1`;
- each non-`schema_version` key to be an object with status `recorded` or
  `not_recorded`;
- each `not_recorded` entry to include a non-empty `reason`;
- `provenance.artifact_hashes` to be recorded and object-valued;
- every checked artifact path listed on the item to have a corresponding
  `artifact_hashes.value[path].sha256` string.

The test should deliberately verify hash entry presence only. It must not
recompute SHA-256 digests because #381 owns digest correctness.

For renderer wiring, require `site/app.js` to contain `renderProvenance`,
`renderProvenance(item.provenance)`, `item.provenance`, `recorded`,
`not_recorded`, and `artifact_hashes`. These source markers prove checked cards
still consume manifest-backed provenance through the helper added by #382.

For hard-coded provenance rejection, require `site/index.html` not to contain
canonical checked provenance implementation keys such as `artifact_hashes`,
`schema_version`, `source_commit`, `cpu_model`, or checked artifact paths. This
keeps checked provenance values in the manifest and renderer path rather than
static HTML.

## Error Handling

Use direct `assert!` and `unwrap_or_else` failures with item ids, provenance
keys, artifact paths, and renderer hook markers in the panic messages. The
negative controls should fail with messages that name the missing
`artifact_hashes` key or missing `renderProvenance(item.provenance)` hook.

## Testing

Use TDD in the Rust integration test:

1. Add a failing test that first includes one stricter requirement not currently
   locked by `site_contract.rs`: every checked artifact, including
   `benchmarks/bb_circuit_bposd_compare/results/full/summary.md`, must have a
   matching provenance hash entry.
2. Run the focused test and verify the initial RED failure.
3. Add the minimal helper/test code and update the existing BB checked artifact
   expectation if needed so source-level contract coverage includes every
   checked artifact.
4. Run:

```sh
cargo test -p rstim --test site_contract checked_benchmark_provenance_is_manifest_backed -q
cargo test -p rstim --test site_contract -q
cargo test
```

5. Run the issue's negative controls by temporarily mutating source files, then
   restoring them:

```sh
cargo test -p rstim --test site_contract checked_benchmark_provenance_is_manifest_backed -q
```

The focused test must exit nonzero when `provenance.artifact_hashes` is deleted
from a checked item, and it must exit nonzero when
`renderProvenance(item.provenance)` is removed from `site/app.js`.

Out of scope: full site builds, Python hash recomputation, benchmark reruns, and
site architecture changes.

## Self Review

- Placeholder scan: no unresolved placeholders.
- Consistency check: the design only adds a Rust source-level contract and
  leaves Python manifest/hash validation owned by dependency issues.
- Scope check: the change is focused enough for one implementation task.
- Ambiguity check: required provenance fields, checked artifact hash coverage,
  renderer markers, and negative-control expectations are explicit.
