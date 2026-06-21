# Issue #123 BB144 Reproduction Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Check in reviewer-readable BB144 circuit-level BP-OSD reproduction evidence for issue #123.

**Architecture:** Add one durable evidence note under `docs/`, copy the two pinned reference figures into `docs/figures/bb144_reference/`, and protect the evidence with a focused integration test. Fix the CLI negative-control parsing at the flag boundary so existing validation reports invalid physical error rates.

**Tech Stack:** Rust 2024, `rsinter` CLI tests, Markdown docs, pinned Git reference assets, `cargo test`.

## Global Constraints

- Evidence point parameters are `physical_error_rate = 0.003`, `num_cycles = 12`, `max_bp_iterations = 10000`, and `osd_order = 7`.
- The completed checked evidence run is the lower-budget surrogate `num_trials = 5`, `seed = 12345`, with output `0.003	12	5	0`.
- The note must explain that the full `num_trials = 50_000` run and the issue's 100-trial reviewer command were too expensive in this worker environment.
- The note must distinguish implementation smoke evidence from statistical agreement with the upstream 50,000-trial Monte Carlo point.
- The note must explicitly name `small_ldpc.png` and `ldpc_vs_surface.png` and state which BB144 target each figure contributes.
- The two reference figures must be copied from pinned commit `3f66ab3e803d0836b3fb12601d6a9c44149ab11c`.
- The negative-control command with `--physical-error-rate -0.1` must fail before printing a completed four-column result line and stderr must identify `physical_error_rate`.
- Do not change the decoder algorithm, schedule, code construction, or default upstream parameter mapping.

---

## File Structure

- Create: `docs/bb144_circuit_bposd_reproduction.md`
  - Reviewer evidence note with commands, output, confidence interval, reference figure interpretation, smoke command, full manual command, negative control, and limits.
- Create: `docs/figures/bb144_reference/small_ldpc.png`
  - Local copy of the pinned small LDPC family reference figure.
- Create: `docs/figures/bb144_reference/ldpc_vs_surface.png`
  - Local copy of the pinned LDPC vs surface-code reference figure.
- Modify: `rsinter/tests/bench_cli.rs`
  - Add an evidence-note contract test and a negative-control CLI test.
- Modify: `rsinter/src/bin/rsinter.rs`
  - Allow the physical-error-rate clap argument to accept negative-looking values so library validation can reject them.

## Task 1: Evidence Note, Figures, And Contract Test

**Files:**
- Create: `docs/bb144_circuit_bposd_reproduction.md`
- Create: `docs/figures/bb144_reference/small_ldpc.png`
- Create: `docs/figures/bb144_reference/ldpc_vs_surface.png`
- Modify: `rsinter/tests/bench_cli.rs`

**Interfaces:**
- Produces: a docs note whose durable tokens are enforced by `bb144_reproduction_evidence_note_records_required_context`.
- Consumes: pinned Git commit `3f66ab3e803d0836b3fb12601d6a9c44149ab11c`.

- [ ] **Step 1: Write the failing evidence-note contract test**

Append this test to `rsinter/tests/bench_cli.rs`:

```rust
#[test]
fn bb144_reproduction_evidence_note_records_required_context() {
    let note = include_str!("../../docs/bb144_circuit_bposd_reproduction.md");

    for required in [
        "0.003\t12\t5\t0",
        "--num-trials 50000",
        "--seed 12345",
        "95% one-sided Clopper-Pearson upper bound",
        "does not claim statistical agreement",
        "small_ldpc.png",
        "red [[144,12,12]] LDPC curve",
        "ldpc_vs_surface.png",
        "red-diamond LDPC [[144,12,12]] curve",
        "physical_error_rate must be finite and lie in [0, 1)",
    ] {
        assert!(note.contains(required), "missing evidence token: {required}");
    }
}
```

- [ ] **Step 2: Run the new test and confirm it fails**

Run:

```sh
cargo test -p rsinter --offline --test bench_cli bb144_reproduction_evidence_note_records_required_context
```

Expected: FAIL because `docs/bb144_circuit_bposd_reproduction.md` does not exist yet.

- [ ] **Step 3: Copy the pinned reference figures into the repository**

Run:

```sh
git fetch origin 3f66ab3e803d0836b3fb12601d6a9c44149ab11c
git checkout FETCH_HEAD -- docs/figures/bb144_reference/small_ldpc.png docs/figures/bb144_reference/ldpc_vs_surface.png
```

Expected: the two PNG files exist at the paths named above.

- [ ] **Step 4: Add the evidence note**

Create `docs/bb144_circuit_bposd_reproduction.md` with this content:

````markdown
# BB144 Circuit-Level BP-OSD Reproduction Evidence

This note records the checked reproduction evidence for the upstream
bivariate-bicycle `[[144,12,12]]` circuit-level BP-OSD memory point requested
in issue #123.

## Reference Targets

