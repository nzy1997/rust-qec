# Verified near-Clifford diagnostic measurements

Measured source: `bb11a636b0c855ed7deb547d5dbdd2baff96b987`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 658, "retained_failure_events": 14, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `None`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": null}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Lifecycle phase sums exclude diagnostic conversion and destruction; they are not end-to-end wall-clock time.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS; lifecycle.csv for every history/budget.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.942 | 2.815 | 1.449× | 1.437–1.461 | clifft |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.953 | 2.817 | 1.442× | 1.44–1.454 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/strict | 14.564 | 18.976 | 1.303× | 1.297–1.335 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 14.628 | 19.032 | 1.301× | 1.297–1.313 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 226.984 | 199.073 | 0.877× | 0.8749–0.9424 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 228.062 | 198.887 | 0.8721× | 0.8669–0.8764 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 57.604 | 8.725 | 0.1515× | 0.1482–0.1527 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 37.882 | 8.738 | 0.2307× | 0.2297–0.2324 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 1604.663 | 387.619 | 0.2416× | 0.235–0.2484 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 1374.187 | 389.152 | 0.2832× | 0.2803–0.3022 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 25683.520 | 6202.481 | 0.2415× | 0.2413–0.2447 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 22033.333 | 6172.296 | 0.2801× | 0.2794–0.2856 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 13.478 | 4.017 | 0.298× | 0.2951–0.3037 | clifft |
| pure_surface_d7_r7_p1e-3/1/fused | 13.517 | 4.025 | 0.2977× | 0.2945–0.3013 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 31.664 | 29.322 | 0.9261× | 0.922–0.9342 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 32.365 | 30.006 | 0.9271× | 0.918–1.085 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 472.085 | 165.184 | 0.3499× | 0.3459–0.3559 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 469.604 | 164.492 | 0.3503× | 0.3477–0.3558 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 29.666 | 4.559 | 0.1537× | 0.1506–0.158 | clifft |
| pure_surface_d9_r9_p1e-3/1/fused | 28.574 | 4.463 | 0.1562× | 0.152–0.161 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 69.229 | 55.501 | 0.8017× | 0.7613–0.9312 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 66.076 | 52.760 | 0.7985× | 0.7881–0.9347 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 936.003 | 327.553 | 0.3499× | 0.342–0.3529 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 933.010 | 328.006 | 0.3516× | 0.3489–0.3534 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust uses native counts without early rejection; peers use native counts/early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
