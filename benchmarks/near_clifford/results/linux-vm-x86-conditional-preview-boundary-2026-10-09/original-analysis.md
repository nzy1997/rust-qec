# Verified near-Clifford diagnostic measurements

Measured source: `bacdcfa3a0c86619087a0f30944f08e2c4527974`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
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
| msc5/s1/c67108864/strict | 60.984 | 27.433 | 0.4498× | 0.4194–0.6325 | clifft-scheduled |
| msc5/s64/c67108864/strict | 2418.325 | 1405.833 | 0.5813× | 0.5545–0.5961 | clifft-scheduled |
| msc5/s1024/c67108864/strict | 40055.452 | 22200.456 | 0.5542× | 0.553–0.5929 | clifft-scheduled |
| msc5/s8/c67108864/strict | 500.064 | 179.543 | 0.359× | 0.3529–0.3656 | clifft-scheduled |
| msc5/s32/c67108864/strict | 1430.955 | 705.594 | 0.4931× | 0.488–0.6685 | clifft-scheduled |
| msc5/s63/c67108864/strict | 2687.058 | 1368.923 | 0.5095× | 0.498–0.8212 | clifft-scheduled |
| msc5/s65/c67108864/strict | 3038.651 | 1422.308 | 0.4681× | 0.4659–0.5556 | clifft-scheduled |
| msc5/s256/c67108864/strict | 9829.251 | 5588.993 | 0.5686× | 0.5536–0.954 | clifft-scheduled |
| msc5/s4096/c67108864/strict | 160317.972 | 88931.245 | 0.5547× | 0.5363–0.8771 | clifft-scheduled |
| terminal/s1/c67108864/strict | 0.596 | 3.804 | 6.377× | 6.263–6.42 | symft |
| terminal/s8/c67108864/strict | 4.376 | 5.439 | 1.243× | 1.223–1.243 | clifft-scheduled |
| terminal/s32/c67108864/strict | 5.665 | 7.473 | 1.319× | 1.273–1.323 | clifft-scheduled |
| terminal/s63/c67108864/strict | 9.981 | 10.266 | 1.029× | 1.016–1.035 | clifft-scheduled |
| terminal/s64/c67108864/strict | 10.145 | 10.255 | 1.011× | 1.003–1.088 | clifft |
| terminal/s65/c67108864/strict | 11.320 | 10.426 | 0.921× | 0.9161–0.9298 | clifft-scheduled |
| terminal/s256/c67108864/strict | 40.200 | 27.325 | 0.6797× | 0.6748–0.6839 | clifft |
| terminal/s1024/c67108864/strict | 160.250 | 93.587 | 0.584× | 0.5808–0.59 | clifft-scheduled |
| terminal/s4096/c67108864/strict | 638.733 | 357.526 | 0.5597× | 0.5558–0.5604 | clifft-scheduled |
| msc3/s1/c67108864/strict | 3.919 | 5.396 | 1.377× | 1.364–1.442 | clifft |
| msc3/s8/c67108864/strict | 30.999 | 12.998 | 0.4193× | 0.3753–0.4429 | clifft-scheduled |
| msc3/s32/c67108864/strict | 20.556 | 24.601 | 1.197× | 1.191–1.312 | symft |
| msc3/s63/c67108864/strict | 38.103 | 36.266 | 0.9518× | 0.9471–0.9521 | symft |
| msc3/s64/c67108864/strict | 36.715 | 38.661 | 1.053× | 1.037–1.06 | symft |
| msc3/s65/c67108864/strict | 39.982 | 44.396 | 1.11× | 1.102–1.115 | clifft-scheduled |
| msc3/s256/c67108864/strict | 146.594 | 133.374 | 0.9098× | 0.8925–0.9194 | clifft-scheduled |
| msc3/s1024/c67108864/strict | 575.234 | 485.896 | 0.8447× | 0.8292–0.8496 | symft |
| msc3/s4096/c67108864/strict | 2299.461 | 1864.223 | 0.8107× | 0.8074–0.8191 | symft |
| qec32/s1/c67108864/strict | 1.839 | 4.697 | 2.554× | 2.518–2.558 | clifft-scheduled |
| qec32/s8/c67108864/strict | 14.305 | 7.111 | 0.4971× | 0.4969–0.5037 | clifft-scheduled |
| qec32/s32/c67108864/strict | 8.786 | 10.842 | 1.234× | 1.221–1.241 | clifft-scheduled |
| qec32/s63/c67108864/strict | 14.828 | 15.563 | 1.05× | 1.042–1.125 | clifft-scheduled |
| qec32/s64/c67108864/strict | 12.805 | 15.953 | 1.246× | 1.242–1.261 | clifft |
| qec32/s65/c67108864/strict | 14.865 | 16.151 | 1.087× | 1.041–1.106 | clifft-scheduled |
| qec32/s256/c67108864/strict | 50.430 | 46.355 | 0.9192× | 0.9114–0.9291 | clifft-scheduled |
| qec32/s1024/c67108864/strict | 201.196 | 166.257 | 0.8263× | 0.8239–0.8384 | clifft |
| qec32/s4096/c67108864/strict | 803.315 | 652.254 | 0.812× | 0.8083–0.8201 | clifft |
| msc5/s1/c67108864/fused | 62.801 | 27.396 | 0.4362× | 0.428–0.5125 | clifft-scheduled |
| msc5/s64/c67108864/fused | 1897.706 | 1397.343 | 0.7363× | 0.7215–0.7455 | clifft-scheduled |
| msc5/s1024/c67108864/fused | 30293.906 | 22800.119 | 0.7526× | 0.7499–1.297 | clifft |
| msc5/s8/c67108864/fused | 517.160 | 180.589 | 0.3492× | 0.3447–0.4148 | clifft-scheduled |
| msc5/s32/c67108864/fused | 1462.598 | 724.679 | 0.4955× | 0.4871–0.8451 | clifft |
| msc5/s63/c67108864/fused | 2732.327 | 1397.300 | 0.5114× | 0.4984–0.7095 | clifft-scheduled |
| msc5/s65/c67108864/fused | 2608.803 | 1413.249 | 0.5417× | 0.541–0.8544 | clifft-scheduled |
| msc5/s256/c67108864/fused | 7466.777 | 6340.458 | 0.8492× | 0.7459–1.063 | clifft-scheduled |
| msc5/s4096/c67108864/fused | 118589.655 | 88508.324 | 0.7463× | 0.7409–0.7556 | clifft-scheduled |
| terminal/s1/c67108864/fused | 0.599 | 3.809 | 6.364× | 6.132–6.664 | symft |
| terminal/s8/c67108864/fused | 4.370 | 5.354 | 1.225× | 1.205–1.233 | clifft-scheduled |
| terminal/s32/c67108864/fused | 5.661 | 7.458 | 1.317× | 1.31–1.33 | clifft-scheduled |
| terminal/s63/c67108864/fused | 10.005 | 10.242 | 1.024× | 1.009–1.027 | clifft-scheduled |
| terminal/s64/c67108864/fused | 10.146 | 10.294 | 1.015× | 1.007–1.015 | clifft-scheduled |
| terminal/s65/c67108864/fused | 11.316 | 10.455 | 0.9239× | 0.9232–0.9323 | clifft-scheduled |
| terminal/s256/c67108864/fused | 40.172 | 27.270 | 0.6788× | 0.6695–0.6807 | clifft |
| terminal/s1024/c67108864/fused | 160.581 | 93.710 | 0.5836× | 0.5795–0.5869 | clifft-scheduled |
| terminal/s4096/c67108864/fused | 640.012 | 357.376 | 0.5584× | 0.5544–0.5597 | clifft-scheduled |
| msc3/s1/c67108864/fused | 3.932 | 5.394 | 1.372× | 1.363–1.506 | clifft-scheduled |
| msc3/s8/c67108864/fused | 31.471 | 12.787 | 0.4063× | 0.3651–0.4178 | clifft |
| msc3/s32/c67108864/fused | 20.377 | 24.584 | 1.207× | 1.202–1.212 | symft |
| msc3/s63/c67108864/fused | 38.204 | 36.356 | 0.9516× | 0.947–1.068 | symft |
| msc3/s64/c67108864/fused | 36.723 | 38.451 | 1.047× | 1.042–1.049 | symft |
| msc3/s65/c67108864/fused | 39.734 | 44.254 | 1.114× | 1.108–1.118 | clifft-scheduled |
| msc3/s256/c67108864/fused | 145.167 | 133.605 | 0.9204× | 0.9181–0.9223 | clifft-scheduled |
| msc3/s1024/c67108864/fused | 574.860 | 485.970 | 0.8454× | 0.8356–0.8489 | symft |
| msc3/s4096/c67108864/fused | 2303.341 | 1866.426 | 0.8103× | 0.803–0.8139 | symft |
| qec32/s1/c67108864/fused | 1.842 | 4.701 | 2.553× | 2.332–2.563 | clifft-scheduled |
| qec32/s8/c67108864/fused | 14.272 | 7.127 | 0.4994× | 0.4559–0.5052 | clifft |
| qec32/s32/c67108864/fused | 8.778 | 10.805 | 1.231× | 1.226–1.279 | clifft |
| qec32/s63/c67108864/fused | 14.821 | 15.641 | 1.055× | 1.047–1.058 | clifft-scheduled |
| qec32/s64/c67108864/fused | 12.777 | 15.715 | 1.23× | 1.224–1.25 | clifft-scheduled |
| qec32/s65/c67108864/fused | 14.845 | 16.135 | 1.087× | 1.072–1.105 | clifft |
| qec32/s256/c67108864/fused | 50.374 | 46.424 | 0.9216× | 0.9119–0.9711 | clifft |
| qec32/s1024/c67108864/fused | 202.233 | 165.109 | 0.8164× | 0.8083–0.8489 | clifft-scheduled |
| qec32/s4096/c67108864/fused | 803.515 | 651.307 | 0.8106× | 0.8055–0.8179 | clifft-scheduled |
