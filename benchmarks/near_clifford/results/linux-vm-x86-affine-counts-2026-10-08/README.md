# Linux VM x86 affine native counts

This closed schema-v5 campaign measures the same bounded affine counts producer as the Apple M4 campaign: `469fb50c156d91d89e63118c749927e99a8e382e`, retained by `benchmark-source/affine-counts-bounded-2026-10-08`. Original gates, all-zero raw detector postselection, and XOR-folded raw observable 0 are preserved. Complete structured records and subsequent RNG continuation are independently checked outside timing.

## Retained evidence

- [Hosted run](https://github.com/nzy1997/rust-qec/actions/runs/37686190596) completed successfully. Four original fixtures × 1/64/1024 attempted shots × separate Strict/Fused = 24/24 finite-valid and complete comparisons; 658 events, 480 timing processes, 3360 observations, 14 unsupported original-gate capability outcomes.
- Five rotated/reversed process rounds, seven observations of at least 50 ms. Source, binary and peer-environment identities match before and after collection.
- Azure Linux x86 virtual machine reports Intel Xeon Platinum 8370C; collector and child processes restricted to logical CPU 0. [host.json](host.json) records topology, virtualization and affinity. Rust 1.93.1, `RUSTFLAGS=-C target-cpu=native`; numerical-library thread counts set to one, CUDA disabled.
- Clifft 0.11.0 default/scheduled and SymFT 0.1.1 source c89b985 are pinned by package, loaded-file and source digests; native peer build flags are not fully attested.

## Results and remaining directions

Within this campaign, Fused surface d7 fastest-peer/rstim ratios at 1/64/1024 shots are 11.946×/2.977×/1.323×; d9 ratios are 5.936×/3.221×/1.675×. All their paired process-median ranges exceed one. MSC d3 ratios are 2.117×/1.381×/0.990×; its 1024-shot paired range is entirely below one. MSC d5 remains behind at 0.584×/0.420×/0.413×. See [comparisons.csv](comparisons.csv) and [analysis.md](analysis.md) for every point.

The x86 MSC d5 gap identifies further coherent-simulation work. The warm surface leads reproduce across two measured hosts; they do not establish universal SOTA or a cold single-request advantage. Compilation and first-call preparation are separate in [warm.csv](warm.csv). Do not compare unrelated campaigns to quantify a code change.

This is a shared virtual machine; fixed affinity is not exclusive-core isolation. Paired ranges are descriptive, not confidence intervals. Linux `ru_maxrss` is a whole-process high-water mark and may inherit launcher memory across exec, so these receipts cannot establish simulator memory usage or cross-backend memory differences. Finite witnesses do not certify rare conditional logical-error accuracy or circuit physics.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/linux-vm-x86-affine-counts-2026-10-08 --kind application_counts --check
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-affine-counts-2026-10-08 --git-sources
```

Independent full review and final-head hosted CI are required before merge.
