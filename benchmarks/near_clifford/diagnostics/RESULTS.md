# Near-Clifford diagnostic findings, 2026-10-07

Production source is unchanged from PR780, `3ef5030db205b3e9b2126e31b2602f760d4665cc`. Strict FP64 remains the default; Fused is explicit. These results identify further optimization directions; they do not establish universal SOTA performance.

Formal measurements use five independent process rounds, seven warm observations per process and independently replayed finite correctness witnesses. The reported latency is the median of process medians. Paired ranges are not confidence intervals. Compilation, preparation and first calls remain separate.

## Completed evidence

| Campaign | Finite-valid cells | Complete four-backend timing comparisons | Other coverage |
| --- | ---: | ---: | --- |
| [M4 cache/lifecycle](../results/apple-m4-diagnostics-cache-lifetime-2026-10-07/analysis.md) | 24/24 | 24/24 | 560 lifecycle comparison keys |
| [M4 boundaries](../results/apple-m4-diagnostics-boundary-2026-10-07/analysis.md) | 72/72 | 72/72 | 0 retained failures |
| [M4 structure/incumbents](../results/apple-m4-diagnostics-structure-2026-10-07/analysis.md) | 126/126 | 122/126 | 25 retained timing process failures |
| [M4 original application counts](../results/apple-m4-application-counts-2026-10-07/analysis.md) | 24/24 | 24/24 | 7 unsupported originals × 2 arithmetic policies |
| [x86 VM original application counts](../results/linux-vm-x86-application-counts-2026-10-07/analysis.md) | 24/24 | 24/24 | 7 unsupported originals × 2 arithmetic policies |
| [x86 VM cache/lifecycle](../results/linux-vm-x86-diagnostics-cache-lifetime-2026-10-07/analysis.md) | 24/24 | 24/24 | 560 lifecycle comparison keys |
| [x86 VM boundaries](../results/linux-vm-x86-diagnostics-boundary-2026-10-07/analysis.md) | 72/72 | 72/72 | 0 retained failures |
| [x86 VM structure/incumbents](../results/linux-vm-x86-diagnostics-structure-incumbent-2026-10-07/analysis.md) | 102/102 | 102/102 | 0 retained timing process failures |
| [x86 VM rank](../results/linux-vm-x86-diagnostics-rank-2026-10-07/analysis.md) | 24/24 | 22/24 | 10 retained timing process failures |

The M4 is a shared, unpinned Apple M4 with 32GiB RAM and 10 logical CPUs. The completed x86 counts run used one pinned logical CPU on an AMD EPYC7763 Microsoft VM with four visible vCPUs and about15.6GiB RAM; Rust1.93.1 used `-C target-cpu=native`. Compare each backend with its peers on the same host. Absolute M4/x86 times do not isolate an architecture effect. The separate x86 structure/incumbent VM presents an AMD EPYC9V45 96-Core Processor; it also uses its own pinned logical CPU and hardware receipt. The separate x86 rank VM presents an AMD EPYC9V74 80-Core Processor. The cache/lifecycle and boundary VMs present EPYC7763 models but retain separate receipts. Native peer build flags are not fully attested.

## Largest directions

### Native counts and early rejection

The counts output contract is all-zero raw detector postselection and XOR-folded raw observable0, with reference normalization disabled. Rust currently creates full structured records and then filters/counts. Clifft and SymFT use their native counts paths. The comparison therefore includes avoided simulation/output work and early rejection. It is distinct from full raw-record throughput.

At1024 attempted shots per call, slowdown relative to the fastest peer is:

| Original application | M4 Strict | M4 Fused | x86 VM Strict | x86 VM Fused |
| --- | ---: | ---: | ---: | ---: |
| MSCd3 | 9.44× | 9.73× | 8.34× | 8.32× |
| MSCd5 | 8.37× | 6.23× | 15.84× | 8.81× |
| Surface d7 | 61.97× | 60.54× | 66.89× | 67.26× |
| Surface d9 | 91.73× | 58.40× | 60.31× | 60.31× |

The pure Clifford inputs also show a large gap, so a coherent rotation kernel alone cannot resolve this application bottleneck. First implement a public native counts contract with streaming annotation reduction, then test early rejection while preserving the requested raw detector/observable semantics. Counts correctness must be checked separately from raw-record correctness. Accepted-throughput figures and acceptance rates are retained in each `warm.csv`; surface-d9 acceptance is roughly0.5%, and these finite witnesses do not certify rare conditional logical-error accuracy. All current native SymFT counts receipts report `active_components=False`.

### Compiler scheduling and state elimination

The rank generator varies rotation-target count, which is not necessarily the compiled active rank. On M4 at1024 raw-record shots, Strict Rust is about4.65× slower at target8 and64.59× slower at target12 than scheduled Clifft. The corresponding Rust active ranks are7 and11. At target16 and64 shots, Rust is about8643× slower than the fastest peer; the Rust active rank is16, scheduled Clifft peak active width is5, and SymFT reports `max_active_qubits=24`. These API metrics are not equivalent dense-state dimensions.

