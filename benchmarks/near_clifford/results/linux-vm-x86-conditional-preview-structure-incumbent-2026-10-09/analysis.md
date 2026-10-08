# Verified near-Clifford diagnostic measurements

Measured source: `bacdcfa3a0c86619087a0f30944f08e2c4527974`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
Verification: `{"events": 2550, "lifetime_comparison_keys": 0, "retained_failure_events": 10, "selected_cells": 102, "valid_cells": 102}`.
Complete timing comparisons: `100/102` selected cells; verifier valid_cells counts finite-witness acceptance.
Five independent rotated/reversed process rounds; seven observations of at least50ms per warm process.
Ranges below are paired process-median ranges, not confidence intervals. Strict/Fused are separate.
Clifft0.11.0 and SymFT0.1.1 sourcec89b985 are distribution/import hash bound; native peer build flags are not fully attested.
OS RSS is whole-process high-water, including probe/interpreter allocations. Linux ru_maxrss may retain launcher memory across exec; these Linux receipts cannot establish simulator memory usage or cross-backend memory differences. Mac measurements have no pinned-core claim.
Collector CPU affinity: `[0]`; compiler environment: `{"CARGO_ENCODED_RUSTFLAGS": null, "CC": null, "CFLAGS": null, "CXX": null, "CXXFLAGS": null, "RUSTFLAGS": "-C target-cpu=native"}`.
Empty CSV RSS/cache entries mean unmeasured; raw flat peer workers do not report RSS. Peer preparation is included in compilation.
Raw compile/prepare/first-call metadata is retained; this selected diagnostic campaign has no derived lifecycle comparison. Use a dedicated fresh-seed cold campaign for phase comparisons.
Activity metrics preserve their API names: rstim peak_active_rank, Clifft peak_active_width, SymFT max_active_qubits. They are not a common cross-engine rank scale.
The active_components column is the reported native SymFT counts-sampler flag where available; empty means unmeasured, including raw-record workers.
See warm.csv for all backends, cold phases, named activity metrics, throughput and RSS.

