# Verified near-Clifford diagnostic measurements

Measured source: `b555f785af3e432cf850de67acad46d01a33e9c1`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
Verification: `{"events": 658, "retained_failure_events": 14, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `[0]`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": "-C target-cpu=native"}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Raw compile/prepare/first-call metadata is retained; this counts campaign has no derived lifecycle comparison. Use a dedicated fresh-seed cold campaign for phase comparisons.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.863 | 7.224 | 3.877× | 3.84–3.97 | clifft |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.862 | 7.235 | 3.887× | 3.838–3.943 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/strict | 29.436 | 39.211 | 1.332× | 1.295–1.353 | symft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 29.195 | 39.420 | 1.35× | 1.335–1.358 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 459.983 | 443.286 | 0.9637× | 0.9577–0.969 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 462.269 | 443.142 | 0.9586× | 0.955–0.9686 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 9.697 | 14.755 | 1.522× | 1.186–1.576 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 10.126 | 14.852 | 1.467× | 1.42–1.491 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 576.375 | 491.700 | 0.8531× | 0.842–0.8593 | symft |
| msc_d5_inject_cultivate_p1e-3/64/fused | 615.011 | 491.969 | 0.7999× | 0.7972–0.8089 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 9988.217 | 7408.311 | 0.7417× | 0.7358–0.7499 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 10623.808 | 7408.520 | 0.6974× | 0.6911–0.7028 | symft |
| pure_surface_d7_r7_p1e-3/1/strict | 0.910 | 9.282 | 10.2× | 9.977–10.3 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 0.913 | 9.100 | 9.966× | 9.939–10.65 | clifft |
| pure_surface_d7_r7_p1e-3/64/strict | 17.542 | 53.032 | 3.023× | 2.994–3.188 | clifft |
| pure_surface_d7_r7_p1e-3/64/fused | 17.733 | 52.731 | 2.974× | 2.959–3.025 | clifft |
| pure_surface_d7_r7_p1e-3/1024/strict | 269.067 | 329.355 | 1.224× | 1.212–1.235 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1024/fused | 268.425 | 329.640 | 1.228× | 1.223–1.245 | clifft |
| pure_surface_d9_r9_p1e-3/1/strict | 1.955 | 10.940 | 5.596× | 5.443–5.621 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 1.954 | 10.910 | 5.585× | 5.56–5.707 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 38.094 | 105.844 | 2.778× | 2.743–2.839 | clifft |
| pure_surface_d9_r9_p1e-3/64/fused | 38.206 | 104.873 | 2.745× | 2.725–2.798 | clifft |
| pure_surface_d9_r9_p1e-3/1024/strict | 570.543 | 746.946 | 1.309× | 1.298–1.338 | clifft |
| pure_surface_d9_r9_p1e-3/1024/fused | 569.588 | 746.176 | 1.31× | 1.301–1.318 | clifft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
