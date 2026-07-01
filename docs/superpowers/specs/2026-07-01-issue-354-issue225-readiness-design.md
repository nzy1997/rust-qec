# Issue 354 Issue-225 Readiness Report Design

Issue: #354 Add an issue-225 acceleration evidence and closure report

Date: 2026-07-01

## Context

Issue #225 is no longer just a missing random-window method. The current branch
contains the merged benchmark and acceleration chain for #337, #338, #339,
#343, #344, #345, #346, #351, #352, and #353. The local benchmark package under
`benchmarks/qec_code_random_window/` already has release/no-target ladder and
multi-seed smoke targets, summary CSV/Markdown generation, search counters, and
per-stage timing fields.

`gh issue view` is blocked by the Agent Desk shell proxy, so this design uses
the manager-supplied issue body, GitHub connector metadata for the merged PRs,
issue #225 comments, and local benchmark code/tests as the authoritative
context.

## Automatic Answers

This run is non-interactive, so the required brainstorming gates use the
standing answer policy:

- No visual companion is needed because the deliverable is a CLI/Markdown
  readiness report.
- The design is approved from the issue text and adjacent benchmark report
  patterns.
- Use a small standard-library Python module and a committed evidence JSON
  manifest. This follows the issue recommendation and gives reviewers an
  auditable local source for the issue/PR chain without requiring network access.
- Reuse the existing no-target ladder and multi-seed smoke Make targets instead
  of adding another benchmark runner.
- Treat semantic gaps as failures, not timing gates. The report validates
  release/no-target semantics, counters, timing-field presence, seed coverage,
  and expected upper bounds; it does not enforce wall-clock thresholds.

## Approaches Considered

1. Add `benchmarks/qec_code_random_window/issue225_readiness.py`, a committed
   `issue225_evidence.json`, focused unit tests, and a Make target that runs the
   existing smoke targets before writing `report.md` and a greppable decision.
   This is recommended because it keeps generation local, avoids external
   dependencies, and validates the exact benchmark artifacts reviewers need.
2. Extend `summarize.py` with issue-225 readiness mode. This would reuse CSV
   helpers, but the summarizer is case-generic and should not own issue-specific
   closure policy or PR-chain prose.
3. Add only a static documentation page. This would be easy to review, but it
   would not reject missing BB144 rows, targeted runs, missing timing fields, or
   loose upper bounds.

## Design

Add `benchmarks/qec_code_random_window/issue225_readiness.py` with a CLI:

```bash
python3 -m benchmarks.qec_code_random_window.issue225_readiness \
  --evidence benchmarks/qec_code_random_window/issue225_evidence.json \
  --ladder-runs benchmarks/out/qec_code_random_window/no-target-ladder-smoke/local-runs.jsonl \
  --ladder-summary benchmarks/out/qec_code_random_window/no-target-ladder-smoke/summary/summary.csv \
  --multiseed-runs benchmarks/out/qec_code_random_window/no-target-multiseed-smoke/local-runs.jsonl \
  --multiseed-summary benchmarks/out/qec_code_random_window/no-target-multiseed-smoke/summary/summary.csv \
  --out-dir benchmarks/out/qec_code_random_window/issue225-readiness-smoke
```

The module reads:

- `issue225_evidence.json`, containing the milestone grouping, exact issue
  links, merged PR links, titles, merge timestamps, and short evidence notes for
  #337, #338, #339, #343, #344, #345, #346, #351, #352, and #353;
- no-target ladder JSONL and summary CSV for `surface_rotated_d5`, `toric_d5`,
  `bb72`, and `bb144`;
- multi-seed JSONL and summary CSV for `bb72_no_target_smoke` and
  `bb144_no_target_smoke`.

The checker fails if any required case or issue is missing, any successful
benchmark row is not `build_profile = "release"`, any row has non-null
`target_weight`, any command includes `--target-weight`, any no-target
`search_stats.target_reached` is true, any required counter or timing field is
missing, any required timing bucket is empty, the multi-seed smoke rows do not
cover seeds `7`, `11`, and `17`, or the ladder best upper bounds are not exactly
`5`, `5`, `6`, and `12`.

When checks pass, the report includes:

- `issue_225_readiness: PASS` near the top and in `summary.txt`;
- the issue-225 acceleration chain grouped into M1, M2, and M3 tables;
- no-target ladder results with best upper bounds, `target_weight = null`,
  `target_reached = false`, and `build_profile = release`;
- multi-seed BB72/BB144 stability rows for seeds `7`, `11`, and `17`;
- search counters including `weight_pruned_candidates`,
  `kernel_basis_generations`, `component_candidates_generated`, and
  `target_reached`;
- timing buckets including `kernel_basis_time_ns`, `span_filter_time_ns`,
  `witness_validation_time_ns`, and `total_search_time_ns`.

The root `Makefile` adds
`qec-code-random-window-bench-issue225-readiness-smoke`. It serially runs the
existing no-target ladder and no-target multi-seed smoke targets, then invokes
the readiness module and writes generated artifacts under
`benchmarks/out/qec_code_random_window/issue225-readiness-smoke/`.

## Error Handling

Validation errors are accumulated and reported together. The CLI writes no PASS
summary for failed evidence, prints each error to stderr, and exits nonzero.
Errors name the offending case and field, for example `bb144 field
"target_weight"` or `bb144 search_stats.total_search_time_ns`.

## Testing

Use TDD in
`benchmarks/qec_code_random_window/tests/test_issue225_readiness.py`:

1. Known-good fixture JSONL/CSV/evidence inputs produce a Markdown report with
   `issue_225_readiness: PASS`, every required issue number, ladder upper bounds
   `5`, `5`, `6`, and `12`, no-target semantics, required counters, required
   timing buckets, and multi-seed `7;11;17` rows.
2. Missing BB144, a non-null `target_weight`, and
   `search_stats.target_reached = true` are rejected with errors naming the
   offending case and field.
3. Missing timing fields or a loose BB144 upper bound are rejected with errors
   naming the offending case and field.
4. The Makefile/docs test asserts the new target, output directory, and module
   invocation are documented and wired.

Required verification:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness -q
make qec-code-random-window-bench-issue225-readiness-smoke
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness.Issue225ReadinessTest.test_rejects_missing_bb144_or_targeted_run -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_issue225_readiness.Issue225ReadinessTest.test_rejects_missing_timing_or_loose_upper_bound -q
cargo test
```

Out of scope: closing #225 automatically, adding a new distance-search
algorithm, adding external solver/tool dependencies, committing generated
benchmark output, or introducing wall-clock performance thresholds.
