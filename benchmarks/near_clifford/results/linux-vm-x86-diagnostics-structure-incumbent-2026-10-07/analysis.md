# Verified near-Clifford diagnostic measurements

Measured source: `8dac954b402518d620669124a4d2852ae17ed9bf`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
Verification: `{"events": 2550, "lifetime_comparison_keys": 0, "retained_failure_events": 0, "selected_cells": 102, "valid_cells": 102}`.
Complete timing comparisons: `102/102` selected cells; verifier valid_cells counts finite-witness acceptance.
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
| qubits-16/s1/c67108864/strict | 16.757 | 3.735 | 0.2229× | 0.2163–0.2297 | clifft-scheduled |
| qubits-16/s64/c67108864/strict | 598.146 | 70.271 | 0.1175× | 0.112–0.1288 | clifft-scheduled |
| qubits-16/s1024/c67108864/strict | 9185.115 | 976.368 | 0.1063× | 0.1034–0.119 | symft |
| qubits-64/s1/c67108864/strict | 5.413 | 3.808 | 0.7036× | 0.6721–0.7068 | clifft-scheduled |
| qubits-64/s64/c67108864/strict | 119.174 | 40.087 | 0.3364× | 0.317–0.3507 | clifft-scheduled |
| qubits-64/s1024/c67108864/strict | 1862.890 | 567.519 | 0.3046× | 0.291–0.3193 | clifft-scheduled |
| qubits-129/s1/c67108864/strict | 7.342 | 4.062 | 0.5532× | 0.5244–0.5672 | clifft-scheduled |
| qubits-129/s64/c67108864/strict | 126.822 | 44.681 | 0.3523× | 0.3307–0.3632 | clifft-scheduled |
| qubits-129/s1024/c67108864/strict | 1955.620 | 632.592 | 0.3235× | 0.3079–0.3345 | clifft-scheduled |
| qubits-256/s1/c67108864/strict | 10.664 | 4.691 | 0.4399× | 0.4276–0.445 | clifft-scheduled |
| qubits-256/s64/c67108864/strict | 139.075 | 52.225 | 0.3755× | 0.3594–0.3792 | clifft-scheduled |
| qubits-256/s1024/c67108864/strict | 2231.452 | 763.351 | 0.3421× | 0.3415–0.3539 | clifft-scheduled |
| depth-1/s1/c67108864/strict | 0.595 | 2.678 | 4.5× | 4.341–4.562 | clifft-scheduled |
| depth-1/s64/c67108864/strict | 3.537 | 4.346 | 1.229× | 1.18–1.25 | clifft-scheduled |
| depth-1/s1024/c67108864/strict | 52.733 | 26.789 | 0.508× | 0.4644–0.5174 | clifft-scheduled |
| depth-8/s1/c67108864/strict | 4.553 | 3.637 | 0.7987× | 0.7546–0.8037 | clifft-scheduled |
| depth-8/s64/c67108864/strict | 121.093 | 38.655 | 0.3192× | 0.3103–0.3253 | clifft-scheduled |
| depth-8/s1024/c67108864/strict | 1887.582 | 557.212 | 0.2952× | 0.2885–0.3003 | clifft-scheduled |
| depth-32/s1/c67108864/strict | 1313.692 | 24.611 | 0.01873× | 0.01864–0.01886 | clifft-scheduled |
| depth-32/s64/c67108864/strict | 43147.127 | 296.470 | 0.006871× | 0.006555–0.007173 | symft |
| depth-32/s1024/c67108864/strict | 808985.768 | 4183.550 | 0.005171× | 0.004953–0.005337 | symft |
| noise-0/s1/c67108864/strict | 4.480 | 3.523 | 0.7864× | 0.7524–0.812 | clifft-scheduled |
| noise-0/s64/c67108864/strict | 118.547 | 37.687 | 0.3179× | 0.3165–0.3216 | clifft-scheduled |
| noise-0/s1024/c67108864/strict | 1923.466 | 555.294 | 0.2887× | 0.2843–0.2935 | clifft-scheduled |
| noise-0.0001/s1/c67108864/strict | 4.894 | 3.681 | 0.7522× | 0.7383–0.7613 | clifft-scheduled |
| noise-0.0001/s64/c67108864/strict | 124.779 | 39.816 | 0.3191× | 0.2969–0.3295 | clifft-scheduled |
| noise-0.0001/s1024/c67108864/strict | 1929.458 | 556.960 | 0.2887× | 0.2803–0.292 | clifft-scheduled |
| noise-0.001/s1/c67108864/strict | 4.893 | 3.738 | 0.764× | 0.7464–0.8442 | clifft-scheduled |
| noise-0.001/s64/c67108864/strict | 123.963 | 39.696 | 0.3202× | 0.3153–0.3236 | clifft-scheduled |
| noise-0.001/s1024/c67108864/strict | 1932.236 | 560.054 | 0.2898× | 0.2838–0.293 | clifft-scheduled |
| noise-0.01/s1/c67108864/strict | 5.298 | 3.884 | 0.7331× | 0.6299–0.7456 | clifft-scheduled |
| noise-0.01/s64/c67108864/strict | 134.684 | 44.272 | 0.3287× | 0.316–0.3374 | clifft-scheduled |
| noise-0.01/s1024/c67108864/strict | 2113.439 | 630.441 | 0.2983× | 0.2948–0.3106 | clifft-scheduled |
| reset_every-1/s1/c67108864/strict | 10.113 | 4.634 | 0.4582× | 0.4541–0.469 | clifft-scheduled |
| reset_every-1/s64/c67108864/strict | 184.303 | 120.705 | 0.6549× | 0.6453–0.6629 | clifft-scheduled |
| reset_every-1/s1024/c67108864/strict | 3018.435 | 1840.550 | 0.6098× | 0.5645–0.6304 | clifft-scheduled |
| reset_every-4/s1/c67108864/strict | 7.029 | 3.802 | 0.5409× | 0.5356–0.5609 | clifft-scheduled |
| reset_every-4/s64/c67108864/strict | 158.918 | 41.090 | 0.2586× | 0.2538–0.2599 | clifft-scheduled |
| reset_every-4/s1024/c67108864/strict | 2631.804 | 594.859 | 0.226× | 0.2245–0.2314 | clifft-scheduled |
| reset_every-32/s1/c67108864/strict | 4.639 | 3.608 | 0.7777× | 0.7572–0.8045 | clifft-scheduled |
| reset_every-32/s64/c67108864/strict | 125.382 | 39.294 | 0.3134× | 0.3079–0.3187 | clifft-scheduled |
| reset_every-32/s1024/c67108864/strict | 1994.273 | 558.338 | 0.28× | 0.2688–0.2872 | clifft-scheduled |
| brick16/s1/c67108864/strict | 0.459 | 2.717 | 5.922× | 5.893–5.971 | clifft |
| brick16/s64/c67108864/strict | 11.369 | 12.311 | 1.083× | 1.057–1.147 | clifft-scheduled |
| brick16/s1024/c67108864/strict | 179.031 | 154.825 | 0.8648× | 0.8584–0.8738 | clifft-scheduled |
| parity129/s1/c67108864/strict | 2.554 | 3.869 | 1.515× | 1.497–1.526 | clifft-scheduled |
| parity129/s64/c67108864/strict | 17.579 | 19.796 | 1.126× | 1.109–1.145 | clifft-scheduled |
| parity129/s1024/c67108864/strict | 283.651 | 258.432 | 0.9111× | 0.902–0.9216 | clifft-scheduled |
| rounds129/s1/c67108864/strict | 2.599 | 3.910 | 1.505× | 1.487–1.53 | clifft-scheduled |
| rounds129/s64/c67108864/strict | 14.903 | 15.939 | 1.07× | 1.049–1.099 | clifft-scheduled |
| rounds129/s1024/c67108864/strict | 234.410 | 203.655 | 0.8688× | 0.8535–0.894 | clifft-scheduled |
| qubits-16/s1/c67108864/fused | 16.134 | 3.849 | 0.2386× | 0.2335–0.2432 | clifft-scheduled |
| qubits-16/s64/c67108864/fused | 595.223 | 73.007 | 0.1227× | 0.1189–0.1248 | symft |
| qubits-16/s1024/c67108864/fused | 9190.740 | 1007.467 | 0.1096× | 0.1076–0.11 | symft |
| qubits-64/s1/c67108864/fused | 5.900 | 3.899 | 0.6609× | 0.6503–0.6687 | clifft-scheduled |
| qubits-64/s64/c67108864/fused | 119.542 | 41.404 | 0.3464× | 0.3335–0.3491 | clifft-scheduled |
| qubits-64/s1024/c67108864/fused | 1976.953 | 598.372 | 0.3027× | 0.2975–0.3134 | clifft-scheduled |
| qubits-129/s1/c67108864/fused | 8.604 | 4.197 | 0.4878× | 0.4669–0.5272 | clifft-scheduled |
| qubits-129/s64/c67108864/fused | 125.695 | 46.239 | 0.3679× | 0.3547–0.3695 | clifft-scheduled |
| qubits-129/s1024/c67108864/fused | 2028.164 | 657.445 | 0.3242× | 0.3137–0.3352 | clifft-scheduled |
| qubits-256/s1/c67108864/fused | 11.793 | 4.788 | 0.406× | 0.401–0.414 | clifft-scheduled |
| qubits-256/s64/c67108864/fused | 139.647 | 53.463 | 0.3828× | 0.3736–0.3943 | clifft-scheduled |
| qubits-256/s1024/c67108864/fused | 2240.065 | 780.455 | 0.3484× | 0.3064–0.3629 | clifft-scheduled |
| depth-1/s1/c67108864/fused | 0.615 | 2.709 | 4.402× | 4.333–4.542 | clifft-scheduled |
| depth-1/s64/c67108864/fused | 3.586 | 4.423 | 1.234× | 1.221–1.258 | clifft-scheduled |
| depth-1/s1024/c67108864/fused | 54.475 | 27.628 | 0.5072× | 0.5033–0.5134 | clifft-scheduled |
| depth-8/s1/c67108864/fused | 4.936 | 3.693 | 0.7481× | 0.7339–0.7884 | clifft-scheduled |
| depth-8/s64/c67108864/fused | 117.788 | 39.772 | 0.3377× | 0.3248–0.3388 | clifft-scheduled |
| depth-8/s1024/c67108864/fused | 1911.012 | 560.805 | 0.2935× | 0.2872–0.3027 | clifft-scheduled |
| depth-32/s1/c67108864/fused | 1260.301 | 25.539 | 0.02026× | 0.01999–0.02033 | clifft-scheduled |
| depth-32/s64/c67108864/fused | 49511.730 | 312.102 | 0.006304× | 0.006145–0.006753 | symft |
| depth-32/s1024/c67108864/fused | 786866.669 | 4278.151 | 0.005437× | 0.005161–0.005584 | symft |
| noise-0/s1/c67108864/fused | 4.359 | 3.611 | 0.8284× | 0.7703–0.8837 | clifft-scheduled |
| noise-0/s64/c67108864/fused | 116.718 | 37.962 | 0.3252× | 0.32–0.3418 | clifft-scheduled |
| noise-0/s1024/c67108864/fused | 1845.706 | 562.801 | 0.3049× | 0.2952–0.3132 | clifft-scheduled |
| noise-0.0001/s1/c67108864/fused | 4.655 | 3.726 | 0.8005× | 0.7708–0.8243 | clifft-scheduled |
| noise-0.0001/s64/c67108864/fused | 119.912 | 39.375 | 0.3284× | 0.3188–0.3358 | clifft-scheduled |
| noise-0.0001/s1024/c67108864/fused | 1888.871 | 563.025 | 0.2981× | 0.2917–0.3053 | clifft-scheduled |
| noise-0.001/s1/c67108864/fused | 4.803 | 3.743 | 0.7792× | 0.7627–0.7946 | clifft-scheduled |
| noise-0.001/s64/c67108864/fused | 119.712 | 39.748 | 0.332× | 0.3282–0.3405 | clifft-scheduled |
| noise-0.001/s1024/c67108864/fused | 1880.044 | 569.471 | 0.3029× | 0.2979–0.3073 | clifft-scheduled |
| noise-0.01/s1/c67108864/fused | 5.023 | 3.933 | 0.7828× | 0.7632–0.7972 | clifft-scheduled |
| noise-0.01/s64/c67108864/fused | 131.115 | 44.807 | 0.3417× | 0.337–0.3573 | clifft-scheduled |
| noise-0.01/s1024/c67108864/fused | 2015.448 | 639.649 | 0.3174× | 0.3064–0.3278 | clifft-scheduled |
| reset_every-1/s1/c67108864/fused | 9.472 | 4.582 | 0.4837× | 0.4793–0.4916 | clifft-scheduled |
| reset_every-1/s64/c67108864/fused | 186.112 | 120.088 | 0.6452× | 0.6341–0.6628 | clifft-scheduled |
| reset_every-1/s1024/c67108864/fused | 2997.057 | 1843.430 | 0.6151× | 0.5826–0.6315 | clifft-scheduled |
| reset_every-4/s1/c67108864/fused | 6.337 | 3.760 | 0.5933× | 0.5449–0.6086 | clifft-scheduled |
| reset_every-4/s64/c67108864/fused | 167.916 | 41.755 | 0.2487× | 0.2409–0.2686 | clifft-scheduled |
| reset_every-4/s1024/c67108864/fused | 2519.958 | 596.634 | 0.2368× | 0.2283–0.239 | clifft-scheduled |
| reset_every-32/s1/c67108864/fused | 4.291 | 3.659 | 0.8527× | 0.7985–0.8704 | clifft-scheduled |
| reset_every-32/s64/c67108864/fused | 116.474 | 39.122 | 0.3359× | 0.3315–0.339 | clifft-scheduled |
| reset_every-32/s1024/c67108864/fused | 1868.876 | 561.114 | 0.3002× | 0.2899–0.3025 | clifft-scheduled |
| brick16/s1/c67108864/fused | 0.458 | 2.738 | 5.973× | 5.79–6.127 | clifft-scheduled |
| brick16/s64/c67108864/fused | 11.155 | 12.199 | 1.094× | 1.005–1.11 | clifft-scheduled |
| brick16/s1024/c67108864/fused | 182.030 | 155.897 | 0.8564× | 0.8412–0.8611 | clifft-scheduled |
| parity129/s1/c67108864/fused | 2.544 | 3.829 | 1.505× | 1.451–1.563 | clifft-scheduled |
| parity129/s64/c67108864/fused | 17.644 | 19.616 | 1.112× | 1.111–1.122 | clifft-scheduled |
| parity129/s1024/c67108864/fused | 279.070 | 254.391 | 0.9116× | 0.9112–0.9231 | clifft-scheduled |
| rounds129/s1/c67108864/fused | 2.512 | 3.780 | 1.505× | 1.475–1.51 | clifft-scheduled |
| rounds129/s64/c67108864/fused | 14.450 | 15.457 | 1.07× | 1.056–1.081 | clifft-scheduled |
| rounds129/s1024/c67108864/fused | 229.110 | 197.671 | 0.8628× | 0.8388–0.8792 | clifft-scheduled |
