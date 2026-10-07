# Formal S3 independent offline audit

PASS: Critical 0, Important 0, Minor 0 unresolved; artifact/report harm gate PASS. Audited actual completed S3 campaign at clean source HEAD `1f015586d7e6133c6f3d5067ad4b67051841a939`. No simulator, build, test, profile or timing was executed by this reviewer; no harness verifier or its helpers was imported.

Frozen result SHA256: `ebe9dabd754c05fd020fae5b8fffc2697f631431dd24b7291cfd0bca6dbac06c`. Actual campaign ran2026-10-07 04:28:34.260142–04:32:17.177227 UTC.

## Complete actual evidence

All seven original cases and all21 configurations, in the frozen order, are present. Independently checked three process pairs per configuration, four actual backends per pair, seven fresh first-call observations and seven normalized warm windows per process. Every warm window has at least50ms accumulated public calls; normalized means equal total_ns/calls. Process order rotates and reverses as frozen. Compile, prepare, seed setup, imports, validation/tuning and caller output destruction retain the documented timing boundaries; peer API-internal seed work stays timed.

Every peer tuning candidate is present: Clifft/default-scheduled use1/64/256/1024/auto; SymFT additionally has its distinct scalar executor. All choices equal the minimum valid three-window process median. There are zero tuning failures. Complete tuning bytes/hash are frozen before final timing according to the actual launcher order. Each selected timing and raw transcript reports the actual selected batch/scalar configuration, one requested CPU thread, peer loaded-file identity and consumed native-input digest. Every rstim payload reports actual compiled API, immutable Fused policy, SmallRng/rand0.8.7 and bounded default-cache reservation.

Independently read all28 default and84 selected full raw payloads, each8192 shots. All112 artifact hashes, complete bit vectors, dimensions, metadata and lossless compact transcripts match. Decompression termination/padding are checked. Recomputed every marginal, adjacent parity, contiguous block-of-four, full parity and original detector/observable mask from raw bits, including original XOR/observable accumulation. All default and selected all-backend pairwise differences satisfy the independently recomputed family threshold. Selected samples use the exact timed shots-per-call and selected batch; default SymFT uses its actual scalar executor. Cross-backend raw records and RNG streams are not asserted equal: these are finite distribution witnesses, not complete joint-distribution proof.

All130 formal artifacts have an exact known inventory and recorded hashes in formal-artifact-hashes.json. Native/rstim inputs are byte-identical records-only projections, with no lowering/spectator/extra-measurement transformation; source annotations alone are omitted. Physical and measurement widths and all mask counts were independently recalculated.

## Source, peer and end closure

The complete formal production inventory is97 files:94 rstim Rust sources plus three Cargo files. Each hash matches both the current clean checkout and the recorded Git commit. All21 harness files, the actual built probe digest, pinned package files, actual imported wrapper/extensions and complete211-file SymFT checkout match current bytes. The package/source/import before/end summaries recompute exactly. Clifft0.11.0 and SymFT0.1.1, with NumPy2.4.6 environments, remain bound to actual distribution/import files. SymFT checkout is the clean pinned c89b985 revision. This establishes identity/stability; it does not cryptographically prove extension provenance or attest peer compiler flags.

The separate source-S3 binding and earlier workspace check/test results were also associated to actual current source/Cargo, including renvelope sources/manifest. Their before/end records match, and the guarded current library/Cargo bytes match the fresh S3 commit. Those tests ran before the source-only commit at the recorded earlier HEAD; no fictitious postcommit rerun is claimed. workspace-byte-association.json preserves that distinction. The formal harness provides its stated source/harness/binary/package end checks; it is not the expanded compiler-read attestation protocol used by separate diagnostic campaigns.

Current source/Git state, harness, binary, packages, imports and SymFT source were checked again at audit end. Formal result/report/analysis/host-sidecar hashes stayed unchanged. The actual harness verifier was read as contract material; all raw/count/statistic checks here were implemented independently.

## Recomputed results

Geomeans weight each configuration equally and compare against the fastest independently tuned valid peer's median for that configuration. They are not total-work or elapsed-campaign speedups.

| Scope | Warm speedup geomean |
| --- | ---: |
|All21|1.50790771x|
|shots1|2.19728313x|
|shots64|1.39762747x|
|shots1024|1.11646942x|
|Cultivation|1.00586976x|
|Adaptive|1.62014375x|
|Entangled|2.15493783x|
|Noisy syndrome|1.92574047x|
|Parity|1.36723468x|
|Terminal|1.90594651x|

Independently recomputed all21 backend median-of-process-medians, selected fastest peer, speedup and paired speedup range. They agree with analysis.json and every warm report row. Sixteen configurations win by median and sixteen have every reported paired ratio above1. All21 rows, complete cold metrics and tuning failures are in independent-analysis.json and all21.csv. Ranges describe three paired process medians; they are not confidence intervals.

The five negative median cases remain:

| Case | Shots | rstim warm us | Fastest peer warm us | Speedup | Paired ratio range |
| --- | ---: | ---: | ---: | ---: | --- |
|terminal|1024|59.03868|49.89465|0.84512x|0.84023–0.84990|
|msc3|1024|228.57096|222.04609|0.97145x|0.96677–1.00110|
|msc5|1|32.68547|26.10381|0.79864x|0.79754–0.79975|
|msc5|64|1323.49447|1202.86519|0.90886x|0.90679–0.91125|
|msc5|1024|21205.87533|18129.54167|0.85493x|0.84776–0.86228|

MSC5 loses at every shot size in every pair. Cultivation's aggregate near1 therefore does not meet a claim of clearly exceeding peers. The campaign supports continued bounded investigation, not completion of the broader superiority objective.

## Cold and host limitations

All84 cold report rows were independently matched to raw compile/prepare/first-call medians, ranks and actual reservation maxima. Cold is not silently combined with warm. For MSC5, rstim compile medians are about13.15–13.27ms versus peers around10.44–13.61ms. Rstim first-call medians at1/64/1024 are69.458us /3.399ms /24.894ms; scheduled Clifft gives34.750us /1.211ms /18.153ms. These costs remain unfavorable. Brick16 and MSC3 also retain large first-call costs despite warm wins in smaller shot configurations. The independent cold data additionally preserves the range of each process's first observation, which a seven-observation median can hide.

Host conditions were observed at04:28:55.435599 UTC, after campaign start, with after observation at04:33:02.443659 UTC. That ordering was checked. Shared Apple M4 host, ten logical CPUs, load averages about3.18/3.12/3.64 during start and2.44/2.57/3.24 after; the recorded thermal/power text reports no recorded warnings. No pre-start, idle/exclusive-host or pinned-core condition is inferred. Compiler overrides in that during-start observation are null; this is not an attestation of every peer build flag or earlier environment event.

Scope is this Apple M4 fp64 CPU/full-raw-record contract, current pinned peers and opt-in compiled Fused API. Original constructors remain Strict by default. There is no GPU/x86 SIMD, universal workload, postselection/counts-only, elapsed causality between prior campaigns, or general SOTA claim. Broader unsupported published rotation/U3 workloads remain excluded explicitly rather than recast as wins.
