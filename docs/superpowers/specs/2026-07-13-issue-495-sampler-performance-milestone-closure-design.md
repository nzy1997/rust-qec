# Issue 495 Sampler Performance Milestone Closure Design

Issue: #495 Close completed sampler-performance milestones
Date: 2026-07-13

## Context

Issue #494 is closed and its readiness gate now exists on `master`. Issue #495
is the operational follow-up: after the readiness gate passes, close exactly the
eight sampler-performance milestone objects and verify their live GitHub state.

No repository instruction file is present in this checkout. This Agent Desk run
is non-interactive, so the Standing Answer Policy resolves the Superpowers
gates:

- Visual companion: not used because this is a backend/GitHub operations task.
- Clarifying questions: answered from issue #495 and the already-merged #494
  readiness gate.
- Design approval: accepted automatically because the issue supplies exact
  milestone titles, ordering constraints, negative control wording, and
  verification commands.
- Spec review: this document is approved for planning after checking for
  placeholders, contradictions, ambiguity, and unrelated scope.

## Approaches Considered

1. Extend the existing readiness checker so `--verify-github` reads milestone
   objects by exact title, requires all eight named milestones to be closed,
   writes their state into the readiness JSON, and prints a second PASS line
   for milestone closure. This keeps the issue #495 verification command as the
   source of truth and gives the negative control stable wording.
2. Leave the readiness checker unchanged and close milestones with ad hoc `gh`
   commands only. This would update GitHub state, but the required verification
   output and negative control would not match the issue.
3. Add a separate milestone-closure script. This would work, but it would split
   the approved readiness gate from the operational state check and require a
   second command that issue #495 does not ask reviewers to run.

The selected approach is option 1. It is the smallest durable repository change
and matches the issue's required interface.

## Milestone Verification

Modify `tools/check_sampler_performance_readiness.py`.

Define the exact titles in a single constant:

- `P0: Fair CLI Benchmark`
- `P1A: Reusable Compiled Sampler`
- `P1B: Packed Inverse Reference Tableau`
- `P1C: Instruction-wide Sparse Noise`
- `M1: Portable Evidence Foundation`
- `M2: Direct Inverse Measurement`
- `M3: Repeat-Aware Reference Sampling`
- `M4: Measured Optimization Closure`

When `--verify-github <owner/repo>` is present, the checker will:

1. Fetch open and closed milestones through GitHub.
2. Match milestones by exact title.
3. Fail with `not ready: milestone remains open: <title>` if any named
   milestone is still open.
4. Fail with `not ready: sampler-performance milestone missing: <title>` if a
   named milestone is absent.
5. Fail with `not ready: duplicate sampler-performance milestone title:
   <title>` if GitHub returns duplicates for a required title.
6. On success, write `issues.milestone.status = "closed"`,
   `issues.milestone.closed = 8`, `issues.milestone.open = 0`, and one
   milestone entry per required title.
7. Print the existing readiness PASS line followed by
   `PASS milestone closure closed=8 open=0`.

`--github-json` remains a unit-test hook. For milestone-state tests it supplies
the mocked milestone array directly instead of mocked issues.

## Live State Change

The live state change is intentionally outside the checker. After the offline
readiness command without `--verify-github` passes, close exactly the eight
required milestones with GitHub's milestone update API. Do not close or modify
issues #38, #379, or #406, and do not edit issue bodies or project metadata.

The closure loop will use milestone numbers discovered by exact title from the
live GitHub milestone list. It will not infer titles, close similarly named
milestones, or touch milestones outside the required set.

## Tests

Update `tools/test_check_sampler_performance_readiness.py`.

Coverage:

- closed mocked milestones print both PASS lines and write closed/open counts;
- an open mocked required milestone fails with
  `milestone remains open: <title>`;
- a missing required milestone fails with
  `sampler-performance milestone missing: <title>`;
- duplicate required milestone titles fail with
  `duplicate sampler-performance milestone title: <title>`.

Final verification:

```sh
python3 tools/check_sampler_performance_readiness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  --out /tmp/rstim-sampler-readiness-offline.json
python3 -m unittest tools.test_check_sampler_performance_readiness -q
cargo test
python3 tools/check_sampler_performance_readiness.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml \
  --verify-github nzy1997/rstim \
  --out /tmp/rstim-sampler-readiness.json
```

The final command must include:

```text
PASS sampler performance readiness bundles=4 reference_speedup>=2 frame_ratio<=1.05
PASS milestone closure closed=8 open=0
```

## Out Of Scope

This PR does not close issues #38, #379, or #406; edit issue bodies; update the
site; add project-board or `auto-resolve` wiring; rerun timing benchmarks; or
claim broad rstim/Stim parity.

## Self-Review

- No placeholders remain.
- The eight required titles are copied exactly from issue #495.
- The state change occurs only after offline readiness passes.
- The checker verifies milestone objects, not only open issues assigned to a
  milestone.
- The negative control wording matches issue #495.
