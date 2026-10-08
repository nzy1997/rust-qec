# Linux x86 VM master-to-compact-replay source effects

Baseline: merged master `e1453a7b634275d3055ef64b2f4a7b33d3995870`.
Candidate: clean measured `d989531d837fe874a77ac05745f5d77ea5639d52`.
All four warm/control campaigns retain all 24 cells and 288 events each:
48 independent four-seed records/counts/RNG validations followed by 240 timed
processes, five rotated/reversed paired rounds and seven observations at least
50 ms long. Both A/A controls use the exact candidate source and binary under
both labels. No peer or SOTA claim follows from these source-effect ratios.

## Complete warm and null-control results

Ratios are baseline time / candidate time. All median losses and process outliers
remain; a median gain alone does not establish a stable small source effect.

| Campaign | Circuit | Policy | Shots | Baseline / candidate | Paired range |
| --- | --- | --- | ---: | ---: | ---: |
| warm | MSC d3 | strict | 1 | 0.983305× | 0.969550–1.027585 |
| warm | MSC d3 | fused | 1 | 0.994556× | 0.991631–1.014046 |
| warm | MSC d3 | strict | 64 | 0.994264× | 0.992354–0.998356 |
| warm | MSC d3 | fused | 64 | 0.995662× | 0.988508–1.000302 |
| warm | MSC d3 | strict | 1024 | 0.998371× | 0.994176–1.004759 |
| warm | MSC d3 | fused | 1024 | 0.993536× | 0.991451–0.995588 |
| warm | MSC d5 | strict | 1 | 0.997877× | 0.988912–1.010507 |
| warm | MSC d5 | fused | 1 | 1.029811× | 1.026068–1.034436 |
| warm | MSC d5 | strict | 64 | 1.384511× | 1.359695–1.397523 |
| warm | MSC d5 | fused | 64 | 1.415852× | 1.350738–1.435064 |
| warm | MSC d5 | strict | 1024 | 1.360178× | 1.347414–1.397664 |
| warm | MSC d5 | fused | 1024 | 1.405447× | 1.355085–1.442753 |
| warm | surface d7 | strict | 1 | 1.006594× | 0.988432–1.012802 |
| warm | surface d7 | fused | 1 | 1.005320× | 1.000998–1.008602 |
| warm | surface d7 | strict | 64 | 1.006387× | 1.001797–1.021622 |
| warm | surface d7 | fused | 64 | 1.004150× | 0.995205–1.020722 |
| warm | surface d7 | strict | 1024 | 0.997859× | 0.977404–1.003338 |
| warm | surface d7 | fused | 1024 | 0.999389× | 0.993909–1.000668 |
| warm | surface d9 | strict | 1 | 0.973159× | 0.963288–0.980421 |
| warm | surface d9 | fused | 1 | 0.978362× | 0.961877–0.983270 |
| warm | surface d9 | strict | 64 | 0.994757× | 0.979002–1.010183 |
| warm | surface d9 | fused | 64 | 1.002658× | 0.993710–1.010265 |
| warm | surface d9 | strict | 1024 | 1.006399× | 0.999328–1.011843 |
| warm | surface d9 | fused | 1024 | 0.998117× | 0.992340–1.008758 |
| confirmation | MSC d3 | strict | 1 | 0.975769× | 0.959103–0.995913 |
| confirmation | MSC d3 | fused | 1 | 1.004096× | 1.002459–1.010236 |
| confirmation | MSC d3 | strict | 64 | 0.999356× | 0.993214–1.011227 |
| confirmation | MSC d3 | fused | 64 | 0.999927× | 0.993949–0.999927 |
| confirmation | MSC d3 | strict | 1024 | 0.995392× | 0.985747–1.000262 |
| confirmation | MSC d3 | fused | 1024 | 0.996827× | 0.993849–0.999654 |
| confirmation | MSC d5 | strict | 1 | 0.997294× | 0.989834–1.009390 |
| confirmation | MSC d5 | fused | 1 | 1.041002× | 1.023584–1.044842 |
| confirmation | MSC d5 | strict | 64 | 1.347243× | 1.311511–1.383387 |
| confirmation | MSC d5 | fused | 64 | 1.390143× | 1.373776–1.414858 |
| confirmation | MSC d5 | strict | 1024 | 1.365486× | 1.365486–1.411352 |
| confirmation | MSC d5 | fused | 1024 | 1.364155× | 1.357600–1.411882 |
| confirmation | surface d7 | strict | 1 | 1.009630× | 1.001222–1.013102 |
| confirmation | surface d7 | fused | 1 | 1.013199× | 0.999724–1.017655 |
| confirmation | surface d7 | strict | 64 | 1.004302× | 1.000456–1.007593 |
| confirmation | surface d7 | fused | 64 | 1.002378× | 1.000189–1.007168 |
| confirmation | surface d7 | strict | 1024 | 1.004322× | 0.994718–1.017127 |
| confirmation | surface d7 | fused | 1024 | 1.003087× | 0.996728–1.005724 |
| confirmation | surface d9 | strict | 1 | 0.979668× | 0.970432–0.993788 |
| confirmation | surface d9 | fused | 1 | 0.980732× | 0.978326–0.984917 |
| confirmation | surface d9 | strict | 64 | 1.006301× | 0.999638–1.032198 |
| confirmation | surface d9 | fused | 64 | 0.993668× | 0.981237–1.004877 |
| confirmation | surface d9 | strict | 1024 | 1.006657× | 0.998225–1.012383 |
| confirmation | surface d9 | fused | 1024 | 1.000288× | 0.998958–1.033212 |
| same-binary-warm | MSC d3 | strict | 1 | 0.989941× | 0.966952–1.043493 |
| same-binary-warm | MSC d3 | fused | 1 | 1.001502× | 0.982536–1.050473 |
| same-binary-warm | MSC d3 | strict | 64 | 0.998139× | 0.994362–1.006168 |
| same-binary-warm | MSC d3 | fused | 64 | 1.005349× | 0.994593–1.007141 |
| same-binary-warm | MSC d3 | strict | 1024 | 1.000512× | 0.980628–1.018592 |
| same-binary-warm | MSC d3 | fused | 1024 | 1.006534× | 0.998087–1.018768 |
| same-binary-warm | MSC d5 | strict | 1 | 1.006727× | 0.972378–1.013820 |
| same-binary-warm | MSC d5 | fused | 1 | 1.006086× | 0.987882–1.012601 |
| same-binary-warm | MSC d5 | strict | 64 | 0.997075× | 0.979561–1.031749 |
| same-binary-warm | MSC d5 | fused | 64 | 1.006715× | 0.977914–1.020705 |
| same-binary-warm | MSC d5 | strict | 1024 | 1.004384× | 0.977449–1.011766 |
| same-binary-warm | MSC d5 | fused | 1024 | 0.992934× | 0.966418–1.039198 |
| same-binary-warm | surface d7 | strict | 1 | 0.999577× | 0.986596–1.010772 |
| same-binary-warm | surface d7 | fused | 1 | 0.994367× | 0.984486–1.011628 |
| same-binary-warm | surface d7 | strict | 64 | 1.000943× | 1.000081–1.004912 |
| same-binary-warm | surface d7 | fused | 64 | 0.996260× | 0.990509–1.003163 |
| same-binary-warm | surface d7 | strict | 1024 | 0.994842× | 0.990968–1.003317 |
| same-binary-warm | surface d7 | fused | 1024 | 1.000362× | 0.996107–1.004167 |
| same-binary-warm | surface d9 | strict | 1 | 1.000055× | 0.996877–1.002518 |
| same-binary-warm | surface d9 | fused | 1 | 1.000507× | 0.999346–1.014150 |
| same-binary-warm | surface d9 | strict | 64 | 0.997348× | 0.994921–1.004002 |
| same-binary-warm | surface d9 | fused | 64 | 1.001090× | 0.998924–1.019217 |
| same-binary-warm | surface d9 | strict | 1024 | 1.004099× | 0.971325–1.013290 |
| same-binary-warm | surface d9 | fused | 1024 | 1.000739× | 0.984603–1.004051 |
| same-binary-confirmation | MSC d3 | strict | 1 | 1.003106× | 0.984873–1.028476 |
| same-binary-confirmation | MSC d3 | fused | 1 | 0.986805× | 0.928275–1.006057 |
| same-binary-confirmation | MSC d3 | strict | 64 | 1.002485× | 0.988428–1.007938 |
| same-binary-confirmation | MSC d3 | fused | 64 | 0.999313× | 0.989999–1.000315 |
| same-binary-confirmation | MSC d3 | strict | 1024 | 1.007508× | 0.999926–1.009169 |
| same-binary-confirmation | MSC d3 | fused | 1024 | 0.998594× | 0.994406–1.002050 |
| same-binary-confirmation | MSC d5 | strict | 1 | 1.004274× | 0.982467–1.009570 |
| same-binary-confirmation | MSC d5 | fused | 1 | 0.996003× | 0.983942–1.001325 |
| same-binary-confirmation | MSC d5 | strict | 64 | 1.027422× | 0.992124–1.045535 |
| same-binary-confirmation | MSC d5 | fused | 64 | 1.021680× | 0.980313–1.044383 |
| same-binary-confirmation | MSC d5 | strict | 1024 | 0.997987× | 0.990800–1.032179 |
| same-binary-confirmation | MSC d5 | fused | 1024 | 0.985980× | 0.981387–1.007290 |
| same-binary-confirmation | surface d7 | strict | 1 | 1.005803× | 0.998541–1.023707 |
| same-binary-confirmation | surface d7 | fused | 1 | 1.001039× | 0.994758–1.007119 |
| same-binary-confirmation | surface d7 | strict | 64 | 1.002455× | 0.999661–1.004548 |
| same-binary-confirmation | surface d7 | fused | 64 | 1.000258× | 0.975586–1.005793 |
| same-binary-confirmation | surface d7 | strict | 1024 | 1.001553× | 0.998483–1.006771 |
| same-binary-confirmation | surface d7 | fused | 1024 | 1.001987× | 0.991822–1.017351 |
| same-binary-confirmation | surface d9 | strict | 1 | 0.996426× | 0.973205–0.998980 |
| same-binary-confirmation | surface d9 | fused | 1 | 0.998144× | 0.988978–1.003555 |
| same-binary-confirmation | surface d9 | strict | 64 | 1.008894× | 0.991379–1.015544 |
| same-binary-confirmation | surface d9 | fused | 64 | 1.000593× | 0.998793–1.009745 |
| same-binary-confirmation | surface d9 | strict | 1024 | 1.005522× | 1.000097–1.022062 |
| same-binary-confirmation | surface d9 | fused | 1024 | 1.002443× | 0.999014–1.015285 |

