# Verified near-Clifford diagnostic measurements

Measured source: `2a54d756b04b814fa1db129e27168615fb407b58`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.033 | 3.808 | 3.685× | 3.52–4.07 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.038 | 3.732 | 3.595× | 3.548–3.732 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 16.910 | 22.240 | 1.315× | 1.299–1.335 | symft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 16.818 | 22.758 | 1.353× | 1.292–1.397 | symft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 271.530 | 266.589 | 0.9818× | 0.9537–1.004 | symft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 270.654 | 265.459 | 0.9808× | 0.9706–1.017 | symft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 5.572 | 6.329 | 1.136× | 1.115–1.171 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 5.543 | 6.414 | 1.157× | 1.134–1.167 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 256.983 | 194.403 | 0.7565× | 0.7497–0.7804 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 256.218 | 190.625 | 0.744× | 0.7234–0.7966 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 4577.224 | 3083.461 | 0.6737× | 0.6563–0.6861 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 4349.010 | 3042.358 | 0.6996× | 0.6937–0.7252 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.404 | 4.734 | 11.71× | 11.56–11.87 | clifft |
| pure_surface_d7_r7_p1e-3/1/fused | 0.407 | 4.744 | 11.66× | 11.45–12.28 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 11.150 | 28.753 | 2.579× | 2.542–2.631 | clifft |
| pure_surface_d7_r7_p1e-3/64/fused | 11.246 | 28.676 | 2.55× | 2.54–2.592 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1024/strict | 175.362 | 211.307 | 1.205× | 1.199–1.216 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1024/fused | 176.261 | 210.765 | 1.196× | 1.176–1.224 | clifft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.947 | 5.495 | 5.804× | 5.598–6.207 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 0.914 | 5.371 | 5.875× | 5.716–6.027 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 22.428 | 58.441 | 2.606× | 2.269–2.734 | clifft |
| pure_surface_d9_r9_p1e-3/64/fused | 21.971 | 56.994 | 2.594× | 2.527–2.635 | clifft |
| pure_surface_d9_r9_p1e-3/1024/strict | 342.436 | 458.332 | 1.338× | 1.32–1.353 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 343.034 | 457.710 | 1.334× | 1.264–1.376 | clifft-scheduled |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
