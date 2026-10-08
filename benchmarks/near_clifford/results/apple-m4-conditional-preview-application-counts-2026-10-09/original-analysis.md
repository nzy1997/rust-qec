# Verified near-Clifford diagnostic measurements

Measured source: `bacdcfa3a0c86619087a0f30944f08e2c4527974`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 0.889 | 2.901 | 3.265× | 3.241–3.369 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 0.889 | 2.928 | 3.294× | 3.201–3.316 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 13.254 | 20.018 | 1.51× | 1.468–1.524 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 13.393 | 20.037 | 1.496× | 1.48–1.574 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 198.663 | 200.440 | 1.009× | 0.9903–1.049 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 197.035 | 198.707 | 1.008× | 0.9843–1.037 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 5.650 | 8.569 | 1.516× | 1.492–1.53 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 5.243 | 8.501 | 1.621× | 1.615–1.652 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 263.947 | 368.080 | 1.395× | 1.386–1.407 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 233.019 | 369.022 | 1.584× | 1.572–1.604 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 4519.722 | 5914.634 | 1.309× | 1.29–1.316 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 3985.968 | 5887.195 | 1.477× | 1.472–1.486 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.417 | 3.990 | 9.56× | 9.431–9.679 | clifft |
| pure_surface_d7_r7_p1e-3/1/fused | 0.413 | 3.937 | 9.523× | 9.374–9.681 | clifft |
| pure_surface_d7_r7_p1e-3/64/strict | 8.660 | 32.773 | 3.784× | 3.739–4.283 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 8.633 | 32.567 | 3.772× | 3.717–3.798 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 132.090 | 160.350 | 1.214× | 1.18–1.218 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 132.412 | 160.099 | 1.209× | 1.191–1.214 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.875 | 4.453 | 5.09× | 4.986–5.229 | clifft |
| pure_surface_d9_r9_p1e-3/1/fused | 0.891 | 4.430 | 4.97× | 4.731–5.091 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 17.841 | 61.155 | 3.428× | 3.374–3.942 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 17.761 | 60.685 | 3.417× | 3.406–3.499 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 262.160 | 329.401 | 1.256× | 1.229–1.26 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 261.420 | 328.280 | 1.256× | 1.224–1.262 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