D5 bulk gains reproduce in both A/B campaigns. Other median and paired losses
remain visible above; A/A cells with every pair on one side of one demonstrate
that this protocol cannot turn every small shift into a source claim.

The actual source-effect host reports AMD EPYC 9V74, CPU [0]/4, Rust 1.93.1,
native compiler flags and one numerical/Rayon thread. Actual native coefficient,
CDF, compact-span and public counts/RNG gates passed. Both retained production
ELFs contain packed YMM FMA and scalar FMA. These facts do not establish that
the baseline lacked SIMD or that it ran on the separate peer VM.
[Closed producer workflow](https://github.com/nzy1997/rust-qec/actions/runs/37743955356).
No x86 cold campaign is claimed.

## Diagnostic direction and retention

A separate current d5/1024 Strict/Fused diagnostic campaign ran in
[workflow 37751156104](https://github.com/nzy1997/rust-qec/actions/runs/37751156104).
Its whole-process cpu-clock profiles include startup, warmup and teardown and are
all `performance_valid=false`. Candidate leaf samples concentrate on rotation,
probability and projection kernels; this identifies an experiment direction,
not elapsed phase fractions, measured gains or peer leadership. Raw perf data,
stack exports and actual ELF bytes remain in the workflow artifact. The perf
wrapper is identified; the underlying perf ELF lacks separate SHA attestation.

All original schemas, drivers, headers, closures, events, seven probe inputs and
build logs per role remain unchanged. Historical source snapshots are evidence,
not runnable relocated Cargo projects. Producer tags/branches remain. Native
binaries/disassemblies stay in original ignored evidence/workflow artifacts;
portable replay checks byte-identifying receipts without hashing absent binaries.
M4 cold and warm binary identities are checked separately. Recollection requires
new clean checkouts, corrected output paths and new measured identities.

```sh
python3 benchmarks/near_clifford/verify_compact_zero_spans_scouts.py
python3 -O benchmarks/near_clifford/verify_compact_zero_spans_scouts.py
python3 benchmarks/near_clifford/test_compact_zero_spans_scouts.py
python3 -O benchmarks/near_clifford/test_compact_zero_spans_scouts.py
```

Replay checks all sources/probes, driver digests, receipts, exact schedules,
validations, observations and derived summaries. Resealed adversarial cases must
fail in normal and optimized Python. The cold-prefix option preserves the older
240-event cold contracts by default. Formal independent full PR review and final
head CI are separate merge gates.

The local `.gitattributes` preserves original native test logs with their trailing
blank line while keeping other whitespace checks active.
