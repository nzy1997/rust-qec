# Apple M4 packed counts rejection

This closed v4 campaign benchmarks original circuits with all-zero raw detector
postselection and XOR-folded raw observable 0 counts. Native Rust counts retire
packed lanes at nonzero detector readers and reduce selected observable parities
during execution. Scalar and admission-fallback rejection remain enabled. Every
typed random draw is retained, including rejected rows; accepted fallback rows
are replayed from their complete tape. The reference record/RNG check is outside
the timed native route.

## Retained evidence

- Measured clean source: `aeebd87f52728c26b2e72c31ec08ed2391b40eda`, retained by
  `benchmark-source/packed-rejection-2026-10-08`.
- Source, binary and peer-environment identities are identical before/after.
- Closure: `2026-10-07T18:49:22.777750+00:00`; 658 events, 24/24 finite-valid
  and complete comparisons, 480 independent timed processes, 3360 observations.
- Four supported original circuits, 1/64/1024 attempted shots, Strict/Fused
  separately; fourteen unsupported capability outcomes remain in the ledger.
- Five rotated/reversed process rounds, seven observations of at least 50 ms.
- Clifft 0.11.0 default and scheduled, SymFT 0.1.1 at c89b985 are actual
  distribution/import/source bound. Each peer tunes its native batch independently.

## Results

These are same-run comparisons to the fastest peer, not divisions across campaigns.
In Fused, MSCd3 leads by 1.579×/1.523× at 1/64 shots; its 1024-shot ratio is
1.011×, near parity. Surface d7/d9 lead by 1.239×/1.067× at 64 shots. MSCd5
remains 1.397×/1.096×/1.088× slower at 1/64/1024; surface d7/d9 remain
2.622×/2.445× slower at 1024. See [comparisons.csv](comparisons.csv) and
[analysis.md](analysis.md) for every Strict/Fused cell and paired process range.
These results identify further optimization directions and do not establish
universal SOTA performance.

The host is shared and unpinned. Finite accepted/error probability witnesses
do not certify rare conditional logical-error accuracy or the circuit physics.
RSS is whole-process high-water; native peer build flags are not fully attested.
Raw-record throughput and counts output remain separate contracts.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/apple-m4-packed-rejection-2026-10-08 --kind application_counts --check
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-packed-rejection-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-packed-rejection-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-packed-rejection-2026-10-08
```

Original and published source replay and derived output verification pass.
The normal and optimized corruption suites require exact producer binding,
including resealed v4-to-v3/v2 downgrade mutations with execution labels rewritten.
Old v1/v2/v3 publications and their derived bytes remain frozen.
