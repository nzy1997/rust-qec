# Verified near-Clifford diagnostic measurements

Measured source: `bacdcfa3a0c86619087a0f30944f08e2c4527974`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
Verification: `{"events": 600, "lifetime_comparison_keys": 0, "retained_failure_events": 0, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `[0]`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": "-C target-cpu=native"}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Lifecycle phase sums exclude diagnostic conversion and destruction; they are not end-to-end wall-clock time.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS; lifecycle.csv for every history/budget.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| rank-4/s1/c67108864/strict | 3.347 | 5.614 | 1.677× | 1.663–1.722 | clifft-scheduled |
| rank-4/s64/c67108864/strict | 65.732 | 33.430 | 0.5086× | 0.4526–0.5675 | clifft-scheduled |
| rank-4/s1024/c67108864/strict | 965.902 | 436.920 | 0.4523× | 0.4212–0.4665 | clifft-scheduled |
| rank-8/s1/c67108864/strict | 5.981 | 6.602 | 1.104× | 1.081–1.171 | clifft-scheduled |
| rank-8/s64/c67108864/strict | 175.364 | 61.129 | 0.3486× | 0.3301–0.3637 | clifft-scheduled |
| rank-8/s1024/c67108864/strict | 2850.784 | 862.410 | 0.3025× | 0.266–0.3087 | clifft-scheduled |
| rank-12/s1/c67108864/strict | 8.271 | 7.535 | 0.911× | 0.8993–0.9448 | clifft-scheduled |
| rank-12/s64/c67108864/strict | 602.823 | 91.298 | 0.1515× | 0.1414–0.1656 | clifft-scheduled |
| rank-12/s1024/c67108864/strict | 8682.389 | 1321.796 | 0.1522× | 0.1482–0.1567 | clifft-scheduled |
| rank-16/s1/c67108864/strict | 15.771 | 9.625 | 0.6103× | 0.6047–0.6261 | clifft-scheduled |
| rank-16/s64/c67108864/strict | 257.921 | 215.693 | 0.8363× | 0.7958–0.8579 | symft |
| rank-16/s1024/c67108864/strict | 4256.598 | 2838.996 | 0.667× | 0.6568–0.686 | symft |
| rank-4/s1/c67108864/fused | 3.375 | 5.630 | 1.668× | 1.625–1.681 | clifft-scheduled |
| rank-4/s64/c67108864/fused | 63.181 | 33.407 | 0.5288× | 0.5016–0.5329 | clifft-scheduled |
| rank-4/s1024/c67108864/fused | 992.714 | 434.635 | 0.4378× | 0.4115–0.4449 | clifft-scheduled |
| rank-8/s1/c67108864/fused | 5.828 | 6.614 | 1.135× | 1.061–1.192 | clifft-scheduled |
| rank-8/s64/c67108864/fused | 178.095 | 61.550 | 0.3456× | 0.3315–0.3616 | clifft-scheduled |
| rank-8/s1024/c67108864/fused | 2940.739 | 876.008 | 0.2979× | 0.2798–0.3046 | clifft-scheduled |
| rank-12/s1/c67108864/fused | 8.069 | 7.582 | 0.9395× | 0.9058–0.9574 | clifft-scheduled |
| rank-12/s64/c67108864/fused | 540.385 | 90.773 | 0.168× | 0.1603–0.174 | clifft-scheduled |
| rank-12/s1024/c67108864/fused | 8619.192 | 1315.471 | 0.1526× | 0.1503–0.1595 | clifft-scheduled |
| rank-16/s1/c67108864/fused | 16.008 | 9.614 | 0.6006× | 0.5898–0.6035 | clifft-scheduled |
| rank-16/s64/c67108864/fused | 266.023 | 220.705 | 0.8296× | 0.8039–0.8349 | symft |
| rank-16/s1024/c67108864/fused | 4412.338 | 2836.937 | 0.643× | 0.6393–0.6941 | symft |