This makes legal operation scheduling, measurement/state elimination and component factorization stronger candidates than small arithmetic-loop changes. The scan is a workload profile, not causal proof of one operation. Add an ablation on the failing rank profiles and inspect the compiled schedule before selecting an implementation. Do not assume SymFT allocates a single `2^24` array from its public metric.

Four M4 cells lack a complete timing comparison: rank16/1024 and depth32/1024 under both policies. All126 selected structural cells passed their finite correctness witnesses. The25 retained timeout events are180second **whole-process** bounds covering mandatory cold/warmup/warm calls, not180second per-shot latency estimates. No throughput is invented for these failures.

The separate x86 rank VM confirms the scaling gap: at1024 shots Strict is about4.06× slower at target8 and51.39× slower at target12 than scheduled Clifft. At target16/64 it is about5122× slower than SymFT. All24 finite witnesses passed; rank16/1024 under both policies lacks complete timing because of10 retained180second whole-process failures. These are separate from the M4 timeouts, and no per-call estimate is inferred.

On the separate x86 structure/incumbent VM, all102 selected cells have complete timing comparisons and no retained failures. At depth32/1024, Strict Rust takes808986µs versus4184µs for SymFT (about193× slower); Fused is about184× slower. Small incumbent calls can win, while1024-shot brick16, parity129 and rounds129 calls remain slower than the fastest peer. This reinforces workload-specific schedule and batch-output optimization; it does not establish uniform leadership.

### Cache admission and lifetime

MSCd5 cache64MiB improves some one-shot warm calls but raises first-call costs. For Strict one-shot calls, warm latency is61.83→53.22µs at cache0→64MiB, while first latency is68.29→105.67µs. Fused is39.70→34.07µs warm and47.29→81.62µs first. Batch-once lifecycle phase sums are15.13% worse for Strict and11.27% worse for Fused with64MiB. Cache benefits depend on reuse and shot count. Test reuse-aware admission and reduced preparation, rather than changing the default from one warm scalar result.

The separate x86 cache/lifecycle VM shows the same tradeoff: cache64MiB improves scalar warm latency15.64% Strict /13.37% Fused while first-call cost rises81.64%/117.77%; batch-once phase sums worsen66.82%/70.09%. Cache1MiB improves1024-shot warm latency8.42%/13.73%. The experiments support reuse/budget ablations, not blanket disabling or maximizing the cache.

Cache budget is a ceiling, reservation is a simulator ledger, and process RSS includes other allocations. M4 whole-probe high-water marks include all measured cold/warm work, not one first-call allocation. Linux `ru_maxrss` can retain launcher memory across exec; the current Linux receipts cannot establish simulator-only memory costs or relative backend memory. Future Linux memory comparisons need separately attributed measurements.

### Batch dispatch and tails

M4 Fused MSCd3 totals at63/64/65 shots are17.297/14.408/16.115µs; QEC32 is7.108/5.713/6.746µs. These support investigating small-batch dispatch and partial-lane cost. Terminal totals are3.887/3.975/4.447µs, so there is no claim that terminal63 is slower than64. A32-shot packed dispatcher,64-shot noise packets and full-lane compact consumers are source hypotheses, not proven causes. Target follow-up crossover/tail ablations rather than expanding every Cartesian benchmark axis.

## Coverage limits and continuation

Eleven original upstream application circuits retain their exact text, annotations, REPEAT blocks, licenses and source hashes. Seven contain unsupported original gates such as arbitrary R_X/R_Z or U3. They remain capability failures; no gate rewriting is used. Four supported circuits have complete native-count comparisons on both hosts. Finite probability/parity checks are not proof of arbitrary joint distributions or physical circuit certification.

Finite union-bound budgets apply per runner invocation. Separate hosts and shards do not inherit one shared0.001 whole-campaign guarantee.

The initial monolithic x86 raw campaign was canceled near its180minute CI cap. Its retained artifact has4666 events but no completion closure, so it is not certified evidence and contributes no published comparison. Four independently bounded x86 jobs completed cache/lifecycle, boundary, rank and remaining structure/incumbent profiles (run37621320692); each VM has its own hardware receipt. All four shards have completion closures and passed strengthened normal/optimized replay. Their222 selected flat cells cover216 unique profiles because6 MSCd5 profiles overlap; lifecycle histories add560 comparison keys. These shards must not be pooled as one hardware measurement, even when the visible CPU model matches.

The next implementation priorities are native counts/early rejection and compiled schedule/state reduction, followed by cache admission and dispatch/tail ablations. Further broad benchmark expansion is lower priority until these measured gaps are addressed. Each optimization should be compared on the relevant frozen profiles, then on the incumbent raw and original counts suites, with independent review and final-head CI before squash merge.
