# Verified near-Clifford diagnostic measurements

Measured source: `d989531d837fe874a77ac05745f5d77ea5639d52`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 0.883 | 2.878 | 3.258× | 3.21–3.29 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 0.875 | 2.882 | 3.293× | 3.256–3.347 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 12.820 | 19.410 | 1.514× | 1.504–1.528 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 12.784 | 19.346 | 1.513× | 1.5–1.564 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 201.670 | 202.618 | 1.005× | 1.001–1.032 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 203.417 | 202.575 | 0.9959× | 0.9906–1.033 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 5.896 | 8.809 | 1.494× | 1.489–1.518 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 5.364 | 8.839 | 1.648× | 1.602–1.666 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 272.283 | 377.229 | 1.385× | 1.351–1.397 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 244.815 | 379.819 | 1.551× | 1.484–1.591 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 4702.773 | 6039.495 | 1.284× | 1.279–1.285 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 4168.914 | 6040.625 | 1.449× | 1.444–1.454 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.429 | 4.180 | 9.747× | 9.004–10.05 | clifft |
| pure_surface_d7_r7_p1e-3/1/fused | 0.426 | 4.111 | 9.642× | 9.455–9.686 | clifft |
| pure_surface_d7_r7_p1e-3/64/strict | 8.819 | 31.476 | 3.569× | 3.324–3.949 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 8.716 | 28.922 | 3.318× | 3.303–3.338 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 137.120 | 163.799 | 1.195× | 1.188–1.203 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 137.738 | 162.348 | 1.179× | 1.177–1.187 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.873 | 4.412 | 5.056× | 4.984–5.227 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 0.882 | 4.390 | 4.979× | 4.869–5.112 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 17.359 | 59.972 | 3.455× | 3.374–3.789 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 17.306 | 58.909 | 3.404× | 3.36–3.446 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 264.189 | 328.885 | 1.245× | 1.224–1.256 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 265.166 | 328.176 | 1.238× | 1.231–1.242 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
