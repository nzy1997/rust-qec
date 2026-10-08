# Verified near-Clifford diagnostic measurements

Measured source: `d0e053c32155811327da2b1a0505ae2aaac0560a`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 0.859 | 2.845 | 3.313× | 3.241–3.381 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 0.856 | 2.872 | 3.353× | 3.282–3.433 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 12.977 | 19.381 | 1.493× | 1.484–1.503 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 12.933 | 19.365 | 1.497× | 1.488–1.507 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 203.925 | 202.860 | 0.9948× | 0.9915–1.051 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 203.731 | 202.575 | 0.9943× | 0.9853–0.9993 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 7.382 | 8.924 | 1.209× | 1.188–1.241 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 5.205 | 8.514 | 1.636× | 1.561–1.677 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 419.584 | 370.605 | 0.8833× | 0.8809–0.8866 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 292.253 | 370.464 | 1.268× | 1.257–1.293 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 7033.614 | 5910.630 | 0.8403× | 0.8122–0.8406 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 5079.317 | 5898.949 | 1.161× | 1.144–1.183 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.415 | 3.950 | 9.508× | 9.37–9.527 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 0.416 | 3.939 | 9.48× | 9.425–9.625 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 8.737 | 29.087 | 3.329× | 3.273–3.961 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 8.744 | 34.113 | 3.901× | 3.275–3.963 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 133.260 | 159.150 | 1.194× | 1.185–1.201 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 133.564 | 159.390 | 1.193× | 1.185–1.195 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.912 | 4.598 | 5.04× | 4.406–5.158 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 0.895 | 4.541 | 5.071× | 5.002–5.251 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 18.043 | 55.777 | 3.091× | 3.049–3.533 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 18.066 | 61.333 | 3.395× | 3.08–3.552 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 274.126 | 344.927 | 1.258× | 1.257–1.269 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 273.869 | 344.713 | 1.259× | 1.243–1.279 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
