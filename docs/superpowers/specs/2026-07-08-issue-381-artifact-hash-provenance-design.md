# Issue 381 Artifact Hash Provenance Design

Issue: #381 Verify checked artifact hashes from provenance

Date: 2026-07-08

## Context

Issue #380 added canonical provenance validation for checked benchmark evidence
items and intentionally left hash-content verification out of scope. Issue
#381 completes that gap for checked artifacts in `site/benchmark-site.json`.
The current checker already validates checked artifact paths, copies those
paths into `_site`, and provides `iter_checked_artifact_paths()` for traversing
checked evidence. The manifest currently records `provenance.artifact_hashes`
as `not_recorded`, so a present provenance key is not enough to prove that the
listed repository artifact and copied built-site artifact match the recorded
evidence.

This Agent Desk run is non-interactive. The standing answer policy resolves the
normal Superpowers gates:

- No visual companion is needed because this is a manifest schema and validator
  change.
- The design is approved from issue #381, parent issue #379, dependency issue
  #380, merged PR #393, and the existing manifest checker patterns.
- Use the recommended Superpowers execution option, Subagent-Driven
  Development, because the writing-plans skill marks it recommended.
- Keep validation scoped to checked artifacts only; do not require local-only or
  future evidence entries to record checked artifact hashes before promotion.

## Approaches Considered

1. Extend `tools/check_site_manifest.py` so checked evidence provenance must
   record a per-artifact SHA-256 digest, then validate each digest against the
   repository file and, when `--site-root` is provided, the copied site file.
   This is the chosen approach because it reuses the existing manifest checker
   and checked-artifact traversal path.
2. Add a separate hash verification tool. This is rejected because issue #381
   explicitly asks to compute hashes during normal
   `tools/check_site_manifest.py` validation.
3. Accept `provenance.artifact_hashes` as `not_recorded` for historical checked
   artifacts. This is rejected for checked artifacts because #381 requires every
   checked artifact path to have recorded SHA-256 metadata.

## Design

Add `sha256_file(path)` in `tools/check_site_manifest.py` using Python's
standard `hashlib` library. Hash files in chunks so the helper remains safe for
larger artifacts.

Change checked-artifact traversal to expose item context and artifact path
together. The current helper returns `(item_id, artifact_path)` tuples; this is
enough for site-copy checks but not enough to inspect the owning item's
provenance. Replace it with a helper that returns `(item, item_id,
artifact_path)` for every dictionary artifact with `checked: true` and a string
`path`. Keep all callers on that helper so manifest traversal stays in one
place.

Extend checked provenance validation:

- `provenance.artifact_hashes` must be an object with
  `{ "status": "recorded", "value": { ... } }` for checked evidence.
- `value` must be an object keyed by every checked artifact path for that
  evidence item.
- Each path entry must be an object with only supported hash shape
  `{ "sha256": "<64 lowercase hex characters>" }`.
- Missing hash entries, non-object entries, missing `sha256`, non-string
  digests, non-hex digests, or unsupported extra algorithms are validation
  errors naming the evidence item, artifact path, and `sha256`.
- The recorded digest must match `repo_root / artifact_path`.

When `--site-root` is supplied, keep the existing copied-artifact existence
check and add digest verification for `site_root / artifact_path`. The copied
site file must match the same recorded SHA-256 digest. The error should name the
evidence item, artifact path, and `sha256` so reviewers can diagnose stale
copied artifacts.

Update `site/benchmark-site.json` so every checked artifact path under
`surface-decoder-full` and `bb-circuit-full` has a recorded SHA-256 digest.
These hashes are derived from the committed artifact files only; benchmark
artifact contents are not regenerated or changed.

## Error Handling

The manifest checker continues accumulating errors and printing them through
the existing CLI path. Hash validation messages should be reviewer-readable and
include enough context for the issue's negative controls:

- `evidence item surface-decoder-full: provenance.artifact_hashes missing hash entry for benchmarks/.../results.csv`
- `evidence item surface-decoder-full: provenance.artifact_hashes entry for benchmarks/.../results.csv must include sha256`
- `evidence item surface-decoder-full: artifact benchmarks/.../results.csv sha256 digest does not match repository file`
- `evidence item surface-decoder-full: copied artifact benchmarks/.../results.csv sha256 digest does not match recorded hash`

## Testing

Use TDD around `tools/test_check_site_manifest.py`:

1. Add fixture SHA-256 hashes for both checked fixture artifacts and assert the
   valid fixture still validates.
2. Add a test that mutates one checked artifact digest and expects a failure
   naming `surface-decoder-full`, the artifact path, and `sha256`.
3. Add a test that removes one checked artifact hash entry and expects a
   failure naming `surface-decoder-full` and the missing artifact path.
4. Add a test that writes copied `_site` checked artifacts, mutates the copied
   CSV, and expects site-root validation failure naming `surface-decoder-full`,
   the artifact path, and `sha256`.
5. Add malformed-shape coverage for unsupported hash shapes so the checker
   rejects non-object path entries, missing `sha256`, non-string or invalid
   digests, and extra algorithms.
6. Run the new tests and observe the expected failures before implementing
   production validation.
7. Implement the hash validator, update the real manifest hashes, and re-run
   the full issue verification.

Required verification:

```sh
python3 -m unittest tools.test_check_site_manifest -q
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
make build-site
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
cargo test
```

Negative controls must also be run against temporary copies or modified copied
site files:

- Change the SHA-256 value for
  `benchmarks/surface_decoder_compare/results/full/results.csv`, run the
  checker, and confirm nonzero exit with `surface-decoder-full`, `results.csv`,
  and `sha256`.
- Remove that artifact path from `provenance.artifact_hashes.value`, run the
  checker, and confirm nonzero exit with `surface-decoder-full` and the missing
  artifact path.
- After `make build-site`, modify
  `_site/benchmarks/surface_decoder_compare/results/full/results.csv` without
  changing the manifest, run the checker with `--site-root`, and confirm
  nonzero exit with `surface-decoder-full`, `results.csv`, and `sha256`.

Out of scope: changing benchmark artifact contents, generating new benchmark
outputs, or requiring local-only/future benchmark entries to provide checked
artifact hashes before promotion.

## Self Review

- Marker scan: no unresolved open markers.
- Consistency check: the design validates repository and site-copy files against
  the same recorded provenance hash.
- Scope check: the design only touches the manifest checker, its unit tests,
  the site manifest hash metadata, and Superpowers workflow artifacts.
- Ambiguity check: required hash shape, digest format, site-root behavior, and
  negative controls are explicit.
