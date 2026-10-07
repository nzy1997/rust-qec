# Verified near-Clifford diagnostic measurements

Measured source: `a89f1189b030bc5f71073adcc3725954f14c4944`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 1800, "lifetime_comparison_keys": 0, "retained_failure_events": 0, "selected_cells": 72, "valid_cells": 72}`.
Complete timing comparisons: `72/72` selected cells; verifier valid_cells counts finite-witness acceptance.
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
| msc5/s1/c67108864/strict | 51.199 | 25.851 | 0.5049× | 0.5009–0.5078 | clifft-scheduled |
| msc5/s64/c67108864/strict | 1565.966 | 1207.729 | 0.7712× | 0.7583–0.8078 | clifft-scheduled |
| msc5/s1024/c67108864/strict | 25353.562 | 18918.389 | 0.7462× | 0.7294–0.7664 | clifft-scheduled |
| msc5/s8/c67108864/strict | 444.448 | 195.321 | 0.4395× | 0.4354–0.4413 | clifft-scheduled |
| msc5/s32/c67108864/strict | 832.509 | 660.325 | 0.7932× | 0.7904–0.7965 | clifft-scheduled |
| msc5/s63/c67108864/strict | 1753.681 | 1214.676 | 0.6926× | 0.6679–0.695 | clifft-scheduled |
| msc5/s65/c67108864/strict | 1742.777 | 1258.621 | 0.7222× | 0.7173–0.724 | clifft-scheduled |
| msc5/s256/c67108864/strict | 6356.943 | 4819.087 | 0.7581× | 0.754–0.7772 | clifft-scheduled |
| msc5/s4096/c67108864/strict | 102597.708 | 78931.750 | 0.7693× | 0.7631–0.804 | clifft-scheduled |
| terminal/s1/c67108864/strict | 0.248 | 1.555 | 6.276× | 6.236–6.321 | symft |
| terminal/s8/c67108864/strict | 1.930 | 2.374 | 1.23× | 1.197–1.245 | clifft-scheduled |
| terminal/s32/c67108864/strict | 2.221 | 3.522 | 1.586× | 1.571–1.601 | clifft-scheduled |
| terminal/s63/c67108864/strict | 3.853 | 4.986 | 1.294× | 1.284–1.339 | clifft |
| terminal/s64/c67108864/strict | 3.884 | 4.999 | 1.287× | 1.279–1.349 | clifft-scheduled |
| terminal/s65/c67108864/strict | 4.451 | 5.122 | 1.151× | 1.132–1.171 | clifft |
| terminal/s256/c67108864/strict | 15.504 | 14.236 | 0.9183× | 0.8948–0.9268 | clifft-scheduled |
| terminal/s1024/c67108864/strict | 61.907 | 50.876 | 0.8218× | 0.8194–0.8262 | clifft-scheduled |
| terminal/s4096/c67108864/strict | 246.245 | 196.654 | 0.7986× | 0.795–0.8086 | clifft |
| msc3/s1/c67108864/strict | 1.710 | 2.348 | 1.373× | 1.033–1.4 | clifft-scheduled |
| msc3/s8/c67108864/strict | 13.776 | 7.327 | 0.5319× | 0.5235–0.5379 | clifft |
| msc3/s32/c67108864/strict | 10.164 | 12.618 | 1.241× | 1.204–1.269 | symft |
| msc3/s63/c67108864/strict | 19.280 | 20.346 | 1.055× | 0.9957–1.09 | symft |
| msc3/s64/c67108864/strict | 14.803 | 18.385 | 1.242× | 1.176–1.262 | symft |
| msc3/s65/c67108864/strict | 16.240 | 21.467 | 1.322× | 1.313–1.333 | clifft |
| msc3/s256/c67108864/strict | 58.670 | 58.330 | 0.9942× | 0.9925–1.022 | clifft |
| msc3/s1024/c67108864/strict | 233.948 | 223.586 | 0.9557× | 0.9524–0.9605 | clifft |
| msc3/s4096/c67108864/strict | 945.799 | 894.704 | 0.946× | 0.9266–0.9722 | clifft |
| qec32/s1/c67108864/strict | 0.739 | 1.944 | 2.631× | 2.606–2.656 | clifft-scheduled |
| qec32/s8/c67108864/strict | 5.771 | 3.692 | 0.6398× | 0.6249–0.6576 | symft |
| qec32/s32/c67108864/strict | 4.316 | 6.872 | 1.592× | 1.571–1.592 | clifft-scheduled |
| qec32/s63/c67108864/strict | 7.271 | 10.745 | 1.478× | 1.449–1.496 | clifft-scheduled |
| qec32/s64/c67108864/strict | 5.733 | 10.627 | 1.854× | 1.821–1.889 | clifft |
| qec32/s65/c67108864/strict | 7.041 | 11.372 | 1.615× | 1.517–1.642 | clifft-scheduled |
| qec32/s256/c67108864/strict | 23.092 | 35.446 | 1.535× | 1.509–1.541 | clifft-scheduled |
| qec32/s1024/c67108864/strict | 94.433 | 135.513 | 1.435× | 1.421–1.468 | clifft-scheduled |
| qec32/s4096/c67108864/strict | 362.616 | 515.209 | 1.421× | 0.8424–1.427 | clifft-scheduled |
| msc5/s1/c67108864/fused | 33.626 | 26.652 | 0.7926× | 0.7777–0.9334 | clifft-scheduled |
| msc5/s64/c67108864/fused | 1482.831 | 1263.924 | 0.8524× | 0.6061–0.9317 | clifft-scheduled |
| msc5/s1024/c67108864/fused | 24393.125 | 21848.306 | 0.8957× | 0.8675–0.9263 | clifft-scheduled |
| msc5/s8/c67108864/fused | 306.900 | 205.534 | 0.6697× | 0.6576–0.712 | clifft-scheduled |
| msc5/s32/c67108864/fused | 815.992 | 677.926 | 0.8308× | 0.8216–0.8368 | clifft-scheduled |
| msc5/s63/c67108864/fused | 1744.382 | 1213.256 | 0.6955× | 0.6936–0.6967 | clifft-scheduled |
| msc5/s65/c67108864/fused | 1554.264 | 1259.370 | 0.8103× | 0.7744–0.8144 | clifft-scheduled |
| msc5/s256/c67108864/fused | 5356.625 | 4804.629 | 0.897× | 0.8916–0.9113 | clifft-scheduled |
| msc5/s4096/c67108864/fused | 85064.334 | 74627.041 | 0.8773× | 0.8672–0.9062 | clifft |
| terminal/s1/c67108864/fused | 0.245 | 1.537 | 6.279× | 6.149–6.343 | symft |
| terminal/s8/c67108864/fused | 1.890 | 2.325 | 1.23× | 1.216–1.259 | clifft |
| terminal/s32/c67108864/fused | 2.237 | 3.442 | 1.539× | 1.519–1.584 | clifft-scheduled |
| terminal/s63/c67108864/fused | 3.887 | 5.024 | 1.292× | 1.279–1.313 | clifft-scheduled |
| terminal/s64/c67108864/fused | 3.975 | 5.065 | 1.274× | 1.234–1.295 | clifft |
| terminal/s65/c67108864/fused | 4.447 | 4.976 | 1.119× | 1.106–1.161 | clifft-scheduled |
| terminal/s256/c67108864/fused | 15.401 | 14.212 | 0.9227× | 0.903–0.9394 | clifft-scheduled |
| terminal/s1024/c67108864/fused | 61.196 | 50.044 | 0.8178× | 0.803–0.8345 | clifft-scheduled |
| terminal/s4096/c67108864/fused | 245.108 | 194.509 | 0.7936× | 0.78–0.7993 | clifft |
| msc3/s1/c67108864/fused | 1.666 | 2.306 | 1.384× | 1.376–1.402 | clifft-scheduled |
| msc3/s8/c67108864/fused | 13.121 | 7.042 | 0.5367× | 0.532–0.7725 | clifft-scheduled |
| msc3/s32/c67108864/fused | 9.904 | 12.490 | 1.261× | 1.227–1.637 | symft |
| msc3/s63/c67108864/fused | 17.297 | 17.728 | 1.025× | 1.001–1.031 | symft |
| msc3/s64/c67108864/fused | 14.408 | 17.794 | 1.235× | 1.211–1.242 | symft |
| msc3/s65/c67108864/fused | 16.115 | 21.315 | 1.323× | 1.305–1.337 | clifft |
| msc3/s256/c67108864/fused | 56.783 | 56.737 | 0.9992× | 0.9963–1.006 | clifft |
| msc3/s1024/c67108864/fused | 226.476 | 219.685 | 0.97× | 0.96–0.9986 | clifft |
| msc3/s4096/c67108864/fused | 904.744 | 866.898 | 0.9582× | 0.9549–0.9601 | clifft |
| qec32/s1/c67108864/fused | 0.723 | 1.924 | 2.661× | 2.619–2.679 | clifft |
| qec32/s8/c67108864/fused | 5.736 | 3.652 | 0.6367× | 0.6323–0.7631 | symft |
| qec32/s32/c67108864/fused | 4.284 | 6.836 | 1.596× | 1.564–1.615 | clifft |
| qec32/s63/c67108864/fused | 7.108 | 10.601 | 1.491× | 1.474–1.501 | clifft-scheduled |
| qec32/s64/c67108864/fused | 5.713 | 10.647 | 1.864× | 1.853–1.876 | clifft |
| qec32/s65/c67108864/fused | 6.746 | 10.901 | 1.616× | 1.607–1.63 | clifft-scheduled |
| qec32/s256/c67108864/fused | 22.693 | 34.532 | 1.522× | 1.502–1.542 | clifft-scheduled |
| qec32/s1024/c67108864/fused | 91.581 | 130.437 | 1.424× | 1.4–1.446 | clifft |
| qec32/s4096/c67108864/fused | 361.522 | 510.665 | 1.413× | 1.404–1.421 | clifft |