| Cell | rstim µs | Fastest peer µs | Speedup | Paired range | Peer |
| --- | ---: | ---: | ---: | --- | --- |
| qubits-16/s1/c67108864/strict | 6.801 | 6.631 | 0.975× | 0.9176–0.9969 | clifft-scheduled |
| qubits-16/s64/c67108864/strict | 172.686 | 106.076 | 0.6143× | 0.5919–0.6205 | symft |
| qubits-16/s1024/c67108864/strict | 2747.240 | 1416.065 | 0.5154× | 0.508–0.5199 | symft |
| qubits-64/s1/c67108864/strict | 7.968 | 6.954 | 0.8728× | 0.8624–0.8821 | clifft-scheduled |
| qubits-64/s64/c67108864/strict | 225.104 | 64.923 | 0.2884× | 0.2794–0.297 | clifft-scheduled |
| qubits-64/s1024/c67108864/strict | 3563.192 | 916.350 | 0.2572× | 0.2487–0.2649 | clifft-scheduled |
| qubits-129/s1/c67108864/strict | 13.540 | 7.638 | 0.5641× | 0.5401–0.5859 | clifft-scheduled |
| qubits-129/s64/c67108864/strict | 251.673 | 74.142 | 0.2946× | 0.2813–0.3121 | clifft-scheduled |
| qubits-129/s1024/c67108864/strict | 3855.823 | 1040.371 | 0.2698× | 0.2572–0.2707 | clifft-scheduled |
| qubits-256/s1/c67108864/strict | 22.705 | 9.068 | 0.3994× | 0.3863–0.3995 | clifft-scheduled |
| qubits-256/s64/c67108864/strict | 260.376 | 91.329 | 0.3508× | 0.3339–0.3544 | clifft-scheduled |
| qubits-256/s1024/c67108864/strict | 4562.071 | 1281.497 | 0.2809× | 0.2756–0.3105 | clifft-scheduled |
| depth-1/s1/c67108864/strict | 1.192 | 4.810 | 4.035× | 3.996–4.082 | clifft-scheduled |
| depth-1/s64/c67108864/strict | 6.593 | 8.667 | 1.315× | 1.311–1.331 | clifft-scheduled |
| depth-1/s1024/c67108864/strict | 101.572 | 59.386 | 0.5847× | 0.5776–0.5881 | clifft-scheduled |
| depth-8/s1/c67108864/strict | 5.626 | 6.608 | 1.175× | 1.163–1.189 | clifft-scheduled |
| depth-8/s64/c67108864/strict | 172.856 | 61.017 | 0.353× | 0.3523–0.3559 | clifft-scheduled |
| depth-8/s1024/c67108864/strict | 2784.533 | 860.044 | 0.3089× | 0.2967–0.3129 | clifft-scheduled |
| depth-32/s1/c67108864/strict | 299.984 | 51.902 | 0.173× | 0.1716–0.1733 | clifft-scheduled |
| depth-32/s64/c67108864/strict | 10700.267 | 455.059 | 0.04253× | 0.0411–0.04294 | symft |
| noise-0/s1/c67108864/strict | 5.488 | 6.387 | 1.164× | 1.09–1.19 | clifft-scheduled |
| noise-0/s64/c67108864/strict | 147.121 | 58.063 | 0.3947× | 0.3885–0.407 | clifft-scheduled |
| noise-0/s1024/c67108864/strict | 2237.993 | 837.757 | 0.3743× | 0.3363–0.4003 | clifft-scheduled |
| noise-0.0001/s1/c67108864/strict | 5.586 | 6.625 | 1.186× | 1.163–1.21 | clifft-scheduled |
| noise-0.0001/s64/c67108864/strict | 149.726 | 60.128 | 0.4016× | 0.3816–0.417 | clifft-scheduled |
| noise-0.0001/s1024/c67108864/strict | 2261.425 | 846.263 | 0.3742× | 0.3657–0.3841 | clifft-scheduled |
| noise-0.001/s1/c67108864/strict | 5.562 | 6.598 | 1.186× | 1.164–1.199 | clifft-scheduled |
| noise-0.001/s64/c67108864/strict | 175.770 | 61.206 | 0.3482× | 0.3386–0.3614 | clifft-scheduled |
| noise-0.001/s1024/c67108864/strict | 2750.635 | 860.483 | 0.3128× | 0.3011–0.3261 | clifft-scheduled |
| noise-0.01/s1/c67108864/strict | 6.164 | 6.878 | 1.116× | 1.08–1.13 | clifft-scheduled |
| noise-0.01/s64/c67108864/strict | 346.900 | 67.905 | 0.1957× | 0.1945–0.2157 | clifft-scheduled |
| noise-0.01/s1024/c67108864/strict | 5006.677 | 964.106 | 0.1926× | 0.1908–0.1951 | clifft-scheduled |
| reset_every-1/s1/c67108864/strict | 15.223 | 8.147 | 0.5352× | 0.5223–0.5537 | clifft-scheduled |
| reset_every-1/s64/c67108864/strict | 289.364 | 185.248 | 0.6402× | 0.6278–0.6499 | clifft-scheduled |
| reset_every-1/s1024/c67108864/strict | 4624.915 | 2806.529 | 0.6068× | 0.5918–0.6109 | clifft-scheduled |
| reset_every-4/s1/c67108864/strict | 6.690 | 6.885 | 1.029× | 1.02–1.046 | clifft-scheduled |
| reset_every-4/s64/c67108864/strict | 433.934 | 65.004 | 0.1498× | 0.1491–0.1525 | clifft-scheduled |
| reset_every-4/s1024/c67108864/strict | 6918.112 | 917.654 | 0.1326× | 0.1314–0.1347 | clifft-scheduled |
| reset_every-32/s1/c67108864/strict | 5.496 | 6.565 | 1.195× | 1.182–1.205 | clifft-scheduled |
| reset_every-32/s64/c67108864/strict | 169.187 | 60.135 | 0.3554× | 0.3529–0.363 | clifft-scheduled |
| reset_every-32/s1024/c67108864/strict | 2695.322 | 846.471 | 0.3141× | 0.3067–0.3236 | clifft-scheduled |
| brick16/s1/c67108864/strict | 0.804 | 4.832 | 6.009× | 5.953–6.021 | clifft-scheduled |
| brick16/s64/c67108864/strict | 19.369 | 19.984 | 1.032× | 1.016–1.061 | clifft-scheduled |
| brick16/s1024/c67108864/strict | 307.956 | 246.380 | 0.8× | 0.7966–0.8029 | clifft-scheduled |
| parity129/s1/c67108864/strict | 4.322 | 6.599 | 1.527× | 1.469–1.545 | clifft-scheduled |
| parity129/s64/c67108864/strict | 33.889 | 34.521 | 1.019× | 0.9509–1.027 | clifft-scheduled |
| parity129/s1024/c67108864/strict | 539.970 | 455.962 | 0.8444× | 0.8405–0.8581 | clifft-scheduled |
| rounds129/s1/c67108864/strict | 4.418 | 6.510 | 1.473× | 1.457–1.497 | clifft-scheduled |
| rounds129/s64/c67108864/strict | 26.281 | 29.099 | 1.107× | 1.095–1.122 | clifft-scheduled |
| rounds129/s1024/c67108864/strict | 420.908 | 371.006 | 0.8814× | 0.878–0.8895 | clifft-scheduled |
| qubits-16/s1/c67108864/fused | 6.709 | 6.692 | 0.9974× | 0.9897–1.019 | clifft-scheduled |
| qubits-16/s64/c67108864/fused | 177.030 | 105.961 | 0.5986× | 0.5966–0.6175 | symft |
| qubits-16/s1024/c67108864/fused | 2774.552 | 1415.289 | 0.5101× | 0.5014–0.52 | symft |
| qubits-64/s1/c67108864/fused | 8.399 | 7.031 | 0.8371× | 0.8013–0.9003 | clifft-scheduled |
| qubits-64/s64/c67108864/fused | 212.438 | 64.931 | 0.3056× | 0.2989–0.3095 | clifft-scheduled |
| qubits-64/s1024/c67108864/fused | 3440.883 | 913.073 | 0.2654× | 0.263–0.2694 | clifft-scheduled |
| qubits-129/s1/c67108864/fused | 13.743 | 7.676 | 0.5585× | 0.5101–0.5827 | clifft-scheduled |
| qubits-129/s64/c67108864/fused | 232.008 | 74.114 | 0.3194× | 0.3182–0.3218 | clifft-scheduled |
| qubits-129/s1024/c67108864/fused | 3750.263 | 1036.993 | 0.2765× | 0.2674–0.2777 | clifft-scheduled |
| qubits-256/s1/c67108864/fused | 22.689 | 9.024 | 0.3977× | 0.3929–0.4077 | clifft-scheduled |
| qubits-256/s64/c67108864/fused | 254.345 | 90.951 | 0.3576× | 0.3564–0.3609 | clifft-scheduled |
| qubits-256/s1024/c67108864/fused | 4195.850 | 1275.662 | 0.304× | 0.2955–0.3158 | clifft-scheduled |
| depth-1/s1/c67108864/fused | 1.197 | 4.838 | 4.041× | 3.958–4.075 | clifft-scheduled |
| depth-1/s64/c67108864/fused | 6.611 | 8.654 | 1.309× | 1.287–1.33 | clifft-scheduled |
| depth-1/s1024/c67108864/fused | 101.449 | 59.237 | 0.5839× | 0.5724–0.5849 | clifft-scheduled |
| depth-8/s1/c67108864/fused | 5.608 | 6.562 | 1.17× | 1.152–1.199 | clifft-scheduled |
| depth-8/s64/c67108864/fused | 168.565 | 60.911 | 0.3613× | 0.3526–0.375 | clifft-scheduled |
| depth-8/s1024/c67108864/fused | 2816.103 | 859.876 | 0.3053× | 0.2912–0.316 | clifft-scheduled |
| depth-32/s1/c67108864/fused | 298.182 | 51.776 | 0.1736× | 0.1728–0.1747 | clifft-scheduled |
| depth-32/s64/c67108864/fused | 10470.734 | 453.960 | 0.04336× | 0.0414–0.0437 | symft |
| noise-0/s1/c67108864/fused | 5.589 | 6.428 | 1.15× | 1.12–1.166 | clifft-scheduled |
| noise-0/s64/c67108864/fused | 145.996 | 57.923 | 0.3967× | 0.3885–0.4045 | clifft-scheduled |
| noise-0/s1024/c67108864/fused | 2195.946 | 837.617 | 0.3814× | 0.3612–0.3922 | clifft-scheduled |
| noise-0.0001/s1/c67108864/fused | 5.543 | 6.582 | 1.187× | 1.181–1.207 | clifft-scheduled |
| noise-0.0001/s64/c67108864/fused | 154.123 | 60.293 | 0.3912× | 0.385–0.4018 | clifft-scheduled |
| noise-0.0001/s1024/c67108864/fused | 2292.841 | 851.282 | 0.3713× | 0.3598–0.3819 | clifft-scheduled |
| noise-0.001/s1/c67108864/fused | 5.708 | 6.605 | 1.157× | 1.076–1.19 | clifft-scheduled |
| noise-0.001/s64/c67108864/fused | 180.839 | 61.191 | 0.3384× | 0.3267–0.3472 | clifft-scheduled |
| noise-0.001/s1024/c67108864/fused | 2923.899 | 859.944 | 0.2941× | 0.2845–0.3071 | clifft-scheduled |
| noise-0.01/s1/c67108864/fused | 6.262 | 6.838 | 1.092× | 1.002–1.127 | clifft-scheduled |
| noise-0.01/s64/c67108864/fused | 324.901 | 67.840 | 0.2088× | 0.1926–0.2118 | clifft-scheduled |
| noise-0.01/s1024/c67108864/fused | 5170.647 | 961.968 | 0.186× | 0.1796–0.187 | clifft-scheduled |
| reset_every-1/s1/c67108864/fused | 15.232 | 8.048 | 0.5284× | 0.519–0.5317 | clifft-scheduled |
| reset_every-1/s64/c67108864/fused | 285.339 | 185.590 | 0.6504× | 0.6309–0.6556 | clifft-scheduled |
| reset_every-1/s1024/c67108864/fused | 4607.364 | 2830.528 | 0.6143× | 0.5875–0.6297 | clifft-scheduled |
| reset_every-4/s1/c67108864/fused | 6.795 | 6.869 | 1.011× | 1–1.024 | clifft-scheduled |
| reset_every-4/s64/c67108864/fused | 437.824 | 64.749 | 0.1479× | 0.1448–0.1494 | clifft-scheduled |
| reset_every-4/s1024/c67108864/fused | 7003.404 | 919.732 | 0.1313× | 0.1301–0.1341 | clifft-scheduled |
| reset_every-32/s1/c67108864/fused | 5.448 | 6.529 | 1.198× | 1.16–1.218 | clifft-scheduled |
| reset_every-32/s64/c67108864/fused | 169.264 | 60.223 | 0.3558× | 0.3237–0.3629 | clifft-scheduled |
| reset_every-32/s1024/c67108864/fused | 2760.381 | 848.441 | 0.3074× | 0.3046–0.3178 | clifft-scheduled |
| brick16/s1/c67108864/fused | 0.819 | 4.797 | 5.854× | 5.765–6.067 | clifft-scheduled |
| brick16/s64/c67108864/fused | 19.580 | 20.041 | 1.024× | 1.02–1.036 | clifft |
| brick16/s1024/c67108864/fused | 310.910 | 246.312 | 0.7922× | 0.7896–0.7975 | clifft-scheduled |
| parity129/s1/c67108864/fused | 4.315 | 6.557 | 1.52× | 1.494–1.557 | clifft-scheduled |
| parity129/s64/c67108864/fused | 33.982 | 34.662 | 1.02× | 1.019–1.024 | clifft-scheduled |
| parity129/s1024/c67108864/fused | 541.466 | 455.951 | 0.8421× | 0.8397–0.8442 | clifft-scheduled |
| rounds129/s1/c67108864/fused | 4.355 | 6.480 | 1.488× | 1.465–1.527 | clifft-scheduled |
| rounds129/s64/c67108864/fused | 26.381 | 28.991 | 1.099× | 1.094–1.112 | clifft-scheduled |
| rounds129/s1024/c67108864/fused | 419.204 | 372.421 | 0.8884× | 0.8773–0.893 | clifft-scheduled |

Cells without a complete successful timing comparison (original failures remain in events):
`depth-32/s1024/c67108864/strict`, `depth-32/s1024/c67108864/fused`
