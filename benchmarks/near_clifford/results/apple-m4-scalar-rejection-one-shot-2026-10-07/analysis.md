# Verified near-Clifford diagnostic measurements

Measured source: `376cfc6f7503575d2fbde0ac3eb2a67ee1c8b871`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 234, "retained_failure_events": 14, "selected_cells": 8, "valid_cells": 8}`.
Complete timing comparisons: `8/8` selected cells; verifier valid_cells counts finite-witness acceptance.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.937 | 2.844 | 1.468× | 1.445–1.488 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.955 | 2.852 | 1.459× | 1.435–1.492 | clifft |
| msc_d5_inject_cultivate_p1e-3/1/strict | 17.442 | 8.650 | 0.4959× | 0.4922–0.5022 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 15.750 | 8.788 | 0.558× | 0.5422–0.5811 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 11.213 | 3.926 | 0.3501× | 0.345–0.3548 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/fused | 11.217 | 3.964 | 0.3534× | 0.3447–0.3594 | clifft |
| pure_surface_d9_r9_p1e-3/1/strict | 22.403 | 4.443 | 0.1983× | 0.1962–0.2013 | clifft-scheduled |
| pure_surface_d9_r9_p1e-3/1/fused | 22.433 | 4.412 | 0.1967× | 0.1948–0.2016 | clifft-scheduled |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust uses native counts with scalar/admission-fallback early rejection; live packed lanes complete simulation.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
