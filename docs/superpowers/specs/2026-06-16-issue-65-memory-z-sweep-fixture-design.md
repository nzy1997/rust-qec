# Issue 65 Memory-Z Sweep Fixture Design

Scope: extend PR #74 with a fixed Stim/PyMatching reference sweep, an RStim
statistical regression test, and a comparison figure.

## Goal

Record the issue #65 rotated-memory-Z sweep as a durable reference dataset and
prove that rsinter/rmatching agrees with it statistically. The comparison should
use the same fifteen cases as the reported Stim/sinter task:

- distances `3, 5, 7`
- rounds `distance * 3`
- physical error rates `0.008, 0.009, 0.010, 0.011, 0.012`
- noise channels:
  - `after_clifford_depolarization`
  - `after_reset_flip_probability`
  - `before_measure_flip_probability`
  - `before_round_data_depolarization`
- stop rule: `max_shots = 1_000_000`, `max_errors = 5_000`
- decoder: PyMatching for the Stim reference and rmatching for the Rust path

## Data Artifacts

Add a checked-in Stim reference fixture:

`rsinter/tests/fixtures/bench/issue65_memory_z_stim_pymatching_sweep.json`

The fixture stores:

- generator metadata: command, versions, timestamp, decoder, stop rule
- one row per `(distance, rounds, p)`
- `shots`, `logical_errors`, `logical_error_rate`
- `num_detectors`, `num_observables`

The fixture is generated once from Stim/PyMatching. If Python `sinter` is
available, the generator should use `sinter.collect` directly; otherwise it may
use the equivalent local Stim + PyMatching flow used by the repository benchmark
drivers. The checked-in fixture is the test input, so regular Rust tests do not
need Python `sinter`.

Add a shared TOML spec for the Rust side:

`rsinter/tests/fixtures/bench/issue65_memory_z_sweep.toml`

It uses `input_type = "memory-z"` and expands to the same fifteen cases.

## Statistical Comparison

Use the existing rsinter binomial interval implementation:

`fit_binomial(shots, logical_errors, 10_000.0)`

For each case:

1. compute the Stim confidence interval from fixture `shots/errors`;
2. run rsinter/rmatching on the matching case set;
3. compute the Rust confidence interval from Rust `shots/errors`;
4. require interval overlap and require the Stim best estimate to be inside the
   Rust interval.

This intentionally does not require an exact logical error rate match. The
higher likelihood factor is used because the requested assertion compares the
Stim point estimate against the Rust interval, even though both estimates are
sampled. Neither side needs a newly introduced seed control. With
`max_errors = 5_000`, the intervals should still be tight enough that a real
mismatch is visible, while normal sampling noise should pass.

## Comparison Figure

Add a generated figure under:

`docs/figures/issue-65-memory-z-stim-vs-rsinter.png`

The plot should show logical error rate versus physical error rate for each
distance. Both Stim/PyMatching and RStim/rmatching must be drawn with binomial
confidence-interval error bars. The figure is a PR review artifact and should
make it visually obvious whether the two curves agree.

## Testing

Add a Rust integration test that:

- loads the fixed Stim fixture;
- runs the Rust sweep from the shared TOML spec;
- asserts all cases are present and `status == "ok"`;
- checks interval overlap and Stim-in-Rust-interval for every case.

The test is intentionally not seed-fixed beyond the existing benchmark harness
behavior. If future CI shows excessive random flakiness, the next change can
tighten the workflow by pinning seeds or by moving this sweep to a heavier
explicit test target.

## Out Of Scope

- Do not change decoder algorithms.
- Do not change the existing memory-X default for specs that omit `input_type`.
- Do not require Python `sinter` during ordinary Rust test execution.
