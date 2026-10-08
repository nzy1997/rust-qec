# Apple M4 Strict coefficient pairs

Measured clean source: `b555f785af3e432cf850de67acad46d01a33e9c1`, retained by
`benchmark-source/strict-coefficient-pairs-scout-2026-10-08`.
The complete unchanged schema-v5 matrix retains 658 events, 24/24 finite-valid
comparisons, 480 timing processes, 3360 observations and 151 source inputs.
Fourteen original unsupported capability outcomes remain. Four supported original
circuits use 1/64/1024 attempted shots and separate Strict/Fused policies, five
rotated/reversed process rounds and seven observations of at least 50 ms.
Closure: `2026-10-08T02:11:14.377551+00:00`.

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
| strict / 1 | 1.4067× | 1.2583–1.6363 |
| fused / 1 | 1.5882× | 1.1508–1.7005 |
| strict / 64 | 1.1159× | 0.8773–1.2431 |
| fused / 64 | 1.2128× | 1.0810–1.3913 |
| strict / 1024 | 1.0424× | 0.8718–1.1793 |
| fused / 1024 | 1.0851× | 1.0117–1.2721 |

See [comparisons.csv](comparisons.csv), [warm.csv](warm.csv) and
[analysis.md](analysis.md) for every cell, cold phases, acceptance and process RSS.
[Same-host Rust scouts](../apple-m4-strict-coefficient-pairs-rust-ablation-2026-10-08/README.md)
estimate source effects and retain all regressions. Compilation and first-call
costs are excluded from warm throughput and retained separately.

The shared Apple M4 host is unpinned. Strict d5 bulk and both-policy d3 1024 paired ranges cross one; their median lead is not a stable process-pair lead. Numerical library threads are one; CUDA is disabled. Paired ranges are
descriptive, not confidence intervals. Native peer build flags are not fully
attested. Finite witnesses do not certify rare conditional logical-error accuracy
or original circuit physics. Counts and full-record throughput are separate
contracts. These results do not establish universal SOTA leadership.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/apple-m4-strict-coefficient-pairs-2026-10-08 --kind application_counts --check
python3 benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-strict-coefficient-pairs-2026-10-08 --git-sources
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-strict-coefficient-pairs-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-strict-coefficient-pairs-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-strict-coefficient-pairs-2026-10-08
```

Original headers, closures and losslessly compressed events are retained.
Independent full review and final-head CI are separate merge gates.
