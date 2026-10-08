# Apple M4 bounded coefficient-state interning

The clean measured source is `0177cf3d4c2b988b35c692cc9a5098a3bf4f322f`,
retained by `benchmark-source/coefficient-intern-large-scout-2026-10-08`.
This schema-v5 campaign retains 658 events, 24/24 finite-valid comparisons,
480 timing processes and 3360 observations: four supported original circuits,
1/64/1024 attempted shots and separate Strict/Fused policies. Fourteen unsupported
original capability outcomes are retained. Five rotated/reversed process rounds
and seven observations of at least 50 ms bind source, binary and peer identities
before/after collection. Clifft 0.11.0 default/scheduled and SymFT 0.1.1 source
c89b985 tune their own native batches. All-zero raw detector postselection and
XOR-folded raw observable 0 are checked against independent complete records;
RNG continuation is checked outside timing.

The optional index reuses coefficient states only at the same producing node
and with identical arity and every FP64 bit. Its sparse fingerprint is only a
filter. Construction waits for 128 retained states and vectors of 32..4096
coefficients; allocation and actual capacity remain inside the original 64 MiB
cache cap. Closed admission bypasses hashing. The original arithmetic and typed
random producer are unchanged.

Closure: `2026-10-08T00:03:39.031820+00:00`. The same producer has a separate [linux-vm-x86-coefficient-intern-2026-10-08](../linux-vm-x86-coefficient-intern-2026-10-08/README.md) campaign.

## Results and limits

Fused MSC d5 fastest-peer/rstim ratios at 1/64/1024 shots are
0.891×/1.252×/1.160×. Both bulk paired ranges exceed one; the single-shot
range and all Strict d5 ranges remain below one. MSC d3 leads at 1/64 shots;
its 1024-shot paired ranges cross one. Every surface d7/d9 cell has its paired
range above one. These are same-campaign peer comparisons; use the
[Rust-only scouts](../apple-m4-coefficient-intern-rust-ablation-2026-10-08/README.md)
to estimate the source-change effect, including its cold and non-target regressions.

See [comparisons.csv](comparisons.csv), [warm.csv](warm.csv) and [analysis.md](analysis.md)
for all cells, cold phases, acceptance and whole-process RSS. Compilation and
first-call costs are excluded from warm throughput and retained separately.

The Apple M4 host is shared and unpinned. Numerical library thread counts are one; CUDA is disabled.
Paired ranges are descriptive, not confidence intervals. Native peer build flags
are not fully attested. Finite witnesses do not certify rare conditional logical-error
accuracy or circuit physics. Counts and raw-record throughput are separate
contracts. These results do not establish universal SOTA leadership.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/apple-m4-coefficient-intern-2026-10-08 --kind application_counts --check
python3 benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-coefficient-intern-2026-10-08 --git-sources
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-coefficient-intern-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-coefficient-intern-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-coefficient-intern-2026-10-08
```

Original headers, closure and losslessly compressed event bytes are retained.
Independent full review and final-head hosted CI are required before merge.
