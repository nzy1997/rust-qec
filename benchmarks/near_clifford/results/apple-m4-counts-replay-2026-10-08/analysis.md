# Verified near-Clifford diagnostic measurements

Measured source: `53cfe45e7375292a76dcdc0db2c4e3e455939154`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.648 | 2.751 | 1.669× | 1.643–1.746 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.679 | 2.753 | 1.64× | 1.632–1.68 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 12.446 | 18.676 | 1.501× | 1.481–1.513 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 12.537 | 18.787 | 1.499× | 1.488–1.52 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 197.088 | 196.958 | 0.9993× | 0.9906–1.032 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 198.602 | 197.331 | 0.9936× | 0.9887–1.005 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 11.282 | 8.593 | 0.7616× | 0.7536–0.7816 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 9.440 | 8.494 | 0.8998× | 0.8939–0.9097 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 435.451 | 366.391 | 0.8414× | 0.8355–0.8489 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 305.482 | 366.516 | 1.2× | 1.189–1.214 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 7227.738 | 5867.241 | 0.8118× | 0.8107–0.8199 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 5097.792 | 5874.616 | 1.152× | 1.145–1.156 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.428 | 4.103 | 9.58× | 9.471–9.949 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 0.429 | 4.096 | 9.54× | 9.445–9.907 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 8.753 | 30.449 | 3.479× | 3.277–3.992 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 9.092 | 30.081 | 3.308× | 3.205–3.638 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 135.456 | 162.030 | 1.196× | 1.178–1.206 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 134.407 | 161.756 | 1.203× | 1.177–1.209 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.899 | 4.571 | 5.086× | 4.954–5.246 | clifft |
| pure_surface_d9_r9_p1e-3/1/fused | 0.886 | 4.535 | 5.117× | 4.983–5.248 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 18.165 | 55.544 | 3.058× | 3.044–3.502 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 17.826 | 56.154 | 3.15× | 3.103–3.522 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 272.093 | 341.799 | 1.256× | 1.239–1.256 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 276.471 | 345.325 | 1.249× | 1.231–1.271 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
