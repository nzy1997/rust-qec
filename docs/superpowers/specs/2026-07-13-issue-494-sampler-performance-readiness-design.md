# Issue 494 Sampler Performance Readiness Design

Issue: #494 Publish sampler-performance readiness evidence
Date: 2026-07-13

## Context

Issue #494 closes the sampler-performance evidence chain without changing
milestone state or site-facing content. The required prerequisite issues are
closed on `master`:

- #484 added `tools/check_all_portable_evidence.py`, which validates the
  schema-v2 portable catalog and all four committed bundles without live Stim or
  Cargo.
- #491 refreshed `fair-cli-release` with a preserved baseline, candidate run,
  derived comparison, and honest prose guard.
- #493 refreshed `frame-instruction-wide-release` with paired frame-noise
  baseline/candidate timing, correctness validation, and a ratio gate.

This Agent Desk run is non-interactive. The Standing Answer Policy resolves the
Superpowers gates:

- Visual companion: not used because this is a backend evidence/checker task.
- Clarifying questions: answered from issue #494 and the committed evidence
  contracts.
- Design approval: accepted automatically because the issue supplies exact
  files, thresholds, negative controls, and verification commands.
- Spec review: this document is approved for planning after checking for
  placeholders, contradictions, ambiguity, and unrelated scope.

## Approaches Considered

1. Add a new readiness checker that calls the existing semantic checkers and
   writes a derived JSON/Markdown readiness pair. This reuses the source of
   truth for portable bundle validation, reference-build evidence, fair CLI
   evidence, frame-noise evidence, distribution correctness, and historical
   #406 preservation while adding only the cross-bundle readiness policy.
2. Duplicate all bundle validation logic inside a new checker. This would make
   the readiness command self-contained but would create a second validator for
   every artifact contract and increase drift risk.
3. Publish only static JSON/Markdown readiness files without a checker. This
   would satisfy the artifact names but would not provide the required single
   PASS/FAIL command or negative controls.

The selected approach is option 1. It is the narrowest change that proves the
existing committed evidence and adds the reviewer-readable readiness artifact.

## Readiness Checker

Add `tools/check_sampler_performance_readiness.py`.

Inputs:

- `--catalog`, required, pointing at
  `benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml`.
- `--out`, required, used to write the derived readiness JSON.
- `--markdown-out`, optional, defaulting to `sampler-performance-readiness.md`.
- `--verify-github <owner/repo>`, optional, for the follow-up operational issue.
- `--github-json`, hidden test hook, used by unit tests to pass mocked GitHub
  issue data without network access.

Validation steps:

1. Load and validate the portable evidence catalog.
2. Require exactly the four schema-v2 bundle IDs and a registered checker for
   each bundle.
3. Run the four portable bundle checkers through the same registry as
   `tools/check_all_portable_evidence.py`.
4. Run the reference-build semantic checker and require:
   `direct_speedup >= 2.0`, zero direct-path canonical materializations, and one
   direct-path executed repeat iteration for the d11/r100 reference records.
5. Run the fair CLI semantic checker and require a complete comparison with:
   `baseline_rstim_over_stim`, `candidate_rstim_over_stim`,
   `ratio_delta_from_baseline`, `reference_strategy =
   direct_inverse_repeat_folded`, and no unsupported parity wording.
6. Run the instruction-wide frame-noise semantic checker and require
   `candidate_over_baseline <= 1.05`, `correctness-summary.json` pass, and the
   existing deterministic frame counters.
7. Run expanded distribution correctness validation against the committed
   distribution artifacts and full correctness summary.
8. Run the historical #406 guard to prove
   `benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json` is
   unchanged.
9. If `--verify-github` is supplied, query open GitHub issues for the repository
   and fail when an open issue whose title or body indicates sampler-performance
   milestone state remains unresolved. The failure must include the issue title.

On success the checker prints exactly:

```text
PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05
```

On failure it prints:

```text
not ready: <reason>
```

