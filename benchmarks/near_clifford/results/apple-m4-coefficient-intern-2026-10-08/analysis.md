# Verified near-Clifford diagnostic measurements

Measured source: `0177cf3d4c2b988b35c692cc9a5098a3bf4f322f`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.637 | 2.786 | 1.702× | 1.659–1.74 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.649 | 2.803 | 1.699× | 1.671–1.723 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/strict | 12.599 | 18.815 | 1.493× | 1.488–1.547 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 12.633 | 18.838 | 1.491× | 1.488–1.504 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 197.870 | 197.752 | 0.9994× | 0.985–1.035 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 200.277 | 197.332 | 0.9853× | 0.9827–1.009 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 11.214 | 8.560 | 0.7633× | 0.7523–0.7685 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 9.520 | 8.479 | 0.8906× | 0.8834–0.905 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 419.349 | 368.953 | 0.8798× | 0.8739–0.8841 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 293.667 | 367.793 | 1.252× | 1.238–1.269 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 7192.566 | 5858.037 | 0.8145× | 0.8116–0.8192 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 5069.667 | 5879.032 | 1.16× | 1.145–1.165 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.411 | 3.913 | 9.517× | 9.408–9.739 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 0.407 | 3.920 | 9.631× | 9.45–9.922 | clifft |
| pure_surface_d7_r7_p1e-3/64/strict | 8.673 | 28.785 | 3.319× | 3.29–3.835 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 8.527 | 33.860 | 3.971× | 3.253–4.016 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 133.368 | 158.585 | 1.189× | 1.172–1.21 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 130.823 | 158.701 | 1.213× | 1.178–1.224 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.864 | 4.365 | 5.054× | 5.002–5.237 | clifft |
| pure_surface_d9_r9_p1e-3/1/fused | 0.850 | 4.355 | 5.126× | 5.048–5.27 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 16.873 | 53.691 | 3.182× | 3.078–3.655 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 16.819 | 53.075 | 3.156× | 3.068–3.229 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 263.990 | 324.284 | 1.228× | 1.211–1.274 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 264.182 | 324.700 | 1.229× | 1.224–1.279 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
