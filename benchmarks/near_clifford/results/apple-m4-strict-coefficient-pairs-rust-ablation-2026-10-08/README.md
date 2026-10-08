# Same-host Rust-only Strict coefficient pairs

All three complete experiments compare reviewed zero-noise-span publication source
`191c10bbe1b3a8ea86ed5f7d8e6d6d814cc30909` against clean candidate
`b555f785af3e432cf850de67acad46d01a33e9c1`. Each covers all 24 original
MSC d3/d5 and surface d7/d9 cells, 1/64/1024 shots and Strict/Fused separately.
No peer/SOTA claim follows from these source-effect scouts.

The warm runs each retain 288 events: 48 independent four-seed full-record/counts/RNG
validations and 240 timed processes, five rotated/reversed paired rounds and seven
observations of at least 50 ms. The cold run retains 240 processes and 7680 observations
with fresh compilation/sampler for each fixed seed 739..770. Compile, prepare, first
call and phase sum remain separate; no 50 ms minimum applies to cold observations.
Counts, sixteen-word RNG continuation and the 64 MiB cache cap are replayed.

## Benefits and regressions

Warm MSC d5 Strict improves in every process pair in both experiments. Fused d5
paired ranges cross one; no stable Fused effect is established.

| Campaign / policy / shots | Baseline / candidate | Paired process range |
| --- | ---: | ---: |
| first-warm / strict / 1 | 1.2554× | 1.1956–1.5308 |
| first-warm / fused / 1 | 0.9765× | 0.8512–1.0970 |
| first-warm / strict / 64 | 1.2883× | 1.2464–1.2919 |
| first-warm / fused / 64 | 0.9925× | 0.9925–1.0194 |
| first-warm / strict / 1024 | 1.3045× | 1.2892–1.3887 |
| first-warm / fused / 1024 | 1.0186× | 0.8595–1.0332 |
| confirmation-warm / strict / 1 | 1.2538× | 1.2399–1.2632 |
| confirmation-warm / fused / 1 | 1.0089× | 0.9616–1.1958 |
| confirmation-warm / strict / 64 | 1.2461× | 1.2094–1.7032 |
| confirmation-warm / fused / 64 | 0.9965× | 0.8543–1.0912 |
| confirmation-warm / strict / 1024 | 1.2645× | 1.1010–1.3131 |
| confirmation-warm / fused / 1024 | 0.9220× | 0.8746–1.1480 |

Confirmation MSC d3 Strict 1024 has ratio 0.9858 and range 0.9646–0.9974 below one;
this does not repeat in first-warm or cold first-call data. Cold d5 Strict first-call
64/1024 ratios are 1.2340/1.2665 with all pairs above one. Its 1024 phase sum is
1.1246 with range 1.0996–1.1635; this sum is not an end-to-end wall-clock measurement.
Cold d5 Strict first single-shot is 0.9752 with all pairs below one, while its phase
sum is 1.0090 because compilation dominates. Every cold cell with all process pairs below one is listed below; all other cells
remain in original summaries.

| Original case / policy / shots / phase | Baseline / candidate | Paired process range |
| --- | ---: | ---: |
| msc_d5_inject_cultivate_p1e-3 / strict / 1 / first_ns | 0.9752× | 0.8952–1.0000 |
| msc_d5_inject_cultivate_p1e-3 / fused / 1024 / prepare_ns | 0.9675× | 0.9347–0.9999 |
| pure_surface_d7_r7_p1e-3 / strict / 1 / prepare_ns | 0.9430× | 0.9038–0.9796 |
| pure_surface_d7_r7_p1e-3 / fused / 1 / prepare_ns | 0.9742× | 0.9192–0.9998 |
| pure_surface_d7_r7_p1e-3 / fused / 64 / first_ns | 0.9879× | 0.9866–0.9940 |
| pure_surface_d7_r7_p1e-3 / strict / 1024 / compile_ns | 0.9953× | 0.9863–0.9997 |
| pure_surface_d7_r7_p1e-3 / strict / 1024 / prepare_ns | 0.9423× | 0.9259–0.9595 |
| pure_surface_d7_r7_p1e-3 / strict / 1024 / first_ns | 0.9867× | 0.9822–0.9949 |
| pure_surface_d7_r7_p1e-3 / fused / 1024 / prepare_ns | 0.9556× | 0.8626–0.9935 |
| pure_surface_d9_r9_p1e-3 / strict / 1 / first_ns | 0.9655× | 0.7179–0.9804 |
| pure_surface_d9_r9_p1e-3 / fused / 1 / first_ns | 0.9507× | 0.9176–0.9851 |

The M4 host is shared and unpinned; paired ranges are descriptive. Both builds use
recorded native flags. The independent complete [M4](../apple-m4-strict-coefficient-pairs-2026-10-08/README.md)
and [x86](../linux-vm-x86-strict-coefficient-pairs-2026-10-08/README.md) peer measurements
remain separate from these same-host source-effect experiments.

## Reproduction and verification

`bindings.json` maps every original probe path to unchanged retained bytes and every
clean measured identity to its original commit. Headers, closures, raw events and
original drivers are unchanged. Warm and cold probe sources are archived for both
roles; historical manifests are source artifacts, not directly runnable at relocated
paths. Original drivers preserve historical checkout/output paths. Recollection uses
new clean checkouts, adjusted paths and a newly recorded identity. No measured binaries
are vendored. Producer tags and branches remain.

```sh
python3 benchmarks/near_clifford/verify_strict_coefficient_pairs_scouts.py
python3 -O benchmarks/near_clifford/verify_strict_coefficient_pairs_scouts.py
python3 benchmarks/near_clifford/test_strict_coefficient_pairs_scouts.py
python3 -O benchmarks/near_clifford/test_strict_coefficient_pairs_scouts.py
```

Explicit checks replay complete source/probe/driver inventory, closure, rotated
schedule, validation, timing arithmetic, seeds/counts/RNG/cache cap and every derived
summary. Resealed negatives must fail in both modes; collector assertions are not
relied on for verification.

The original warm collectors used the historical schema spelling
`exploratory.zero-strict-pair-ablation.v1`; cold uses
`exploratory.strict-coefficient-pairs-cold.v1`. Verification maps this family
to exactly those two strings. Original headers and drivers retain their bytes;
other-family and unexpected schemas must fail.