to stderr and exits nonzero. The wording is intentionally stable so the issue's
negative controls can assert `not ready`.

## Readiness JSON

Add the checked output
`benchmarks/rstim_vs_stim_simulator/results/sampler-performance-readiness.json`.
The checker derives the same structure when run with `--out`.

Top-level fields:

- `status`: `"ready"`.
- `catalog_path`, `catalog_sha256`, `bundle_count`, and `bundle_ids`.
- `portable_bundles`: one entry per bundle with id, bundle path, checker name,
  and pass line.
- `reference_build`: direct/canonical speedup, direct canonical materialization
  count, direct executed repeat iteration count, direct skipped repeat count,
  variant names, and links to summary/raw/report artifacts.
- `fair_cli`: baseline ratio, candidate ratio, ratio delta, reference strategy,
  and links to comparison/summary/report artifacts.
- `frame_noise`: candidate-over-baseline ratio, outcome, correctness status,
  frame counters, and links to paired/correctness artifacts.
- `distribution_correctness`: status, distribution case count, and links to
  summary/rollup/report/full correctness artifacts.
- `historical_406`: status, selected case label, pinned SHA-256, and historical
  ratio.
- `focused_rust_tests`: the exact command from issue #494 as an argv array.
- `claim_limits`: explicit limits that the readiness artifact is not a broad
  Stim parity claim, does not close #406 or #379, and leaves site-facing #379
  work separate.
- `issues`: links for #38, #406, and #379.

The committed JSON is generated from the current repository artifacts. It may
not include machine-local absolute paths.

## Readiness Markdown

Add root `sampler-performance-readiness.md`.

The checker writes this file from the JSON every time it succeeds. The Markdown
must link the four bundle directories and issue #38, #406, and #379. It must
state:

- readiness is limited to the committed bundles and focused Rust tests;
- reference direct/canonical speedup is at least `2.0x`;
- frame-noise candidate/baseline ratio is at most `1.05`;
- distribution and frame correctness are checked;
- historical #406 evidence is unchanged;
- site-facing #379 remains separate and is not closed by this artifact.

The Markdown is derived from JSON fields instead of manually restating values.

## Tests

Add `tools/test_check_sampler_performance_readiness.py`.

Coverage:

- direct script `--help` imports without side effects;
- committed catalog produces the exact PASS line and writes JSON and Markdown;
- committed Markdown is regenerated byte-for-byte from the committed JSON;
- an absolute provenance value in one catalog entry fails with `not ready`;
- a mocked reference speedup below `2.0` fails with `not ready`;
- a mocked frame ratio above `1.05` fails with `not ready`;
- a mocked GitHub response with one open sampler-performance milestone issue
  fails and includes its title.

Final verification:

```sh
python3 tools/check_sampler_performance_readiness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  --out /tmp/rstim-sampler-readiness.json
python3 -m unittest tools.test_check_sampler_performance_readiness -q
cargo test -p rstim \
  --test reusable_compiled_measurement_sampler \
  --test packed_inverse_tableau_storage \
  --test packed_inverse_tableau_clifford \
  --test packed_inverse_tableau_measurement \
  --test packed_inverse_direct_collapse \
  --test packed_reference_routing \
  --test reference_sample_tree \
  --test repeat_aware_reference_sample \
  --test rare_error_iterator \
  --test frame_instruction_wide_one_qubit_noise \
  --test frame_instruction_wide_depolarize2
cargo test
```

## Out Of Scope

This PR does not change milestone state, close #406 or #379, update the site,
add board wiring, rerun timing benchmarks, or claim broad rstim/Stim parity.

## Self-Review

- No placeholders remain.
- The exact issue thresholds are present: reference speedup `>= 2.0` and frame
  ratio `<= 1.05`.
- The four portable bundle checkers, distribution correctness checker, and
  historical #406 checker remain the source of truth.
- The Markdown derives from JSON and carries the required links and claim
  limits.
- The optional GitHub path is non-mutating and only reads issue state.
