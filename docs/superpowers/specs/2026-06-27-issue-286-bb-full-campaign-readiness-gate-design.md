# Issue 286 BB Full-Campaign Readiness Gate Design

Issue: #286 Add a full-campaign readiness gate for BB small-LDPC runs

Date: 2026-06-27

## Context

Issue #209 warns that the Rust runner should not launch the full 50,000-trial
BB small-LDPC campaign until semantic parity, performance, setup/run
separation, catalog coverage, and diagnostic compare evidence are all present.
Issues #279, #282, #283, #284, and #285 are merged on this branch and provide
the prerequisite artifact contracts:

- #279 adds the BB90 hard-syndrome replay CSV and `verify_replay.py`.
- #282 adds the release-mode hard-syndrome counter smoke and documents its
  printed JSON fields.
- #283 adds setup/run separation counters through the BB p-point runner profile.
- #284 adds the complete `SMALL_LDPC_CASES` catalog and manifest dry run.
- #285 adds paired high-p diagnostic compare rows and `verify_diagnostic.py`.

GitHub API access is unavailable in the Agent Desk sandbox, so this design uses
the issue body supplied by the manager plus local merged issue/PR docs and code
as the authoritative context.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming gates use
the standing answer policy:

- No visual companion is needed because this is a CLI artifact validation
  command, not a visual design.
- The design is approved from the issue text, local merged dependency context,
  and the existing compare-harness verifier patterns.
- Use a dedicated readiness command instead of extending `verify_replay.py` or
  `verify_diagnostic.py`, because the gate must summarize multiple independent
  prerequisites and fail the full campaign when any one is missing.
- Treat missing Python dependency rows as readiness failures. The full campaign
  should not be marked ready from Rust-only diagnostics.
- Avoid wall-clock freshness thresholds. Staleness is defined by artifact
  identity and schema/content mismatches: missing files, malformed CSV/JSON,
  mismatched case catalogs, skipped/error rows, missing counter fields, and
  setup counters that do not prove one setup for the run.
- Keep optional provenance informational. If a provenance artifact is present,
  parse and report its recognized fields; do not require a particular hash or
  timestamp format for readiness.

## Approaches Considered

1. Add `benchmarks/bb_circuit_bposd_compare/ready_for_full.py` as a dedicated
   gate that checks required artifacts by name and reuses existing validators
   for hard replay, diagnostics, and catalog contracts.
   This is recommended because it is explicit, testable, and keeps each
   prerequisite's detailed semantics in the module that already owns them.
2. Add readiness checks into `run_compare.py` as another tier.
   This would keep one CLI surface, but it blurs execution and validation:
   the readiness gate should not run decoders or accidentally start the 50k
   campaign.
3. Add a README checklist only.
   This would help humans but would not give automation a nonzero exit for
   missing, stale, or failing prerequisites.

## Design

