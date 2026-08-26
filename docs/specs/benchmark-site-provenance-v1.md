# Benchmark Site Provenance v1

Status: **stable contract** (versioned). Companion to
[`site/benchmark-site.json`](../../site/benchmark-site.json), validated by
`tools/check_site_manifest.py` and rendered by `site/static/js/benchmarks.js`.

Every checked evidence item in the site benchmark manifest carries a canonical
`provenance` object that records what environment and command produced the
checked artifacts. Historical artifacts that predate provenance capture do not
fabricate missing details; they declare `not_recorded` with a reason instead.

## Schema: provenance object (`schema_version` = 1)

```json
"provenance": {
  "schema_version": 1,
  "artifact_date": { "status": "recorded", "value": "2026-06-28" },
  "cpu_model": { "status": "not_recorded", "reason": "historical artifact predates provenance capture" }
}
```

Required keys (all fourteen must be present):

- `schema_version` — exactly the integer `1`.
- `artifact_date` — when the artifact was produced (or, for historical
  artifacts, when the checked bytes entered git, labelled as such).
- `source_commit` — the commit whose source produced the artifact.
- `commands` — the command lines that produced the artifact.
- `os` — operating system of the producing environment.
- `cpu_model` — CPU model of the producing environment.
- `rust_version` — Rust toolchain version.
- `python_version` — Python version for Python-driven campaigns.
- `dependency_versions` — versions of the dependencies that materially affect
  the result (decoder libraries, compared tools, output formats).
- `external_repository_commits` — pinned commits of external repositories the
  artifact compares against; an empty list is a valid recorded value.
- `seed_policy` — the sampler seeds and their scope.
- `build_profile` — cargo profile and binary paths used.
- `shots_or_error_budget` — shot counts, error budgets, and stop policies.
- `artifact_hashes` — sha256 of every checked artifact of the item.

## Recorded versus not_recorded

Every key except `schema_version` is a status object:

- `{ "status": "recorded", "value": ... }` — the value was captured. `value`
  may be a string, number, list, or object, whichever fits the field.
- `{ "status": "not_recorded", "reason": "..." }` — the value was not
  captured. `reason` is a short non-empty string explaining why. A
  `not_recorded` entry without a reason is a validation error.

Any other `status`, a missing `value` on a recorded entry, or a missing key is
a validation error. `artifact_hashes` must always be `recorded` for checked
evidence items, with exactly one `{"sha256": "<64 lowercase hex>"}` entry per
checked artifact; the validator recomputes the digests against the repository
files and against the copies under `_site/`.

## Scope and promotion rule

The schema applies to evidence items with at least one checked artifact.
Local-only and future items may carry lighter `provenance_requirements` notes,
but they must satisfy this schema before any of their artifacts can be
promoted to `checked: true`. New benchmark campaigns are expected to record
every field at capture time; `not_recorded` exists so historical artifacts can
state an honest provenance status instead of silently omitting fields.

## Enforcement

- `tools/check_site_manifest.py` rejects checked evidence items that are
  missing a `provenance` object, missing any required key, carrying an
  unsupported status, carrying a `not_recorded` entry without a reason, or
  whose recorded `artifact_hashes` drift from the on-disk bytes.
- `tools/check_site_build.py` reports a reviewer-readable
  `PASS/FAIL checked benchmark provenance` line summarizing recorded and
  `not_recorded` field counts per checked item in the built site.
- `site/static/js/benchmarks.js` renders the provenance status of each checked
  benchmark card from the manifest, so readers see which fields are recorded
  and which are `not_recorded` with their reasons.
- `rstim/tests/site_contract.rs` asserts the manifest-backed renderer exposes
  provenance status and that checked items carry the canonical keys.
