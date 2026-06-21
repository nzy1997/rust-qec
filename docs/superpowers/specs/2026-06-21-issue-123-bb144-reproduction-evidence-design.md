# Issue #123 BB144 Reproduction Evidence Design

Date: 2026-06-21
Status: Non-interactive Agent Desk design, auto-approved by standing policy
Scope: GitHub issue #123, BB144 circuit-level BP-OSD reproduction evidence

## Context

PR #115 merged the `rsinter bb-circuit-bposd-memory` path for the upstream
BB144 circuit-level BP-OSD memory simulation. Issue #123 asks for checked,
reviewer-readable evidence for the upstream default point:

- `physical_error_rate = 0.003`
- `num_cycles = 12`
- `num_trials = 50_000`
- `max_bp_iterations = 10000`
- `osd_order = 7`

The repository does not currently contain the uploaded reference figures from
the issue, and the 100-trial reviewer command is already expensive in this
Agent Desk environment. A first dev-profile 100-trial attempt ran for 495s
without producing a result, and a release-profile 100-trial attempt ran for
403s without producing a result. A 5-trial default-parameter run completed in
23.02s and printed `0.003	12	5	0`.

The evidence therefore needs to document a lower-budget surrogate honestly:
it can prove the merged path runs the upstream default parameter mapping and
can provide a binomial confidence interval for the observed failure fraction,
but it cannot make a precise 50,000-trial Monte Carlo agreement claim.

## Goals

- Add a checked-in evidence note under `docs/` with the exact command line,
  output, seed policy, confidence-interval interpretation, reference-figure
  interpretation, smoke command, full manual command, and limits.
- Keep the uploaded reference figures available in-repo under
  `docs/figures/bb144_reference/` so future reviewers do not depend on private
  issue uploads or remote raw URLs.
- Add a reviewer-facing test fixture that checks the evidence note names the
  required commands, output, reference figures, and limits.
- Fix the negative-control CLI path so `--physical-error-rate -0.1` reaches
  the `SimulationConfig` validation and produces an invalid physical error
  rate message instead of being parsed as an unexpected flag.
- Keep algorithm and default decoder semantics unchanged.

## Non-Goals

- Do not run or claim completion of the full 50,000-trial campaign in this
  worker run.
- Do not digitize or fit the reference curves in this issue.
- Do not add a broader benchmark campaign format.
- Do not change the BP-OSD algorithm, schedule, code construction, or default
  parameter mapping from PR #115.

## Approach Options

### Recommended: Documentation Evidence Plus Contract Test

Add a focused `docs/bb144_circuit_bposd_reproduction.md` note, check in the two
reference PNGs, and add a small `rsinter` integration test that verifies the
note contains the exact evidence tokens expected by the issue. Add one CLI
negative-control test for the `-0.1` physical error rate path and fix clap
parsing with the smallest possible flag-level change.

This is the least invasive approach. It creates reviewer-readable evidence,
keeps the artifact durable, and protects the issue requirements without
turning a manual statistical run into CI work.

### Alternative: Add A Benchmark Campaign Spec

Create a reusable campaign manifest for BB144 figure reproduction and connect
it to the `rsinter bench` flow. This could eventually support whole-curve
reproduction, but it is broader than issue #123 and would couple a manual
long-running campaign to benchmark infrastructure that is tracked separately.

### Alternative: Store Only A PR Comment Or Unchecked Note

Put the observed command and output only in the PR description or add a doc
without tests. This is fast, but it makes the evidence easier to drift or omit
from future reviews.

## Design

### Evidence Note

Create `docs/bb144_circuit_bposd_reproduction.md` with:

- the completed lower-budget command and output:
  `0.003	12	5	0`;
- the aborted 100-trial timing observations that justify the lower budget;
- the manual full 50,000-trial command with the upstream defaults;
- the seed policy: checked evidence uses `--seed 12345`, upstream/default
  unseeded behavior uses entropy;
- a binomial interpretation for 0 failures in 5 trials, including the 95%
  one-sided Clopper-Pearson upper bound of about `0.451`;
- explicit language that this smoke point is not a statistical reproduction
  of the full 50,000-trial point;
- local links to `small_ldpc.png` and `ldpc_vs_surface.png`, naming the BB144
  red curve or red-diamond target each figure contributes;
- the negative-control command and expected validation message.

### Reference Figures

Copy the pinned files from commit
`3f66ab3e803d0836b3fb12601d6a9c44149ab11c` into:

- `docs/figures/bb144_reference/small_ldpc.png`
- `docs/figures/bb144_reference/ldpc_vs_surface.png`

The evidence note links to these local paths.

### Tests

Extend `rsinter/tests/bench_cli.rs` with two tests:

- `bb144_reproduction_evidence_note_records_required_context` reads the note
  and verifies it contains the exact smoke output, full manual command,
  `small_ldpc.png`, `ldpc_vs_surface.png`, and the documented limitation.
- `rsinter_bb_circuit_bposd_memory_rejects_negative_physical_error_rate`
  invokes the negative-control command and verifies the command fails without
  producing a four-column result line and that stderr names
  `physical_error_rate`.

### CLI Parsing Fix

Set the clap physical-error-rate argument to accept hyphenated values so
`--physical-error-rate -0.1` is parsed as the argument value and then rejected
by existing validation:

```rust
#[arg(long, default_value_t = 0.003, allow_hyphen_values = true)]
physical_error_rate: f64,
```

No library validation changes are required because
`validate_physical_error_rate` already rejects negative values with
`physical_error_rate must be finite and lie in [0, 1)`.

## Approval

The run is non-interactive. The standing answer policy chooses the recommended
documentation evidence plus contract test approach because it is conservative,
reviewable, and scoped to issue #123. It also chooses the lower-budget
surrogate because the required 100-trial and 50,000-trial commands exceed the
available worker budget.
