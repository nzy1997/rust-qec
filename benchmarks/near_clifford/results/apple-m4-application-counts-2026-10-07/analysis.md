# Verified near-Clifford diagnostic measurements

Measured source: `46f974d1dcd037203ffcf0fb28f5265cc652f358`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 2.796 | 4.404 | 1.575× | 1.336–1.682 | clifft |
| msc_d3_inject_cultivate_p1e-3/1/fused | 2.385 | 3.926 | 1.646× | 1.541–1.759 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/strict | 156.512 | 25.657 | 0.1639× | 0.1327–0.1738 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 164.194 | 25.334 | 0.1543× | 0.1514–0.1703 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 2516.052 | 266.602 | 0.106× | 0.09434–0.1161 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 2514.083 | 258.511 | 0.1028× | 0.09468–0.113 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 66.218 | 10.571 | 0.1596× | 0.1366–0.1756 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 46.757 | 11.689 | 0.25× | 0.1872–0.2709 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 5061.679 | 580.843 | 0.1148× | 0.1054–0.1202 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 2955.407 | 454.563 | 0.1538× | 0.1538–0.1899 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 61755.709 | 7378.101 | 0.1195× | 0.1125–0.1197 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 55506.417 | 8913.375 | 0.1606× | 0.1251–0.2261 | clifft |
| pure_surface_d7_r7_p1e-3/1/strict | 16.763 | 7.034 | 0.4196× | 0.3654–0.4479 | clifft |
| pure_surface_d7_r7_p1e-3/1/fused | 18.113 | 6.742 | 0.3723× | 0.3042–0.3881 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 926.002 | 40.486 | 0.04372× | 0.04058–0.04812 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 923.107 | 45.686 | 0.04949× | 0.04348–0.05586 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 15504.239 | 250.188 | 0.01614× | 0.01519–0.01804 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 12982.883 | 214.460 | 0.01652× | 0.01596–0.01747 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 33.491 | 5.622 | 0.1679× | 0.1511–0.2478 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 39.505 | 7.390 | 0.1871× | 0.1821–0.1992 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 2446.689 | 102.536 | 0.04191× | 0.03933–0.05241 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 1890.188 | 118.210 | 0.06254× | 0.04058–0.08297 | clifft |
| pure_surface_d9_r9_p1e-3/1024/strict | 39132.312 | 426.608 | 0.0109× | 0.009534–0.02178 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 40502.291 | 693.534 | 0.01712× | 0.01581–0.0179 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust builds full structured records then filters/counts; peers use native counts/early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
