# Verified near-Clifford diagnostic measurements

Measured source: `3b0cb5a3ab4721e70c559e70bc851576e24faba1`. Host: `macOS-27.0.1-arm64-arm-64bit-Mach-O`.
Verification: `{"events": 3150, "lifetime_comparison_keys": 0, "retained_failure_events": 25, "selected_cells": 126, "valid_cells": 126}`.
Complete timing comparisons: `122/126` selected cells; verifier valid_cells counts finite-witness acceptance.
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
| rank-4/s1/c67108864/strict | 1.346 | 2.254 | 1.674× | 1.625–1.708 | clifft-scheduled |
| rank-4/s64/c67108864/strict | 21.906 | 19.089 | 0.8714× | 0.649–0.8888 | clifft-scheduled |
| rank-4/s1024/c67108864/strict | 346.430 | 243.580 | 0.7031× | 0.6981–0.709 | clifft-scheduled |
| rank-8/s1/c67108864/strict | 3.562 | 2.874 | 0.8068× | 0.7831–0.8499 | clifft-scheduled |
| rank-8/s64/c67108864/strict | 129.358 | 33.208 | 0.2567× | 0.2519–0.2619 | clifft-scheduled |
| rank-8/s1024/c67108864/strict | 2072.830 | 446.350 | 0.2153× | 0.2125–0.2181 | clifft-scheduled |
| rank-12/s1/c67108864/strict | 73.194 | 3.444 | 0.04705× | 0.04695–0.04719 | clifft-scheduled |
| rank-12/s64/c67108864/strict | 2763.976 | 48.390 | 0.01751× | 0.01751–0.02324 | clifft-scheduled |
| rank-12/s1024/c67108864/strict | 42779.521 | 662.344 | 0.01548× | 0.01486–0.01563 | clifft-scheduled |
| rank-16/s1/c67108864/strict | 10739.308 | 5.395 | 0.0005023× | 0.0005002–0.0005147 | clifft-scheduled |
| rank-16/s64/c67108864/strict | 697620.416 | 80.711 | 0.0001157× | 0.0001144–0.0001186 | symft |
| qubits-16/s1/c67108864/strict | 16.087 | 3.126 | 0.1943× | 0.186–0.1966 | clifft-scheduled |
| qubits-16/s64/c67108864/strict | 713.048 | 40.845 | 0.05728× | 0.05668–0.05752 | symft |
| qubits-16/s1024/c67108864/strict | 11430.725 | 536.625 | 0.04695× | 0.04534–0.0481 | symft |
| qubits-64/s1/c67108864/strict | 4.611 | 3.085 | 0.669× | 0.6–0.7996 | clifft-scheduled |
| qubits-64/s64/c67108864/strict | 134.687 | 37.148 | 0.2758× | 0.2751–0.2885 | clifft-scheduled |
| qubits-64/s1024/c67108864/strict | 2137.392 | 493.818 | 0.231× | 0.223–0.2338 | clifft-scheduled |
| qubits-129/s1/c67108864/strict | 7.173 | 3.692 | 0.5147× | 0.4466–0.5507 | clifft-scheduled |
| qubits-129/s64/c67108864/strict | 152.451 | 44.936 | 0.2948× | 0.2616–0.3863 | clifft-scheduled |
| qubits-129/s1024/c67108864/strict | 2181.607 | 579.644 | 0.2657× | 0.2652–0.2848 | clifft-scheduled |
| qubits-256/s1/c67108864/strict | 11.044 | 4.059 | 0.3675× | 0.3612–0.3954 | clifft-scheduled |
| qubits-256/s64/c67108864/strict | 150.093 | 58.928 | 0.3926× | 0.3859–0.4027 | clifft-scheduled |
| qubits-256/s1024/c67108864/strict | 2343.500 | 757.072 | 0.3231× | 0.3144–0.3328 | clifft-scheduled |
| depth-1/s1/c67108864/strict | 0.511 | 1.972 | 3.86× | 3.82–3.895 | clifft-scheduled |
| depth-1/s64/c67108864/strict | 2.646 | 5.299 | 2.003× | 1.972–2.035 | clifft-scheduled |
| depth-1/s1024/c67108864/strict | 41.510 | 53.942 | 1.299× | 1.237–1.428 | clifft-scheduled |
| depth-8/s1/c67108864/strict | 3.587 | 2.858 | 0.7969× | 0.6997–0.8269 | clifft-scheduled |
| depth-8/s64/c67108864/strict | 134.264 | 33.542 | 0.2498× | 0.2466–0.2721 | clifft-scheduled |
| depth-8/s1024/c67108864/strict | 2079.585 | 451.498 | 0.2171× | 0.2085–0.2311 | clifft-scheduled |
| depth-32/s1/c67108864/strict | 1410.720 | 34.822 | 0.02468× | 0.02366–0.02542 | symft |
| depth-32/s64/c67108864/strict | 48172.855 | 163.467 | 0.003393× | 0.003258–0.003407 | symft |
| noise-0/s1/c67108864/strict | 3.555 | 2.785 | 0.7836× | 0.777–0.8185 | clifft-scheduled |
| noise-0/s64/c67108864/strict | 128.739 | 30.977 | 0.2406× | 0.2347–0.267 | clifft-scheduled |
| noise-0/s1024/c67108864/strict | 2047.812 | 439.235 | 0.2145× | 0.2129–0.2165 | clifft-scheduled |
| noise-0.0001/s1/c67108864/strict | 3.528 | 2.834 | 0.8032× | 0.7922–0.8336 | clifft-scheduled |
| noise-0.0001/s64/c67108864/strict | 128.984 | 32.636 | 0.253× | 0.2522–0.254 | clifft-scheduled |
| noise-0.0001/s1024/c67108864/strict | 2065.962 | 444.779 | 0.2153× | 0.1899–0.2159 | clifft-scheduled |
| noise-0.001/s1/c67108864/strict | 3.446 | 2.883 | 0.8366× | 0.7511–0.8658 | clifft-scheduled |
| noise-0.001/s64/c67108864/strict | 128.697 | 32.935 | 0.2559× | 0.2548–0.2603 | clifft-scheduled |
| noise-0.001/s1024/c67108864/strict | 2047.415 | 444.221 | 0.217× | 0.209–0.236 | clifft-scheduled |
| noise-0.01/s1/c67108864/strict | 3.667 | 2.996 | 0.8169× | 0.807–0.8441 | clifft-scheduled |
| noise-0.01/s64/c67108864/strict | 134.743 | 37.586 | 0.2789× | 0.2782–0.2799 | clifft-scheduled |
| noise-0.01/s1024/c67108864/strict | 2154.141 | 519.049 | 0.241× | 0.2401–0.2416 | clifft-scheduled |
| reset_every-1/s1/c67108864/strict | 8.731 | 3.922 | 0.4492× | 0.4437–0.455 | clifft-scheduled |
| reset_every-1/s64/c67108864/strict | 314.498 | 88.552 | 0.2816× | 0.2701–0.2873 | clifft-scheduled |
| reset_every-1/s1024/c67108864/strict | 8126.250 | 1235.765 | 0.1521× | 0.1224–0.4363 | clifft-scheduled |
| reset_every-4/s1/c67108864/strict | 9.284 | 5.466 | 0.5888× | 0.4942–0.6981 | clifft-scheduled |
| reset_every-4/s64/c67108864/strict | 208.985 | 47.179 | 0.2258× | 0.1819–0.2258 | clifft-scheduled |
| reset_every-4/s1024/c67108864/strict | 3191.675 | 511.846 | 0.1604× | 0.1368–0.1659 | clifft-scheduled |
| reset_every-32/s1/c67108864/strict | 3.737 | 2.953 | 0.7904× | 0.7346–0.8251 | clifft-scheduled |
| reset_every-32/s64/c67108864/strict | 167.337 | 39.543 | 0.2363× | 0.1804–0.2368 | clifft-scheduled |
| reset_every-32/s1024/c67108864/strict | 2694.333 | 555.832 | 0.2063× | 0.1815–0.5069 | clifft-scheduled |
| brick16/s1/c67108864/strict | 0.361 | 2.081 | 5.77× | 5.498–6.092 | clifft-scheduled |
| brick16/s64/c67108864/strict | 6.527 | 9.529 | 1.46× | 1.428–1.512 | clifft |
| brick16/s1024/c67108864/strict | 118.225 | 121.196 | 1.025× | 0.9861–1.166 | clifft |
| parity129/s1/c67108864/strict | 2.094 | 3.000 | 1.433× | 1.347–1.507 | clifft-scheduled |
| parity129/s64/c67108864/strict | 14.431 | 18.377 | 1.273× | 1.265–1.324 | clifft-scheduled |
| parity129/s1024/c67108864/strict | 226.471 | 249.117 | 1.1× | 1.096–1.113 | clifft-scheduled |
| rounds129/s1/c67108864/strict | 2.061 | 2.992 | 1.452× | 1.433–1.501 | clifft-scheduled |
| rounds129/s64/c67108864/strict | 9.833 | 17.551 | 1.785× | 1.762–1.8 | clifft-scheduled |
| rounds129/s1024/c67108864/strict | 157.497 | 240.389 | 1.526× | 1.515–1.537 | clifft-scheduled |
| rank-4/s1/c67108864/fused | 1.388 | 2.327 | 1.676× | 1.641–1.858 | clifft-scheduled |
| rank-4/s64/c67108864/fused | 22.490 | 19.410 | 0.863× | 0.8393–0.9321 | clifft-scheduled |
| rank-4/s1024/c67108864/fused | 372.333 | 247.275 | 0.6641× | 0.6495–0.6932 | clifft-scheduled |
| rank-8/s1/c67108864/fused | 3.628 | 2.842 | 0.7834× | 0.7393–0.8352 | clifft-scheduled |
| rank-8/s64/c67108864/fused | 123.112 | 33.578 | 0.2727× | 0.2641–0.2736 | clifft-scheduled |
| rank-8/s1024/c67108864/fused | 1968.500 | 451.755 | 0.2295× | 0.2249–0.2304 | clifft-scheduled |
| rank-12/s1/c67108864/fused | 69.289 | 3.389 | 0.04891× | 0.04828–0.04955 | clifft-scheduled |
| rank-12/s64/c67108864/fused | 2676.732 | 48.218 | 0.01801× | 0.01272–0.01824 | clifft-scheduled |
| rank-12/s1024/c67108864/fused | 43073.333 | 701.400 | 0.01628× | 0.01566–0.02052 | clifft-scheduled |
| rank-16/s1/c67108864/fused | 10606.733 | 5.479 | 0.0005165× | 0.0004894–0.000525 | clifft-scheduled |
| rank-16/s64/c67108864/fused | 664689.084 | 81.737 | 0.000123× | 0.0001221–0.0001233 | symft |
| qubits-16/s1/c67108864/fused | 25.434 | 5.296 | 0.2082× | 0.1579–0.233 | clifft-scheduled |
| qubits-16/s64/c67108864/fused | 1055.785 | 56.536 | 0.05355× | 0.04383–0.05898 | symft |
| qubits-16/s1024/c67108864/fused | 17610.013 | 705.825 | 0.04008× | 0.03733–0.04324 | symft |
| qubits-64/s1/c67108864/fused | 6.551 | 4.314 | 0.6585× | 0.6049–0.6864 | clifft-scheduled |
| qubits-64/s64/c67108864/fused | 204.389 | 51.213 | 0.2506× | 0.2348–0.2983 | clifft-scheduled |
| qubits-64/s1024/c67108864/fused | 3128.477 | 699.837 | 0.2237× | 0.2083–0.252 | clifft-scheduled |
| qubits-129/s1/c67108864/fused | 10.627 | 4.666 | 0.4391× | 0.3116–0.4731 | clifft-scheduled |
| qubits-129/s64/c67108864/fused | 213.723 | 59.971 | 0.2806× | 0.2713–0.291 | clifft-scheduled |
| qubits-129/s1024/c67108864/fused | 3139.471 | 749.143 | 0.2386× | 0.2229–0.2409 | clifft-scheduled |
| qubits-256/s1/c67108864/fused | 14.222 | 5.488 | 0.3859× | 0.3702–0.4339 | clifft-scheduled |
| qubits-256/s64/c67108864/fused | 189.176 | 68.543 | 0.3623× | 0.3472–0.408 | clifft-scheduled |
| qubits-256/s1024/c67108864/fused | 2341.405 | 775.365 | 0.3312× | 0.2628–0.3849 | clifft-scheduled |
| depth-1/s1/c67108864/fused | 0.531 | 2.011 | 3.786× | 3.484–4.18 | clifft-scheduled |
| depth-1/s64/c67108864/fused | 2.695 | 5.393 | 2.001× | 1.984–2.048 | clifft-scheduled |
| depth-1/s1024/c67108864/fused | 41.800 | 52.042 | 1.245× | 1.207–1.261 | clifft-scheduled |
| depth-8/s1/c67108864/fused | 3.823 | 2.883 | 0.7542× | 0.6896–0.8094 | clifft-scheduled |
| depth-8/s64/c67108864/fused | 127.974 | 34.810 | 0.272× | 0.2552–0.302 | clifft-scheduled |
| depth-8/s1024/c67108864/fused | 2057.193 | 461.898 | 0.2245× | 0.2229–0.233 | clifft-scheduled |
| depth-32/s1/c67108864/fused | 1398.628 | 35.199 | 0.02517× | 0.0242–0.02571 | symft |
| depth-32/s64/c67108864/fused | 50492.250 | 168.501 | 0.003337× | 0.003161–0.003448 | symft |
| noise-0/s1/c67108864/fused | 4.222 | 3.264 | 0.7731× | 0.5789–0.8153 | clifft-scheduled |
| noise-0/s64/c67108864/fused | 154.576 | 34.692 | 0.2244× | 0.2216–0.2492 | clifft-scheduled |
| noise-0/s1024/c67108864/fused | 2323.474 | 528.980 | 0.2277× | 0.2132–0.2383 | clifft-scheduled |
| noise-0.0001/s1/c67108864/fused | 4.919 | 3.388 | 0.6887× | 0.556–0.8084 | clifft-scheduled |
| noise-0.0001/s64/c67108864/fused | 206.820 | 50.443 | 0.2439× | 0.2056–0.2895 | clifft-scheduled |
| noise-0.0001/s1024/c67108864/fused | 3343.217 | 663.556 | 0.1985× | 0.1774–0.2279 | clifft-scheduled |
| noise-0.001/s1/c67108864/fused | 6.041 | 4.510 | 0.7465× | 0.6656–0.8277 | clifft-scheduled |
| noise-0.001/s64/c67108864/fused | 215.795 | 49.097 | 0.2275× | 0.1853–0.2333 | clifft-scheduled |
| noise-0.001/s1024/c67108864/fused | 3216.977 | 677.980 | 0.2108× | 0.1863–0.2516 | clifft-scheduled |
| noise-0.01/s1/c67108864/fused | 6.745 | 4.738 | 0.7024× | 0.4003–0.8912 | clifft-scheduled |
| noise-0.01/s64/c67108864/fused | 219.739 | 54.893 | 0.2498× | 0.238–0.2642 | clifft-scheduled |
| noise-0.01/s1024/c67108864/fused | 3427.792 | 751.803 | 0.2193× | 0.2011–0.2324 | clifft-scheduled |
| reset_every-1/s1/c67108864/fused | 14.286 | 6.257 | 0.438× | 0.363–0.5008 | clifft-scheduled |
| reset_every-1/s64/c67108864/fused | 345.747 | 102.576 | 0.2967× | 0.213–0.3336 | clifft-scheduled |
| reset_every-1/s1024/c67108864/fused | 4057.977 | 1191.216 | 0.2935× | 0.2691–0.3075 | clifft-scheduled |
| reset_every-4/s1/c67108864/fused | 7.012 | 3.410 | 0.4863× | 0.478–0.5474 | clifft-scheduled |
| reset_every-4/s64/c67108864/fused | 216.985 | 40.329 | 0.1859× | 0.1738–0.1955 | clifft-scheduled |
| reset_every-4/s1024/c67108864/fused | 3403.531 | 590.362 | 0.1735× | 0.1557–0.1735 | clifft-scheduled |
| reset_every-32/s1/c67108864/fused | 4.056 | 3.226 | 0.7954× | 0.7169–0.8647 | clifft-scheduled |
| reset_every-32/s64/c67108864/fused | 190.302 | 44.609 | 0.2344× | 0.2093–0.2566 | clifft-scheduled |
| reset_every-32/s1024/c67108864/fused | 3164.654 | 643.107 | 0.2032× | 0.1688–0.2244 | clifft-scheduled |
| brick16/s1/c67108864/fused | 0.519 | 3.075 | 5.922× | 4.556–9.146 | clifft-scheduled |
| brick16/s64/c67108864/fused | 7.906 | 10.297 | 1.303× | 1.152–2.124 | clifft-scheduled |
| brick16/s1024/c67108864/fused | 119.933 | 127.944 | 1.067× | 1.04–1.126 | clifft |
| parity129/s1/c67108864/fused | 2.100 | 3.892 | 1.853× | 1.491–1.882 | clifft |
| parity129/s64/c67108864/fused | 19.182 | 22.758 | 1.186× | 1.122–1.341 | clifft-scheduled |
| parity129/s1024/c67108864/fused | 290.152 | 320.985 | 1.106× | 1.053–1.203 | clifft-scheduled |
| rounds129/s1/c67108864/fused | 2.371 | 3.425 | 1.445× | 1.248–1.589 | clifft |
| rounds129/s64/c67108864/fused | 12.125 | 21.146 | 1.744× | 1.526–1.954 | clifft-scheduled |
| rounds129/s1024/c67108864/fused | 194.682 | 288.714 | 1.483× | 1.39–1.552 | clifft-scheduled |

Cells without a complete successful timing comparison (original failures remain in events):
`rank-16/s1024/c67108864/strict`, `depth-32/s1024/c67108864/strict`, `rank-16/s1024/c67108864/fused`, `depth-32/s1024/c67108864/fused`
