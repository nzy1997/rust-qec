# Verified near-Clifford diagnostic measurements

Measured source: `b85f6f3d684d7b467280038ce2b0cc622449b5b6`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
Verification: `{"events": 658, "retained_failure_events": 14, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `[0]`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": "-C target-cpu=native"}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Lifecycle phase sums exclude diagnostic conversion and destruction; they are not end-to-end wall-clock time.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS; lifecycle.csv for every history/budget.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| msc_d3_inject_cultivate_p1e-3/1/strict | 3.620 | 7.437 | 2.054× | 2.003–2.067 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 3.642 | 7.464 | 2.05× | 1.873–2.063 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 234.122 | 39.264 | 0.1677× | 0.1631–0.1719 | symft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 232.721 | 39.429 | 0.1694× | 0.1524–0.1718 | symft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 3714.791 | 445.407 | 0.1199× | 0.1195–0.1215 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 3718.938 | 446.924 | 0.1202× | 0.1188–0.1213 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 124.883 | 15.355 | 0.123× | 0.1208–0.1231 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 68.280 | 15.426 | 0.2259× | 0.2233–0.227 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 8000.073 | 521.778 | 0.06522× | 0.06437–0.06691 | symft |
| msc_d5_inject_cultivate_p1e-3/64/fused | 4432.971 | 516.725 | 0.1166× | 0.115–0.1205 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 126568.375 | 7991.934 | 0.06314× | 0.06193–0.06393 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 70363.444 | 7984.707 | 0.1135× | 0.112–0.1143 | symft |
| pure_surface_d7_r7_p1e-3/1/strict | 21.450 | 9.550 | 0.4452× | 0.4426–0.4516 | clifft |
| pure_surface_d7_r7_p1e-3/1/fused | 21.133 | 9.467 | 0.4479× | 0.4456–0.4566 | clifft |
| pure_surface_d7_r7_p1e-3/64/strict | 1348.931 | 53.914 | 0.03997× | 0.03977–0.0403 | clifft |
| pure_surface_d7_r7_p1e-3/64/fused | 1368.096 | 54.109 | 0.03955× | 0.03919–0.04169 | clifft |
| pure_surface_d7_r7_p1e-3/1024/strict | 21971.173 | 328.487 | 0.01495× | 0.01483–0.01517 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1024/fused | 22136.896 | 329.137 | 0.01487× | 0.01436–0.01511 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/strict | 43.568 | 11.277 | 0.2588× | 0.2548–0.2637 | clifft |
| pure_surface_d9_r9_p1e-3/1/fused | 43.442 | 11.220 | 0.2583× | 0.2562–0.2651 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 2782.688 | 107.343 | 0.03858× | 0.03841–0.03973 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/fused | 2773.392 | 106.748 | 0.03849× | 0.03643–0.04052 | clifft |
| pure_surface_d9_r9_p1e-3/1024/strict | 44852.290 | 743.692 | 0.01658× | 0.01629–0.01728 | clifft |
| pure_surface_d9_r9_p1e-3/1024/fused | 44858.070 | 743.773 | 0.01658× | 0.01642–0.01681 | clifft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust builds full structured records then filters/counts; peers use native counts/early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
