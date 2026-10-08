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

For bottleneck diagnosis, the existing manual x86 application-counts workflow
also accepts `profile_only=true` with an exact `baseline_ref` commit. This runs
native coefficient/CDF/RNG gates, builds both unchanged production probes, then
records four whole-process `cpu-clock:u` profiles for d5 Strict/Fused 1024-shot
counts. Raw samples, stack exports, ELF binaries/build IDs, probe inputs, command
outputs and source closures remain in the workflow artifact. Ordinary peer and
paired timing modes retain their defaults.

The native correctness preflight also records a separate four-amplitude AVX-512
rotation gate in `x86-scout-avx512-direct-bits.log`. Eligible Strict imaginary
rotations use four-amplitude groups only for vectors of at least 16 amplitudes,
with AVX512F/FMA and the verified private field layout. Each product is separately
rounded before the original-order addition; this kernel contains no FMA. Smaller
vectors and other CPUs/layouts retain the existing paths; Fused keeps its existing
AVX2 dispatch. A skipped gate is explicit
in its log and is not evidence that the wider kernel ran. A source-effect claim
for this kernel requires the admitted native gate and the complete paired A/B,
confirmation and identical-binary A/A campaigns on the recorded host.
Dispatch paired experiments, current-peer runs, and profiles for this kernel with
`require_avx512=true`. The preflight rejects missing AVX512F before installing
the Rust toolchain or building probes and retains the hardware rejection in
`x86-scout-avx512-hardware-admission.log`. Hardware presence still requires the
actual admitted numerical-gate marker before any timing or profile. The default
is false for experiments targeting existing fallback CPUs; a skipped native gate
remains fallback evidence only.

These profiles include startup, warmup and teardown. Every instrumented result
is marked `performance_valid=false`; use profiles to locate work, never to claim
speedups or SOTA. `native_cpu_profile.py --verify DIR` requires the original
Linux tool/environment, checks Git input bytes, and replays raw samples with the
recorded trusted `perf` executable. Checksum-only fixture tests explicitly skip
Git/tool execution and do not certify actual profiles. The sampling options are
documented in the [Linux perf source](https://github.com/torvalds/linux/blob/master/tools/perf/Documentation/perf-record.txt).

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

The opt-in `CompiledNearCliffordExecutor` evaluates the physical Clifford frame
at compile time and samples raw records with a virtual Pauli frame and compact
amplitudes. The [experimental compiled CPU protocol](compiled_sota/README.md)
uses identical native MPP/readout inputs for every backend, full observable
parities, and independently bound consumed-input hashes. Its RNG policy differs
from the legacy executor; successful scalar, flat, cached and split calls using
the same fixed compiled plan and arithmetic policy preserve its own stream.
Changing the compiler revision or measurement schedule can change seeded results.
The earlier matrices and baseline remain separate.

The [complete compiled M4 comparison](results/apple-m4-compiled-sota-2026-10-07/analysis.md)
retains all 21 configurations with the explicit Fused FP64 policy. Its warm
geometric mean advantage is 1.4011× against each configuration's fastest valid
peer, while cultivation and cold-call costs remain optimization targets.


The [compiled Noise/lazy M4 comparison](results/apple-m4-compiled-noise-lazy-2026-10-07/analysis.md)
retains the complete S3 source-bound 21-configuration result: 16 warm wins and a
1.5079× geometric-mean ratio against each configuration's fastest independently
tuned current peer. Shot-count geomeans are 2.1973×/1.3976×/1.1165× at 1/64/1024
shots. All five losses remain, including every cultivation d5 configuration and
its unfavorable cold costs. The measured compiled policy is explicitly Fused
FP64; Strict remains the public default. This shared-host Apple M4, full-record,
non-postselected comparison has independently audited raw/source/import closure,
without peer compiler-flag attestation or a universal SOTA claim. Subsequent
bounded prototypes left no clear measured, contract-feasible next small change;
the retained source and known limitations are published without claiming the
optimization space is exhausted.

The [bounded counts replay campaign](results/apple-m4-counts-replay-2026-10-08/README.md)
retains 24 original postselection cells and [three paired Rust ablations](results/apple-m4-counts-replay-rust-ablation-2026-10-08/README.md).
Fused cultivation d5 bulk counts lead on the measured M4 host, while Strict,
one-shot and cold costs remain targets. Small ablation regressions are retained.

The [prepared zero-noise-span campaign](results/apple-m4-zero-noise-spans-2026-10-08/README.md)
retains complete M4/x86 peer comparisons and [paired warm/cold Rust scouts](results/apple-m4-zero-noise-spans-rust-ablation-2026-10-08/README.md).
Single-shot MSC improvements reproduce; bulk and cold compilation remain targets,
with all regressions retained. The new x86 host reports EPYC 9V45, distinct from
the earlier 7763 campaigns, so cross-campaign differences are not source effects.

The CPU profile collector retains the native exec PID, executable digest and CPU
affinity and checks sample process/thread/CPU plus native executable stack frames.
The privileged recording has a shorter process-group watchdog; command timeouts
retain failed receipts and partial logs. After recording closes, only that raw
file is chowned to the runner so unprivileged exports and artifact retention work.


The [compact scalar zero-noise replay campaign](results/apple-m4-compact-zero-spans-2026-10-08/README.md)
retains complete M4 and [x86 peer matrices](results/linux-vm-x86-compact-zero-spans-2026-10-08/README.md),
master A/B experiments, same-binary A/A controls and fresh-seed cold phases.
D5 warm bulk source effects reproduce on both architectures; the M4 peer lead
is not reproduced on x86, where d5 bulk remains 0.67–0.76× the fastest Clifft
path. D3 bulk is near parity and cold phase-sum benefits are small.
Every regression and outlier is retained, with source-bound offline replay and
actual native coefficient/CDF/counts/RNG gate receipts. Current diagnostic profiles
point to rotation/probability/projection kernels, with no elapsed-fraction claim.
These results leave concrete optimization directions; the campaign is continuing.
