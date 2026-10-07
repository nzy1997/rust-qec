# Compiled near-Clifford CPU comparison after Noise and lazy-state changes

The retained compiled implementation wins 16 of 21 warm configurations against
each configuration's fastest valid, independently tuned current peer. Its
geometric-mean speed ratio is 1.5079× across this Apple M4 collection, with
2.1973×, 1.3976× and 1.1165× at 1, 64 and 1024 shots respectively. This is a scoped
benchmark advantage; five configurations remain slower, including every d5
cultivation configuration. It does not establish universal SOTA superiority.

The measured source is `1f015586d7e6133c6f3d5067ad4b67051841a939` (S3). The
experiment uses the opt-in `CompiledNearCliffordExecutor` with the explicit
immutable **Fused FP64** rotation policy and original default 64 MiB cache budget.
**Strict remains the public default.** The subsequent row-pair prototype is not
part of this retained source or formal comparison.

## Complete warm results

Each ratio is `fastest valid peer warm time / rstim warm time`; values above 1
favor rstim. Every configuration has equal weight in the geometric mean, which
is not a total-work or elapsed-campaign speedup. The [raw campaign](results.json),
[complete timing report](report.md) and [derived values](analysis.json) retain
all backends, selected tuning choices, observations and cold metrics.

| Fixture | 1 shot | 64 shots | 1024 shots |
| --- | ---: | ---: | ---: |
| terminal | 6.2375× | 1.3134× | **0.8451×** |
| brick16 | 5.8603× | 1.4810× | 1.1530× |
| parity129 | 1.5436× | 1.3860× | 1.1946× |
| rounds129 | 1.5509× | 1.7957× | 1.5270× |
| qec32 | 2.5749× | 1.8935× | 1.4648× |
| msc3 (cultivation d3) | 1.3741× | 1.2503× | **0.9715×** |
| msc5 (cultivation d5) | **0.7986×** | **0.9089×** | **0.8549×** |
| Geometric mean | 2.1973× | 1.3976× | 1.1165× |

All 16 median wins also exceed 1 in every reported paired ratio. The five losses
are preserved below; the d3/1024 range crosses 1, whereas d5 loses in every pair
at every shot size. Ranges summarize three paired process medians, not
confidence intervals.

| Fixture | Shots | rstim warm µs | Fastest peer warm µs | Fastest peer | Paired ratio range |
| --- | ---: | ---: | ---: | --- | --- |
| terminal | 1024 | 59.03868 | 49.89465 | Clifft | 0.84023–0.84990× |
| msc3 | 1024 | 228.57096 | 222.04609 | Clifft | 0.96677–1.00110× |
| msc5 | 1 | 32.68547 | 26.10381 | scheduled Clifft | 0.79754–0.79975× |
| msc5 | 64 | 1323.49447 | 1202.86519 | scheduled Clifft | 0.90679–0.91125× |
| msc5 | 1024 | 21205.87533 | 18129.54167 | scheduled Clifft | 0.84776–0.86228× |

Cultivation's family geometric mean is 1.0059×, so this family does not show a
clear overall advantage. The stronger aggregate result does not remove its
d5 limitation.

## Cold costs remain separate

d5 first calls are also unfavorable. The comparison below uses scheduled
Clifft, the fastest warm peer for all three d5 configurations; it is not a
selection of the fastest cold peer at each shot count.

| d5 shots | rstim first call ms | Scheduled Clifft first call ms |
| --- | ---: | ---: |
| 1 | 0.069458 | 0.034750 |
| 64 | 3.399000 | 1.211000 |
| 1024 | 24.893875 | 18.152750 |

rstim's d5 compile medians are 13.15–13.27 ms, with preparation about 1.75–1.79 µs;
peer compile medians span approximately 10.44–13.61 ms. The accounted retained
cache reservation is close to 64 MiB. That ledger is not measured process RSS.
Brick16 and d3 also have substantial first-call costs despite their smaller-shot
warm wins. Compile, prepare, first-call latency and warm throughput remain
distinct metrics rather than being combined into an implied lifetime gain.

## Evidence and measurement limits

