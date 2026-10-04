# Near-Clifford refinement measurements

This entry reuses the immutable six timing boundaries from `../scale/main.rs`
and the independent physics witnesses from `../entangled/fixtures.rs`.
The complete matrix contains 87 configurations: 43 scale controls, 28 entangled
controls, and 16 additional distinct rank-11/12/16 shot-count cases.

```sh
python3 benchmarks/near_clifford/refinements/run.py --baseline BASE_SHA --candidate CANDIDATE_SHA --scratch /absolute/fresh/drafts/campaign
python3 benchmarks/near_clifford/refinements/verify.py /absolute/fresh/drafts/campaign/results.json --git-sources --binaries /absolute/fresh/drafts/campaign
```

Use `--only rank_11 rank_12 rank_16 brick_12_12_3 brick_16_16_3` for a targeted
campaign; all shot counts for each selected fixture are retained. Selected
matrices are recorded explicitly. Every measured case has three alternating
process pairs and three repetitions; paired ranges are not confidence intervals.

Pristine timing builds use the pinned unified lock with instrumentation disabled.
Separate diagnostic builds compare 256-shot prepared, flat and individual outputs,
including RNG continuation, against both pristine revisions. Physics witnesses
validate exact compact circuits and reduced witnesses for wide entangled families;
they do not establish full-width distribution correctness for every wide circuit.
The fixture unit tests run before timing. Compilation, verification, diagnostics,
and optional profiling stay outside measurement windows.

Results bind the entry, generated driver, historic timing driver, circuits,
fixture generator, dense oracle overlay, dependency lock, production sources and
all four binaries. The verifier rejects missing/reordered cases, altered medians,
semantic disagreements and invalid cache reservations. It uses explicit exceptions
and remains active with `python3 -O`.

The historical eight counters remain present for comparison. Their coefficient
fallback label predates the new admission policy and should not be interpreted as
an exact breakdown of new rejection reasons. Cache-policy analysis uses the new
`cache_reservation` ledger: reserved bytes, byte limit, root measurement present,
and root child count. Legacy samplers and unplanned samplers report `null`.
A mandatory root over budget must retain no optional measurement or child data.
The reservation is a conservative per-sampler cache estimate, not process RSS;
the timing driver retains both structured and flat samplers together.
