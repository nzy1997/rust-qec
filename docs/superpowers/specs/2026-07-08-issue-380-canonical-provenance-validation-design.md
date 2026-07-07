# Issue 380 Canonical Provenance Validation Design

Issue: #380 Add canonical provenance validation for checked benchmark evidence

Date: 2026-07-08

## Context

Issue #379 defines a canonical provenance schema for site-facing checked
benchmark artifacts. The current benchmark manifest already distinguishes
checked artifacts, status, claim limits, `provenance_requirements`, and
`provenance_sources`, but the checked evidence items do not yet carry a
validated `provenance` object.

Issue #380 narrows the work to manifest data and manifest validation. The
site-rendering contract from #379 is out of scope for this branch. The affected
files are `site/benchmark-site.json`, `tools/check_site_manifest.py`, and
`tools/test_check_site_manifest.py`.

This Agent Desk run is non-interactive. The standing answer policy resolves the
normal Superpowers gates:

- No visual companion is needed because this is a JSON schema and validator
  contract, not a visual design problem.
- The design is approved from issue #380, its parent issue #379, and the
  existing manifest checker patterns.
- Use the recommended Superpowers execution option, Subagent-Driven
  Development, because the writing-plans skill marks it recommended.
- Use numeric `schema_version: 1` for the provenance schema, matching #379's
  suggested representation and the manifest's existing numeric schema style.

## Approaches Considered

1. Add a dedicated canonical provenance validator in
   `tools/check_site_manifest.py` and require it only for evidence items with
   one or more `checked: true` artifacts. This is the chosen approach because it
   preserves local-only and future entries while making checked evidence fail
   closed.
2. Treat `provenance_requirements` and `provenance_sources` as satisfying the
   schema. This is rejected because issue #380 explicitly keeps those fields as
   descriptive legacy/methodology text.
3. Require provenance on every evidence item, regardless of checked artifacts.
   This would be stricter than #380 and would force local-only or future
   planning entries into a checked-artifact schema before promotion.

## Design

Add constants to `tools/check_site_manifest.py`:

- `PROVENANCE_SCHEMA_VERSION = 1`
- `PROVENANCE_REQUIRED_FIELDS` containing `schema_version`, `artifact_date`,
  `source_commit`, `commands`, `os`, `cpu_model`, `rust_version`,
  `python_version`, `dependency_versions`, `external_repository_commits`,
  `seed_policy`, `build_profile`, `shots_or_error_budget`, and
  `artifact_hashes`

Add `validate_checked_item_provenance(scope, item, errors)`. It will run after
artifact validation for any evidence item whose `artifacts` list contains a
dictionary with `checked` set to `True`.

Validation rules:

- `provenance` must exist and be a JSON object.
- `provenance.schema_version` must be exactly `1`.
- Each required non-`schema_version` field must exist and be an object.
- A recorded field must be `{ "status": "recorded", "value": ... }` and must
  include the `value` key.
- A not-recorded field must be
  `{ "status": "not_recorded", "reason": "..." }` and the reason must be a
  non-empty string.
- Any other status, missing status, or malformed field is a validation error
  naming the evidence item and field.

Update `site/benchmark-site.json` so the checked evidence items
`surface-decoder-full` and `bb-circuit-full` each carry a canonical
`provenance` object. Use `recorded` for facts already represented in the
manifest, such as commands and artifact paths. Use `not_recorded` with a short
historical-artifact reason for environment or version details that were not
captured when the committed artifacts were produced. This avoids inventing
provenance while still making every required key explicit.

Keep `provenance_requirements` and `provenance_sources` unchanged as
methodology/source text. They remain required by the existing manifest schema
but never satisfy canonical provenance validation.

## Error Handling

The manifest checker will keep accumulating all errors and print them through
the existing CLI path. Error messages should include the evidence item id and
the failing provenance key so the issue's negative controls can assert useful
reviewer-facing failures.

Examples:

- `evidence item surface-decoder-full: missing required field provenance`
- `evidence item surface-decoder-full: provenance missing required field cpu_model`
- `evidence item surface-decoder-full: provenance.cpu_model not_recorded entry must include non-empty reason`
- `evidence item surface-decoder-full: provenance.schema_version must be 1`

## Testing

Use TDD around `tools/test_check_site_manifest.py`:

1. Add canonical provenance to the valid fixture and assert the fixture still
   validates.
2. Add negative-control unit tests that remove the whole `provenance` object,
   remove `provenance.cpu_model`, set `provenance.cpu_model` to
   `{ "status": "not_recorded" }`, and mutate `provenance.schema_version` to an
   unsupported value and a wrong type.
3. Run the focused unit tests and observe the new tests fail before adding
   production validation.
4. Implement the validator and manifest data.
5. Re-run the issue verification commands and the required repository
   `cargo test`.

Required verification:

```sh
python3 -m unittest tools.test_check_site_manifest -q
python3 tools/check_site_manifest.py --self-test
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
cargo test
```

Negative controls must also be run against a temporary copy of
`site/benchmark-site.json`:

- delete `provenance.cpu_model`, run the checker, and confirm nonzero exit with
  `surface-decoder-full` and `cpu_model`
- set `provenance.cpu_model` to `{ "status": "not_recorded" }`, run the
  checker, and confirm nonzero exit with `surface-decoder-full`, `cpu_model`,
  and `reason`
- delete the whole `provenance` object, run the checker, and confirm nonzero
  exit with `surface-decoder-full` and `provenance`

Out of scope: hash-content verification, new benchmark campaigns, benchmark
artifact content changes, and site rendering changes.

## Self Review

- Marker scan: no unresolved open markers.
- Consistency check: validation is scoped to checked evidence items and does
  not let legacy methodology fields satisfy the canonical schema.
- Scope check: the design only touches the three files named in #380 plus this
  Superpowers design artifact.
- Ambiguity check: schema version value, required keys, recorded/not-recorded
  shapes, and negative controls are explicit.
