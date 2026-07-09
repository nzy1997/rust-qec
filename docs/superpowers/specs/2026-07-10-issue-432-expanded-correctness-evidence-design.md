# Issue 432 Expanded Correctness Evidence Design

## Objective

Publish a checked evidence directory that ties together the existing full
`rstim`-vs-Stim correctness summary with the source-grounded small-circuit
distribution verifier results from issues #429 and #430. The new evidence must
be reviewable from stable files, must be checked by a single command, and must
avoid claiming formal proof or broad Stim parity.

## Context

Issue #429 is merged in PR #439 and provides
`benchmarks/rstim_vs_stim_simulator/distribution_cases.toml` plus catalog
validation. Issue #430 is merged in PR #443 and provides
`benchmarks.rstim_vs_stim_simulator.verify_distributions`, which emits per-case
JSON distribution evidence. The repository already contains
`benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json`
with top-level `status = "pass"`.

The issue explicitly excludes the issue #431 frame possible-output regression
tests unless a later issue promotes them into checked JSON evidence. The public
benchmark site is also out of scope.

## Explored Approaches

1. Update the public benchmark site to add a new evidence item. This would make
   the new evidence visible, but the issue explicitly says not to update the
   public benchmark site.
2. Extend the existing full correctness summary under `results/full/`. This
   would mix historical d11/r100 evidence with newly generated distribution
   evidence, making the stable fixture harder to audit.
3. Create a separate `results/distributions/` directory with distribution
   `summary.json`, an expanded rollup JSON, a short report, and a dedicated
   checker. This keeps historical artifacts stable, gives reviewers one checked
   directory for the expanded evidence, and directly matches the requested
   checker command.

Selected approach: option 3. Add the separate checked directory, add a checker
under `tools/`, and lightly extend the distribution verifier summary metadata
so the checked evidence records catalog provenance and environment details.

## Artifact Set

Create these checked files:

- `benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json`
- `benchmarks/rstim_vs_stim_simulator/results/distributions/expanded-correctness.json`
- `benchmarks/rstim_vs_stim_simulator/results/distributions/report.md`

The `summary.json` file is produced by the issue #430 verifier. It records the
distribution cases, pass/fail status, shots, seeds, command lists, catalog hash,
and environment metadata. The `expanded-correctness.json` file links the
distribution summary to the existing full correctness summary by relative path
and SHA-256 hash. The report is reviewer-readable and states the limited scope.

## Verifier Metadata

Extend `benchmarks/rstim_vs_stim_simulator/verify_distributions.py` so emitted
summaries include:

- `catalog_sha256`;
- `environment.rstim_binary_path` when the selected `rstim` command is a direct
  binary path;
- `environment.stim_version` or Stim version command failure details;
- `environment.rustc_version`;
- `environment.cargo_version`;
- `command_line` for CLI runs.

The verifier still does not publish checked evidence by itself and still avoids
performance timing fields.

## Checker

Create `tools/check_rstim_vs_stim_expanded_correctness.py` with the requested
interface:

```sh
python3 tools/check_rstim_vs_stim_expanded_correctness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --distribution-dir benchmarks/rstim_vs_stim_simulator/results/distributions \
  --full-summary benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json
```

The checker validates:

- the catalog parses and lists distribution cases;
- `summary.json` exists, has top-level `status = "pass"`, has one passing entry
  for every catalog case, and has no unknown case IDs;
- `summary.json` records the SHA-256 hash of the catalog being checked;
- `expanded-correctness.json` exists, has `status = "pass"`, points to the
  distribution summary and full correctness summary, and records matching
  SHA-256 hashes for both artifacts;
- `report.md` exists and mentions both the distribution summary and full
  correctness summary paths;
- the full fixture correctness summary still has top-level `status = "pass"`.

A passing checker prints exactly:

```text
PASS expanded rstim-vs-Stim correctness evidence
```

## Tests

Add `tools/test_check_rstim_vs_stim_expanded_correctness.py` with subprocess
tests that build temporary fixtures and run the checker as an external command.
The tests cover:

- a valid fixture exits 0 and prints the exact PASS line;
- a missing catalog case fails with a message containing
  `missing distribution evidence for case`;
- a distribution case with non-pass status is rejected;
- a stale or missing catalog hash is rejected;
- a full summary whose top-level status is not `pass` is rejected;
- a rollup hash mismatch is rejected.

Update the existing distribution verifier tests with a focused test that the
summary records catalog hash and environment metadata.

## Scope Limits

Do not update the public benchmark site. Do not claim all Stim workloads are
covered. Do not incorporate issue #431 frame possible-output tests. Keep the new
distribution evidence separate from `results/full/`.

## Self-Review

- No placeholder text remains.
- The selected checker command matches the issue request.
- The artifact directory is separate from `results/full/`.
- The checker rejects stale catalog hashes and incomplete distribution case
  summaries.
- The full fixture summary remains linked instead of regenerated or modified.
- The public benchmark site and issue #431 artifacts remain out of scope.
