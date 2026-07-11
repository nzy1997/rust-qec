# Issue 479 Portable Evidence Provenance Design

Issue: #479 Define a portable checked-evidence provenance contract
Date: 2026-07-11

## Context

The checked rstim-vs-Stim release bundles for fair CLI sampling, compiled
steady-state sampling, packed reference-build timing, and instruction-wide frame
noise evidence are committed under
`benchmarks/rstim_vs_stim_simulator/results/`. Their measurement artifacts must
remain intact, but their existing bundle-specific checkers can still depend on
publishing-machine paths or live binaries. Issue #479 adds the shared portable
contract that future checkers and site-facing provenance can consume without
requiring the original Agent Desk checkout.

This Agent Desk run is non-interactive. The Standing Answer Policy resolves the
Superpowers gates:

- Visual companion: not used because this is a backend catalog and validator.
- Clarifying questions: answered from issue #479 and the existing evidence
  bundle/checker patterns.
- Design approval: accepted automatically because the issue supplies the exact
  interface files, required bundle IDs, schema version, negative controls, and
  verification commands.
- Spec review: this document is approved for planning after checking for
  placeholders, contradictions, ambiguity, and unrelated scope.

## Approaches Considered

1. Rewrite the existing JSON and JSONL provenance fields in every release
   bundle to replace host paths with `tool://` roles and repo-relative paths.
   This would make the historical artifacts visibly portable, but it would
   change committed bundle metadata and force bundle-specific semantic checkers
   to change in the same PR.
2. Add a portable side-by-side schema v2 catalog and validator that checks
   bundle-relative artifacts, repo-relative inputs, logical executable roles,
   runtime identities, and portable checked command/provenance templates. This
   avoids altering timing measurements or the existing semantic checkers while
   defining the shared contract requested by #479.
3. Add only a free-form catalog with no artifact or path validation. This would
   document intent, but it would not prove relocatability or catch the negative
   controls.

The selected approach is option 2. It is the smallest change that defines a
portable checked-evidence contract, keeps existing bundle measurements stable,
and leaves site-facing provenance plus bundle-specific semantic checker updates
to their tracked issues.

## Schema V2

Add `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml` with:

- `schema = 2` and `suite = "rstim_vs_stim_simulator"`;
- exactly four bundle tables, in this order:
  `fair-cli-release`, `compiled-steady-release`,
  `reference-build-release`, and `frame-instruction-wide-release`;
- `bundle_path` as a repo-relative POSIX path;
- `artifacts` as bundle-relative file entries with lowercase SHA-256 digests;
- `repository_inputs` as repo-relative POSIX paths with lowercase SHA-256
  digests;
- `logical_executables` as role URIs such as `tool://stim`,
  `tool://rstim`, and `tool://python`;
- `runtime_identities` containing only `role`, `version`, `basename`, and
  `sha256`, with no required live path;
- `checked_commands` and `checked_provenance` entries that record portable
  command/provenance values using logical executable roles and repo-relative or
  bundle-relative paths.

The catalog is the portable contract. Existing historical environment or raw
files can still be consumed by the old semantic checkers, but schema-v2 checked
command/provenance entries must not contain host-absolute paths.

## Validator Behavior

Add `benchmarks/rstim_vs_stim_simulator/portable_provenance.py` as the shared
implementation module. It will expose:

- `SCHEMA_VERSION = 2`;
- `EXPECTED_BUNDLE_IDS` for the four required bundle IDs;
- `load_catalog(path: Path) -> dict[str, Any]`;
- `validate_catalog(catalog: dict[str, Any], catalog_path: Path) -> list[str]`;
- helpers for SHA-256 validation, repo-relative POSIX path validation,
  bundle-relative path validation, logical role validation, and recursive
  host-absolute-path detection.

Add `benchmarks/rstim_vs_stim_simulator/validate_evidence_bundles.py` as the
CLI wrapper. It accepts:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

On success it prints exactly:

```text
PASS portable evidence catalog bundles=4 schema=2
```

On failure it prints each validation error to stderr and exits nonzero.

Validation errors include:

- catalog schema must be 2;
- bundle IDs must be exactly the required four IDs;
- repository paths must be relative POSIX paths that stay inside the repo;
- bundle artifact paths must be relative to the bundle and stay inside it;
- artifact and repository-input SHA-256 values must match current file bytes;
- executable roles must be `tool://...` URIs;
- runtime identities must contain role, version, basename, and SHA-256;
- `required_live_path = true` is rejected with
  `checked evidence must not require a live runtime path`;
- checked command/provenance values are recursively scanned and must not contain
  host-absolute paths.

## Tests

Add
`benchmarks/rstim_vs_stim_simulator/tests/test_validate_evidence_bundles.py`.

Coverage:

- the committed catalog CLI prints the exact success line;
- the committed catalog pins the exact four bundle IDs and schema version;
- a temporary catalog fixture containing `/tmp/fixture.stim` in
  `repository_inputs` fails with `repository path must be relative`;
- a temporary catalog fixture containing `required_live_path = true` in a
  runtime identity fails with
  `checked evidence must not require a live runtime path`;
- a temporary checked command containing `/tmp/...` fails the host-absolute
  path scanner.

Final verification:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
python3 -m unittest \
  benchmarks.rstim_vs_stim_simulator.tests.test_validate_evidence_bundles -q
cargo test
```

## Out Of Scope

This PR does not alter benchmark measurements, site metadata, the historical
#406 artifact, or bundle-specific semantic checkers. The catalog establishes
the portable provenance contract that those consumers can adopt separately.

## Self-Review

- No placeholders remain.
- The schema version, bundle IDs, negative controls, and success line match
  issue #479.
- Repository inputs and bundle artifacts have separate path rules.
- Runtime identities do not require live executable paths.
- Checked command/provenance values reject host-absolute paths without forcing
  historical measurement artifact rewrites in this issue.
