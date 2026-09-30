# Near-Clifford boundary validation and CPU profile

On the Apple M4 host used for the [P0/P1 matrix](apple-m4-2026-09-29.md),
two small prototype changes removed the measured cliffs. These are diagnostic
experiments, not production changes. The simulator base was `85406e09`.
All comparisons use the same circuits, `StdRng`, one thread, release builds,
1,000 shots, nine internal timing repetitions per invocation, and four
alternating-order process pairs. Figures below are medians across those four
invocations; RSS is the process peak. The example verifies batch output against
individual runs and flat against structured output, including RNG continuation,
before timing.

## Symbolic suffix width

The prototype changes the symbolic stabilizer signs, measurement mask, and
random-bit accumulator from `u64` to `u128`, extending the symbolic suffix
from 64 to 128 independent random bits. The raw paired data are in
[apple-m4-boundary-validation-u128.json](apple-m4-boundary-validation-u128.json).

| Independent random measurements | Baseline first flat | `u128` first flat | Baseline warm flat | `u128` warm flat | Peak RSS baseline → `u128` |
| --- | ---: | ---: | ---: | ---: | ---: |
| 64 | 0.300 ms | 0.314 ms | 0.277 ms | 0.292 ms | 2.83 → 2.79 MiB |
| 73 | 30.096 ms | 0.362 ms | 0.361 ms | 0.333 ms | 67.18 → 2.96 MiB |
| 74 | 41.589 ms | 0.368 ms | 13.904 ms | 0.339 ms | 67.55 → 3.03 MiB |
| 80 | 55.532 ms | 0.390 ms | 20.962 ms | 0.359 ms | 70.79 → 2.95 MiB |

At 74 bits, the prototype is about 113× faster on the first prepared call and
41× faster warm. The sharp change at this boundary is caused by the 64-bit
symbolic limit: after that limit, the sampler explores random prefixes in its
bounded cache. At 73 bits the prefix still fits enough to make warm calls fast,
but first-call time and RSS have already increased sharply. The 64-bit case is
about 5% slower with `u128`, so a production design should retain a compact
`u64` representation where possible. `u128` merely moves the width limit; a
multiword representation needs its own memory and workload tests.

A new focused unit test in the prototype checks parity across bits 63 and 64,
128 random bits, repeated measurement correlations, exact sample output and RNG
continuation against individual measurements, and fallback at a 129th
independent bit. `cargo test --locked -p rstim --lib symbolic_suffix` passed all
three tests; `cargo test --locked -p rstim --test near_clifford_batch` passed all
14 integration tests with both prototype changes present.

## Rank-cache boundary

macOS `sample` captured ten seconds at 1 ms intervals while a release build
with debug symbols and frame pointers repeatedly ran 1,000-shot batches. The
baseline `rank_11` profile had 7,963 main-thread samples in the work loop.
The exclusive top-of-stack counts were 2,830 in
`ActiveState::collapse_pauli_measurement` (35.5%), 1,985 in
`ActiveState::pauli_probability_zero` (24.9%), 899 in
`ActiveState::physical_pauli` (11.3%), and 886 in `Vec::from_iter` (11.1%).
These stack samples locate the cost in active-state coefficient work and its
allocations; they are not exact cycle percentages. In the `rank_10` control,
5,072 of 8,484 work-loop samples (59.8%) were in RNG generation. The hot loop
can be reproduced with `near_clifford_matrix_bench profile_rank_11 15` or
`profile_rank_10 15`, then attached to macOS `sample` by printed PID.

To isolate the cache gate, both variants below use the same `u128` prototype.
Only the coefficient check in `CachedTerminalSampler::sample_one` changes from
`<= 1024` to `<= 2048`; the cached-node cap is unchanged. The paired raw data
are in [apple-m4-rank-cap-isolated.json](apple-m4-rank-cap-isolated.json).

| Requested rank | Coefficient gate | First flat | Warm flat | Peak RSS |
| --- | ---: | ---: | ---: | ---: |
| 10 | 1024 | 0.296 ms | 0.105 ms | 4.04 MiB |
| 10 | 2048 | 0.292 ms | 0.105 ms | 4.05 MiB |
| 11 | 1024 | 15.362 ms | 15.381 ms | 3.13 MiB |
| 11 | 2048 | 0.440 ms | 0.157 ms | 4.59 MiB |

The isolated gate change yields about 98× faster warm rank-11 sampling for this
fixture, with about 1.46 MiB more process RSS. The rank labels describe circuit
construction, not a traced maximum active rank. A global cap increase is not
yet justified: `terminal_cache_node_limit` budgets a fixed 16 KiB for active
state data per cached node, whereas 2,048 complex coefficients alone require
more space, and larger circuits could exceed the intended 64 MiB cache budget.

## Next implementation decision

Implement a width-aware symbolic suffix with the `u64` fast path preserved, then
add correctness and performance coverage at 64/65, 73/74, 128/129 bits and on
correlated repeated measurements. Separately, make cache admission account for
actual coefficient storage (or a conservative bound) before allowing 2,048
coefficients. Compare rank 10/11 and wide/high-rank combined fixtures with
peak RSS and cache-node counts. The current prototypes should remain separate
until those limits are explicit.

The width comparison used ordinary release builds. The rank-gate comparison
and CPU profile used `CARGO_PROFILE_RELEASE_DEBUG=1` and
`RUSTFLAGS='-C force-frame-pointers=yes'` for both compared binaries. Timing
excludes process launch; the profile hot loop includes output destruction.
