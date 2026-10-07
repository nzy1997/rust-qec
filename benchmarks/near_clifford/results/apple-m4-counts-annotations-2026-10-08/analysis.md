# Verified near-Clifford diagnostic measurements

Measured source: `24b7fdb5d7c076de9372e5074e7fd1175b47797d`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 658, "retained_failure_events": 14, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `None`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": null}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
This counts campaign records no lifecycle phase measurements.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.811 | 2.898 | 1.6× | 1.498–1.618 | clifft |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.802 | 2.872 | 1.594× | 1.571–1.616 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/strict | 14.767 | 19.579 | 1.326× | 1.295–1.333 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 14.950 | 19.390 | 1.297× | 1.283–1.316 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 232.009 | 202.485 | 0.8727× | 0.8698–0.8784 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 230.892 | 202.399 | 0.8766× | 0.8743–0.8805 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 14.419 | 8.856 | 0.6142× | 0.6102–0.619 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 12.497 | 8.830 | 0.7066× | 0.7021–0.7097 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 1573.854 | 379.389 | 0.2411× | 0.2404–0.2418 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 1358.611 | 379.014 | 0.279× | 0.2714–0.2813 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 25153.375 | 6070.037 | 0.2413× | 0.2402–0.2415 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 21503.972 | 6064.102 | 0.282× | 0.2793–0.2828 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 9.045 | 4.115 | 0.4549× | 0.4543–0.4634 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 9.054 | 4.080 | 0.4507× | 0.4493–0.4553 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 27.543 | 33.404 | 1.213× | 1.185–1.239 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 27.689 | 33.460 | 1.208× | 1.193–1.276 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 435.654 | 174.972 | 0.4016× | 0.3906–0.4139 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 440.327 | 175.488 | 0.3985× | 0.3785–0.4033 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 17.258 | 4.603 | 0.2667× | 0.2643–0.2701 | clifft |
| pure_surface_d9_r9_p1e-3/1/fused | 17.136 | 4.550 | 0.2655× | 0.2601–0.2693 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 60.632 | 55.241 | 0.9111× | 0.882–0.954 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 58.789 | 54.878 | 0.9335× | 0.9096–1.087 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 913.740 | 336.313 | 0.3681× | 0.2602–0.3743 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 923.505 | 334.843 | 0.3626× | 0.3586–0.3765 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust uses native counts with scalar/admission-fallback early rejection; live packed lanes complete simulation.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
