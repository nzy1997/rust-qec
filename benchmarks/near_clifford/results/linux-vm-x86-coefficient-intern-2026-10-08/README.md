# Linux VM x86 bounded coefficient-state interning

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

Closure: `2026-10-07T23:58:58.490225+00:00`. The same producer has a separate [apple-m4-coefficient-intern-2026-10-08](../apple-m4-coefficient-intern-2026-10-08/README.md) campaign.

## Results and limits

Fused MSC d5 fastest-peer/rstim ratios at 1/64/1024 shots are
0.739×/0.776×/0.682×. All paired ranges are below one; all Strict d5 cells
also trail. MSC d3 leads at 1/64 shots and trails at 1024 with both paired
ranges below one. Every surface d7/d9 paired range exceeds one. The single-shot
and coherent cultivation gaps remain clear optimization directions.

See [comparisons.csv](comparisons.csv), [warm.csv](warm.csv) and [analysis.md](analysis.md)
for all cells, cold phases, acceptance and whole-process RSS. Compilation and
first-call costs are excluded from warm throughput and retained separately.

The host is an Azure VM, AMD EPYC 7763, logical CPU 0 with `-C target-cpu=native`,
Rust 1.93.1, numerical library thread counts one and CUDA disabled. The original
[host receipt](host.json) is retained. Workflow [37703342459](https://github.com/nzy1997/rust-qec/actions/runs/37703342459)
completed successfully at the exact measured source. CPU model agreement with
an earlier campaign does not establish controlled host identity; unrelated
campaign timing is not a code-change ablation.
Paired ranges are descriptive, not confidence intervals. Native peer build flags
are not fully attested. Finite witnesses do not certify rare conditional logical-error
accuracy or circuit physics. Counts and raw-record throughput are separate
contracts. These results do not establish universal SOTA leadership.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/linux-vm-x86-coefficient-intern-2026-10-08 --kind application_counts --check
python3 benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-coefficient-intern-2026-10-08 --git-sources
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-coefficient-intern-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/linux-vm-x86-coefficient-intern-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/linux-vm-x86-coefficient-intern-2026-10-08
```

Original headers, closure and losslessly compressed event bytes are retained.
Independent full review and final-head hosted CI are required before merge.
