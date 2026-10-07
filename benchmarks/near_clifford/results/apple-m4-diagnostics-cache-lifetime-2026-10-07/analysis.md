# Verified near-Clifford diagnostic measurements

Measured source: `b8db06741c8dea964f4ef59cbf14ca6ddebf4d95`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 1273, "lifetime_comparison_keys": 560, "retained_failure_events": 0, "selected_cells": 24, "valid_cells": 24}`.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Mac measurements have no pinned-core claim.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Lifecycle phase sums exclude diagnostic conversion and destruction; they are not end-to-end wall-clock time.
See warm.csv for all backends, cold phases, actual rank, throughput and RSS; lifecycle.csv for every history/budget.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| msc5/s1/c0/strict | 61.828 | 26.342 | 0.426× | 0.424–0.430 | clifft-scheduled |
| msc5/s1/c1048576/strict | 62.000 | 26.410 | 0.426× | 0.423–0.429 | clifft-scheduled |
| msc5/s1/c16777216/strict | 61.675 | 26.576 | 0.431× | 0.207–0.527 | clifft-scheduled |
| msc5/s1/c67108864/strict | 53.224 | 26.739 | 0.502× | 0.449–0.526 | clifft-scheduled |
| msc5/s64/c0/strict | 1607.785 | 1236.810 | 0.769× | 0.511–0.791 | clifft-scheduled |
| msc5/s64/c1048576/strict | 1638.625 | 1246.922 | 0.761× | 0.712–1.194 | clifft-scheduled |
| msc5/s64/c16777216/strict | 1635.028 | 1233.400 | 0.754× | 0.752–0.762 | clifft-scheduled |
| msc5/s64/c67108864/strict | 1697.078 | 1303.556 | 0.768× | 0.740–0.791 | clifft-scheduled |
| msc5/s1024/c0/strict | 31859.417 | 23033.708 | 0.723× | 0.712–0.821 | clifft-scheduled |
| msc5/s1024/c1048576/strict | 25869.500 | 22574.875 | 0.873× | 0.726–1.107 | clifft |
| msc5/s1024/c16777216/strict | 31587.188 | 23513.681 | 0.744× | 0.609–0.784 | clifft-scheduled |
| msc5/s1024/c67108864/strict | 31394.250 | 23295.667 | 0.742× | 0.719–0.818 | clifft |
| msc5/s1/c0/fused | 39.696 | 26.890 | 0.677× | 0.673–0.708 | clifft-scheduled |
| msc5/s1/c1048576/fused | 39.352 | 27.012 | 0.686× | 0.600–0.696 | clifft-scheduled |
| msc5/s1/c16777216/fused | 38.537 | 27.350 | 0.710× | 0.532–0.797 | clifft-scheduled |
| msc5/s1/c67108864/fused | 34.067 | 26.844 | 0.788× | 0.600–0.792 | clifft-scheduled |
| msc5/s64/c0/fused | 1416.606 | 1246.569 | 0.880× | 0.864–1.054 | clifft-scheduled |
| msc5/s64/c1048576/fused | 1411.517 | 1260.403 | 0.893× | 0.879–0.909 | clifft-scheduled |
| msc5/s64/c16777216/fused | 1523.828 | 1343.094 | 0.881× | 0.881–0.916 | clifft |
| msc5/s64/c67108864/fused | 1628.546 | 1453.975 | 0.893× | 0.815–1.024 | clifft-scheduled |
| msc5/s1024/c0/fused | 23171.208 | 20037.986 | 0.865× | 0.734–0.908 | clifft-scheduled |
| msc5/s1024/c1048576/fused | 22166.555 | 19791.292 | 0.893× | 0.873–0.916 | clifft-scheduled |
| msc5/s1024/c16777216/fused | 21398.667 | 19240.347 | 0.899× | 0.895–0.902 | clifft-scheduled |
| msc5/s1024/c67108864/fused | 21702.848 | 19358.125 | 0.892× | 0.867–0.909 | clifft-scheduled |
