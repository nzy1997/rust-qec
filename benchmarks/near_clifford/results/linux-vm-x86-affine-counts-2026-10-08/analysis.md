# Verified near-Clifford diagnostic measurements

Measured source: `469fb50c156d91d89e63118c749927e99a8e382e`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 3.372 | 7.093 | 2.104× | 2.084–2.145 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 3.349 | 7.090 | 2.117× | 2.1–2.183 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 28.653 | 39.633 | 1.383× | 1.382–1.388 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/fused | 28.675 | 39.590 | 1.381× | 1.38–1.384 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 450.556 | 443.714 | 0.9848× | 0.9839–0.9932 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 450.050 | 445.395 | 0.9897× | 0.9836–0.9908 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 32.115 | 14.376 | 0.4476× | 0.4354–0.4806 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 24.521 | 14.311 | 0.5836× | 0.5589–0.5922 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 1455.837 | 400.098 | 0.2748× | 0.2696–0.2792 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 962.033 | 403.658 | 0.4196× | 0.4125–0.4289 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 22674.444 | 6253.382 | 0.2758× | 0.2741–0.2843 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 15318.838 | 6324.593 | 0.4129× | 0.4068–0.4268 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.701 | 8.504 | 12.14× | 12.01–12.32 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 0.707 | 8.445 | 11.95× | 11.71–12.18 | clifft |
| pure_surface_d7_r7_p1e-3/64/strict | 16.727 | 49.857 | 2.981× | 2.935–3.077 | clifft |
| pure_surface_d7_r7_p1e-3/64/fused | 16.762 | 49.895 | 2.977× | 2.969–2.998 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1024/strict | 258.789 | 342.523 | 1.324× | 1.316–1.339 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1024/fused | 258.725 | 342.289 | 1.323× | 1.316–1.327 | clifft |
| pure_surface_d9_r9_p1e-3/1/strict | 1.668 | 9.934 | 5.954× | 5.861–6.006 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 1.672 | 9.923 | 5.936× | 5.892–5.966 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 32.539 | 106.073 | 3.26× | 3.243–3.329 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/fused | 32.674 | 105.226 | 3.221× | 3.203–3.26 | clifft |
| pure_surface_d9_r9_p1e-3/1024/strict | 502.034 | 846.095 | 1.685× | 1.683–1.686 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1024/fused | 504.667 | 845.395 | 1.675× | 1.656–1.704 | clifft-scheduled |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
