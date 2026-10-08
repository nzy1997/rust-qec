# Verified near-Clifford diagnostic measurements

Measured source: `aeebd87f52728c26b2e72c31ec08ed2391b40eda`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.751 | 2.790 | 1.594× | 1.574–1.625 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.771 | 2.797 | 1.579× | 1.555–1.611 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 12.558 | 18.935 | 1.508× | 1.49–1.524 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 12.439 | 18.941 | 1.523× | 1.513–1.524 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 196.246 | 198.200 | 1.01× | 1.003–1.066 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 195.914 | 198.069 | 1.011× | 1.001–1.013 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 13.727 | 8.517 | 0.6204× | 0.6176–0.6274 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 11.978 | 8.574 | 0.7158× | 0.7101–0.7203 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 532.103 | 370.023 | 0.6954× | 0.6879–0.7126 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 405.068 | 369.672 | 0.9126× | 0.9012–0.9252 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 8794.722 | 5888.431 | 0.6695× | 0.6658–0.6879 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 6410.677 | 5894.532 | 0.9195× | 0.8888–0.9304 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 8.886 | 4.072 | 0.4583× | 0.4538–0.4612 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 9.104 | 4.054 | 0.4453× | 0.4417–0.4618 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 27.194 | 33.746 | 1.241× | 1.222–1.26 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 27.450 | 34.015 | 1.239× | 1.221–1.412 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 429.350 | 164.789 | 0.3838× | 0.3804–0.3927 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 431.676 | 164.635 | 0.3814× | 0.3771–0.3895 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 17.035 | 4.572 | 0.2684× | 0.2663–0.2791 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 16.981 | 4.592 | 0.2704× | 0.2684–0.2724 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/64/strict | 50.801 | 55.291 | 1.088× | 1.07–1.208 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 50.721 | 54.131 | 1.067× | 1.063–1.222 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 778.153 | 328.427 | 0.4221× | 0.4079–0.4259 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 827.171 | 338.292 | 0.409× | 0.3999–0.4127 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust uses native counts with scalar/admission-fallback rejection and packed detector retirement; packed annotations reduce counts during execution.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
