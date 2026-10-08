# Verified near-Clifford diagnostic measurements

Measured source: `b555f785af3e432cf850de67acad46d01a33e9c1`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 658, "retained_failure_events": 14, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `None`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": "-C target-cpu=native"}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Raw compile/prepare/first-call metadata is retained; this counts campaign has no derived lifecycle comparison. Use a dedicated fresh-seed cold campaign for phase comparisons.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.428 | 4.560 | 3.194× | 2.889–3.842 | clifft |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.436 | 4.677 | 3.256× | 3.064–3.732 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/strict | 15.419 | 23.869 | 1.548× | 1.054–1.652 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/fused | 15.962 | 24.121 | 1.511× | 1.438–1.653 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 269.621 | 283.678 | 1.052× | 0.9225–1.1 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 270.940 | 277.848 | 1.025× | 0.9681–1.137 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 8.189 | 11.519 | 1.407× | 1.258–1.636 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/fused | 7.154 | 11.361 | 1.588× | 1.151–1.7 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 432.522 | 482.645 | 1.116× | 0.8773–1.243 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 389.130 | 471.919 | 1.213× | 1.081–1.391 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 7467.089 | 7783.661 | 1.042× | 0.8718–1.179 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 9119.278 | 9895.347 | 1.085× | 1.012–1.272 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.495 | 5.251 | 10.62× | 8.71–11.27 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 0.698 | 7.463 | 10.7× | 8.727–13.62 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 10.700 | 50.436 | 4.713× | 3.827–4.775 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 10.210 | 39.197 | 3.839× | 3.538–4.021 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 148.123 | 188.056 | 1.27× | 1.233–1.325 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 141.890 | 180.784 | 1.274× | 1.247–1.295 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.926 | 4.748 | 5.127× | 4.781–5.173 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 0.927 | 4.759 | 5.132× | 4.519–5.173 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 19.148 | 59.366 | 3.1× | 2.494–3.582 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 18.777 | 65.438 | 3.485× | 3.016–3.623 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 357.353 | 627.834 | 1.757× | 1.62–1.937 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 361.330 | 611.724 | 1.693× | 1.528–2.06 | clifft-scheduled |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
