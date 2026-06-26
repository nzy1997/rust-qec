# Issue 287 BB BP-OSD Readiness Report Design

Issue: #287 Generate a reviewer-readable BB BP-OSD readiness report

Date: 2026-06-27

## Context

Issue #286 is merged on this branch as PR #298. It adds
`benchmarks/bb_circuit_bposd_compare/ready_for_full.py`, a machine gate that
checks the complete readiness artifact tree and returns `PASS`, `WARN`, or
`FAIL`. The report in #287 must reuse that verdict so the human-facing report
cannot drift from the machine readiness gate.

GitHub issue body access through `gh issue view` is blocked by the Agent Desk
sandbox proxy, so this design uses the manager-supplied issue text plus local
merged #286 code, tests, README, and Superpowers docs as the authoritative
context.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming gates use
the standing answer policy:

- No visual companion is needed because the output is a Markdown CLI report,
  not an interactive or visual layout.
- The design is approved from the issue text and the merged #286 readiness gate
  contract.
- Use a dedicated writer and validator module. This matches the issue's likely
  file names, keeps generation separate from gate execution, and lets
  automation validate a previously generated report against a results
  directory.
- Generate Markdown rather than HTML. Markdown is simpler to review in a PR,
  diff, and terminal, and still supports tables.
- Include compact machine-readable section snapshots in HTML comments. The
  visible report is reviewer-readable, while the validator can compare exact
  artifact hashes, row counts, counters, coverage summaries, and the final
  verdict against current source artifacts.
- Treat a stale report as a validator failure. A report generated from one
  artifact tree must fail when validated against a different or mutated tree,
  even if the report text still says `PASS`.

## Approaches Considered

1. Add `write_readiness_report.py` and `validate_readiness_report.py`, with the
   writer building a report model from the #286 gate and source artifacts, and
   the validator rebuilding that model from `--results-dir` to compare against
   the generated report.
   This is recommended because it directly satisfies the requested CLI
   interface, preserves #286 as the verdict source of truth, and gives the
   negative controls a real stale-artifact check.
2. Add a `--report` mode to `ready_for_full.py`.
   This would reduce file count, but it makes the gate responsible for both
   machine policy and reviewer presentation, and it does not naturally validate
   a report file produced earlier.
3. Generate a report from the gate's console summary only.
   This is too shallow: it would not expose the requested semantic rows,
   counters, setup/run evidence, diagnostic rows, and complete catalog coverage
   in reviewer-readable tables.

## Design

Add two module entry points:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.write_readiness_report --results-dir /tmp/rstim-bb-ready --out /tmp/bb-bposd-readiness.md
python3 -m benchmarks.bb_circuit_bposd_compare.validate_readiness_report --results-dir /tmp/rstim-bb-ready --report /tmp/bb-bposd-readiness.md
```

`write_readiness_report.py` builds a report model from the results directory:

- call `ready_for_full.check_results_dir(results_dir)` and
  `ready_for_full.readiness_verdict(results)` for the gate checks and final
  verdict;
- read `hard-replay/results.csv` for semantic replay rows;
- read `hard-profile/profile.json` for BB90 hard-profile counters;
- read `setup-run/profile.json` for setup/run split evidence;
- read `diagnostic/results.csv` for paired Rust/Python diagnostic rows;
- read `small-ldpc-catalog/manifest.csv` and summarize complete coverage by
  code id, cycle count, p values, case count, and catalog status;
- hash each required source artifact with SHA-256 and store those hashes in the
  model.

The Markdown report contains:

- title, source results directory, generation timestamp, and final readiness
  verdict;
- a gate summary table for every #286 check;
- `Semantic Parity Replay` table with Rust/Python row status, logical
  prediction, expected logical, syndrome weight, timings, and LER;
- `BB90 Hard-Profile Counters` table with planner, candidate bound, bounded
  candidate count, OSD use, BP iterations, GF(2) counts, per-basis decode call
  split, and timings;
- `Setup/Run Split Evidence` table with build counts, sample count, trial
  count, decode call split, and setup/sample/decode timings;
- `Diagnostic Rust/Python Compare Rows` table for the BB90 and BB144 high-p
  diagnostic rows;
- `Small-LDPC Case Coverage` table summarizing all 31 manifest cases and
  unsupported constructor statuses;
- an embedded `rstim-bb-readiness-snapshot` JSON comment containing the exact
  model used to render the visible sections.

`validate_readiness_report.py` reads the report and source artifacts, then:

- requires the expected report headings and visible final verdict line;
- rebuilds the report model from `--results-dir`;
- parses the embedded snapshot from the report and compares it exactly to the
  rebuilt model, including artifact hashes and the #286 verdict;
- checks that important visible tokens from each section are present in the
  report text, so a metadata-only or placeholder report is rejected;
- exits nonzero with section-specific failure messages for missing snapshots,
  stale source hashes, mismatched verdicts, missing artifact-backed tokens, or
  absent required sections.

## Error Handling

The writer should still generate a report for `FAIL` or `WARN` artifact trees;
reviewers need to see why readiness failed. Missing or malformed artifacts
therefore appear in the gate summary with the #286 failure messages, and the
detail sections show empty or partial tables where source files cannot be read.

The validator is stricter. It fails if the report cannot be read, lacks the
snapshot, lacks required sections, has a visible final verdict different from
the current #286 verdict, or has a snapshot different from the current source
artifact model. Failure messages name the affected section or artifact class.

## Testing

Use TDD in
`benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py`:

1. A complete fixture tree produces a Markdown report containing semantic
   replay status, BB90 hard-profile counters, setup/run split evidence,
   diagnostic compare rows, complete small-LDPC coverage, and the final #286
   verdict.
2. The validator accepts that generated report for the same results directory.
3. A report generated from a complete tree is rejected after the source
   artifacts are mutated or missing, and the error names the stale or missing
   section.
4. A report edited to say `PASS` while #286's gate returns `FAIL` is rejected
   for a final verdict mismatch.
5. A placeholder report with headings but without artifact-backed snapshot and
   visible section tokens is rejected.

Required verification:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py
python3 -m benchmarks.bb_circuit_bposd_compare.write_readiness_report --results-dir /tmp/rstim-bb-ready --out /tmp/bb-bposd-readiness.md
python3 -m benchmarks.bb_circuit_bposd_compare.validate_readiness_report --results-dir /tmp/rstim-bb-ready --report /tmp/bb-bposd-readiness.md
cargo test
```

Out of scope: publication plotting, running the full benchmark campaign,
changing benchmark data, and changing #286 readiness policy.
