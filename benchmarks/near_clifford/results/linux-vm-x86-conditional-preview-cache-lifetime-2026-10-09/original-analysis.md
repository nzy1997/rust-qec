# Verified near-Clifford diagnostic measurements

Measured source: `bacdcfa3a0c86619087a0f30944f08e2c4527974`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
Verification: `{"events": 1273, "lifetime_comparison_keys": 560, "retained_failure_events": 0, "selected_cells": 24, "valid_cells": 24}`.
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
| msc5/s1/c0/strict | 67.151 | 33.540 | 0.4995× | 0.488–0.5061 | clifft-scheduled |
| msc5/s1/c1048576/strict | 66.142 | 33.580 | 0.5077× | 0.5039–0.5135 | clifft-scheduled |
| msc5/s1/c16777216/strict | 63.753 | 33.693 | 0.5285× | 0.525–0.5301 | clifft-scheduled |
| msc5/s1/c67108864/strict | 59.450 | 33.654 | 0.5661× | 0.5623–0.5767 | clifft-scheduled |
| msc5/s64/c0/strict | 2947.772 | 1774.710 | 0.6021× | 0.6001–0.6058 | symft |
| msc5/s64/c1048576/strict | 2828.317 | 1769.385 | 0.6256× | 0.6192–0.6301 | symft |
| msc5/s64/c16777216/strict | 2716.552 | 1763.378 | 0.6491× | 0.6437–0.6533 | symft |
| msc5/s64/c67108864/strict | 2725.051 | 1768.174 | 0.6489× | 0.6369–0.6537 | symft |
| msc5/s1024/c0/strict | 46971.251 | 28914.095 | 0.6156× | 0.6075–0.6233 | clifft-scheduled |
| msc5/s1024/c1048576/strict | 43641.498 | 28905.628 | 0.6623× | 0.6599–0.6688 | clifft-scheduled |
| msc5/s1024/c16777216/strict | 43504.497 | 29216.747 | 0.6716× | 0.6608–0.6774 | clifft-scheduled |
| msc5/s1024/c67108864/strict | 44108.171 | 29051.862 | 0.6587× | 0.6486–0.6652 | clifft-scheduled |
| msc5/s1/c0/fused | 69.375 | 33.661 | 0.4852× | 0.4852–0.4887 | clifft-scheduled |
| msc5/s1/c1048576/fused | 68.438 | 33.615 | 0.4912× | 0.4892–0.4982 | clifft-scheduled |
| msc5/s1/c16777216/fused | 66.149 | 33.854 | 0.5118× | 0.4914–0.5121 | clifft-scheduled |
| msc5/s1/c67108864/fused | 60.850 | 33.568 | 0.5516× | 0.5422–0.5549 | clifft-scheduled |
| msc5/s64/c0/fused | 2197.937 | 1769.759 | 0.8052× | 0.7992–0.8119 | symft |
| msc5/s64/c1048576/fused | 2048.414 | 1768.537 | 0.8634× | 0.8541–0.8726 | symft |
| msc5/s64/c16777216/fused | 1926.012 | 1770.344 | 0.9192× | 0.901–0.9306 | symft |
| msc5/s64/c67108864/fused | 1999.818 | 1769.999 | 0.8851× | 0.8478–0.8964 | symft |
| msc5/s1024/c0/fused | 34949.733 | 29466.493 | 0.8431× | 0.8176–0.8603 | clifft-scheduled |
| msc5/s1024/c1048576/fused | 31359.539 | 29081.345 | 0.9274× | 0.9203–0.9524 | clifft-scheduled |
| msc5/s1024/c16777216/fused | 30809.228 | 28895.345 | 0.9379× | 0.9307–0.9488 | clifft-scheduled |
| msc5/s1024/c67108864/fused | 31854.258 | 28997.591 | 0.9103× | 0.9057–0.9273 | clifft-scheduled |
