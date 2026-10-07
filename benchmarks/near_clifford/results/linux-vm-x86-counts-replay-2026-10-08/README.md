# Linux VM x86 bounded counts replay

This closed schema-v5 campaign measures the same counts replay source as the
[Apple M4 campaign](../apple-m4-counts-replay-2026-10-08/README.md):
`53cfe45e7375292a76dcdc0db2c4e3e455939154`, retained by
`benchmark-source/compact-counts-replay-2026-10-08`. Original gates, all-zero raw
detector postselection and XOR-folded raw observable 0 are preserved. Complete
structured records and subsequent RNG continuation are independently checked
outside timing.

## Retained evidence

- [Hosted run](https://github.com/nzy1997/rust-qec/actions/runs/37691764047)
  completed successfully: four original fixtures × 1/64/1024 attempted shots ×
  separate Strict/Fused = 24/24 finite-valid and complete comparisons. There are
  658 events, 480 timing processes, 3360 observations and fourteen unsupported
  original-gate capability outcomes.
- Five rotated/reversed process rounds; seven observations of at least 50 ms.
  Source, binary and peer-environment identities match before/after collection.
- Azure Linux x86 VM with AMD EPYC 7763; collector and children
  restricted to logical CPU 0. [host.json](host.json) retains topology,
  virtualization and affinity. Rust 1.93.1, `RUSTFLAGS=-C target-cpu=native`;
  numerical-library thread counts one, CUDA disabled.
- Clifft 0.11.0 default/scheduled and SymFT 0.1.1 source c89b985 are pinned by
  package, loaded-file and source digests; native peer build flags are not fully
  attested.

## Results and remaining directions

Within this campaign, Fused fastest-peer/rstim ratios at 1/64/1024 shots are
2.218×/1.337×/0.963× for MSC d3 and 0.744×/0.767×/0.715× for MSC d5. All MSC d5
paired ranges and the MSC d3 1024 range remain below one. Fused surface d7
ratios are 10.720×/3.014×/1.222× and d9 ratios are 6.045×/2.783×/1.297×, with
all surface paired ranges above one. See [comparisons.csv](comparisons.csv) for
every Strict/Fused point.

The x86 coherent-workload gap remains a concrete direction. Surface warm counts
leads reproduce on two measured hosts; this does not establish universal SOTA
or a cold single-request advantage. Compilation and first-call phases are
separate in [warm.csv](warm.csv). Do not compare unrelated campaigns to quantify
a code change; the [paired Rust ablations](../apple-m4-counts-replay-rust-ablation-2026-10-08/README.md)
isolate its effects on M4 and retain small regressions.

This VM CPU differs from the earlier affine campaign’s Xeon CPU, so their
timings must not be used as a hardware-controlled code comparison.
This is a shared VM; fixed affinity is not exclusive-core isolation. Paired
ranges are descriptive, not confidence intervals. Linux `ru_maxrss` is a
whole-process high-water mark and may inherit launcher memory across exec;
these receipts cannot establish simulator memory usage or cross-backend memory
differences. Finite witnesses do not certify rare conditional logical-error
accuracy or circuit physics. Counts and raw-record throughput are separate contracts.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/linux-vm-x86-counts-replay-2026-10-08 --kind application_counts --check
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/linux-vm-x86-counts-replay-2026-10-08 --git-sources
```

Independent full review and final-head hosted CI are required before merge.
