# Apple M4 bounded counts replay

This closed schema-v5 campaign preserves the original circuits, all-zero raw
detector postselection and XOR-folded raw observable 0. Recorded rejected rows
retire their already-consumed random tape in constant time. Scalar counts
fallback reads compact Noise/Independent sidecars only for visited events;
active events retain the original typed tape. Complete structured records and
subsequent RNG continuation are checked independently outside timing.

## Retained evidence

- Clean measured source: `53cfe45e7375292a76dcdc0db2c4e3e455939154`, retained by
  `benchmark-source/compact-counts-replay-2026-10-08`.
- Closure: `2026-10-07T21:59:10.319609+00:00`; 658 events, 24/24 finite-valid
  and complete comparisons, 480 timing processes and 3360 observations.
- Four supported originals × 1/64/1024 attempted shots × separate Strict/Fused;
  fourteen unsupported original capability outcomes remain recorded.
- Five rotated/reversed process rounds; seven observations of at least 50 ms.
  Source, binary and peer-environment identities match before/after collection.
- Clifft 0.11.0 default/scheduled and SymFT 0.1.1 source c89b985 are bound by
  package, loaded-file and source digests; each tunes its native batch independently.

The same source has a closed [Linux VM x86 campaign](../linux-vm-x86-counts-replay-2026-10-08/README.md)
with separate host-specific comparisons.

## Results and remaining directions

Within this campaign, Fused MSC d5 fastest-peer/rstim ratios at 1/64/1024 shots
are 0.900×/1.200×/1.152×. Both bulk paired ranges exceed one; the one-shot range
and all Strict d5 ranges remain below one. Fused MSC d3 leads by 1.640×/1.499×
at 1/64 shots and is near parity at 1024 (0.994×, paired range crosses one).
Fused surface d7 ratios are 9.540×/3.308×/1.203×; d9 ratios are
5.117×/3.150×/1.249×. All surface paired ranges exceed one. See
[comparisons.csv](comparisons.csv) and [analysis.md](analysis.md) for every cell.

The [paired Rust-only ablations](../apple-m4-counts-replay-rust-ablation-2026-10-08/README.md)
isolate the two changes and retain their confirmation experiment. Recorded-tail
retirement increases MSC d5 one-shot throughput by 22–25%; compact replay improves its bulk
throughput by 16–27% against recorded-tail retirement. Small other-cell
regressions remain, including about 1.8% lower MSC d3 Fused 1024 throughput and
about 1% lower surface d7 one-shot throughput in the confirmation. Unrelated
campaign peer timing is not used to estimate a code-change effect.

Surface affine preparation remains in `first_ns`: Fused cell-median first-call
costs range from 1.520 to 1.667 ms for d7 and 3.400 to 3.709 ms for d9, in
addition to compilation. These are ranges across shot-cell process medians.
Warm leads do not establish a cold single-request advantage. Cold phases,
attempted and accepted throughput, acceptance rates and whole-process RSS are
retained in [warm.csv](warm.csv).

The host is shared and unpinned; numerical library thread counts are one and
CUDA is disabled. Paired ranges are descriptive, not confidence intervals.
Native peer build flags are not fully attested. Finite witnesses do not certify
rare conditional logical-error accuracy or circuit physics. Raw-record throughput
and counts are separate contracts. These are measured workload leads, not a
universal SOTA result. Strict/coherent and cold-compilation gaps remain directions.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/apple-m4-counts-replay-2026-10-08 --kind application_counts --check
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-counts-replay-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-counts-replay-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-counts-replay-2026-10-08
```

Both modes reject all thirty corruption controls, including resealed producer
schema downgrades. Earlier publications remain unchanged. Independent full review
and final-head hosted CI are required before merge.
