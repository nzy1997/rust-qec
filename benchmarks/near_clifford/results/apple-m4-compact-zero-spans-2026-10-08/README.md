# Apple M4 compact zero-noise replay counts

Clean measured producer: `d989531d837fe874a77ac05745f5d77ea5639d52`. Runtime source is
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
| MSC d3 | strict | 1 | clifft-scheduled | 3.258254× | 3.210164–3.290431 |
| MSC d3 | fused | 1 | clifft-scheduled | 3.292686× | 3.255678–3.347016 |
| MSC d3 | strict | 64 | clifft | 1.514068× | 1.503876–1.527801 |
| MSC d3 | fused | 64 | clifft | 1.513304× | 1.499558–1.564458 |
| MSC d3 | strict | 1024 | clifft | 1.004702× | 1.000792–1.031956 |
| MSC d3 | fused | 1024 | clifft-scheduled | 0.995859× | 0.990624–1.033043 |
| MSC d5 | strict | 1 | clifft-scheduled | 1.493886× | 1.488850–1.517636 |
| MSC d5 | fused | 1 | clifft-scheduled | 1.647814× | 1.601917–1.666219 |
| MSC d5 | strict | 64 | clifft-scheduled | 1.385430× | 1.350502–1.396803 |
| MSC d5 | fused | 64 | clifft-scheduled | 1.551454× | 1.483906–1.591379 |
| MSC d5 | strict | 1024 | clifft-scheduled | 1.284241× | 1.278950–1.285173 |
| MSC d5 | fused | 1024 | clifft-scheduled | 1.448969× | 1.443970–1.454157 |
| surface d7 | strict | 1 | clifft | 9.746664× | 9.004467–10.046150 |
| surface d7 | fused | 1 | clifft | 9.642383× | 9.454543–9.686364 |
| surface d7 | strict | 64 | symft | 3.569115× | 3.324497–3.948963 |
| surface d7 | fused | 64 | symft | 3.318386× | 3.302747–3.337520 |
| surface d7 | strict | 1024 | symft | 1.194567× | 1.187976–1.202693 |
| surface d7 | fused | 1024 | symft | 1.178673× | 1.176534–1.187492 |
| surface d9 | strict | 1 | clifft-scheduled | 5.056398× | 4.983855–5.227317 |
| surface d9 | fused | 1 | clifft | 4.979108× | 4.868843–5.111814 |
| surface d9 | strict | 64 | symft | 3.454866× | 3.373522–3.789006 |
| surface d9 | fused | 64 | symft | 3.404072× | 3.360416–3.446265 |
| surface d9 | strict | 1024 | symft | 1.244884× | 1.223724–1.256192 |
| surface d9 | fused | 1024 | symft | 1.237628× | 1.230932–1.242084 |

D5 bulk leads each measured process pair here; d3/1024 remains near parity
(Strict 1.004702×, Fused 0.995859×). The shared M4 host is unpinned; transient
background Python activity was observed during collection, with ownership
unknown. No quiet-machine, thermal or noise-floor claim follows.

See [analysis.md](analysis.md), [warm.csv](warm.csv) and
[comparisons.csv](comparisons.csv) for retained numerical details, and the
[same-host source effects](../apple-m4-compact-zero-spans-rust-ablation-2026-10-08/README.md) for master A/B and same-binary
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
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/apple-m4-compact-zero-spans-2026-10-08 --kind application_counts --check
python3 benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-compact-zero-spans-2026-10-08 --git-sources
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-compact-zero-spans-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-compact-zero-spans-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-compact-zero-spans-2026-10-08
python3 benchmarks/near_clifford/verify_compact_zero_spans_scouts.py
```
