# Atom-loss correctness and benchmark evidence

This suite tests the persistent-loss semantics used by RustQEC's Mid-SWAP
walkthrough, then measures sampling and decoding on explicitly stated workloads.
It is an initial benchmark, not a threshold study or a claim of optimal decoding.
The generated publication bundle is in `site/static/data/atom-loss/` and is served
unchanged at `data/atom-loss/` on the documentation site.

## Reproduce

From the repository root, use a fresh working directory for every run. Dataset
export intentionally refuses to overwrite existing bundles.

```sh
python3 -m venv drafts/atom-loss-venv
drafts/atom-loss-venv/bin/pip install -r benchmarks/atom_loss/requirements.txt
cargo build --release --locked -p rustqec-cli --features benchmark-tools,ilp \
  --bin rustqec --example export_matching_benchmark
cargo build --release --locked -p rstim --example atom_loss_sampling_benchmark
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 RAYON_NUM_THREADS=1
drafts/atom-loss-venv/bin/python -m unittest benchmarks.atom_loss.test_reference
drafts/atom-loss-venv/bin/python -m benchmarks.atom_loss.run \
  --work drafts/atom-loss-reproduction --out drafts/atom-loss-results
drafts/atom-loss-venv/bin/python -m benchmarks.atom_loss.decoder_reference \
  --out drafts/atom-loss-results/decoder-correctness.json
drafts/atom-loss-venv/bin/python -m benchmarks.atom_loss.publish \
  --out drafts/atom-loss-results
drafts/atom-loss-venv/bin/python -m benchmarks.atom_loss.verify drafts/atom-loss-results
```

The recorded run uses macOS, serial processes and no explicit CPU affinity.
`provenance-all.json` records the actual CPU, OS, package versions, compiler,
binary hashes, source hashes, command, seed configuration, and timing boundary.
`source-snapshot.json` preserves the exact benchmark sources used in that run;
overlay its `files` on the recorded base commit for historical reproduction.
`bundle.json` seals the figures and raw results with SHA-256 checksums.
Numeric times will vary across machines. Both backends receive exactly the same
public corpus at each decoding point; their private answer key is used only after
decoding. A shared seed does **not** imply identical random samples between Stim
and RustQEC: the sampler check compares distributions, not row equality.

## Correctness reference

`reference.py` independently parses the supported circuit subset, samples loss
onset histories and lowers each history into an ordinary Stim circuit. Gates and
Pauli noise touching an absent wire are skipped until its reset. A lost wire is
left unobserved; retaining its inaccessible state gives the same surviving-wire
statistics as tracing it out. `ML` / `MRL` produce a loss flag followed by the value
(or the documented placeholder). Reset and measurement-reset restore the wire.
Distinct histories are compiled separately; only identical histories are batched.
This Python implementation is deliberately simple and is **not** an optimized
Stim atom-loss implementation.

Supported operations: Z-basis reset and measurement (including inverted and
loss-visible measurement), H/X/Y/Z, CX/CZ, one/two-qubit depolarization and Pauli
errors, LOSS and nested REPEAT. Sampling ignores coordinate/detector annotations.
Unsupported operations and inline measurement noise raise an error. This is not
a reference for every circuit accepted by RustQEC.

`correctness.py` compares complete joint output distributions for 12 small
circuits, with 32,768 shots per implementation per case. Four circuits additionally
have hand-computed deterministic answers. A conservative Hoeffding union bound
covers all histogram bins at familywise alpha <= 1e-6. Removing skipped-gate
semantics must fail a known answer; an unsupported operation must be rejected.
These statistical checks cannot prove equality or validate arbitrarily rare
fault probabilities.

`decoder_reference.py` independently specifies a three-wire parity-check graph,
its base weights log(9), and the loss-conditioned weights. It checks the exported
graph against those hand-derived values, enumerates all eight correction masks
for all 64 possible flag/value input rows, and checks native and PyMatching
predictions against the full set of minimum-weight answers (including ties).
This covers public-row canonicalization, loss-edge mapping and the matching
objective. It does **not** independently validate the general loss-envelope
compiler or claim Bayes-optimal logical-class decoding. It also tests altered
lost-value placeholders. Removing loss conditioning must change the oracle's
optimal-answer set, and a deliberately flipped known answer must be rejected.

## Three experiments

