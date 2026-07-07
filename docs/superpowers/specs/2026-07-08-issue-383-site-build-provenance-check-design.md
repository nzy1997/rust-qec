# Issue 383 Site Build Provenance Check Design

Issue: #383 Add provenance to the reviewer-readable site build checker

Date: 2026-07-08

## Context

Issues #380 and #381 added canonical checked-evidence provenance validation and
SHA-256 validation to `tools/check_site_manifest.py`. Issue #382 added
manifest-backed provenance rendering on checked benchmark result cards and a
narrow built-site wiring check in the manifest validator. The remaining gap is
the reviewer-facing command:

```sh
python3 tools/check_site_build.py _site
```

It currently delegates manifest and copied-artifact validation, but its
summary does not contain a provenance-specific PASS/FAIL area. Reviewers can see
that the generic manifest check passed, but not which checked benchmark
provenance items were covered.

This Agent Desk run is non-interactive. The standing answer policy resolves the
normal Superpowers gates:

- No visual companion is needed because this is a command-line checker summary
  and self-test change.
- The design is approved from issue #383, parent issue #379, dependency issues
  #380 through #382, and merged PRs #393, #395, and #398.
- Use the recommended Superpowers execution option, Subagent-Driven
  Development, because the writing-plans skill marks it recommended.
- Keep the built-site checker focused on reviewer-readable reporting and site
  exposure. Do not duplicate provenance schema or SHA-256 validation logic from
  `tools.check_site_manifest`.

## Approaches Considered

1. Add a dedicated `checked benchmark provenance` result line in
   `tools/check_site_build.py` that reports checked item IDs, recorded field
   counts, `not_recorded` counts, and checked artifact hash counts after
   delegated manifest/site validation succeeds. This is the chosen approach
   because it gives reviewers a visible PASS line while preserving the manifest
   validator as the schema/hash authority.
2. Expand `tools/check_site_build.py` with a full provenance schema and hash
   validator. This is rejected because #383 explicitly asks not to reimplement
   the schema or SHA-256 logic.
3. Only change the generic `manifest and copied artifacts` line text. This is
   rejected because reviewers need a provenance-specific PASS/FAIL area and
   self-test coverage for provenance-specific mutations.

## Design

Update `tools/check_site_build.py` so `check_manifest_and_artifacts()` returns
the delegated manifest/site-root errors alongside the loaded manifest. Add a
small `check_checked_provenance(manifest, delegated_errors)` function that:

- fails with delegated provenance/hash/wiring errors when
  `tools.check_site_manifest` reports missing or malformed provenance,
  `artifact_hashes`, SHA-256 mismatch, copied-artifact hash mismatch, or
  provenance renderer wiring failure;
- fails if the built manifest cannot be loaded or if no checked evidence items
  are available;
- otherwise iterates checked evidence through
  `check_site_manifest.iter_checked_artifacts()` and builds a reviewer-readable
  summary.

The summary is intentionally descriptive, not a validator. For each checked
item it counts:

- status-bearing provenance fields with `status: "recorded"`;
- status-bearing provenance fields with `status: "not_recorded"`;
- SHA-256 artifact-hash entries under `provenance.artifact_hashes.value`;
- checked artifact paths associated with the item.

The PASS line should name the checked items, for example:

```text
PASS checked benchmark provenance: surface-decoder-full (2 recorded fields, 11 not_recorded fields, 2 checked artifact hashes); bb-circuit-full (2 recorded fields, 11 not_recorded fields, 4 checked artifact hashes)
```

This wording avoids implying that historical `not_recorded` fields were
recorded, while still showing reviewers how much provenance status was exposed.

The checker continues to call:

```python
check_site_manifest.validate_manifest(repo_root, manifest_path, site_root=site_root)
check_site_manifest.validate_site_root(site_root, manifest_path)
```

Those calls remain the only source of canonical provenance schema, copied
artifact, hash, and renderer-wiring validation.

## Self-Test Coverage

Extend `run_self_test()` in `tools/check_site_build.py` with two new mutations:

- remove `surface-decoder-full.provenance` from `_site/data/benchmark-site.json`
  and require a failure naming `surface-decoder-full` and `provenance`;
- corrupt `_site/benchmarks/surface_decoder_compare/results/full/results.csv`
  without changing the manifest and require a delegated hash failure naming the
  checked artifact path.

Add unit coverage in `tools/test_check_site_build.py` that verifies the valid
fixture summary contains `PASS checked benchmark provenance`, names
`surface-decoder-full` and `bb-circuit-full`, and fails for the two mutations
above.

## Error Handling

The new provenance summary must not hide the existing generic manifest failure.
When delegated validation fails, the checker should keep the existing
`FAIL manifest and copied artifacts` line and also emit
`FAIL checked benchmark provenance` with the delegated provenance/hash/wiring
detail relevant to reviewers.

Malformed or incomplete manifest shapes should produce a FAIL result instead of
raising. The helper may do shallow type checks only to summarize or avoid
crashes; full validation remains delegated.

## Verification

Use TDD around `tools/test_check_site_build.py`:

1. Add failing tests for the provenance PASS summary, missing
   `surface-decoder-full.provenance`, and copied checked artifact hash
   corruption.
2. Run the focused tests and observe the expected failures.
3. Implement the provenance summary and self-test mutations.
4. Run:

```sh
python3 -m unittest tools.test_check_site_build -q
make build-site
python3 tools/check_site_build.py --self-test
python3 tools/check_site_build.py _site
cargo test
```

Expected result: all commands exit 0, and the final checker output contains a
`PASS checked benchmark provenance` line naming `surface-decoder-full` and
`bb-circuit-full`.

Out of scope: external link checking, browser automation, replacing the static
site stack, and running or regenerating benchmark campaigns.

## Self Review

- Marker scan: no unresolved open markers.
- Consistency check: schema, SHA-256, copied artifact, and renderer-wiring
  validation remain delegated to `tools.check_site_manifest`.
- Scope check: the design only touches the reviewer-readable checker, its
  self-test/unit tests, and Superpowers workflow artifacts.
- Ambiguity check: PASS wording, failure delegation, self-test mutations, and
  required verification commands are explicit.
