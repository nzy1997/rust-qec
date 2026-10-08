# Verified near-Clifford diagnostic measurements

Measured source: `d0e053c32155811327da2b1a0505ae2aaac0560a`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.093 | 4.041 | 3.698× | 3.633–3.791 | clifft |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.089 | 4.007 | 3.681× | 3.627–3.71 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/strict | 17.361 | 23.537 | 1.356× | 1.332–1.379 | symft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 17.325 | 23.109 | 1.334× | 1.312–1.356 | symft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 273.485 | 277.801 | 1.016× | 1.012–1.031 | symft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 270.925 | 277.038 | 1.023× | 0.9955–1.036 | symft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 9.182 | 6.649 | 0.7241× | 0.7093–0.7757 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 5.861 | 6.624 | 1.13× | 1.107–1.15 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 572.974 | 197.814 | 0.3452× | 0.3287–0.3579 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 353.201 | 195.220 | 0.5527× | 0.5455–0.5748 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 9975.435 | 3106.087 | 0.3114× | 0.3083–0.3149 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 6112.135 | 3105.988 | 0.5082× | 0.5053–0.533 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.446 | 4.842 | 10.85× | 10.84–11.14 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 0.437 | 4.828 | 11.05× | 10.45–11.71 | clifft |
| pure_surface_d7_r7_p1e-3/64/strict | 11.445 | 29.054 | 2.539× | 2.499–2.71 | clifft |
| pure_surface_d7_r7_p1e-3/64/fused | 11.165 | 29.132 | 2.609× | 2.545–2.633 | clifft |
| pure_surface_d7_r7_p1e-3/1024/strict | 178.495 | 213.465 | 1.196× | 1.162–1.215 | clifft |
| pure_surface_d7_r7_p1e-3/1024/fused | 178.146 | 211.732 | 1.189× | 1.175–1.232 | clifft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.989 | 5.658 | 5.722× | 5.585–5.899 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 0.993 | 5.754 | 5.794× | 5.558–5.965 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 22.850 | 60.023 | 2.627× | 2.574–2.632 | clifft |
| pure_surface_d9_r9_p1e-3/64/fused | 23.043 | 58.827 | 2.553× | 2.49–2.632 | clifft |
| pure_surface_d9_r9_p1e-3/1024/strict | 358.427 | 464.893 | 1.297× | 1.291–1.326 | clifft |
| pure_surface_d9_r9_p1e-3/1024/fused | 354.812 | 478.429 | 1.348× | 1.328–1.412 | clifft-scheduled |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