1. **Sampling throughput:** Mid-SWAP d = 3, 5, 7, rounds = d, Pauli probability
   0.001, operation and pre-measurement loss probabilities both 0.003. Both
   implementations return loss-visible rows and pack b8, in batches of 256.
   Rust parses once; auto-sampler preparation is timed on each call. The Python
   reference includes parsing, grouping and per-history Stim compilation.
   Imports, process startup and file I/O are excluded. Rust uses two warmups;
   the Python reference uses one. Plot three-run median and full range. This
   measures the cost of the correctness reference, **not native Stim performance**.
2. **Loss sweep:** d = 3, 5, 7, rounds = d; Pauli probability 0.001; each loss
   probability in 0.0001, 0.0003, 0.001, 0.003, 0.01. Each point uses 5,000 shared
   blinded shots with seed 20260911. Compare native envelope matching, PyMatching
   with the same envelope-conditioned graph, and PyMatching with fixed base
   weights. The last is a loss-conditioning ablation: it still receives the same
   canonical syndromes, so it does not remove every use of loss flags.
3. **Accuracy / time:** d = 3, rounds = 2, Pauli 0.001, loss 0.003, 5,000 shared
   shots, seed 20260912. Add envelope MLE with a 500 ms per-shot timeout. Plot
   logical failure probability against amortized compilation + decoding time,
   with fresh decoder caches in each of three repetitions. Include conditioning
   and graph construction; exclude startup and scoring. PyMatching includes the
   measured common Rust compiler and public-row transformation time, but excludes
   JSON transport/loading. Rust's decode timer may include buffered row reads.
   These are adapter/workflow timings, not isolated matching-kernel timings or
   online p99 latency. Do not infer a universal backend speed ranking.

“Loss probability” means probability **per generated loss opportunity**; both
operation loss and pre-measurement loss receive that value. It is not the total
probability that a wire is lost during an experiment. The exact generated circuit
hash, initial logical-X support derived from its coordinates, public-row hash,
private-answer hash and dataset ID are retained at every decoding point.

Logical error bars are pointwise 95% Wilson intervals. The loss sweep uses
logarithmic axes. Its display is restricted to d = 3 and d = 5, with zero-failure
points omitted and no lines joining across those gaps. The full d = 3, 5, 7
sweep, including zero failures and their nonzero Wilson upper bounds, remains
in the raw JSON and summary CSV. Display omissions do not change scoring. Timing ranges are observed min/max, not confidence intervals.
The curves report failure per entire memory experiment; rounds vary with distance.
There is no threshold fit, accuracy ranking by overlapping intervals, or
extrapolation to other circuits or larger distances.

## Backend adapter and failures

The feature-gated exporter reads **only** the public dataset and shares RustQEC's
compiler. It emits canonical syndromes, visible loss patterns, base graph edges,
loss-to-edge mappings and weights. Python applies exactly the native rule:
active time-like edges get 0.25 times the mean base weight; active space/boundary
edges get 0.5 times that mean; all weights share the native scale normalization.
Parallel edges are permitted only with identical logical labels by the native
compiler. PyMatching keeps the smallest parallel weight, preserving this
nonnegative minimum-weight objective rather than combining independent errors.

Both adapters cache at most 1,024 loss patterns FIFO. Rust also enforces a work
budget; actual graph builds and cache hits are recorded. Backend integer weight
quantization and tie choices can produce prediction differences. The run retains
prediction disagreements and paired native-only / Python-only failure counts;
matching predictions are not assumed identical.

A nonzero exit, timeout or unsupported circuit is recorded as an incomplete run.
No logical error rate is reported from its successful prefix. The publication
step requires all three sweep comparators at all 15 settings; a missing/failed
comparator blocks curve publication rather than silently joining across a gap. The all-shot MLE
success requirement is separate from the matching runs. Any missing graph export
is recorded at case level. This first suite does not benchmark QEC-Playground or
a paper implementation of delayed-erasure decoding.

Primary implementation sources:

- [Stim gates](https://github.com/quantumlib/Stim/blob/main/doc/gates.md#HERALDED_ERASE):
  heralded erasure alone is not persistent absent-wire evolution.
- [PyMatching](https://github.com/oscarhiggott/PyMatching): matching backend;
  the pinned package version is in `requirements.txt` and the run provenance.
- `rstim/src/codegen/midswap.rs`, `rstim/src/executor.rs`,
  `rustqec-cli/src/decode/compiler.rs`, `rustqec-cli/src/decode/matching.rs`:
  generated workload and the production semantics being tested.
