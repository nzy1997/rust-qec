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
  --bin rustqec --example export_matching_benchmark --example export_decoder_oracle
cargo build --release --locked -p rstim --example atom_loss_sampling_benchmark
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 RAYON_NUM_THREADS=1
drafts/atom-loss-venv/bin/python -m unittest benchmarks.atom_loss.test_reference
drafts/atom-loss-venv/bin/python -m benchmarks.atom_loss.run \
  --work drafts/atom-loss-reproduction --out drafts/atom-loss-results
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

The review-fix decoding run reuses all 16 original corpora and verifies their
circuit, public-row and private-answer hashes before timing. Every existing
backend's prediction hash must remain unchanged. `provenance-timing.json` and
`source-snapshot-timing.json` describe this new decoding/oracle run; original
sampling evidence keeps its original provenance. To repeat only decoding with
retained corpora (the work directory must not exist):

```sh
drafts/atom-loss-venv/bin/python -m benchmarks.atom_loss.remeasure \
  --corpora drafts/atom-loss-reproduction \
  --baseline site/static/data/atom-loss \
  --work drafts/atom-loss-retimed --out drafts/atom-loss-retimed-results
```

The retiming output contains decoding results and its provenance; combine it
with the unchanged sampling evidence before publishing a complete bundle.

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
covers all histogram bins at alpha <= 5e-7. Another 5e-7 is allocated to
the analytic channel checks below (combined healthy-check bound <= 1e-6). Removing skipped-gate
semantics must fail a known answer; an unsupported operation must be rejected.
These statistical checks cannot prove equality or validate arbitrarily rare
fault probabilities.

`noise_controls.py` adds 16 analytic single-bit/parity cases at p = 0.17,
covering X/Y/Z errors and one/two-qubit depolarization on live, absent and
reset-restored wires, plus noise before loss. A live DEPOLARIZE2 channel flips
Z parity for 8 of its 15 equiprobable nonidentity Pauli pairs: the expected
probability is `8p/15 = 0.0906667`. Both implementations must match the analytic
probabilities; zero-probability controls require exactly zero events. The same
acceptance checks are actually rerun with each of the five native-input channels
deleted. All five mutations must fail. A regression deletes DEPOLARIZE2 from
all native inputs and requires the **overall** correctness report to fail.

`decoder_reference.py` independently specifies three- and five-wire parity-check
graphs, their base weights log(9), and loss-conditioned weights. It checks the
exported graphs and loss-to-edge mapping against hand-derived values, enumerates
all correction masks for all 64 + 1,024 flag/value rows, and checks native and
PyMatching predictions against the full set of minimum-weight answers (including
ties). Altering lost-value placeholders must leave predictions unchanged.

The five-wire witness has two compatible corrections, 11100 and 00011. With the
first three wires lost, fixed costs 3 versus 2 uniquely prefer logical 0;
conditioned costs 1.5 versus 2 uniquely prefer logical 1. An actual PyMatching
adapter that ignores conditioning is run through the same acceptance rule and
must fail this witness. A deliberately flipped native prediction must also fail.
A regression additionally substitutes the broken adapter for the healthy one
and requires the overall correctness report to fail. This covers the matching
objective and public-row transformation, not the general loss-envelope compiler
or Bayes-optimal logical-class decoding.

`chain_reference.py` adds a finite, real Mid-SWAP d=3, two-round chain check
using the committed `fixtures/midswap_d3_r2.stim` (16 detectors). It covers four
private onset histories (none, early, middle, late), enumerates every single
Pauli-fault choice on each physically lowered circuit and samples two measurement
outcomes per trace. This produces 5,996 traces including no-fault controls; duplicate records are removed and
paired alternative lost-value placeholders are added. These enriched inputs are
not IID samples and are never used to estimate a logical failure rate.

Stim independently constructs the correlated Pauli distribution, propagates 897
Pauli probes to build loss-envelope candidate sets, and transforms public rows
to canonical syndromes. The declared envelope model resolves CX as H-CZ-H,
including both target-basis boundaries. The Rust compiler's full correlated
Pauli distribution (coalescing identical effects), candidate sets and row
transformation must match. Altered compiler weights and missing candidates must
be rejected. Exact min-plus dynamic programming over all 131,072 detector/logical
parity states checks matching predictions against the independently constructed
graph. Constant-zero, constant-one and flipped predictions must fail; changing
lost-value placeholders must not change either backend's output.

For MLE, different equivalent Bernoulli decompositions can have different
most-likely **fault configurations**. After independently validating its physical
distribution and candidates, we enumerate the native model's representation
without using ILP and check MLE predictions against all minimum-cost answers.
Thus the MLE objective check uses a **validated native representation**, not an
independently chosen Stim fault decomposition. This is compositional evidence
for this finite fixture and the stated envelope objective; it neither proves
arbitrary compiler inputs correct nor establishes physical logical-class Bayes
optimality. The feature-gated model exporter reads public inputs only and is
separate from the timed matching exporter.

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
   with fresh decoder caches in each of three repetitions. The main PyMatching
   comparators use `decode_batch`; fixed-weight per-shot calls are also retained
   as an API control and must produce identical predictions to the batch path.
   Each repetition reruns and remeasures the common Rust compiler and public-row
   transformation. Python decode time includes array conversion, loss-pattern
   grouping, graph construction, batched decoding and reordering predictions.
   Startup, scoring and JSON transport/loading are excluded. Native decode time
   includes buffered public-row reads and output packing/flush; the exporter's
   transformation stage also includes public-row reads. These I/O boundaries
   differ, so this is not a fully identical end-to-end process comparison.
   Each batch repetition also records array/group/selection, graph construction,
   decode_batch API calls, output reordering, and remaining adapter/timer overhead.
   The supplementary stage chart uses additive means of these phases plus the
   shared compiler/transform stages. Its graph-construction percentage uses
   Python adapter time as the denominator; native decode remains an aggregate.
   Instrumentation overhead is retained, not subtracted.
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
in the raw JSON and summary CSV. An additional full-sweep figure shows all 15
settings: zero-event points use downward arrows at the exact one-sided 95%
binomial upper bound `1 - 0.05**(1/N)` (about 0.000599 for N = 5,000), not a
positive measured rate. Nonzero points retain Wilson intervals. Display
omissions do not change scoring. Timing ranges are observed min/max, not confidence intervals.
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

The native adapter caches at most 1,024 patterns FIFO and enforces a work budget.
The batch Python adapter groups all shots by visible loss pattern, builds one
graph per group, calls the official `decode_batch` interface and restores input
order. Grouping and reordering costs are included; this offline batch policy
requires retaining the batch and differs from the native streaming cache policy.
Only the per-shot Python API control uses the 1,024-entry FIFO cache. Graph builds,
batch calls and available cache statistics are recorded. Backend integer weight
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
- [Stim DEPOLARIZE2 definition](https://github.com/quantumlib/Stim/blob/main/doc/gates.md#the-depolarize2-instruction):
  each of the 15 nonidentity Pauli pairs has probability p/15; eight flip Z parity.
- [PyMatching batch decoding](https://pymatching.readthedocs.io/en/stable/#decoding-stim-circuits):
  official `decode_batch` interface for reducing per-shot Python overhead;
  the pinned package version is in `requirements.txt` and the run provenance.
- `rstim/src/codegen/midswap.rs`, `rstim/src/executor.rs`,
  `rustqec-cli/src/decode/compiler.rs`, `rustqec-cli/src/decode/matching.rs`:
  generated workload and the production semantics being tested.
