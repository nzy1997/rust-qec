# Same-host Rust-only zero-noise spans

All three complete experiments compare reviewed coefficient-intern source
`0b74b9b2b3ed95a5a91f0c8b35e81308ccbb98ea` against clean candidate
`d0e053c32155811327da2b1a0505ae2aaac0560a`. Each covers all 24 original
MSC d3/d5 and surface d7/d9 cells, 1/64/1024 shots and Strict/Fused separately.
No peer/SOTA claim follows from these source-effect scouts.

The two warm runs (`first-warm`, `confirmation-warm`) each retain 288 events:
48 independent four-seed full-record/counts/RNG validations and 240 timed processes,
five rotated/reversed paired rounds and seven observations of at least 50 ms.
The cold run (`first-cold`) retains 240 processes and 7680 observations with fresh
compilation/sampler for each of fixed seeds 739..770. Compile, prepare, first call
and phase sum remain separate; no 50 ms minimum applies to cold observations.
Counts, sixteen-word RNG continuation and 64 MiB cache caps are replayed.

## Benefits and regressions

Warm MSC d3 single-shot Strict/Fused ratios reproduce at 2.0167/2.0287 and
2.0200/2.0041. MSC d5 single-shot ratios reproduce at 1.6172/1.8399 and
1.6282/1.8649; every paired range exceeds one. Bulk benefits are much smaller;
the confirmation's four d5 bulk paired ranges all cross one. The first run's
surface d9 Strict 1024 ratio is 0.9760 with all pairs below one; this does not
repeat, while confirmation surface d7 Strict 1024 is 0.9763 with all pairs below
one. All cells remain in their original summaries.

Cold d3 first single-shot Strict/Fused ratios are 1.2443/1.2326; d5 is
1.6126/1.4239, all paired ranges above one. Compile+prepare+first d5 single-shot
phase sums are 0.9944/1.0028 with ranges crossing one: roughly 12.4 ms compilation
dominates the 6–10 µs first call. Cold d5 Strict first 64/1024 ratios are
0.9901/0.9948, both with all pairs below one. The full compile/prepare/first/total
statistics and all other regressions remain.

The M4 host is shared and unpinned. Paired ranges are descriptive; small effects
are not confidence bounds. Both builds use the source-bound recorded native flags.
The [M4](../apple-m4-zero-noise-spans-2026-10-08/README.md) and
[x86](../linux-vm-x86-zero-noise-spans-2026-10-08/README.md) peer campaigns are
independent complete measurements. A separate interrupted M4 attempt lacks closure
and contributes no formal performance claim.

## Reproduction and verification

`bindings.json` maps every original probe path to unchanged retained bytes and
every clean measured identity to its original commit. Headers, closures, raw events
and original drivers are unchanged. Warm and cold probe sources are archived for
both roles; manifests are historical source artifacts, not directly runnable at
relocated archive paths. Original drivers preserve historical checkout/output paths;
recollection adjusts paths in new clean checkouts and records a new identity.
No measured binaries are vendored. Candidate and diagnostic producer tags remain.

```sh
python3 benchmarks/near_clifford/verify_zero_noise_spans_scouts.py
python3 -O benchmarks/near_clifford/verify_zero_noise_spans_scouts.py
python3 benchmarks/near_clifford/test_zero_noise_spans_scouts.py
python3 -O benchmarks/near_clifford/test_zero_noise_spans_scouts.py
```

Explicit checks replay complete source/probe/driver inventory, closure, rotated
schedule, validation, timing arithmetic, seeds/counts/RNG/cache cap and every derived
summary. Resealed negative controls must fail in both modes. Archived collector
assertions are not relied on for verification.
