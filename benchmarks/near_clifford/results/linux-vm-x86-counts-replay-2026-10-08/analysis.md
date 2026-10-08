# Verified near-Clifford diagnostic measurements

Measured source: `53cfe45e7375292a76dcdc0db2c4e3e455939154`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
Verification: `{"events": 658, "retained_failure_events": 14, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `[0]`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": "-C target-cpu=native"}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
This counts campaign records no lifecycle phase measurements.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| msc_d3_inject_cultivate_p1e-3/1/strict | 3.306 | 7.314 | 2.213× | 2.198–2.223 | clifft |
| msc_d3_inject_cultivate_p1e-3/1/fused | 3.285 | 7.285 | 2.218× | 2.2–2.246 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 29.376 | 39.227 | 1.335× | 1.317–1.351 | symft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 29.315 | 39.201 | 1.337× | 1.336–1.365 | symft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 462.042 | 445.478 | 0.9642× | 0.9582–0.9718 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 463.995 | 446.877 | 0.9631× | 0.96–0.9644 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 25.214 | 14.951 | 0.593× | 0.5808–0.5996 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 19.942 | 14.830 | 0.7437× | 0.7351–0.7723 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 1051.442 | 511.451 | 0.4864× | 0.4487–0.4934 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 664.700 | 510.111 | 0.7674× | 0.7589–0.7805 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 16965.799 | 7731.079 | 0.4557× | 0.4508–0.4629 | symft |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 10863.069 | 7768.625 | 0.7151× | 0.706–0.7275 | symft |
| pure_surface_d7_r7_p1e-3/1/strict | 0.860 | 9.293 | 10.81× | 10.66–11.03 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 0.863 | 9.253 | 10.72× | 10.63–11.05 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 17.625 | 52.928 | 3.003× | 2.994–3.117 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/fused | 17.638 | 53.154 | 3.014× | 2.966–3.059 | clifft |
| pure_surface_d7_r7_p1e-3/1024/strict | 267.973 | 327.661 | 1.223× | 1.212–1.227 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1024/fused | 268.421 | 327.995 | 1.222× | 1.207–1.229 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/strict | 1.841 | 11.065 | 6.01× | 5.98–6.105 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 1.839 | 11.116 | 6.045× | 5.95–6.153 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 37.914 | 105.604 | 2.785× | 2.749–2.79 | clifft |
| pure_surface_d9_r9_p1e-3/64/fused | 37.858 | 105.356 | 2.783× | 2.769–2.803 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1024/strict | 571.423 | 738.150 | 1.292× | 1.282–1.303 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1024/fused | 571.920 | 741.663 | 1.297× | 1.291–1.314 | clifft-scheduled |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
