# Verified near-Clifford diagnostic measurements

Measured source: `8dac954b402518d620669124a4d2852ae17ed9bf`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
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
| msc5/s1/c0/strict | 144.891 | 33.694 | 0.2325× | 0.2304–0.2433 | clifft-scheduled |
| msc5/s1/c1048576/strict | 145.632 | 33.906 | 0.2328× | 0.2316–0.2334 | clifft-scheduled |
| msc5/s1/c16777216/strict | 138.096 | 33.702 | 0.2441× | 0.2399–0.2462 | clifft-scheduled |
| msc5/s1/c67108864/strict | 122.227 | 33.504 | 0.2741× | 0.27–0.2771 | clifft-scheduled |
| msc5/s64/c0/strict | 2823.670 | 1817.978 | 0.6438× | 0.6274–0.6579 | symft |
| msc5/s64/c1048576/strict | 2846.030 | 1836.814 | 0.6454× | 0.6292–0.8267 | symft |
| msc5/s64/c16777216/strict | 2763.454 | 1818.368 | 0.658× | 0.6295–0.6698 | symft |
| msc5/s64/c67108864/strict | 2755.122 | 1821.635 | 0.6612× | 0.6286–0.6705 | symft |
| msc5/s1024/c0/strict | 45379.599 | 29113.266 | 0.6415× | 0.6355–0.6484 | clifft-scheduled |
| msc5/s1024/c1048576/strict | 41557.524 | 29344.970 | 0.7061× | 0.7015–0.7172 | clifft-scheduled |
| msc5/s1024/c16777216/strict | 45911.929 | 29596.127 | 0.6446× | 0.6276–0.6593 | clifft-scheduled |
| msc5/s1024/c67108864/strict | 45859.383 | 29302.271 | 0.639× | 0.6324–0.6473 | clifft-scheduled |
| msc5/s1/c0/fused | 74.377 | 33.797 | 0.4544× | 0.4496–0.4648 | clifft-scheduled |
| msc5/s1/c1048576/fused | 73.405 | 33.719 | 0.4594× | 0.4566–0.4657 | clifft-scheduled |
| msc5/s1/c16777216/fused | 70.841 | 33.964 | 0.4794× | 0.4782–0.4803 | clifft-scheduled |
| msc5/s1/c67108864/fused | 64.435 | 33.818 | 0.5248× | 0.52–0.5291 | clifft-scheduled |
| msc5/s64/c0/fused | 2073.848 | 1826.929 | 0.8809× | 0.8645–0.8877 | symft |
| msc5/s64/c1048576/fused | 2074.246 | 1812.642 | 0.8739× | 0.8625–0.8834 | symft |
| msc5/s64/c16777216/fused | 2007.186 | 1834.874 | 0.9142× | 0.9062–1.16 | symft |
| msc5/s64/c67108864/fused | 1954.775 | 1812.862 | 0.9274× | 0.9012–0.936 | symft |
| msc5/s1024/c0/fused | 33595.166 | 29310.857 | 0.8725× | 0.8576–0.8893 | clifft-scheduled |
| msc5/s1024/c1048576/fused | 28981.743 | 29370.766 | 1.013× | 1.001–1.049 | clifft-scheduled |
| msc5/s1024/c16777216/fused | 32135.075 | 29483.133 | 0.9175× | 0.8928–0.9383 | clifft-scheduled |
| msc5/s1024/c67108864/fused | 32509.278 | 29286.885 | 0.9009× | 0.8909–0.9421 | clifft-scheduled |
