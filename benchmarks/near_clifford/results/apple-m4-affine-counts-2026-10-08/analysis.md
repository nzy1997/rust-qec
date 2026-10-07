# Verified near-Clifford diagnostic measurements

Measured source: `3ef49cf9c3f156a492682d24e392915510f2a3a8`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
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
| msc_d3_inject_cultivate_p1e-3/1/strict | 1.743 | 2.810 | 1.612× | 1.555–1.641 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1/fused | 1.775 | 2.763 | 1.557× | 1.546–1.606 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/64/strict | 12.441 | 18.960 | 1.524× | 1.5–1.545 | clifft |
| msc_d3_inject_cultivate_p1e-3/64/fused | 12.714 | 19.304 | 1.518× | 1.498–1.536 | clifft-scheduled |
| msc_d3_inject_cultivate_p1e-3/1024/strict | 199.329 | 200.800 | 1.007× | 0.9833–1.018 | clifft |
| msc_d3_inject_cultivate_p1e-3/1024/fused | 195.285 | 197.793 | 1.013× | 0.9992–1.078 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/strict | 13.811 | 8.515 | 0.6166× | 0.6129–0.635 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1/fused | 12.016 | 8.454 | 0.7035× | 0.6956–0.7238 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/strict | 528.488 | 373.399 | 0.7065× | 0.6763–0.7126 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/64/fused | 404.331 | 370.789 | 0.917× | 0.9074–0.9606 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/strict | 8657.035 | 5877.991 | 0.679× | 0.6676–0.6992 | clifft-scheduled |
| msc_d5_inject_cultivate_p1e-3/1024/fused | 6520.578 | 5919.852 | 0.9079× | 0.8882–0.916 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/1/strict | 0.408 | 3.885 | 9.53× | 9.227–9.731 | clifft |
| pure_surface_d7_r7_p1e-3/1/fused | 0.410 | 3.902 | 9.506× | 9.375–9.831 | clifft-scheduled |
| pure_surface_d7_r7_p1e-3/64/strict | 8.500 | 29.733 | 3.498× | 3.398–3.902 | symft |
| pure_surface_d7_r7_p1e-3/64/fused | 8.696 | 29.083 | 3.344× | 3.265–3.938 | symft |
| pure_surface_d7_r7_p1e-3/1024/strict | 132.786 | 158.945 | 1.197× | 1.166–1.221 | symft |
| pure_surface_d7_r7_p1e-3/1024/fused | 133.884 | 157.914 | 1.179× | 1.177–1.206 | symft |
| pure_surface_d9_r9_p1e-3/1/strict | 0.909 | 4.574 | 5.029× | 4.886–5.188 | clifft |
| pure_surface_d9_r9_p1e-3/1/fused | 0.909 | 4.520 | 4.973× | 4.882–5.203 | clifft |
| pure_surface_d9_r9_p1e-3/64/strict | 17.548 | 53.997 | 3.077× | 3.033–3.494 | symft |
| pure_surface_d9_r9_p1e-3/64/fused | 17.165 | 53.556 | 3.12× | 3.036–3.707 | symft |
| pure_surface_d9_r9_p1e-3/1024/strict | 275.475 | 339.209 | 1.231× | 1.198–1.27 | symft |
| pure_surface_d9_r9_p1e-3/1024/fused | 272.656 | 335.484 | 1.23× | 1.206–1.26 | symft |

Counts contract: all-zero raw detector postselection and XOR-folded raw observable0, no reference normalization.
Rust may lazily prepare an affine detector/observable model on rotation-free plans; construction is included in first_ns. Other plans retain scalar/packed early rejection.
This is separate from raw-record throughput. Accepted rates are in warm.csv; sparse logical errors do not certify conditional accuracy.
Accepted throughput pools accepted shots/time within each process, then takes the median across processes; zero survivors are retained.
Original-circuit capability failures are retained in capability.csv and events, without gate lowering.