- `docs/figures/bb144_reference/small_ldpc.png` is the small LDPC family
  target. For this issue, it contributes the red [[144,12,12]] LDPC curve in
  the log-log plot of logical error rate `p_L` versus physical error rate `p`.
- `docs/figures/bb144_reference/ldpc_vs_surface.png` is the LDPC versus
  surface-code target. For this issue, it contributes the red-diamond LDPC
  [[144,12,12]] curve. This note does not claim agreement with the plotted
  surface-code curves because those comparison runs were not reproduced here.

The first target point for both figures is the BB144 default
`p = 0.003`, `num_cycles = 12`, `num_trials = 50_000`.

## Checked Lower-Budget Command

The full 50,000-trial run was too expensive for this Agent Desk worker. The
issue's 100-trial reviewer command was also too expensive here: a dev-profile
attempt ran for 495s without producing a result, and a release-profile attempt
ran for 403s without producing a result. I therefore recorded the stricter
default-parameter smoke point below with the same `p`, cycle count, BP
iteration limit, and OSD order, but with `num_trials = 5`.

```bash
cargo run -p rsinter --offline -- bb-circuit-bposd-memory \
  --physical-error-rate 0.003 \
  --num-cycles 12 \
  --num-trials 5 \
  --seed 12345 \
  --max-bp-iterations 10000 \
  --osd-order 7
```

Observed stdout:

```text
0.003	12	5	0
```

Random seed policy: this checked evidence uses `--seed 12345` so the smoke
sample is reproducible. Omitting `--seed` keeps the CLI's entropy-seeded
behavior, matching the upstream script's unseeded Monte Carlo policy.

## Interpretation

The observed lower-budget failure fraction is `0 / 5 = 0`. For a binomial
logical-failure count, 0 failures in 5 trials has a 95% one-sided
Clopper-Pearson upper bound of approximately `0.451`. That interval is far too
wide to estimate the BB144 logical error rate shown in the reference figures.

This lower-budget point is therefore implementation smoke evidence: it checks
that the merged circuit-level path accepts the upstream default physical error
rate, cycle count, BP iteration count, and OSD order, runs with an explicit
seed, and emits the expected four-column result line. It does not claim
statistical agreement with the upstream 50,000-trial Monte Carlo point.

Relative to `small_ldpc.png`, the smoke result exercises the same BB144 red
[[144,12,12]] LDPC curve point at `p = 0.003`, but it is not precise enough to
place a new marker on that curve. Relative to `ldpc_vs_surface.png`, it
exercises the same red-diamond LDPC [[144,12,12]] curve point, and it makes no
claim about the surface-code curves.

## Full Manual Reproduction Command

Use this command for the upstream-budget reproduction when an environment has
enough wall-clock budget:

```bash
cargo run --release -p rsinter -- bb-circuit-bposd-memory \
  --physical-error-rate 0.003 \
  --num-cycles 12 \
  --num-trials 50000 \
  --max-bp-iterations 10000 \
  --osd-order 7
```

The expected output shape is:

```text
0.003	12	50000	<num_failed_trials>
```

For statistical comparison to the reference figures, interpret
`num_failed_trials / 50000` with binomial Monte Carlo uncertainty. A 50,000
trial run is the first run budget in this note that should be used to claim
agreement or disagreement with the upstream plotted BB144 point.

## Smaller Local Sanity Check

For a quick CI or local sanity check that exercises the same CLI path without
the default decoder budget, run:

```bash
cargo run -p rsinter -- bb-circuit-bposd-memory \
  --physical-error-rate 0.003 \
  --num-cycles 12 \
  --num-trials 1 \
  --seed 12345 \
  --max-bp-iterations 10 \
  --osd-order 0
```

This command is an implementation check only. It changes decoder parameters
and is not a reproduction point.

## Negative Control

Invalid physical error rates are rejected before a completed result line is
printed:

```bash
cargo run -p rsinter -- bb-circuit-bposd-memory \
  --physical-error-rate -0.1 \
  --num-cycles 12 \
  --num-trials 100
```

Expected stderr includes:

```text
physical_error_rate must be finite and lie in [0, 1)
```

## Limits

- The checked result is a 5-trial surrogate, not the upstream 50,000-trial
  campaign.
- The reference figures are preserved as visual targets, but their curves were
  not digitized in this note.
- No surface-code comparison runs were reproduced.
- No decoder algorithm, schedule, or BB144 default mapping changes are claimed
  here.
````

- [ ] **Step 5: Run the contract test and confirm it passes**

Run:

