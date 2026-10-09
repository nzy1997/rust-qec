# Original counts comparison against current CPU peers

This manual Linux x86_64 screen compares Rust native postselected counts with
Clifft 0.11.0, its active-width scheduler, and SymFT's pinned official `main`
compiled CPU implementation at
`3f718e9e0c58b277a8fb506b4170863db5c3dbe6` (source package 2026.10.8).
Preparation records the official main and integration-branch refs and requires
exactly one matching `refs/heads/main` record. The offline verifier also requires
the reviewed manifest pin. A matching integration-branch ref cannot substitute
for main. This source build is separate from the PyPI release; revisit pins
before claiming that a result covers currently available peers.

The twelve fixed cells use original MSC distance 3 and 5 circuits, 1/64/1024
attempted shots per call, and separate Strict/Fused Rust arithmetic. All backends
return all-zero **raw** detector postselection and XOR-folded raw observable 0
counts, with reference normalization disabled. SymFT explicitly requests either
legacy or compiled CPU execution; declined compiled requests are retained and
never substituted with legacy timings.

Each of six circuit/call-size combinations tunes five Clifft batches, five
scheduled Clifft batches and seven SymFT executor/batch choices. It freezes the
lowest median of seven valid observations before comparisons. Selected peers
and both Rust policies validate at least 8192 attempts. Rust literal records are
folded independently through the original annotations; peer counts are checked
against Rust and each peer's own raw-record executor using unconditional
Hoeffding bounds with total family alpha 0.001. These finite checks do not certify
rare conditional errors or the full joint distribution. Counts probes retain
an executable RNG carry self-check but do not emit literal continuation words;
no cross-source or cross-backend RNG identity follows from these receipts.

Ten rotated/reversed rounds measure five roles: Rust, direct Clifft, scheduled
Clifft, selected SymFT and an identical Rust-binary control. Each role occupies
each position twice in every cell. The complete ledger retains 759 events:
102 tuning children, 30 selected finite counts children, 18 raw-record children,
600 timing children and nine environment/host inspection children. Each timing
child produces seven adaptive observations of at least 50 ms: 4200 total.
Failed tuning trials and every timing outlier stay in the artifact.

Compilation, preparation, first calls and process high-water RSS are retained
separately. Linux RSS includes interpreter/launcher allocations and does not
establish simulator-only memory costs. The primary comparison is warm counts
work, including the adapters' count conversion. Peer RNGs differ; finite
statistical validation accommodates this while preserving the output contract.

## Collect and verify

Use a clean committed checkout, Python 3.12, gcc/g++, Rust 1.93.1 and an exact
locally available Rust source SHA. Preparation builds native SymFT CPU wheels
with CUDA disabled and verifies the actual verbose compiler flags. It binds
complete wheel code inventories to installed bytes and the wrappers/extensions
actually imported in isolated Python. Rust probes use the reviewed source-pair
preparation with `-C target-cpu=native`, retaining full source inventories,
actual ELF binaries, compiler commands and nonzero arithmetic/RNG test results.
The Rust source must include the required phase-CDF and wide-packet tests.

```sh
python3 -I -m unittest discover -s benchmarks/near_clifford/current_cpu_peer -p 'test_*.py'
python3 -I -O -m unittest discover -s benchmarks/near_clifford/current_cpu_peer -p 'test_*.py'
mkdir -p drafts/current-peer
python3 -I benchmarks/near_clifford/current_cpu_peer/prepare.py \
  --rust-ref "$RUST_SHA" --out drafts/current-peer/preparation
# Choose one permitted logical CPU on a quiet host; preparation must have exited.
taskset -c "$BENCH_CPU" python3 -I benchmarks/near_clifford/current_cpu_peer/run_retained.py \
  --preparation drafts/current-peer/preparation \
  --out drafts/current-peer/output --control drafts/current-peer/control
python3 -I benchmarks/near_clifford/current_cpu_peer/verify.py drafts/current-peer --seal
python3 -I benchmarks/near_clifford/current_cpu_peer/verify.py drafts/current-peer --git-sources \
  --analysis drafts/current-peer/analysis/verified.json
python3 -I -O benchmarks/near_clifford/current_cpu_peer/verify.py drafts/current-peer --git-sources
```

Alternatively dispatch **Near-Clifford x86 application counts** at the committed
protocol branch with `current_cpu_peer=true` and `rust_ref`. Leave
`profile_only=false` and `cdf_source_pair=false`. Its independent Ubuntu VM builds, pins one CPU,
collects, seals before interpretation, verifies normally and under `-O`, and
uploads native originals and partial failures. Existing source-pair and historical
peer modes remain separate.

The offline verifier needs no installed peers or live preparation checkout.
Downloaded bundles can move: original absolute paths are validated against
retained snapshots, wheels, commands, actual imported-module hashes and historical
process-absence receipts. `--git-sources` additionally binds all retained protocol
and Rust source bytes to locally available commits. Keep subsequent reports under
`analysis/`; original bytes and seals stay immutable. A synthetic complete-bundle
fixture tests relocation and resealed corruption rejection, and real transport
tests cover failed, timed-out and cancelled subprocesses. Synthetic inputs supply
no production timing or admission evidence.

A successful screen establishes these fixed workload comparisons on its recorded
host. Confirm gains in an independent complete campaign before making a
performance claim. Runtime-source admission still needs its separate full
resource campaign, fresh review and terminal-green final-head CI.
