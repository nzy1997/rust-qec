# Wide symbolic-suffix follow-up

Apple M4, release build, `StdRng`, one thread, 1,000 shots. The table gives medians
over four process invocations; each invocation contains nine internal timing repetitions.
The [raw JSON](apple-m4-wide-post-fix-2026-09-29.json) records each invocation,
source revision, binary SHA-256, compiler, and platform.

| Independent random bits | First prepared flat | Warm prepared flat | Peak RSS |
| ---: | ---: | ---: | ---: |
| 128 | 0.710 ms | 0.600 ms | 3.20 MiB |
| 129 | 1.328 ms | 1.137 ms | 3.52 MiB |
| 193 | 2.060 ms | 1.740 ms | 4.03 MiB |

The representation changes from `u128` to multiword masks above 128 bits.
The transition costs about 2× here, without the large time and memory cliff
seen at the former 64-bit limit. These are candidate-only measurements;
the baseline was not run at 129 or 193 bits because it cannot compile a
symbolic suffix of that width.
