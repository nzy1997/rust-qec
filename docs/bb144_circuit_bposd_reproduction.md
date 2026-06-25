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
logical-failure count, 0 failures in 5 trials has a 95% one-sided Clopper-Pearson upper bound of approximately `0.451`. That interval is far too wide to estimate
the BB144 logical error rate shown in the reference figures.

This lower-budget point is therefore implementation smoke evidence: it checks
that the merged circuit-level path accepts the upstream default physical error
rate, cycle count, BP iteration count, and OSD order, runs with an explicit
seed, and emits the expected four-column result line. It does not claim statistical agreement with the upstream 50,000-trial Monte Carlo point.

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

## See Also

- [Benchmark Evidence showcase](docs/showcases/benchmark-evidence.md)
- [Showcase index](docs/showcases/README.md)
