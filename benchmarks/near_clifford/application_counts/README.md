# Original-circuit capability and postselected counts

This campaign consumes eleven upstream original stimuli byte-for-byte, including
annotations and REPEAT blocks. Provenance, immutable source commits, hashes and
Apache-2.0 licenses are retained in `manifest.json` and `fixtures/`. Unsupported
original inputs stay in the capability ledger; no R_X/R_Z/U3 gate lowering or
physical certification is supplied. Strict and explicit Fused remain separate.

Four supported candidates (MSCd3/MSCd5/surface d7/d9) are selected for counts at
1/64/1024 attempted shots per call. The contract is all-zero **raw** detector
postselection and XOR-folded observable 0 counts, with reference normalization
disabled. Repeated observable include events contribute XOR, not separate errors.
rstim uses public structured records then filters/counts. Clifft uses native
early rejection and SymFT compiled counts. This application comparison measures
the same output contract with different execution work; it must remain separate
from full raw-record throughput. Report attempted and accepted shots/s and
acceptance rates; sparse logical-error counts do not establish conditional error
accuracy or certify the underlying circuit physics.

```sh
python3 benchmarks/near_clifford/application_counts/run.py \
  --out drafts/counts-formal --python /absolute/clifft-env/bin/python \
  --symft-python /absolute/symft-env/bin/python \
  --symft-source /absolute/pinned/SOFT
python3 benchmarks/near_clifford/application_counts/verify.py drafts/counts-formal --git-sources
python3 -O benchmarks/near_clifford/application_counts/verify.py drafts/counts-formal --git-sources
python3 benchmarks/near_clifford/application_counts/test_contract.py drafts/counts-formal
```

Commit the source before collection. Both isolated Cargo packages build locked.
All source/input bytes, binaries, peer distributions and actual isolated imports
are inventoried before and after. Five independent rotated/reversed process
rounds, each seven observations accumulating at least 50 ms, are formal defaults.
Compilation, preparation, first calls, RSS and count rates are separate metrics.
Rust RNG setup and caller output destruction are outside timing; filtering is
inside. Python count-dictionary conversion is inside timing. OS process high-water
RSS includes interpreter/probe allocations and is not a cache-only measurement.
Use taskset on Linux and report actual affinity; macOS is a shared unpinned host.

Each peer independently tunes all five batches (SymFT also scalar), freezes the
median winner and validates the exact attempted call size with at least8192
attempts. Rust retains complete structured measurement witnesses; annotation
analysis independently reproduces its exact accepted/error totals. Peer native
counts are compared both with Rust's witnesses and with each peer's own full
raw-record executor at the frozen batch. These finite accepted/error probability
checks use two-sample Hoeffding bounds with a combined family alpha budget0.001.
They do not prove arbitrary joint distributions. REPEAT expansion is bounded and
used only for annotation analysis; execution retains original source text.
Failures are retained and rejected cells are excluded from timing. `--pairs 1
--repetitions 1` is smoke-only and requires verifier `--allow-smoke`.
Raw-record reference collection has a600second process bound, separate from the
180second timing/native-counts worker bound. The initial retained smoke used180
seconds and timed out the SymFT surface-d9 scalar8192×one-shot raw reference;
it did not certify or time those two arithmetic-context cells. The formal bound
is enlarged only for this correctness work, with the same frozen batch/call size.

Offline verification needs no peer imports and runs under Python `-O`. It checks
the closure, deterministic matrix, actual input/executor/import identity, tuning,
raw transcript replay, finite comparisons and complete timing order/coverage.
Semantic controls enumerate nontrivial three/four-record distributions, including
repeated observable indices and repeated detector history offsets. Resealed
corruption tests must reject missing coverage, altered counts/input/batch/imports
and malformed timing. Publication accepts exactly one event encoding: `events.jsonl`,
`events.jsonl.gz`, or contiguous `events.part-00000.jsonl.gz` parts. Parts
concatenate after decompression, even across JSON/UTF-8 boundaries. The closure
checks the complete decoded byte digest and event count; missing trailing parts
therefore cannot pass. Each published compressed blob stays below10MiB.

New native SymFT events retain its public counts-sampler `info` receipt, including
the selected active-components flag, raw/reference output policy and dimensions.
The verifier checks reported policy and shape. Older receipts lacking this
additive field make no active-components claim; raw-record workers do not expose
the same receipt. `max_active_qubits` is not treated as a common dense-state
dimension across backends.

Linux `ru_maxrss` receipts may include launcher memory inherited across exec.
The published Linux high-water marks are retained as process observations but
cannot establish simulator-only memory usage or relative engine memory costs.
Future Linux memory comparisons need a separately attributed measurement.
See the [Linux getrusage contract](https://man7.org/linux/man-pages/man2/getrusage.2.html).
