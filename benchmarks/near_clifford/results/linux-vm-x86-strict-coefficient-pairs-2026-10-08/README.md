# Linux VM x86 Strict coefficient pairs

Measured clean source: `b555f785af3e432cf850de67acad46d01a33e9c1`, retained by
`benchmark-source/strict-coefficient-pairs-scout-2026-10-08`.
The complete unchanged schema-v5 matrix retains 658 events, 24/24 finite-valid
comparisons, 480 timing processes, 3360 observations and 151 source inputs.
Fourteen original unsupported capability outcomes remain. Four supported original
circuits use 1/64/1024 attempted shots and separate Strict/Fused policies, five
rotated/reversed process rounds and seven observations of at least 50 ms.
Closure: `2026-10-08T02:12:32.614678+00:00`.

Strict zero-Z multi-X rotations now borrow disjoint highest-X halves and visit
adjacent coefficient pairs, reusing the existing Fused traversal. Four original
input coefficients are snapshotted before stores. A compile-time policy preserves
Strict's separately rounded own-times-c plus partner-times-factor operand order
and Fused's original mul_add; all other masks keep their previous path. No public
API, RNG producer/order, coefficient layout, cache budget or probability reduction
changes. Independent frozen/gather coefficient and cumulative probability bit
oracles cover both policies, phases, expansion, dagger/flip and signed/subnormal
values; public mixed calls check records, counts and following RNG words.

Clifft 0.11.0 default/scheduled and SymFT 0.1.1 source c89b985 tune native batches
independently. All-zero raw detector postselection and XOR-folded raw observable
0 are verified against independent complete records and exact Rust RNG continuation.

## Results

Cultivation d5 comparisons against each cell's fastest peer:

| Policy / shots | Fastest peer / rstim | Paired process range |
| --- | ---: | ---: |
| strict / 1 | 1.5216× | 1.1860–1.5761 |
| fused / 1 | 1.4667× | 1.4198–1.4909 |
| strict / 64 | 0.8531× | 0.8420–0.8593 |
| fused / 64 | 0.7999× | 0.7972–0.8089 |
| strict / 1024 | 0.7417× | 0.7358–0.7499 |
| fused / 1024 | 0.6974× | 0.6911–0.7028 |

See [comparisons.csv](comparisons.csv), [warm.csv](warm.csv) and
[analysis.md](analysis.md) for every cell, cold phases, acceptance and process RSS.
[Same-host Rust scouts](../apple-m4-strict-coefficient-pairs-rust-ablation-2026-10-08/README.md)
estimate source effects and retain all regressions. Compilation and first-call
costs are excluded from warm throughput and retained separately.

The Azure VM reports AMD EPYC 7763 64-Core Processor, logical CPU 0 of four, native CPU flags and Rust 1.93.1. The preceding zero-noise-span VM was EPYC 9V45; cross-campaign differences cannot isolate source effects. Both-policy d5 single-shot leads in every pair, but both bulk shot sizes remain below peers in every pair. The original [host receipt](host.json) and [workflow 37714974340](https://github.com/nzy1997/rust-qec/actions/runs/37714974340) are retained. Numerical library threads are one; CUDA is disabled. Paired ranges are
descriptive, not confidence intervals. Native peer build flags are not fully
attested. Finite witnesses do not certify rare conditional logical-error accuracy
or original circuit physics. Counts and full-record throughput are separate
contracts. These results do not establish universal SOTA leadership.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/linux-vm-x86-strict-coefficient-pairs-2026-10-08 --kind application_counts --check
python3 benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-strict-coefficient-pairs-2026-10-08 --git-sources
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-strict-coefficient-pairs-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/linux-vm-x86-strict-coefficient-pairs-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/linux-vm-x86-strict-coefficient-pairs-2026-10-08
```

Original headers, closures and losslessly compressed events are retained.
Independent full review and final-head CI are separate merge gates.
