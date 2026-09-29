# Near-Clifford P0/P1 benchmark

Run from the repository root:

```sh
python3 benchmarks/near_clifford/run.py
```

The runner builds the release example, then runs each fixture in its own process.
It writes raw repetitions and a readable report to the ignored `drafts/near-clifford-matrix/`
directory. `--group p0` or `--group p1` limits the matrix; `--quick` runs a smoke
measurement, not a publishable performance result.

P0 separates compilation, preparation, the first prepared call, and a retained
sampler, for structured and flat output across shot counts. It includes a
terminal fixture, repeated terminal measurements, stochastic mid-circuit
feedback, and a small synthetic QEC syndrome circuit with three noisy rounds.
The QEC circuit is a workload probe, not a logical-error benchmark.

P1 probes 64/65/128/256 measurement records, 64–80 independent random
measurements, and requested active ranks 8–11. These intentionally straddle
terminal planning, symbolic suffix, and cache coefficient limits. A fixture
name describes how its circuit is constructed; it does not assert that the
runtime reached that peak active rank.

Every result records the exact circuit text, source revision, platform, Rust
version, raw timing repetitions, and process peak RSS. Use paired runs on one
quiet machine to compare code revisions. Times exclude process launch and
result destruction; the Clifft comparison in PR #764 used a separate harness.
