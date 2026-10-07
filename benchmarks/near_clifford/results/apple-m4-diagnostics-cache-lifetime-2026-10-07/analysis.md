# Verified near-Clifford diagnostic measurements

Measured source: `b8db06741c8dea964f4ef59cbf14ca6ddebf4d95`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 1273, "lifetime_comparison_keys": 560, "retained_failure_events": 0, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `24/24` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `None`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": null}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Lifecycle phase sums exclude diagnostic conversion and destruction; they are not end-to-end wall-clock time.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS; lifecycle.csv for every history/budget.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| msc5/s1/c0/strict | 61.828 | 26.342 | 0.4261× | 0.4236–0.4297 | clifft-scheduled |
| msc5/s1/c1048576/strict | 62.000 | 26.410 | 0.426× | 0.4235–0.429 | clifft-scheduled |
| msc5/s1/c16777216/strict | 61.675 | 26.576 | 0.4309× | 0.207–0.5268 | clifft-scheduled |
| msc5/s1/c67108864/strict | 53.224 | 26.739 | 0.5024× | 0.4489–0.5256 | clifft-scheduled |
| msc5/s64/c0/strict | 1607.785 | 1236.810 | 0.7693× | 0.5111–0.7915 | clifft-scheduled |
| msc5/s64/c1048576/strict | 1638.625 | 1246.922 | 0.761× | 0.7122–1.194 | clifft-scheduled |
| msc5/s64/c16777216/strict | 1635.028 | 1233.400 | 0.7544× | 0.7516–0.7623 | clifft-scheduled |
| msc5/s64/c67108864/strict | 1697.078 | 1303.556 | 0.7681× | 0.74–0.7912 | clifft-scheduled |
| msc5/s1024/c0/strict | 31859.417 | 23033.708 | 0.723× | 0.7115–0.8211 | clifft-scheduled |
| msc5/s1024/c1048576/strict | 25869.500 | 22574.875 | 0.8726× | 0.7256–1.107 | clifft |
| msc5/s1024/c16777216/strict | 31587.188 | 23513.681 | 0.7444× | 0.6087–0.7842 | clifft-scheduled |
| msc5/s1024/c67108864/strict | 31394.250 | 23295.667 | 0.742× | 0.7194–0.8175 | clifft |
| msc5/s1/c0/fused | 39.696 | 26.890 | 0.6774× | 0.6725–0.7079 | clifft-scheduled |
| msc5/s1/c1048576/fused | 39.352 | 27.012 | 0.6864× | 0.6002–0.6963 | clifft-scheduled |
| msc5/s1/c16777216/fused | 38.537 | 27.350 | 0.7097× | 0.5322–0.7972 | clifft-scheduled |
| msc5/s1/c67108864/fused | 34.067 | 26.844 | 0.788× | 0.6–0.7919 | clifft-scheduled |
| msc5/s64/c0/fused | 1416.606 | 1246.569 | 0.88× | 0.8638–1.054 | clifft-scheduled |
| msc5/s64/c1048576/fused | 1411.517 | 1260.403 | 0.8929× | 0.8794–0.9092 | clifft-scheduled |
| msc5/s64/c16777216/fused | 1523.828 | 1343.094 | 0.8814× | 0.8806–0.916 | clifft |
| msc5/s64/c67108864/fused | 1628.546 | 1453.975 | 0.8928× | 0.8149–1.024 | clifft-scheduled |
| msc5/s1024/c0/fused | 23171.208 | 20037.986 | 0.8648× | 0.7337–0.9084 | clifft-scheduled |
| msc5/s1024/c1048576/fused | 22166.555 | 19791.292 | 0.8928× | 0.8733–0.9162 | clifft-scheduled |
| msc5/s1024/c16777216/fused | 21398.667 | 19240.347 | 0.8991× | 0.895–0.9016 | clifft-scheduled |
| msc5/s1024/c67108864/fused | 21702.848 | 19358.125 | 0.892× | 0.8671–0.9089 | clifft-scheduled |
