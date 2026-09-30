# Paired near-Clifford scale campaign

From the repository root:

```sh
python3 benchmarks/near_clifford/scale/run.py
```

The fixed 43-configuration matrix compares the merged #764 (`7cc2fa86`) and #766
(`f4b59661`) source snapshots. `git archive` extracts each revision into ignored
`drafts/near-clifford-scale/`. A separate Cargo harness depends on these pristine
sources; the production workspace and simulator are not edited. This also keeps
benchmark instrumentation outside the performance measurements.

There are 23 scale configurations, 12 mixed-circuit configurations, and 8 additional
usage configurations. Each configuration runs in three paired process invocations,
with three repetitions of each mode per process. Baseline/candidate order alternates.
The exact circuits, raw repetitions, binary hashes, harness hashes, lock hashes,
source SHAs, host, and compiler are retained in JSON. Compare within-host ratios;
a different compiler or dependency lock makes older absolute measurements unsuitable
as direct comparisons.

Compile and prepare are separate. Cold includes first preparation (including measurement-order planning), structured sampling,
and sampler disposal; its fresh executor is compiled outside the timer. The public
API and preparation measurements also use fresh executors.
First call excludes prepare. Warm structured and flat calls retain separate samplers
after a 64-shot warmup; caches can continue growing across repetitions. Output
allocation is timed, destruction and RNG initialization are excluded. Each process
reports peak RSS including validation, caches, and output allocations; this is not
an estimate of cache memory alone. Zero-shot times mostly measure call overhead.

Before measurements, each circuit checks 16 seeded shots: structured versus flat,
prepared versus individual execution, and the next RNG value. The individual outputs
and RNG continuation must also match across revisions. These checks establish
implementation consistency, not an independent statistical validation of the physics.
The syndrome workloads are synthetic performance probes, not logical-error benchmarks.

Untimed diagnostic builds apply a version-checked source overlay. They expose prepared
rank/coefficient count, terminal-plan availability and cache-node budgets. After a
64-shot warmup, a 256-shot probe counts symbolic suffix calls, measurement cache
reuse/build events, coefficient/depth/node-limit fallbacks, and unplanned shots.
Observed rank includes executor instruction boundaries and states before axis
retirement; it is an observed maximum on seeded trajectories, not a guarantee over
all possible trajectories. Snapshot and counter field names are recorded in JSON.
Both pristine and diagnostic binaries are hashed. Diagnostic builds repeat the
output/RNG checks against pristine builds before probing. Timings never use the overlay.

`--quick --pairs 1 --repetitions 1` is a smoke run, not performance evidence.
`--only random_129 rank_12` restricts circuits; `--skip-diagnostics` omits probes.
The runner checkpoints completed configurations. A failed verification or timeout
stops the run; only a file with `completed_utc` represents a completed campaign.

After timing finishes, macOS profiles can be reproduced with:

```sh
python3 benchmarks/near_clifford/scale/profile.py path/to/results.json
```

This creates a separate release build with line debug information and frame pointers,
then samples each warmed workload for eight seconds. These profiles are diagnostic
evidence; their driver timings are not campaign measurements. The source revision,
profile driver/binary/lock hashes and exact circuit/profile hashes are recorded.

Use `verify.py results.json` to check the retained campaign and source hashes;
add `--binaries drafts/near-clifford-scale` to check local timing binaries too.
`repeat.py results.json FIXTURE SHOTS --output confirmation.json` repeats one
suspect configuration with those exact binaries (six pairs by default).
`plot.py results.json --output figure-stem` renders the scale panels with matplotlib.

For a new optimization, use clean committed revisions with the same matrix:

```sh
python3 benchmarks/near_clifford/scale/run_pair.py \
  --baseline f4b5966129aee3d617181414f805eb1250e1cc5d --candidate HEAD \
  --output drafts/near-clifford-pauli/results.json
```

The wrapper defaults to a separate `drafts/near-clifford-pauli` build directory;
its hash and selected source commits are retained alongside the original runner hash.

Always use a fresh `--scratch` directory for a new source pair. Archive extraction
restores tracked files but does not remove files deleted between revisions; directory
reuse across different source pairs is unsupported. The retained current campaigns
used separate task-owned directories and their source pairs have no deleted files.
