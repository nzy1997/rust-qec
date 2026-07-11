# Issue 482 Reference-Build Portable Provenance Design

## Context

Issue #482 migrates the checked `reference-build-release` evidence bundle onto
the schema-v2 portable provenance contract from #479. The current bundle already
keeps repository inputs relative and the catalog records the published rstim
worker identity, but `tools/check_rstim_vs_stim_reference_build_evidence.py`
still opens the recorded absolute `rstim_worker_binary_path`. That path points
at an old Agent Desk worktree and prevents validation from a clean checkout or a
git archive.

This is a provenance migration only. The benchmark is not rerun, and the
12,121-bit packed reference output, `summary.json`, and `report.md` remain
unchanged. The required preserved hashes are:

- `summary.json`: `614658cf8213b486752f1fe53b7d864561abbe41c2eefd799fc8fa34883270a5`
- `report.md`: `4a6a2dae36b546be472990651a27be20bfd11f1a3c15e9963a1e212bade1f6ef`

## Chosen Approach

Use the schema-v2 contract directly:

1. Migrate `environment.json` away from host-specific executable paths in the
   checked command fields.
2. Add explicit runtime identity records for the runner Python, Stim Python, and
   rstim reference worker. Each identity contains only logical role, version,
   basename, and SHA-256.
3. Update the checker so default validation verifies raw semantics, backend,
   artifact hashes, runner/worker argv semantics, and recorded runtime identity
   shape without opening the original worker path.
4. Add `--verify-runtime-binary <path>` to optionally hash a supplied rstim
   worker and compare it with the recorded `tool://rstim-reference-worker`
   identity.

Alternatives considered:

- Keep legacy paths and ignore missing files in archive mode. This would make
  validation dependent on hidden mode logic instead of the portable provenance
  schema.
- Rerun the reference build to regenerate environment metadata. This is out of
  scope and would risk changing timing artifacts.
- Move all runtime identity validation into the catalog checker only. This
  would leave the bundle-specific checker unable to enforce the recorded
  identity and optional binary attestation required by #482.

## Data Model

`environment.json` gains a `runtime_identities` array. The roles are:

- `tool://python` for the runner interpreter.
- `tool://stim-python` for the Stim worker interpreter.
- `tool://rstim-reference-worker` for the published rstim worker binary.

`worker_argv`, `canonical_worker_argv`, and `runner_argv` use logical roles for
runtime executables and repo-relative paths for repository inputs and the output
directory. The legacy `python_executable`, `runner_python_executable`, and
`rstim_worker_binary_path` fields are no longer required by the checker.

`artifact-sha256.json` will be updated only because `environment.json` changes.
The existing `raw.jsonl`, `summary.json`, and `report.md` bytes remain
unchanged.

## Checker Behavior

Default checker validation:

- Requires the same six bundle files and rejects unexpected files.
- Recomputes `summary.json` and `report.md` from `raw.jsonl`.
- Validates canonical fixture and manifest repo-relative paths and hashes.
- Validates runner and worker argv as logical commands:
  - Stim worker starts with `tool://stim-python`.
  - rstim worker starts with `tool://rstim-reference-worker`.
  - runner starts with `tool://python`.
  - fixture, manifest, and output directory arguments remain repo-relative or
    bundle-relative as appropriate.
- Validates runtime identities have the expected roles, versions, basenames, and
  SHA-256 digests.
- Verifies `artifact-sha256.json` after semantic checks.

Optional runtime verification:

- `--verify-runtime-binary <path>` resolves only the supplied path.
- It compares that file's SHA-256 to the recorded
  `tool://rstim-reference-worker` identity.
- A mismatch fails with `runtime binary SHA-256 does not match recorded identity`.

## Tests

Update `tools/test_check_rstim_vs_stim_reference_build_evidence.py` so synthetic
fixtures use the portable environment shape. Keep existing semantic negative
controls and add coverage for:

- A valid portable bundle passes without a live `target/release` worker.
- A supplied matching runtime binary passes.
- A supplied different binary fails with the required message.
- Invalid runtime identity data fails before artifact hash validation.
- The checked-in bundle passes from both the working tree and a git archive that
  does not contain `target/release/rstim_reference_build_worker`.

Final verification runs the issue's two checker commands, the focused checker
unit tests, the portable catalog checker, the portable catalog unit tests, and
`cargo test`.

## Scope Boundaries

Do not rerun benchmark construction, optimize reference construction, change
`raw.jsonl`, change the 12,121-bit reference output, or alter
`summary.json`/`report.md`.
