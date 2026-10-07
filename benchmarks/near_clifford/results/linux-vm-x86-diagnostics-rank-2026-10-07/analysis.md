# Verified near-Clifford diagnostic measurements

Measured source: `8dac954b402518d620669124a4d2852ae17ed9bf`. Host: `Linux-6.17.0-1022-azure-x86_64-with-glibc2.39`.
Verification: `{"events": 600, "lifetime_comparison_keys": 0, "retained_failure_events": 10, "selected_cells": 24, "valid_cells": 24}`.
Complete timing comparisons: `22/24` selected cells; verifier valid_cells counts finite-witness acceptance.
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
| rank-4/s1/c67108864/strict | 2.637 | 4.089 | 1.551× | 1.539–1.554 | clifft-scheduled |
| rank-4/s64/c67108864/strict | 45.448 | 28.063 | 0.6175× | 0.6077–0.6252 | clifft-scheduled |
| rank-4/s1024/c67108864/strict | 721.451 | 362.337 | 0.5022× | 0.4976–0.5091 | clifft-scheduled |
| rank-8/s1/c67108864/strict | 6.490 | 4.816 | 0.7421× | 0.7367–0.7665 | clifft-scheduled |
| rank-8/s64/c67108864/strict | 184.975 | 51.643 | 0.2792× | 0.2648–0.2867 | clifft-scheduled |
| rank-8/s1024/c67108864/strict | 2937.740 | 723.915 | 0.2464× | 0.2411–0.2472 | clifft-scheduled |
| rank-12/s1/c67108864/strict | 106.456 | 5.572 | 0.05234× | 0.05209–0.05678 | clifft-scheduled |
| rank-12/s64/c67108864/strict | 4026.065 | 76.424 | 0.01898× | 0.01824–0.0193 | clifft-scheduled |
| rank-12/s1024/c67108864/strict | 57218.944 | 1113.506 | 0.01946× | 0.01906–0.01985 | clifft-scheduled |
| rank-16/s1/c67108864/strict | 14353.799 | 7.688 | 0.0005356× | 0.0005299–0.0005424 | clifft-scheduled |
| rank-16/s64/c67108864/strict | 916315.425 | 178.890 | 0.0001952× | 0.0001947–0.0001965 | symft |
| rank-4/s1/c67108864/fused | 2.647 | 4.106 | 1.552× | 1.527–1.598 | clifft-scheduled |
| rank-4/s64/c67108864/fused | 46.705 | 28.047 | 0.6005× | 0.5475–0.6157 | clifft-scheduled |
| rank-4/s1024/c67108864/fused | 773.072 | 362.399 | 0.4688× | 0.4586–0.4816 | clifft-scheduled |
| rank-8/s1/c67108864/fused | 6.156 | 4.833 | 0.785× | 0.7785–0.793 | clifft-scheduled |
| rank-8/s64/c67108864/fused | 184.352 | 51.318 | 0.2784× | 0.269–0.2806 | clifft-scheduled |
| rank-8/s1024/c67108864/fused | 2936.024 | 724.286 | 0.2467× | 0.2411–0.2501 | clifft-scheduled |
| rank-12/s1/c67108864/fused | 99.419 | 5.566 | 0.05598× | 0.05561–0.05651 | clifft-scheduled |
| rank-12/s64/c67108864/fused | 3833.728 | 76.397 | 0.01993× | 0.01945–0.02013 | clifft-scheduled |
| rank-12/s1024/c67108864/fused | 57121.760 | 1115.952 | 0.01954× | 0.01915–0.02004 | clifft-scheduled |
| rank-16/s1/c67108864/fused | 13318.523 | 7.715 | 0.0005793× | 0.0005774–0.0005864 | clifft-scheduled |
| rank-16/s64/c67108864/fused | 852322.424 | 179.406 | 0.0002105× | 0.0002085–0.0002131 | symft |

Cells without a complete successful timing comparison (original failures remain in events):
`rank-16/s1024/c67108864/strict`, `rank-16/s1024/c67108864/fused`