Add `benchmarks/bb_circuit_bposd_compare/ready_for_full.py` with a module entry
point:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.ready_for_full --results-dir /tmp/rstim-bb-ready
```

The results directory must contain these artifacts:

- `hard-replay/results.csv`: BB90 hard replay rows.
- `hard-profile/profile.json`: counter-bounded hard-profile evidence from
  `bb90_hard_syndrome_release_profile_is_counter_bounded`.
- `setup-run/profile.json`: setup/run separation evidence from a BB p-point
  profile.
- `small-ldpc-catalog/manifest.csv`: dry-run catalog manifest.
- `diagnostic/results.csv`: high-p diagnostic paired rows.
- optional `provenance.json`: informational metadata such as `artifact_hash`,
  `command`, or `timestamp`.

The command prints one readiness line per prerequisite and a final verdict:

- `PASS` when all required checks pass.
- `WARN` when all required checks pass but optional provenance is missing or
  incomplete.
- `FAIL` when any required artifact is missing, malformed, stale, or failing.

The process exit status is `0` only for `PASS` and `WARN`, and nonzero for
`FAIL`. Warnings therefore surface incomplete provenance without blocking the
campaign if all required technical gates pass.

## Prerequisite Checks

### Semantic Replay

Read `hard-replay/results.csv`, require all `CSV_HEADER` columns, and call
`verify_replay.verify_rows(rows, allow_missing_python=False)`. This enforces
the #279 hard fixture metadata, paired Rust/Python rows, matching logical
predictions, completed timing/logical fields, and Rust OSD/GF(2) counters.

### Counter-Bounded Hard Profile

Read `hard-profile/profile.json` and require:

- `osd_planner == "ldpc_osd_cs"`
- `candidate_limit == 16`
- `planned_candidate_count` is a positive integer
- if present, `ldpc_cs_candidate_bound == planned_candidate_count`
- `osd_candidate_count` is a positive integer no larger than
  `planned_candidate_count` and `candidate_limit`
- `gf2_solve_count == 1`
- `gf2_full_elimination_count == 1`
- `decode_call_count == z_decode_call_count + x_decode_call_count`
- `decode_seconds`, `bp_seconds`, and `osd_seconds` are finite nonnegative
  numbers

Timing values remain evidence only; readiness never fails because a timing value
is slow.

### Setup/Run Separation

Read `setup-run/profile.json` and require:

- `code_build_count == 1`
- `syndrome_cycle_build_count == 1`
- `effective_model_build_count == 1`
- `decoder_build_count == 1`
- `sample_count == num_trials`
- `decode_call_count == z_decode_call_count + x_decode_call_count`
- `setup_seconds`, `sample_seconds`, and `decode_seconds` are finite
  nonnegative numbers

This check proves that setup/model/decoder construction happened once for the
artifact's p-point run and that trial-loop sampling/decoding counters match the
declared trial count.

### Small-LDPC Catalog Coverage

Read `small-ldpc-catalog/manifest.csv`, require all `CATALOG_HEADER` columns,
convert each row to `CompareCase`, and call `validate_small_ldpc_catalog()`.
This enforces the exact 31 target points, 50,000-trial budget, seed, decoder
settings, p values, and unsupported-constructor statuses from #284.

### Diagnostic Compare

Read `diagnostic/results.csv`, require all `CSV_HEADER` columns, and call
`verify_diagnostic.verify_rows(rows, allow_missing_python=False)`. This enforces
paired high-p Rust/Python diagnostic rows, exact BB90/BB144 coverage, completed
timing/status fields, and Rust counter coverage from #285.

### Provenance

If `provenance.json` exists, parse it as an object and report any recognized
`artifact_hash`, `command`, and `timestamp` values. If it is missing or lacks
recognized fields, emit a warning. Malformed provenance is a warning, not a
failure, because issue #286 lists provenance as optional.

## Error Handling

The gate should name each prerequisite in both passing and failing output.
Missing artifact errors include the exact expected relative path. Malformed CSV
or JSON errors include the path and parse error. Stale artifacts are reported by
content mismatch rather than age; examples include missing CSV columns, skipped
Python rows, wrong case ids, wrong catalog p values, wrong planner labels, or
setup counters that indicate per-trial rebuilding.

## Testing

Use TDD in `benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py`:

1. A complete fixture tree passes and prints every prerequisite.
2. A missing hard replay artifact fails nonzero and names
   `hard-replay/results.csv`.
3. A malformed or stale catalog CSV fails and names the catalog artifact and
   offending target.
4. A fixture tree with semantic replay, hard profile, catalog, and diagnostic
   rows but no setup/run artifact fails and names `setup-run/profile.json`.
5. A failing hard profile counter fails without relying on timing thresholds.
6. Missing provenance emits `WARN` while preserving a zero exit status when all
   required checks pass.

Required verification:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py
python3 -m benchmarks.bb_circuit_bposd_compare.ready_for_full --results-dir /tmp/rstim-bb-ready
cargo test
```

Out of scope: launching the full campaign, computing plots, auto-downloading
Python dependencies, and adding new Rust runner behavior.
