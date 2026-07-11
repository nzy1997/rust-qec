# Issue 483 Frame-Noise Portable Runtime Identity Design

Issue: #483 Migrate instruction-wide frame-noise evidence to portable provenance

## Context

The checked frame-noise release bundle already has stable measurement,
fixture-load, report, and correctness artifacts. The non-portable part is
`environment.json`: it records `target/release/rstim`, and
`tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py` hashes that
path during default validation. A clean checkout does not contain that binary,
and a developer checkout may contain a different valid local build.

Issue #479 added the schema-v2 portable evidence catalog. The catalog already
contains the `frame-instruction-wide-release` bundle and a schema-v2
`tool://rstim` runtime identity with the publishing binary basename, version,
and SHA-256 digest.

No visual companion is needed; this is a CLI provenance/data migration with no
layout or visual decision.

## Approaches

1. Recommended: migrate the frame bundle checker to use the schema-v2 catalog
   runtime identity as the default authority, and add an optional
   `--verify-runtime-binary <path>` attestation. This keeps validation
   portable by default and still allows a supplied binary to be checked.
2. Keep `environment.rstim_binary` as the default checked path but make missing
   binaries non-fatal. This hides portability failures and would let checked
   evidence silently skip a required identity check.
3. Remove runtime identity checking from the bundle-specific checker and rely
   only on the catalog validator. This loses the issue-requested cross-check
   between checked frame records and the schema-v2 identity.

Choose approach 1. It is the smallest provenance-only migration that satisfies
the issue interface while preserving the existing artifact claims.

## Design

### Data Contract

Update
`benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/environment.json`
to store the publishing binary as a schema-v2 runtime identity:

```json
"runtime_identities": [
  {
    "role": "tool://rstim",
    "version": "rstim 0.1.1",
    "basename": "rstim",
    "sha256": "336ab36864ba884314507d39378628aa653f16f9c51693512da510cbf3982568"
  }
]
```

Remove `rstim_binary` and `rstim_binary_sha256` from the checked environment
payload. Preserve the pinned artifact hashes for `summary.json`, `report.md`,
`correctness-summary.json`, and `fixture-load.json`. Recompute only
`environment.json`, `artifact-sha256.json`, and the corresponding catalog
artifact hashes.

### Checker Behavior

`tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py` will:

- load the schema-v2 catalog from
  `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`;
- locate the `frame-instruction-wide-release` bundle and its single
  `tool://rstim` runtime identity;
- validate that `environment.json` contains exactly the same runtime identity;
- reject live-path runtime fields such as `path` or `required_live_path`;
- avoid opening `target/release/rstim` during default validation;
- when `--verify-runtime-binary <path>` is supplied, hash that path and compare
  it to the recorded `tool://rstim` runtime identity, failing with
  `runtime binary SHA-256 does not match recorded identity` on mismatch.

The semantic validation order remains intact: raw, summary/report,
fixture-load, and correctness checks still run before environment/artifact hash
checks, so mutating `correctness-summary.json` to `failed` fails before artifact
hash validation.

### Tests

Extend `tools/test_check_rstim_vs_stim_instruction_wide_noise_evidence.py` with
TDD regression coverage:

- default validation accepts a bundle with no live `rstim` binary path;
- a supplied matching binary passes optional runtime attestation;
- a supplied different binary fails with the exact required message;
- legacy `rstim_binary`/`rstim_binary_sha256` path fields are rejected instead
  of being hashed by default;
- mutating correctness status to `failed` still fails before artifact hashes.

Run the issue verification command, the clean `git archive` verification, the
Python unit tests covering the checker and catalog validator, and `cargo test`.

## Scope

This migration does not rerun benchmark timing, claim a wall-clock speedup, or
change noise sampling behavior. It changes only provenance representation,
checker validation, and regenerated hashes for files whose contents changed.

## Spec Self-Review

- Placeholder scan: no placeholders, TODOs, or open-ended fields remain.
- Consistency check: the chosen approach matches issue #483 and depends on the
  schema-v2 runtime identity from issue #479.
- Scope check: the implementation is a single provenance migration touching the
  checker, checker tests, frame environment metadata, artifact hashes, and the
  catalog entries for changed artifacts.
- Ambiguity check: default validation never opens a live runtime binary;
  optional attestation hashes only the user-supplied binary path.
