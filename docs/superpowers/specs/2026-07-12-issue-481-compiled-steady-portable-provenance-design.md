# Issue 481 Compiled Steady Portable Provenance Design

Issue: #481 Migrate compiled steady-state evidence to portable provenance
Date: 2026-07-12

## Context

The checked compiled steady-state bundle under
`benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release/` preserves
useful raw sampling telemetry, summary, and report artifacts, but its
`environment.json` records absolute paths from the #454 publishing worktree.
The current checker resolves those paths as live files, so it fails in this
worker checkout before it reaches the archive-portability check required by
#481.

Issue #479 is already merged through PR #496. Its portable catalog defines the
schema-v2 direction for checked evidence: repository inputs are repo-relative
POSIX paths, command executables are logical `tool://` roles, runtime
identities preserve role, version, basename, and SHA-256, and checked evidence
must not require old host paths.

This Agent Desk run is non-interactive. The Standing Answer Policy resolves the
Superpowers gates:

- Visual companion: not used because this is backend provenance and checker
  work.
- Clarifying questions: answered from issue #481, merged #479, and the existing
  compiled-steady checker/runner.
- Design approval: accepted automatically because the issue gives the exact
  preservation hashes, success output, negative control, and out-of-scope
  limits.
- Spec review: this document is approved for planning after checking for
  placeholders, contradictions, ambiguity, and unrelated scope.

## Alternatives Considered

1. Keep the old environment schema and make the checker silently reinterpret
   absolute paths under the current checkout. This would make the current
   bundle pass locally, but it would still encode host-specific provenance and
   fail the negative-control requirement.
2. Migrate only `environment.json`, its artifact hash, the runner's future
   environment writer, and the checker contract to portable provenance. This is
   the chosen approach because it preserves raw measurements,
   `summary.json`, and `report.md`, while making both the committed bundle and
   future generated bundles relocatable.
3. Rerun compiled steady-state timing with the updated runner. This would
   produce portable provenance, but it would violate the issue's out-of-scope
   rule against rerunning timing or changing measured results.

## Chosen Design

The compiled-steady environment remains the provenance artifact checked by
`tools/check_rstim_vs_stim_compiled_steady_evidence.py`, but its path-bearing
fields become portable:

- `fair_manifest_path`, `source_manifest_path`, `fixture_path`, and
  `stim_worker_module_path` are repo-relative POSIX paths.
- `worker_argv`, `canonical_worker_argv`, `workers[*].command`, and
  `known_answer_preflight[*].argv` use logical executable roles. The canonical
  roles are `tool://python`, `tool://stim-worker`, and
  `tool://rstim-worker`.
- Runtime executables and extension modules move to
  `runtime_identities`. Each identity has only `role`, `version`, `basename`,
  and `sha256`; there is no live path and no `required_live_path`.
- `stim_python_probe` keeps status, version, extension module name, stderr,
  and the `tool://stim-extension` role, without a filesystem path.

The checker validates repository inputs by resolving repo-relative paths inside
the current checkout. It validates runtime identities by checking roles,
versions, basenames, and SHA-256 strings only; it does not require the old
Python binary, old Stim extension, or old rstim worker binary to exist. The raw
telemetry, lifecycle counts, summary regeneration, report regeneration, and
artifact hash ordering remain unchanged.

## Bundle Migration

Only `environment.json`, `artifact-sha256.json`, and the compiled-steady entries
in `evidence_bundles.toml` need content changes. The bundle must preserve:

- `summary.json` SHA-256
  `2228e5460be43775d45f30861f28bc36c888557add981eeab8e47deadbfb8680`;
- `report.md` SHA-256
  `84b730190bf7554f63dea3fe7629eb8e787db01cbe9ae387a242c2339605d6f4`;
- lifecycle `compile/reference/sample = 1/1/9`;
- measured record count `14`;
- every raw timing value.

The catalog entry for `compiled-steady-release` must update the
`run_compiled_steady.py`, `environment.json`, and `artifact-sha256.json`
digests after the runner/checker/bundle edits. The catalog's existing logical
commands and runtime identities remain aligned with the migrated environment.

## Tests

Add checker tests that first fail under the old contract:

- the committed compiled-steady bundle is accepted and prints exactly
  `PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9`;
- changing `fair_manifest_path` to an absolute publishing-worktree path,
  rehashing `artifact-sha256.json`, and running the checker fails with
  `fair_manifest_path must be repository-relative`;
- changing a runtime identity to include `required_live_path = true` fails with
  `checked evidence must not require a live runtime path`;
- the checker rejects host-absolute paths inside worker argv while preserving
  semantic-before-hash behavior.

Final verification:

```sh
python3 tools/check_rstim_vs_stim_compiled_steady_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release
tmp="$(mktemp -d)"
git archive HEAD | tar -x -C "$tmp"
(cd "$tmp" && python3 tools/check_rstim_vs_stim_compiled_steady_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release)
python3 -m unittest tools.test_check_rstim_vs_stim_compiled_steady_evidence -q
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
cargo test
```

## Out Of Scope

This design does not rerun timing, alter raw lifecycle records, alter measured
elapsed times, change worker timing scope, change reference semantics, update
site metadata, or broaden portability migration to other bundles.

## Self-Review

- No placeholders remain.
- The design preserves the exact `summary.json` and `report.md` hashes named
  by #481.
- Runtime identities no longer require live host paths.
- The negative-control error string is explicit.
- The archive verification is part of the final gate.
