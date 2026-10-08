# Verified near-Clifford diagnostic measurements

Measured source: `bacdcfa3a0c86619087a0f30944f08e2c4527974`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.845 | 7.243 | 3.926× | 3.894–3.941 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.843 | 7.211 | 3.912× | 3.844–3.93 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 29.250 | 39.400 | 1.347× | 1.34–1.357 | symft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 29.250 | 39.413 | 1.347× | 1.342–1.356 | symft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 461.045 | 444.664 | 0.9645× | 0.9598–0.9759 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 460.863 | 445.457 | 0.9666× | 0.9627–0.9686 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 9.699 | 14.808 | 1.527× | 1.509–1.543 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 9.876 | 14.789 | 1.498× | 1.473–1.519 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 437.862 | 494.671 | 1.13× | 1.122–1.153 | symft |
| msc_d5_inject_cultivate_p1e-3/64/fused | 447.666 | 492.750 | 1.101× | 1.041–1.109 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 7481.563 | 7391.509 | 0.988× | 0.9788–0.9953 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 7749.964 | 7427.384 | 0.9584× | 0.9516–0.9727 | symft |
| pure_surface_d7_r7_p1e-3/1/strict | 0.907 | 9.137 | 10.08× | 9.999–10.25 | clifft |
| pure_surface_d7_r7_p1e-3/1/fused | 0.908 | 9.155 | 10.08× | 9.998–10.12 | clifft |
| pure_surface_d7_r7_p1e-3/64/strict | 17.610 | 52.866 | 3.002× | 2.971–3.027 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/fused | 17.552 | 52.775 | 3.007× | 2.99–3.064 | clifft |
| pure_surface_d7_r7_p1e-3/1024/strict | 270.594 | 327.780 | 1.211× | 1.198–1.226 | clifft |
| pure_surface_d7_r7_p1e-3/1024/fused | 269.196 | 327.442 | 1.216× | 1.212–1.222 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/strict | 1.966 | 10.766 | 5.475× | 5.446–5.52 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 1.969 | 10.907 | 5.54× | 5.441–5.68 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 38.078 | 105.264 | 2.764× | 2.747–2.78 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/fused | 37.962 | 104.916 | 2.764× | 2.751–2.797 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1024/strict | 574.056 | 739.074 | 1.287× | 1.284–1.32 | clifft |
| pure_surface_d9_r9_p1e-3/1024/fused | 570.226 | 740.481 | 1.299× | 1.282–1.301 | clifft-scheduled |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
