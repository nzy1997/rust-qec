# Verified near-Clifford diagnostic measurements

Measured source: `0177cf3d4c2b988b35c692cc9a5098a3bf4f322f`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 3.262 | 7.289 | 2.234× | 2.209–2.295 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 3.257 | 7.267 | 2.231× | 2.206–2.252 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 29.282 | 39.388 | 1.345× | 1.344–1.347 | symft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 29.338 | 39.368 | 1.342× | 1.33–1.361 | symft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 460.084 | 445.470 | 0.9682× | 0.9627–0.9766 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 461.222 | 447.027 | 0.9692× | 0.9644–0.9693 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 24.740 | 14.780 | 0.5974× | 0.5827–0.6029 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 19.989 | 14.775 | 0.7391× | 0.7271–0.7568 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 1040.076 | 511.176 | 0.4915× | 0.4915–0.5049 | symft |
| msc_d5_inject_cultivate_p1e-3/64/fused | 660.398 | 512.348 | 0.7758× | 0.7714–0.783 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 17451.366 | 7870.393 | 0.451× | 0.448–0.455 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 11521.060 | 7859.702 | 0.6822× | 0.6577–0.721 | symft |
| pure_surface_d7_r7_p1e-3/1/strict | 0.864 | 9.258 | 10.72× | 10.65–10.74 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 0.865 | 9.330 | 10.79× | 10.71–11.14 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 17.559 | 53.485 | 3.046× | 2.979–3.069 | clifft |
| pure_surface_d7_r7_p1e-3/64/fused | 17.545 | 53.031 | 3.023× | 3.003–3.044 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1024/strict | 270.325 | 333.490 | 1.234× | 1.224–1.264 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1024/fused | 268.337 | 333.218 | 1.242× | 1.224–1.246 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/strict | 1.852 | 11.039 | 5.962× | 5.922–6.156 | clifft |
| pure_surface_d9_r9_p1e-3/1/fused | 1.848 | 11.002 | 5.952× | 5.892–6.103 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 38.074 | 105.768 | 2.778× | 2.754–2.791 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/fused | 38.348 | 105.777 | 2.758× | 2.754–2.833 | clifft |
| pure_surface_d9_r9_p1e-3/1024/strict | 573.739 | 753.838 | 1.314× | 1.305–1.33 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1024/fused | 575.768 | 753.542 | 1.309× | 1.286–1.326 | clifft-scheduled |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