The [independent offline audit](independent-audit/REPORT.md) accepted the closed
campaign with no unresolved Critical, Important or Minor finding. It independently
recomputed all 21 warm rows, all 84 cold rows, tuning selection, finite statistical
witnesses and source/import/end closure. The source-associated workspace test
evidence ran before the source-only S3 commit; its exact-byte association is
recorded without claiming a fictitious postcommit rerun.

Every backend receives the identical native records-only circuit, including
native MPP and readout operations. Only source annotations are omitted from
sampling inputs; original detector and observable masks are checked from the
full measurement transcripts. There is no postselection, counts-only output,
spectator insertion or workload lowering. The 28 default and 84 selected
full-record witnesses each contain 8192 shots. Lossless embedded transcripts
permit replay of marginals, adjacent/block/full parity and original annotation
masks. Passing these finite distribution witnesses does not prove an arbitrary
joint distribution, and independent backend RNG streams are not asserted equal.
Implementation exact-bit and fixed-plan raw/RNG tests are separate evidence.

The [compiled protocol](../../compiled_sota/README.md) retains all seven fixtures
at all three shot counts, independently tuned peer batches, three rotated/reversed
process pairs and seven observations per process. Each warm observation
accumulates at least 50 ms of public sampling calls. Compilation, preparation,
imports, tuning/validation and caller output destruction are outside warm
windows; peer API-internal seed work remains timed. No tuning failure or
negative configuration is discarded.

Peers are Clifft 0.11.0 and SymFT 0.1.1 with its pinned clean source revision
`c89b985`, using the recorded NumPy 2.4.6 environments. Actual imported wrappers,
extensions, package/source files and probe binary are bound before/end. This
establishes identity and stability, not cryptographic extension build provenance
or peer compiler-flag attestation. Broader unsupported rotation/U3 workloads,
GPUs, x86 SIMD, arbitrary workloads and general SOTA claims are outside scope.

The [host record](host-attestation.json) describes a shared Apple M4 with ten
logical CPUs and 32 GiB RAM; each backend requests one CPU thread. Unrelated
jobs remained active. Its during-start observation was captured after the
campaign began, and its after observation follows completion. Load averages
were about 3.18/3.12/3.64 and 2.44/2.57/3.24 respectively. Recorded thermal/power
text contains no warnings; this does not establish an idle or thermally invariant
host. There is no pre-start, exclusive-host or pinned-core claim. Compiler
override variables were null in the during-start observation, not an attestation
of every peer's historical build environment. Peak RSS was not measured.

The formal campaign ran 2026-10-07 04:28:34–04:32:17 UTC. Earlier campaigns used
different host conditions and are not paired before/after causal measurements of
the Noise/lazy changes. Their absolute ratios remain historical evidence, not
an incremental speedup calculation for this implementation.

## Bounded follow-up and stopping scope

Subsequent bounded prototypes did not establish a clear measured,
contract-feasible next small optimization. The separately paired row-pair scout
compared fresh pure S3 and the candidate under original constructors: Fused,
cache0/default 64 MiB, all seven native plus 32 fixed factorial inputs, flat
shots 1/64/1024 cold and warm, and native mixed/scalar-then-batch lifetimes, in
two opposing orders. It regressed the MSC5 target in warm 64/1024 and complete
mixed/scalar-then-batch histories; the candidate was not retained. This scoped
negative result is not a statement that every prototype cell regressed, and its
timings are separate from the current-peer formal table above. The
[derived scout timings](row-pair-scout.json) retain all observations and native
lifetimes with source hashes; local capture/build guards omitted from this
derived file cannot be replayed from the publication bundle.

Other inspected directions remain speculative: duplicate group work is not an
elapsed-time saving; blanket coherent preference has strong negative controls;
early replay can change public cache reservation, graph insertion, adaptation and
first-error partial state even when it preserves the already-drawn typed tape.
Planner/sign selection still needs a defensible generic rule rather than fitted
rank/probability thresholds. Stopping this campaign reflects the absence of a
clear next measured small change in the investigated directions. It does not
claim theoretical exhaustion, achievement of an all-cell target or universal
SOTA. The three d5 cells and the two other losing configurations remain explicit
limitations.
