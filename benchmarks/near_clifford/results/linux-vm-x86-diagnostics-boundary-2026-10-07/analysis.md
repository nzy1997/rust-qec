# Verified near-Clifford diagnostic measurements

Measured source: `8dac954b402518d620669124a4d2852ae17ed9bf`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
Verification: `{"events": 1800, "lifetime_comparison_keys": 0, "retained_failure_events": 0, "selected_cells": 72, "valid_cells": 72}`.
Complete timing comparisons: `72/72` selected cells; verifier valid_cells counts finite-witness acceptance.
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
| msc5/s1/c67108864/strict | 120.890 | 33.694 | 0.2787× | 0.2751–0.2802 | clifft-scheduled |
| msc5/s64/c67108864/strict | 2811.574 | 1800.335 | 0.6403× | 0.6307–0.6676 | symft |
| msc5/s1024/c67108864/strict | 45350.014 | 29276.130 | 0.6456× | 0.6426–0.6602 | clifft-scheduled |
| msc5/s8/c67108864/strict | 1031.278 | 233.916 | 0.2268× | 0.2237–0.23 | clifft-scheduled |
| msc5/s32/c67108864/strict | 1678.575 | 896.510 | 0.5341× | 0.5297–0.5459 | symft |
| msc5/s63/c67108864/strict | 3171.192 | 1805.659 | 0.5694× | 0.559–0.5946 | clifft-scheduled |
| msc5/s65/c67108864/strict | 3346.255 | 1855.075 | 0.5544× | 0.5474–0.5544 | clifft-scheduled |
| msc5/s256/c67108864/strict | 10965.338 | 7079.147 | 0.6456× | 0.6372–0.6633 | symft |
| msc5/s4096/c67108864/strict | 180594.754 | 118032.251 | 0.6536× | 0.6429–0.6587 | clifft-scheduled |
| terminal/s1/c67108864/strict | 0.554 | 3.811 | 6.874× | 6.842–6.971 | symft |
| terminal/s8/c67108864/strict | 4.114 | 5.552 | 1.349× | 1.339–1.376 | clifft-scheduled |
| terminal/s32/c67108864/strict | 5.500 | 7.520 | 1.367× | 1.36–1.37 | clifft |
| terminal/s63/c67108864/strict | 9.752 | 9.998 | 1.025× | 1.024–1.036 | clifft-scheduled |
| terminal/s64/c67108864/strict | 9.894 | 9.995 | 1.01× | 1.009–1.018 | clifft |
| terminal/s65/c67108864/strict | 11.004 | 10.174 | 0.9246× | 0.8954–0.9337 | clifft |
| terminal/s256/c67108864/strict | 39.050 | 25.512 | 0.6533× | 0.6497–0.6575 | clifft |
| terminal/s1024/c67108864/strict | 155.781 | 86.870 | 0.5576× | 0.5521–0.5603 | clifft |
| terminal/s4096/c67108864/strict | 624.225 | 332.625 | 0.5329× | 0.5234–0.5344 | clifft |
| msc3/s1/c67108864/strict | 3.345 | 5.678 | 1.697× | 1.683–1.744 | clifft |
| msc3/s8/c67108864/strict | 26.565 | 13.765 | 0.5182× | 0.5159–0.5359 | clifft-scheduled |
| msc3/s32/c67108864/strict | 20.736 | 25.705 | 1.24× | 1.216–1.253 | symft |
| msc3/s63/c67108864/strict | 38.379 | 36.603 | 0.9537× | 0.9389–0.9557 | symft |
| msc3/s64/c67108864/strict | 35.745 | 38.910 | 1.089× | 1.079–1.095 | clifft-scheduled |
| msc3/s65/c67108864/strict | 38.836 | 41.389 | 1.066× | 1.054–1.075 | clifft-scheduled |
| msc3/s256/c67108864/strict | 141.270 | 125.494 | 0.8883× | 0.8866–0.895 | clifft-scheduled |
| msc3/s1024/c67108864/strict | 561.022 | 467.457 | 0.8332× | 0.8262–0.8373 | clifft-scheduled |
| msc3/s4096/c67108864/strict | 2247.762 | 1836.721 | 0.8171× | 0.8142–0.8255 | clifft-scheduled |
| qec32/s1/c67108864/strict | 1.687 | 4.943 | 2.93× | 2.904–2.979 | clifft |
| qec32/s8/c67108864/strict | 13.033 | 7.415 | 0.5689× | 0.5594–0.5724 | clifft |
| qec32/s32/c67108864/strict | 8.537 | 11.541 | 1.352× | 1.352–1.369 | clifft-scheduled |
| qec32/s63/c67108864/strict | 14.520 | 16.758 | 1.154× | 1.147–1.167 | clifft-scheduled |
| qec32/s64/c67108864/strict | 12.244 | 16.934 | 1.383× | 1.376–1.392 | clifft-scheduled |
| qec32/s65/c67108864/strict | 14.323 | 17.283 | 1.207× | 1.193–1.22 | clifft |
| qec32/s256/c67108864/strict | 48.559 | 49.990 | 1.029× | 1.021–1.038 | clifft-scheduled |
| qec32/s1024/c67108864/strict | 194.194 | 182.904 | 0.9419× | 0.9269–0.9506 | clifft |
| qec32/s4096/c67108864/strict | 771.552 | 707.151 | 0.9165× | 0.9106–0.9257 | clifft-scheduled |
| msc5/s1/c67108864/fused | 64.031 | 33.700 | 0.5263× | 0.5129–0.5336 | clifft-scheduled |
| msc5/s64/c67108864/fused | 1924.259 | 1815.188 | 0.9433× | 0.9394–0.957 | clifft-scheduled |
| msc5/s1024/c67108864/fused | 31759.194 | 29070.100 | 0.9153× | 0.8906–0.9294 | clifft-scheduled |
| msc5/s8/c67108864/fused | 533.078 | 235.430 | 0.4416× | 0.4387–0.4439 | clifft-scheduled |
| msc5/s32/c67108864/fused | 1571.028 | 899.174 | 0.5723× | 0.5659–0.5963 | symft |
| msc5/s63/c67108864/fused | 3021.507 | 1798.117 | 0.5951× | 0.5948–0.5962 | clifft-scheduled |
| msc5/s65/c67108864/fused | 2642.226 | 1858.375 | 0.7033× | 0.6906–0.7148 | clifft-scheduled |
| msc5/s256/c67108864/fused | 7823.905 | 7206.325 | 0.9211× | 0.9069–0.9524 | symft |
| msc5/s4096/c67108864/fused | 123393.772 | 117010.593 | 0.9483× | 0.9295–0.9666 | clifft-scheduled |
| terminal/s1/c67108864/fused | 0.556 | 3.816 | 6.866× | 6.785–6.965 | symft |
| terminal/s8/c67108864/fused | 4.103 | 5.510 | 1.343× | 1.339–1.362 | clifft-scheduled |
| terminal/s32/c67108864/fused | 5.508 | 7.487 | 1.359× | 1.245–1.37 | clifft |
| terminal/s63/c67108864/fused | 9.744 | 9.984 | 1.025× | 1.023–1.029 | clifft |
| terminal/s64/c67108864/fused | 9.892 | 9.993 | 1.01× | 1.008–1.017 | clifft |
| terminal/s65/c67108864/fused | 10.992 | 10.161 | 0.9244× | 0.9233–0.9271 | clifft |
| terminal/s256/c67108864/fused | 39.365 | 25.528 | 0.6485× | 0.6392–0.6545 | clifft |
| terminal/s1024/c67108864/fused | 155.651 | 86.800 | 0.5577× | 0.5569–0.5644 | clifft-scheduled |
| terminal/s4096/c67108864/fused | 623.964 | 333.566 | 0.5346× | 0.5033–0.5366 | clifft |
| msc3/s1/c67108864/fused | 3.368 | 5.702 | 1.693× | 0.5984–1.727 | clifft-scheduled |
| msc3/s8/c67108864/fused | 26.324 | 13.919 | 0.5288× | 0.5203–0.5352 | clifft |
| msc3/s32/c67108864/fused | 20.858 | 25.876 | 1.241× | 1.235–1.264 | symft |
| msc3/s63/c67108864/fused | 38.482 | 36.616 | 0.9515× | 0.9469–0.9651 | symft |
| msc3/s64/c67108864/fused | 35.586 | 38.828 | 1.091× | 1.088–1.094 | clifft-scheduled |
| msc3/s65/c67108864/fused | 38.753 | 41.428 | 1.069× | 1.059–1.101 | clifft |
| msc3/s256/c67108864/fused | 141.014 | 125.698 | 0.8914× | 0.8892–0.9031 | clifft-scheduled |
| msc3/s1024/c67108864/fused | 560.168 | 465.936 | 0.8318× | 0.828–0.8403 | clifft-scheduled |
| msc3/s4096/c67108864/fused | 2241.662 | 1837.578 | 0.8197× | 0.8104–0.827 | clifft |
| qec32/s1/c67108864/fused | 1.692 | 4.925 | 2.911× | 2.875–2.934 | clifft |
| qec32/s8/c67108864/fused | 13.118 | 7.427 | 0.5662× | 0.5622–0.5732 | clifft |
| qec32/s32/c67108864/fused | 8.600 | 11.532 | 1.341× | 1.302–1.363 | clifft |
| qec32/s63/c67108864/fused | 14.438 | 16.723 | 1.158× | 1.15–1.187 | clifft-scheduled |
| qec32/s64/c67108864/fused | 12.310 | 16.913 | 1.374× | 1.36–1.394 | clifft-scheduled |
| qec32/s65/c67108864/fused | 14.523 | 17.226 | 1.186× | 1.162–1.216 | clifft-scheduled |
| qec32/s256/c67108864/fused | 48.440 | 49.983 | 1.032× | 1.027–1.043 | clifft-scheduled |
| qec32/s1024/c67108864/fused | 192.885 | 183.112 | 0.9493× | 0.9357–0.9705 | clifft-scheduled |
| qec32/s4096/c67108864/fused | 773.033 | 709.670 | 0.918× | 0.911–1.006 | clifft |
