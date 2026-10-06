# Near-Clifford P0/P1 benchmark

Run from the repository root:

```sh
python3 benchmarks/near_clifford/run.py
```

The runner builds the release example, then runs each fixture in its own process.
It writes raw repetitions and a readable report to the ignored `drafts/near-clifford-matrix/`
directory. `--group p0` or `--group p1` limits the matrix; `--quick` runs a smoke
measurement, not a publishable performance result.
`--no-build` reuses an existing binary and marks its source revision unverified;
every run records the binary's SHA-256 hash.

The [Apple M4 measurement and analysis](results/apple-m4-2026-09-29.md) and its
[raw JSON](results/apple-m4-2026-09-29.json) are retained for this matrix.
The subsequent [boundary validation and CPU profile](results/apple-m4-boundary-validation-2026-09-29.md)
include paired raw data for the symbolic-width and rank-cache experiments.
The [post-fix wide-mask measurement](results/apple-m4-wide-post-fix-2026-09-29.md)
retains timings for 128, 129, and 193 independent random bits.

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
result destruction. This matrix uses `StdRng`; the Clifft comparison in PR #764
used a separate harness with `SmallRng`, so its absolute times are not directly
comparable to this report.

The expanded [43-configuration M4 campaign and profiling](results/apple-m4-scale-analysis-2026-09-30.md)
compares the merged #764/#766 revisions, includes rank/width combinations and mixed
circuits, and retains diagnostic cache/fallback counters separately from timing.
See the [scale harness](scale/README.md) for reproduction.

The [single-qubit Pauli fast-path campaign](results/apple-m4-pauli-analysis-2026-09-30.md)
compares the implementation with merged #766, checks the same 43 configurations,
and profiles the remaining axis-canonicalization and high-rank projection costs.

The [fused-projection and canonical-axis campaign](results/apple-m4-projection-analysis-2026-09-30.md)
compares with merged #767, retains the same 43 configurations and final profiles,
and discloses the small pure-Clifford warm regressions alongside high-rank gains.

The [final unified-CLI baseline campaign](results/apple-m4-projection-synced-analysis-2026-09-30.md)
revalidates that optimization after merged #765 with a separate dependency lock.

The [probability-arithmetic campaign after #768](results/apple-m4-probability-analysis-2026-09-30.md)
retains the same 43 configurations and six modes. Rank 12–16 warm-flat improves
1.22–1.27× while preserving the previous probability rounding order; rank-16
oracle and bitwise differential tests cover the new paths.

The [28-case entangled workload campaign](results/apple-m4-entangled-analysis-2026-09-30.md)
extends validation to multilayer entanglement, long-range parity and repeated
measurement/reset/feedback. Reduced independent dense-oracle coverage is explicit;
wide-workload profiles identify independent-measurement absorption/tableau work.

The [tableau row-operation campaign](results/apple-m4-row-ops-analysis-2026-10-03.md)
retains paired 43-case scale and 28-case entangled evidence after #770; its
[new entry and verifier](row_ops/README.md) explicitly bind the changed tableau source.

The [Pauli reconstruction follow-up](results/apple-m4-pauli-analysis-2026-10-03.md)
repeats both matrices after #771 and adds [14 sparse/dense full-width GHZ controls](pauli_products/README.md)
with untimed row-work counters and independent conditional Born checks.

The [current CPU baseline protocol](sota/README.md) freezes Clifft 0.11.0 and the
current SymFT source alongside seven raw-record workloads, including published
magic-state cultivation circuits. It tunes peer batches independently and checks
the exact timed calling paths. This additive comparison preserves the older
matrices and makes no counts-only or GPU claim.
The [complete post-#777 M4 baseline](results/apple-m4-current-sota-2026-10-07/analysis.md)
retains all 21 configurations and the remaining architecture gap.
