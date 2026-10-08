# Linux VM x86 prepared zero-noise spans

Measured clean source: `d0e053c32155811327da2b1a0505ae2aaac0560a`, retained by
`benchmark-source/zero-noise-spans-scout-2026-10-08`.
The complete unchanged schema-v5 matrix has 658 events, 24/24 finite-valid
comparisons, 480 timing processes, 3360 observations and 151 source inputs.
Fourteen unsupported original capability outcomes remain. Four supported original
circuits use 1/64/1024 attempted shots and separate Strict/Fused policies, five
rotated/reversed process rounds and seven observations of at least 50 ms.
Closure: `2026-10-08T01:03:13.362766+00:00`.

The prepared scalar counts interpreter may scan and skip literal zero choices
within precompiled Noise-only operation spans. Every other operation separates
spans; all nonzero choices execute normally. The full original typed random tape
is produced before physics. RNG draws, arithmetic, records, raw/compact/live
paths, preflight and error footprints are unchanged. Optional actual-capacity
metadata is bounded by 1 MiB and charged inside the remaining 64 MiB plan budget;
failed admission/allocation retains the original interpreter.

Clifft 0.11.0 default/scheduled and SymFT 0.1.1 source c89b985 tune native batches
independently. All-zero raw detector postselection and XOR-folded raw observable
0 are verified against independent complete records and exact Rust RNG continuation.

## Results

Cultivation d5 comparisons against each cell's fastest peer:

| Policy / shots | Fastest peer / rstim | Paired process range |
| --- | ---: | ---: |
| strict / 1 | 0.7241× | 0.7093–0.7757 |
| fused / 1 | 1.1302× | 1.1071–1.1500 |
| strict / 64 | 0.3452× | 0.3287–0.3579 |
| fused / 64 | 0.5527× | 0.5455–0.5748 |
| strict / 1024 | 0.3114× | 0.3083–0.3149 |
| fused / 1024 | 0.5082× | 0.5053–0.5330 |

See [comparisons.csv](comparisons.csv), [warm.csv](warm.csv) and
[analysis.md](analysis.md) for every cell, cold phases, acceptance and process RSS.
[Same-host Rust scouts](../apple-m4-zero-noise-spans-rust-ablation-2026-10-08/README.md)
estimate the source-change effect, including cold/bulk regressions. Compilation
and first-call costs are excluded from warm throughput and retained separately.

The Azure VM reports AMD EPYC 9V45 96-Core Processor, logical CPU 0, native CPU flags and Rust 1.93.1. This differs from the earlier EPYC 7763 host; cross-campaign differences do not estimate source effects. The original [host receipt](host.json) and [workflow 37709658873](https://github.com/nzy1997/rust-qec/actions/runs/37709658873) are retained. Numerical library thread counts are one; CUDA is disabled.
Paired ranges are descriptive, not confidence intervals. Native peer build flags
are not fully attested. Finite witnesses do not certify rare conditional logical
error accuracy or original circuit physics. Counts and full-record throughput are
separate contracts. These results do not establish universal SOTA leadership.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/linux-vm-x86-zero-noise-spans-2026-10-08 --kind application_counts --check
python3 benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-zero-noise-spans-2026-10-08 --git-sources
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-zero-noise-spans-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/linux-vm-x86-zero-noise-spans-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/linux-vm-x86-zero-noise-spans-2026-10-08
```

Original headers, closure and losslessly compressed events are retained.
Independent full review and final-head CI are separate merge gates.
