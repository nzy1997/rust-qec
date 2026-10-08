# Verified near-Clifford diagnostic measurements

Measured source: `bacdcfa3a0c86619087a0f30944f08e2c4527974`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 600, "lifetime_comparison_keys": 0, "retained_failure_events": 0, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `None`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": "-C target-cpu=native"}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Lifecycle phase sums exclude diagnostic conversion and destruction; they are not end-to-end wall-clock time.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS; lifecycle.csv for every history/budget.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| rank-4/s1/c67108864/strict | 1.413 | 2.277 | 1.612× | 1.591–1.635 | clifft-scheduled |
| rank-4/s64/c67108864/strict | 22.733 | 19.899 | 0.8753× | 0.8356–0.9021 | clifft-scheduled |
| rank-4/s1024/c67108864/strict | 371.655 | 255.839 | 0.6884× | 0.4577–0.6894 | clifft-scheduled |
| rank-8/s1/c67108864/strict | 2.919 | 2.940 | 1.007× | 0.9754–1.057 | clifft-scheduled |
| rank-8/s64/c67108864/strict | 77.580 | 33.720 | 0.4347× | 0.3473–0.4453 | clifft-scheduled |
| rank-8/s1024/c67108864/strict | 1277.697 | 452.826 | 0.3544× | 0.344–0.3981 | clifft-scheduled |
| rank-12/s1/c67108864/strict | 4.044 | 3.398 | 0.8403× | 0.7916–1.187 | clifft-scheduled |
| rank-12/s64/c67108864/strict | 252.711 | 47.154 | 0.1866× | 0.1832–0.1887 | clifft-scheduled |
| rank-12/s1024/c67108864/strict | 4179.708 | 662.383 | 0.1585× | 0.1521–0.1618 | clifft-scheduled |
| rank-16/s1/c67108864/strict | 7.507 | 5.320 | 0.7086× | 0.6858–0.7263 | clifft-scheduled |
| rank-16/s64/c67108864/strict | 137.896 | 80.203 | 0.5816× | 0.5776–0.5841 | symft |
| rank-16/s1024/c67108864/strict | 2223.866 | 1061.030 | 0.4771× | 0.4694–0.4864 | symft |
| rank-4/s1/c67108864/fused | 1.499 | 2.344 | 1.564× | 1.495–1.706 | clifft-scheduled |
| rank-4/s64/c67108864/fused | 22.958 | 19.529 | 0.8506× | 0.269–0.8583 | clifft-scheduled |
| rank-4/s1024/c67108864/fused | 361.624 | 250.346 | 0.6923× | 0.6701–0.703 | clifft-scheduled |
| rank-8/s1/c67108864/fused | 2.783 | 2.892 | 1.039× | 0.9958–1.053 | clifft-scheduled |
| rank-8/s64/c67108864/fused | 86.472 | 33.871 | 0.3917× | 0.3509–0.4235 | clifft-scheduled |
| rank-8/s1024/c67108864/fused | 1344.894 | 456.697 | 0.3396× | 0.3112–0.3734 | clifft-scheduled |
| rank-12/s1/c67108864/fused | 5.459 | 3.683 | 0.6747× | 0.4019–1.086 | clifft-scheduled |
| rank-12/s64/c67108864/fused | 282.698 | 48.826 | 0.1727× | 0.1594–0.1816 | clifft-scheduled |
| rank-12/s1024/c67108864/fused | 4691.583 | 684.296 | 0.1459× | 0.1339–0.1593 | clifft-scheduled |
| rank-16/s1/c67108864/fused | 7.739 | 5.479 | 0.708× | 0.6436–0.7277 | clifft-scheduled |
| rank-16/s64/c67108864/fused | 132.380 | 83.554 | 0.6312× | 0.6074–0.6498 | symft |
| rank-16/s1024/c67108864/fused | 2086.385 | 1066.017 | 0.5109× | 0.5026–0.5226 | symft |
