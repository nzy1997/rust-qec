# Linux x86 VM compact zero-noise replay counts

Clean measured producer: `2a54d756b04b814fa1db129e27168615fb407b58`. Runtime source is
`d989531d837fe874a77ac05745f5d77ea5639d52`; the x86 producer contains only
subsequent benchmark/tooling amendments, with immutable byte equivalence checked.
The complete schema-v5 matrix retains 658 events, 24 finite-valid cells, 480 timing
processes, 3360 observations and all fourteen capability-failure events.
Every supported original MSC d3/d5 and surface d7/d9 circuit uses 1/64/1024
attempted shots, Strict and Fused separately, five rotated/reversed process pairs,
and seven observations of at least 50 ms per timed process.

Clifft 0.11.0 default/scheduled and SymFT 0.1.1 source
`c89b98514a919240b8afa53a271e08d926d3c987` tune native batches independently.
These were still the latest checked upstream release/source on 2026-10-08:
[Clifft release](https://github.com/unitaryfoundation/clifft/releases/tag/v0.11.0),
[SOFT source](https://github.com/haoliri0/SOFT/commit/c89b98514a919240b8afa53a271e08d926d3c987).
All-zero raw detector postselection and XOR-folded raw observable 0 are checked
against complete records and exact Rust RNG continuation. Raw reference shots,
finite witness bounds, acceptance, first-call metadata and process RSS remain
in the original events and derived reports.

The change skips prepared zero-noise spans in compact scalar counts replay when
cache admission fails. It also retains the original-order Fused highest-X/z0
imaginary AVX2 pair kernel. Strict operand/reduction order, CDF and RNG stream
remain covered by independent coefficient/CDF and mixed public-call gates.
Both x86 source-effect builds already contain packed SIMD/FMA instructions.

## Complete warm comparison

Ratios below are fastest valid peer time / rstim time: greater than one favors
rstim. Ranges are observed paired-process extrema, not confidence intervals.

| Original circuit | Policy | Shots | Fastest peer | Peer / rstim | Paired range |
| --- | --- | ---: | --- | ---: | ---: |
| MSC d3 | strict | 1 | clifft-scheduled | 3.685238× | 3.520183–4.069709 |
| MSC d3 | fused | 1 | clifft-scheduled | 3.595334× | 3.547611–3.732091 |
| MSC d3 | strict | 64 | symft | 1.315188× | 1.298764–1.334891 |
| MSC d3 | fused | 64 | symft | 1.353199× | 1.292158–1.397333 |
| MSC d3 | strict | 1024 | symft | 0.981805× | 0.953739–1.003741 |
| MSC d3 | fused | 1024 | symft | 0.980805× | 0.970569–1.016885 |
| MSC d5 | strict | 1 | clifft-scheduled | 1.135915× | 1.114514–1.170797 |
| MSC d5 | fused | 1 | clifft-scheduled | 1.157147× | 1.133616–1.166913 |
| MSC d5 | strict | 64 | clifft-scheduled | 0.756483× | 0.749687–0.780369 |
| MSC d5 | fused | 64 | clifft-scheduled | 0.743993× | 0.723371–0.796620 |
| MSC d5 | strict | 1024 | clifft-scheduled | 0.673653× | 0.656297–0.686062 |
| MSC d5 | fused | 1024 | clifft-scheduled | 0.699552× | 0.693737–0.725152 |
| surface d7 | strict | 1 | clifft | 11.708424× | 11.558661–11.874353 |
| surface d7 | fused | 1 | clifft-scheduled | 11.659254× | 11.451806–12.279279 |
| surface d7 | strict | 64 | clifft | 2.578791× | 2.541730–2.631448 |
| surface d7 | fused | 64 | clifft-scheduled | 2.549759× | 2.539682–2.591697 |
| surface d7 | strict | 1024 | clifft-scheduled | 1.204972× | 1.199350–1.216303 |
| surface d7 | fused | 1024 | clifft | 1.195752× | 1.176001–1.223997 |
| surface d9 | strict | 1 | clifft-scheduled | 5.803881× | 5.597564–6.207318 |
| surface d9 | fused | 1 | clifft | 5.875098× | 5.716100–6.026862 |
| surface d9 | strict | 64 | clifft | 2.605721× | 2.268512–2.734313 |
| surface d9 | fused | 64 | clifft | 2.594017× | 2.527015–2.635188 |
| surface d9 | strict | 1024 | symft | 1.338448× | 1.319826–1.353358 |
| surface d9 | fused | 1024 | clifft-scheduled | 1.334299× | 1.263692–1.375876 |

D5 bulk loses every measured pair: Strict/Fused ratios are 0.756483/0.743993
at 64 shots and 0.673653/0.699552 at 1024 shots. D3/1024 is near parity.
The peer VM reports AMD EPYC 9V45 with CPU affinity [0] out of four exposed CPUs;
the separate source-effect VM reports EPYC 9V74. Neither is the older 7763 host.
Cross-VM differences cannot establish source effects. Three process-spread
outliers above 10% remain in the raw data. The complete collection and actual
native correctness gates are retained in
[workflow 37747344894](https://github.com/nzy1997/rust-qec/actions/runs/37747344894).

See [analysis.md](analysis.md), [warm.csv](warm.csv) and
[comparisons.csv](comparisons.csv) for retained numerical details, and the
[same-host source effects](../linux-vm-x86-compact-zero-spans-rust-ablation-2026-10-08/README.md) for master A/B and same-binary
A/A controls. Compile/prepare/first-call metadata is retained but this peer
campaign derives no lifecycle comparison; use the separate fresh-seed cold study.

The complete matrix does not establish universal SOTA leadership. Native peer
compiler flags are not fully attested and loaded peer package bytes are identified
but not vendored. Finite witnesses do not certify rare conditional logical-error
accuracy or original-circuit physics. Process RSS is not comparative engine memory.
Full-record and counts throughput are different contracts.

## Retention and offline replay

Original headers, closures and losslessly compressed event bytes remain unchanged.
`production-binaries/receipt.json` identifies the two actual measured Rust binaries;
executables remain in original ignored evidence and workflow artifacts. Portable
CI binds receipts to headers/closures and source inventories, without claiming to
hash executables absent from this publication. M4's older binary receipt has no
header/closure digest fields; the x86 receipt does. Original binary bytes were
independently checked while available. The x86 runtime-equivalence receipt is
recomputed from immutable Git bytes; its historical review-report digest is metadata,
not a separately published review artifact or substitute for the full PR review.

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/linux-vm-x86-compact-zero-spans-2026-10-08 --kind application_counts --check
python3 benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-compact-zero-spans-2026-10-08 --git-sources
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-compact-zero-spans-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/linux-vm-x86-compact-zero-spans-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/linux-vm-x86-compact-zero-spans-2026-10-08
python3 benchmarks/near_clifford/verify_compact_zero_spans_scouts.py
```

The local `.gitattributes` preserves original native test logs with their trailing
blank line while keeping other whitespace checks active.
