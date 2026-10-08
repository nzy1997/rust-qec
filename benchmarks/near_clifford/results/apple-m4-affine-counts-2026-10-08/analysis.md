# Verified near-Clifford diagnostic measurements

Measured source: `469fb50c156d91d89e63118c749927e99a8e382e`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 658, "retained_failure_events": 14, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `None`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": null}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Raw compile/prepare/first-call metadata is retained; this counts campaign has no derived lifecycle comparison. Use a dedicated fresh-seed cold campaign for phase comparisons.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.812 | 2.886 | 1.593× | 1.565–1.604 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.784 | 2.884 | 1.616× | 1.585–1.634 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 12.556 | 18.952 | 1.509× | 1.5–1.515 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 12.521 | 18.988 | 1.517× | 1.495–1.534 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 198.435 | 197.525 | 0.9954× | 0.9889–1.003 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 197.429 | 197.449 | 1× | 0.9971–1.005 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 13.722 | 8.543 | 0.6226× | 0.6126–0.6271 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 12.020 | 8.552 | 0.7115× | 0.7025–0.7217 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 529.945 | 366.394 | 0.6914× | 0.6664–0.7174 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 406.323 | 372.534 | 0.9168× | 0.8874–0.9279 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 8450.292 | 5924.607 | 0.7011× | 0.6756–0.7216 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 6572.479 | 5894.519 | 0.8968× | 0.8887–0.9338 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.428 | 4.092 | 9.559× | 9.477–9.815 | clifft |
| pure_surface_d7_r7_p1e-3/1/fused | 0.428 | 4.034 | 9.426× | 9.321–9.744 | clifft |
| pure_surface_d7_r7_p1e-3/64/strict | 8.924 | 32.820 | 3.678× | 3.652–3.823 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 8.610 | 33.089 | 3.843× | 3.662–4.189 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 134.472 | 159.731 | 1.188× | 1.186–1.208 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 134.318 | 159.268 | 1.186× | 1.182–1.209 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.912 | 4.542 | 4.978× | 4.862–5.12 | clifft |
| pure_surface_d9_r9_p1e-3/1/fused | 0.881 | 4.489 | 5.096× | 4.909–5.155 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 17.834 | 59.208 | 3.32× | 3.274–3.465 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 18.087 | 60.755 | 3.359× | 3.274–4.089 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 275.362 | 340.272 | 1.236× | 1.228–1.252 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 275.824 | 338.939 | 1.229× | 1.225–1.259 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
