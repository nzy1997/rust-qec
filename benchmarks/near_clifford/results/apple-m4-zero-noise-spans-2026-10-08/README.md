# Apple M4 prepared zero-noise spans

Measured clean source: `d0e053c32155811327da2b1a0505ae2aaac0560a`, retained by
`benchmark-source/zero-noise-spans-scout-2026-10-08`.
The complete unchanged schema-v5 matrix has 658 events, 24/24 finite-valid
comparisons, 480 timing processes, 3360 observations and 151 source inputs.
Fourteen unsupported original capability outcomes remain. Four supported original
circuits use 1/64/1024 attempted shots and separate Strict/Fused policies, five
rotated/reversed process rounds and seven observations of at least 50 ms.
Closure: `2026-10-08T01:20:45.557232+00:00`.

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
| strict / 1 | 1.2088× | 1.1878–1.2409 |
| fused / 1 | 1.6359× | 1.5611–1.6768 |
| strict / 64 | 0.8833× | 0.8809–0.8866 |
| fused / 64 | 1.2676× | 1.2572–1.2929 |
| strict / 1024 | 0.8403× | 0.8122–0.8406 |
| fused / 1024 | 1.1614× | 1.1437–1.1833 |

See [comparisons.csv](comparisons.csv), [warm.csv](warm.csv) and
[analysis.md](analysis.md) for every cell, cold phases, acceptance and process RSS.
[Same-host Rust scouts](../apple-m4-zero-noise-spans-rust-ablation-2026-10-08/README.md)
estimate the source-change effect, including cold/bulk regressions. Compilation
and first-call costs are excluded from warm throughput and retained separately.

The shared Apple M4 host is unpinned. Numerical library thread counts are one; CUDA is disabled.
Paired ranges are descriptive, not confidence intervals. Native peer build flags
are not fully attested. Finite witnesses do not certify rare conditional logical
error accuracy or original circuit physics. Counts and full-record throughput are
separate contracts. These results do not establish universal SOTA leadership.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/apple-m4-zero-noise-spans-2026-10-08 --kind application_counts --check
python3 benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-zero-noise-spans-2026-10-08 --git-sources
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-zero-noise-spans-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-zero-noise-spans-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-zero-noise-spans-2026-10-08
```

Original headers, closure and losslessly compressed events are retained.
Independent full review and final-head CI are separate merge gates.
