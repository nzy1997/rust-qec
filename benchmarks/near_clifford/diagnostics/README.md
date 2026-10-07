# Near-Clifford diagnostic benchmarks

This campaign investigates costs left unresolved by the compiled CPU comparison.
It uses unchanged production bytes from squash commit
`3ef5030db205b3e9b2126e31b2602f760d4665cc` (PR 780). Existing published evidence
and the seven-fixture comparison remain immutable. Strict and explicit Fused
FP64 are separate configurations; Strict remains the public default.

The deterministic corpus in `corpus.py` contains:

- MSCd5 at 1/64/1024 shots with 0/1/16/64 MiB coefficient-cache budgets;
- MSCd5, terminal, MSCd3 and qec32 at 1/8/32/63/64/65/256/1024/4096 shots;
- one-parameter input scans of rotation-target count, physical qubits, depth,
  stochastic noise and reset interval; the actual compiled active rank is
  reported, not assumed equal to the generator's rotation-target parameter;
- the other three incumbent fixtures at 1/64/1024 shots;
- eight MSCd5 call histories, including scalar/structured then batch, the reverse,
  repeated batches and mixed sizes, at both arithmetic policies and four budgets.

Increasing a generator parameter may also change the compiled rank/schedule.
These are bounded workload profiles, not causal proof that a single internal
operation explains a slope. Failed, unsupported and timed-out cells are retained.
No arbitrary-rotation or U3 capability is introduced by this harness.

Use the existing compiled-sota pinned peer environments. All backend imports run
under Python `-I`; actual wrappers/extensions, installed package files, clean
SymFT source and before/end identities are bound. Clifft default and scheduled
paths independently tune batch sizes, and SymFT additionally tunes its distinct
scalar executor. Each selected native input returns all raw records without
postselection. Original annotation parities remain in validation.

```sh
python3 benchmarks/near_clifford/diagnostics/run.py \
  --python /absolute/path/to/clifft-env/bin/python \
  --symft-python /absolute/path/to/symft-env/bin/python \
  --symft-source /absolute/path/to/pinned/SOFT \
  --out drafts/diagnostics-p0 --groups cache boundary lifetime
python3 benchmarks/near_clifford/diagnostics/verify.py drafts/diagnostics-p0 --git-sources
python3 -O benchmarks/near_clifford/diagnostics/verify.py drafts/diagnostics-p0 --git-sources
python3 benchmarks/near_clifford/diagnostics/test_contract.py drafts/diagnostics-p0
```

Commit the harness before collecting Git-source-verifiable evidence. The runner
builds its isolated locked Cargo package. `--groups structure incumbent` selects
the structural and incumbent matrix. `--pairs 1 --repetitions 1` is a smoke run
and requires `--allow-smoke` when verified. Each output directory must be fresh.
For Linux measurements, start the runner under `taskset -c CPU`; its children
inherit affinity. The host receipt records the actual visible affinity. macOS
has no pinned-core claim. Unrelated host workloads are not stopped.

Formal timings use five rotated/reversed independent process rounds and seven
observations per process, each accumulating at least 50 ms of public API calls.
Process medians are the statistical units. Compilation, preparation and first
calls remain separate. Caller output destruction and RNG setup are excluded
from warm API timing; peer-internal seed handling remains included.

Lifecycle `phase_sum_ns` is the sum of measured compile, prepare and sampling
phases. It deliberately excludes diagnostic record conversion and destruction,
and must not be relabeled as an end-to-end wall-clock measurement. Rust histories
check exact record digests and 16-word RNG continuation across cache budgets
within a fixed plan/policy/history. Peer RNG streams need not match Rust's.
Peer lifetime tuning uses the largest call in the history, then freezes its
batch for that history; it is not exhaustive history-total tuning. Every distinct
call size/kind is separately validated with full records at that frozen batch,
including lifetime-only campaigns. Rust validates both public flat and structured
paths at each timed policy/budget.

Probe `peak_rss_bytes` uses the OS process high-water mark (macOS bytes, Linux
KiB converted to bytes), includes all live probe allocations and diagnostic
bookkeeping, and is distinct from the simulator's cache reservation ledger.
Python lifetime RSS includes interpreter/import memory. Distribution validation
runs in separate processes and is excluded from timed-probe RSS.

Finite validation collects at least 8192 full-record shots with the exact timed
call size, retaining lossless compressed transcripts. It checks marginals,
adjacent/block/full parity and original detector/observable masks, with a
two-sample Hoeffding union bound allocating alpha 0.0005 to the flat matrix and
0.0005 to lifecycle witnesses (combined family budget at most 0.001). This is not
proof of arbitrary joint distributions. Offline verification
works without peer packages and remains active under `python -O`. Contract tests
reseal corrupted inputs, policies, masks, imports and missing timing/cell evidence
and require rejection.

The runner writes source/environment receipts, fsynced `events.jsonl` checkpoints
and a completion closure. A publication may losslessly gzip the event stream;
the verifier checks its original uncompressed digest. Missing closure denotes
an interrupted campaign and cannot pass verification.