```sh
cargo test -p rsinter --offline --test bench_cli bb144_reproduction_evidence_note_records_required_context
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

Run:

```sh
git add docs/bb144_circuit_bposd_reproduction.md docs/figures/bb144_reference/small_ldpc.png docs/figures/bb144_reference/ldpc_vs_surface.png rsinter/tests/bench_cli.rs
git commit -m "docs: record bb144 reproduction evidence"
```

## Task 2: Negative-Control CLI Parsing

**Files:**
- Modify: `rsinter/tests/bench_cli.rs`
- Modify: `rsinter/src/bin/rsinter.rs`

**Interfaces:**
- Consumes: existing `run_simulation(SimulationConfig)` validation.
- Produces: a CLI that parses `--physical-error-rate -0.1` as a value and rejects it with the existing `physical_error_rate` validation message.

- [ ] **Step 1: Write the failing negative-control test**

Append this test to `rsinter/tests/bench_cli.rs`:

```rust
#[test]
fn rsinter_bb_circuit_bposd_memory_rejects_negative_physical_error_rate() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bb-circuit-bposd-memory",
            "--physical-error-rate",
            "-0.1",
            "--num-cycles",
            "12",
            "--num-trials",
            "100",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "{output:?}");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(
        stdout.trim().is_empty(),
        "invalid command should not print a completed result line: {stdout:?}"
    );
    assert!(
        stderr.contains("physical_error_rate"),
        "stderr should identify the invalid physical error rate: {stderr}"
    );
    assert!(
        !stdout
            .lines()
            .any(|line| line.split_whitespace().count() == 4),
        "invalid command printed a four-column result line: {stdout:?}"
    );
}
```

- [ ] **Step 2: Run the test and confirm it fails for the current parser reason**

Run:

```sh
cargo test -p rsinter --offline --test bench_cli rsinter_bb_circuit_bposd_memory_rejects_negative_physical_error_rate
```

Expected: FAIL because stderr says `unexpected argument '-0' found` and does not contain `physical_error_rate`.

- [ ] **Step 3: Apply the minimal clap parsing fix**

In `rsinter/src/bin/rsinter.rs`, change the physical error rate arg from:

```rust
#[arg(long, default_value_t = 0.003)]
physical_error_rate: f64,
```

to:

```rust
#[arg(long, default_value_t = 0.003, allow_hyphen_values = true)]
physical_error_rate: f64,
```

- [ ] **Step 4: Run the negative-control test and focused CLI tests**

Run:

```sh
cargo test -p rsinter --offline --test bench_cli rsinter_bb_circuit_bposd_memory_rejects_negative_physical_error_rate
cargo test -p rsinter --offline --test bench_cli rsinter_bb_circuit_bposd_memory_prints_four_column_result_line
```

Expected: both commands PASS.

- [ ] **Step 5: Commit Task 2**

Run:

```sh
git add rsinter/src/bin/rsinter.rs rsinter/tests/bench_cli.rs
git commit -m "fix: reject negative bb144 physical error rates"
```

## Task 3: Final Evidence Verification

**Files:**
- No intended source edits.

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: verification evidence for the PR body and final response.

- [ ] **Step 1: Run the checked evidence command**

Run:

```sh
cargo run -p rsinter --offline -- bb-circuit-bposd-memory \
  --physical-error-rate 0.003 \
  --num-cycles 12 \
  --num-trials 5 \
  --seed 12345 \
  --max-bp-iterations 10000 \
  --osd-order 7
```

Expected stdout:

```text
0.003	12	5	0
```

- [ ] **Step 2: Run the negative control manually**

Run:

```sh
cargo run -p rsinter --offline -- bb-circuit-bposd-memory \
  --physical-error-rate -0.1 \
  --num-cycles 12 \
  --num-trials 100
```

Expected: non-zero exit; stdout has no four-column line; stderr contains `physical_error_rate must be finite and lie in [0, 1)`.

- [ ] **Step 3: Run the focused docs and CLI tests**

Run:

```sh
cargo test -p rsinter --offline --test bench_cli bb144_reproduction_evidence_note_records_required_context
cargo test -p rsinter --offline --test bench_cli rsinter_bb_circuit_bposd_memory_rejects_negative_physical_error_rate
cargo test -p rsinter --offline --test bench_cli rsinter_bb_circuit_bposd_memory_prints_four_column_result_line
```

Expected: all commands PASS.

- [ ] **Step 4: Run the required workspace verification**

Run:

```sh
cargo test --offline
```

Expected: PASS.

- [ ] **Step 5: Run status and review gates**

Run:

```sh
git status --short
git log --oneline origin/master..HEAD
```

Expected: only intentional committed changes; no dirty tracked files before PR publishing.

- [ ] **Step 6: Finish through PR workflow**

Use `superpowers:requesting-code-review` for final review, then use
`superpowers:finishing-a-development-branch`. Per the standing answer policy,
choose `Push and create a Pull Request` when the finishing skill presents the
menu. Stop after PR creation.

## Self-Review

- Spec coverage: The plan covers the evidence note, local reference figures,
  checked output, confidence interpretation, reference-target statements,
  smoke command, negative control, and final verification.
- Marker scan: No open markers or open-ended implementation steps are present.
- Type consistency: Test names, file paths, and command flags match the current
  `rsinter` CLI and repository layout.
