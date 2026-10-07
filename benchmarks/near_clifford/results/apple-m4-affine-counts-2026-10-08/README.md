# Apple M4 affine native counts

This closed v5 campaign retains original circuits, all-zero raw detector
postselection and XOR-folded raw observable 0. Rotation-free plans may lazily
build a bounded affine detector/observable model; other plans retain scalar and
packed early rejection. Every typed RNG event is consumed in original order,
including inputs with no influence on the selected outputs. Complete structured
records and subsequent RNG continuation are independently checked outside timing.

## Retained evidence

- Clean measured source: `3ef49cf9c3f156a492682d24e392915510f2a3a8`, retained by
  `benchmark-source/affine-counts-2026-10-08`.
- Closure: `2026-10-07T20:39:27.651731+00:00`; 658 events, 24/24 finite-valid
  and complete comparisons, 480 timing processes and 3360 observations.
- Four supported originals, 1/64/1024 attempted shots, separate Strict/Fused.
  Fourteen unsupported original capability outcomes remain recorded.
- Five rotated/reversed process rounds, seven observations of at least 50 ms.
- Source, binary and peer-environment identities match before/after collection.
- Clifft 0.11.0 default/scheduled and SymFT 0.1.1 source c89b985 are pinned by
  package, loaded-file and source digests; each tunes its native batch independently.

## Results and remaining directions

All ratios below compare to the fastest peer within this campaign. In Fused,
surface d7 leads by 9.506×/3.344×/1.180× at 1/64/1024 shots and surface d9 by
4.973×/3.120×/1.230×. Every paired range for these surface cells exceeds one.
MSC d3 leads by 1.557×/1.518× at 1/64; 1024 is near parity (1.013×, paired
range crosses one). MSC d5 remains 1.421×/1.091×/1.101× slower. See
[comparisons.csv](comparisons.csv) for every Strict/Fused point and paired range.

Affine preparation belongs to `first_ns`: surface d7 Fused cell-median first calls cost
1.43–1.57 ms, d9 2.80–3.39 ms, in addition to circuit compilation. Warm throughput
does not establish a cold single-request advantage. All backend cold phases,
accepted throughput and whole-process RSS remain in [warm.csv](warm.csv).

Separate paired Rust-only ablations against packed rejection show roughly 3×
bulk surface improvement and 19–22× warm one-shot improvement. MSC d5 Fused
1024 regresses about 2.4%, with all five paired ratios below one; Strict 64
regresses about 4.6%. These exploratory regressions are retained and identify
follow-up work; cross-campaign peer timing is not used to estimate them.

The host is shared and unpinned. Finite witnesses do not certify rare conditional
logical-error accuracy or circuit physics. Native peer build flags are not fully
attested. Raw-record throughput and counts are separate contracts. These results
establish measured surface-counts leads, not universal SOTA performance.

## Verification

```sh
python3 benchmarks/near_clifford/analyze_diagnostics.py benchmarks/near_clifford/results/apple-m4-affine-counts-2026-10-08 --kind application_counts --check
python3 -O benchmarks/near_clifford/application_counts/verify.py benchmarks/near_clifford/results/apple-m4-affine-counts-2026-10-08 --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-affine-counts-2026-10-08
python3 -O benchmarks/near_clifford/application_counts/test_contract.py benchmarks/near_clifford/results/apple-m4-affine-counts-2026-10-08
```

The corruption suites include resealed v5-to-v4/v3/v2 mutations with every Rust
execution label rewritten. Immutable older publications and derived bytes remain
unchanged. Independent review and final-head hosted CI are required before merge.
