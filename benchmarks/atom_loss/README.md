# Atom-loss correctness and benchmark evidence

This suite tests the persistent-loss semantics used by RustQEC's Mid-SWAP
walkthrough, then measures sampling and decoding on explicitly stated workloads.
It is an initial benchmark, not a threshold study or a claim of optimal decoding.
The generated publication bundle is in `site/static/data/atom-loss/` and is served
unchanged at `data/atom-loss/` on the documentation site.

## Reproduce

Freeze all experiment source changes in a source commit **S**, then generate the
entire bundle from a fresh, clean detached checkout of S. Use output and work
directories outside that checkout. The entry point builds all five release
binaries from scratch with `--locked`, runs correctness, sampling, the 16-case
decoding experiment and the separate 48-case seed experiment serially, then
packs, renders and verifies the complete bundle.

```sh
# Run from the main checkout after committing the experiment sources.
python3 -m venv drafts/atom-loss-venv
drafts/atom-loss-venv/bin/pip install -r benchmarks/atom_loss/requirements.txt
PYTHON="$(pwd)/drafts/atom-loss-venv/bin/python"
OUTPUT="$(pwd)/drafts/atom-loss-clean-results"
WORK="$(pwd)/drafts/atom-loss-clean-work"
git worktree add --detach ../rustqec-evidence-source HEAD
cd ../rustqec-evidence-source
"$PYTHON" -m benchmarks.atom_loss.evidence_run --work "$WORK" --out "$OUTPUT"
"$PYTHON" -m benchmarks.atom_loss.decoder_replay --root "$OUTPUT"
```

Copy the resulting bundle into `site/static/data/atom-loss/` and commit artifacts
in a subsequent commit **E**. `source_commit` means S, not E: requiring an artifact
to contain its own commit hash would be circular. The verifier independently
reconstructs the complete input inventory from Git and Cargo workspace members.
All files in every workspace member, benchmark code/fixtures/requirements,
workspace manifests/lockfile, toolchain/configuration and the evidence CI workflow
must match the recorded inventory at the current checkout. New, missing or
changed inputs fail; only changes outside that declared source scope, such as
site artifacts, can follow S without invalidating evidence. Squash merging may
remove S from the current checkout's ancestry or object database; when the
object is available, the verifier also checks its tree against the recorded
inventory. If the object is absent, the verifier checks current source-content
equivalence, not the historical identity of S. This deliberately conservative
inventory can require regeneration
even for non-computational changes within a workspace member.

`source-manifest.json` records S, the complete Git blob/mode inventory and digest,
exact build commands, compiler/Cargo versions and all five measured binary hashes.
Builds and measurement subprocesses receive an allowlisted environment with a
fresh isolated Cargo home. Inherited Cargo profile/target options, Rust flags,
native compiler flags and loader overrides are not forwarded. External ancestor
`.cargo/config` files are rejected; checkout-local Cargo configuration is source
bound. The effective build environment is recorded (network credentials are
excluded). A regression executes the real entry point with optimization disabled
in the parent environment and requires an optimized release binary without debug
assertions; an ancestor profile override must be rejected before building.
Every original and independent-seed case records the fixed Pauli probability
0.001. Both bundle and standalone archive validators reject changed or missing
noise metadata; generator replay uses the declared value and requires an exact
circuit match. Coherently resealing JSON, archive indices and checksums cannot
make a false noise setting valid.

Every stage provenance and source snapshot must agree with that manifest and the
current source files. Clean builds and clean source state are checked before and
after measurement. Python packages are pinned in `requirements.txt` and actual
versions, CPU, OS, threading environment and timing boundaries are recorded.
The verifier compares recorded dependencies with the source-bound pins, compiler
identity with the build manifest, and CPU/OS/Python identity across all stages.
Recorded thread settings must match the single-thread measurement policy. These
checks reject contradictory metadata; they do not authenticate historical timings
or require the verification machine to match the measurement host.
`bundle.json` seals all required artifacts with SHA-256; source binding is an
additional check, not inferred from those checksums.

Workload metadata is also checked independently: loss-flag positions are derived
from the archived circuit and unique patterns are counted from public shot bytes.
Offline graph builds, PyMatching batch calls, fixed-weight controls, shot/circuit
identities, adapter API/policy labels and the declared cyclic run order must agree
with those inputs and the fixed experiment definition. Current-code decoder replay
additionally compares exported graph dimensions and all retained non-time compiler,
cache and decoder counters against each original repetition. Approximate native
pattern estimates keep their explicit flag; they are not treated as exact unique
counts. These checks validate reproducible work, not historical wall-clock time.

The full verifier then redraws all eight charts from the validated JSON in a
fresh temporary directory, using the source-bound renderer. SVG IDs are fixed and timestamps omitted. Anonymous path coordinates may differ
by at most 0.000001 pt (one serialized decimal unit) because platform math
libraries can round an endpoint differently. The bound is absolute, never
relative; path commands/separators and all other bytes, including text, styles,
transforms, IDs, references, viewBox and font glyphs, must match exactly. Every
PNG must match in decoded pixels, dimensions, mode and metadata (compression
bytes alone may differ). No perceptual image tolerance or skipped platform check is used. The renderer resets ambient plotting styles, uses bundled DejaVu fonts,
and requires the pinned plotting dependency stack and the wheel's bundled
FreeType 2.6.1. A rendering-environment mismatch fails explicitly; it is not
accepted as evidence equivalence. Published `methodology.md` must also equal this
source-bound README. Replaced SVGs, changed PNG pixels, accidentally swapped
figures and altered methodology must fail even after their checksums are updated,
in both normal and optimized Python. The standalone archive rescorer remains a
stdlib-only numerical check; it does not validate website figures.

CI fetches S, builds the current code and regenerates public samples/private keys
for all 64 corpora. `decoder_replay` separately re-decodes every backend/corpus
combination (213 combinations, covering all 345 archived prediction files), with
only public inputs in each decoder work directory. It compares prediction bytes
and hashes, failure counts, Wilson intervals and recorded paired discordances.
An executable mutation that flips a current native prediction while retaining the
old archive must fail. Missing manifest inputs and a changed decoder/build input
must also fail, including under optimized Python.

CI prediction replay validates results, **not historical performance**. It does
not compare machine-dependent binary hashes or timing values across platforms.
Any change in the measured source scope requires a new clean source commit and
fresh evidence generation; replay alone cannot justify retaining old timings.
Neither Git hashes nor CI provide protection against an author deliberately
rewriting both the experiment and its checks; the source contract remains
reviewable code.

The recorded measurements use serial processes on one machine without explicit
CPU affinity. The three cyclic timing rotations do not completely balance all
backend order positions. Small differences are not evidence of a stable ranking;
timing repeats reuse each corpus and do not increase its accuracy sample size.
Both decoders receive the same public corpus at each point, with scoring keys
used only afterward. A shared seed does not imply row-identical samples between
Stim and RustQEC; independent sampler checks compare distributions.

### Download and rescore without building Rust

`shot-data-v1.zip` contains all 16 synthetic corpora, public/private manifests,
circuits, scoring keys and **all 198 prediction files** (three repetitions of
every backend), plus the decoding/tradeoff result JSON, per-file hashes and a
standalone `rescore.py`. The private keys are published for auditing, but are
never supplied to either decoder during measurement. After downloading the ZIP,
extract `rescore.py` and run with any Python 3.10+ installation:

```sh
python3 rescore.py rescore shot-data-v1.zip
```

Expected: `PASS: 16 corpora; 198 prediction files rescored`. No third-party
Python packages or native binaries are needed. The checker verifies completeness, public/private schema and row formats,
corpus/prediction hashes, scoring answers derived independently from measurement
observable parity XOR input mask, failure rates and paired disagreements. Repository
verification additionally requires the archived results to match the public JSON.
Checksums detect inconsistency, not authenticity against rewriting all evidence;
use the Git commit for the versioned source of the bundle.
The standalone tool requires the fixed 16 settings, four sweep comparators and
six tradeoff comparators, three complete repetitions and all 198 prediction files. It rejects
omitted comparisons or paired fields, stale Wilson intervals, incorrect workload
metadata and non-integer count fields, even when the ZIP index is resealed and
Python optimization is enabled. It validates scoring evidence; historical timing
and build/source provenance require the full repository verifier.


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
loss-visible measurement), H/S/S_DAG/X/Y/Z, CX/CZ, one/two-qubit depolarization and Pauli
errors, LOSS and nested REPEAT. Sampling ignores coordinate/detector annotations.
Unsupported operations and inline measurement noise raise an error. This is not
a reference for every circuit accepted by RustQEC.

`correctness.py` compares complete joint output distributions for 12 small
circuits, with 32,768 shots per implementation per case. Four circuits additionally
have hand-computed deterministic answers. A conservative Hoeffding union bound
covers all histogram bins at alpha <= 5e-7. Another 5e-7 is allocated to
the analytic channel checks below: 4e-7 for the original 16 events and 1e-7 for
all joint bins and marginals of the 26 distribution probes (combined healthy-check
bound <= 1e-6 before the low-probability checks below). Removing skipped-gate
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

`channel_probes.py` adds 26 analytic distribution probes. Bell preparation and
inverse Bell readout resolve all four single-qubit and all sixteen two-qubit
Pauli labels, including identity, at p = 0, 0.17, 0.6 and 1. Nonidentity bins
have probability p/3 or p/15. Higher-noise probes make an omitted component
resolvable at the chosen sample size; the check does not rely solely on p=0.17.
Nine X/Y/Z product-basis combinations check complete two-bit distributions and
both individual marginals. For a two-qubit product eigenstate, the ideal-outcome
probability is 1-4p/5 and each of the other three outcomes has probability 4p/15;
each bit flips with probability 8p/15. Both loss directions, reset restoration,
noise before loss and single-qubit X/Y/Z probes are also covered.

Each probe uses 32,768 samples per implementation, with one-sample Hoeffding
bounds unioned over every joint bin and marginal for both implementations.
Zero/one probabilities must hold exactly. Tests resolve each deterministic
Pauli label against hand-derived Bell syndromes. Five actual replacement
mutations must fail: IX-only, XI-only, independent X errors on both wires, and
X-only or Z-only single-qubit depolarization. An IX-only regression requires
the **overall** sampling report to fail using the same acceptance criteria.
This is finite statistical evidence for the specified stochastic Pauli channels,
not a proof for arbitrary noise channels or arbitrarily small probability biases.

`low_probability.py` adds fifteen **unamplified** probes: each of the five Pauli
channels at p = 0.001 and LOSS at 0.0001, 0.0003, 0.001, 0.003 and 0.01.
The five half-rate primitive LOSS probabilities used at two-qubit operations
are included too, down to 0.00005. Each uses at least 262,144 shots per
implementation (520,000 at p = 0.00005). Exact equal-tail binomial count
intervals cover joint bins, individual marginals and nonidentity events, with
Bonferroni allocation over both implementations (family alpha <= 1e-7).
This avoids applying a high-probability absolute tolerance to rare events.
The smallest LOSS probability has about 26 expected events; zero events are
outside its acceptance interval. These probes resolve channel deletion, not
arbitrarily small relative biases or every possible low-rate Pauli component.

The same module independently checks the actual d=3, rounds=2 Mid-SWAP fixture
(pPauli=0.001, pLoss=0.003) through `dataset export --mode measurements_blinded`,
using 65,536 shots per implementation. It prepares the two logical inputs in
Stim using the native export's private input masks; it does not use the envelope compiler. A separate deterministic check reads
the private answers and verifies every one against observable parity XOR input
mask; private answers still never enter either decoder. Within each input stratum, 50 measurement
bits (including loss flags), 16 detector parities, the observable and 15 adjacent
detector joints are compared. Another 16 detector marginals conditioned on no
visible loss separate the Pauli signal from loss-induced syndromes. The 196
preselected two-sample Fisher tests use Bonferroni alpha <= 1e-7. This is a
finite set of marginal/joint checks, not the full 50-bit joint distribution.

Three actual input mutations delete only low-rate Pauli noise, only low-rate
LOSS, or both (`0 < p < 0.01`). Every affected analytic probe must reject its
mutation; the real export-path comparison must independently reject all three.
The real fixture loses 12, 19 or 31 instructions respectively. A separate
regression corrupts only the exporter, leaving ordinary circuit sampling intact.
The combined healthy sampling-check familywise bound is <= 1.2e-6. These larger
correctness samples are separate from the 5,000-shot accuracy benchmark and do
not increase its statistical sample size.

`summary.csv` is regenerated from raw counts and phase times: every field,
all 66 rows and the exact header are checked, including rate, Wilson interval
and median workflow time. Missing, duplicate or extra rows and resealed changes
to any field must fail. Wilson intervals in the source JSON are also recomputed.

All executed noise deletion/replacement controls retain their measured negative
observations, including the real-circuit Fisher event counts. The independent
report checker recomputes their decisions and failure lists, checks unchanged
reference observations, and verifies the instruction counts and complete affected
probe inventory. Low-probability controls must reject every affected probe;
`LOSS_0.01` is outside the deletion range and cannot substitute for one. Changing
a failure summary or reusing healthy counts as negative evidence fails verification
even after checksums are resealed and with optimized Python.

The updated correctness report is linked to `provenance-correctness.json` and
`source-snapshot-correctness.json`. All sampling, correctness, decoding and seed records are regenerated from the same clean source commit. The fixed workload configurations and declared seeds are retained; timings are newly measured.

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

The three/five-wire report retains the exported graph, every public syndrome and
loss pattern, and all healthy and deliberately defective predictions. A separate
standard-library checker derives the two complementary repetition-code corrections
and checks their logical costs, graph/loss mappings, all rejection lists and the
strict witness. Missing backends, failed case flags and contradictory witness
predictions are rejected even after resealing checksums, including under `python -O`.
CI reruns this oracle and validates the published observations against the same
hand-derived definition; legal choices among tied optima remain acceptable.

`chain_reference.py` adds a finite, real Mid-SWAP d=3, two-round chain check
using the committed `fixtures/midswap_d3_r2.stim` (16 detectors). It covers four
private onset histories (none, early, middle, late), enumerates every single
Pauli-fault choice on each physically lowered circuit and takes one canonical
`reference_sample()` measurement witness per trace. Unlike seeded compiled
sampling, this is independent of the host SIMD random stream. This produces
2,998 traces including no-fault controls; duplicate records are removed and
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
graph. All 752 rows run through native streaming matching, native MLE, and
both actual exported-graph batch adapters (PyMatching and native offline). Each
output is scored against the independent allowed answers, rather than agreement
with another backend. Constant-zero, constant-one and flipped predictions must
fail; changing lost-value placeholders must not change any backend's output.

Both batch adapters also execute three deliberately corrupted exports: empty
edges, empty loss-to-edge mappings, and all observable-labelled edge weights
multiplied by 1e-6 with the mean recomputed. Empty topology may cause an explicit
decoder error; the other mutations must complete and produce oracle-rejected
answers. The report retains row-level allowed answers, predictions and rejection
indices. The verifier recomputes their summaries and requires every backend and
negative control; CI reruns the full chain, compares its independently recomputed
oracle definitions and measurement-row hash with the published artifact, and revalidates both sets of
observations. Different choices among equally optimal logical answers are allowed. These checks cover this finite fixture and four
loss histories, not arbitrary graph exports or all combinations of loss flags.

For MLE, different equivalent Bernoulli decompositions can have different
most-likely **fault configurations**. After independently validating its physical
distribution and candidates, we enumerate the native model's representation
without using ILP and check MLE predictions against all minimum-cost answers.
Thus the MLE objective check uses a **validated native representation**, not an
independently chosen Stim fault decomposition. This is compositional evidence
for this finite fixture and the stated envelope objective; it neither proves
arbitrary compiler inputs correct nor establishes physical logical-class Bayes
optimality. This independent physical oracle covers d=3, two rounds and four loss
histories; replay agreement on d=5/7 and multiple losses is not independent proof
of those broader physical models. The feature-gated model exporter reads public inputs only and is
separate from the timed matching exporter.

## Evidence contract

The verifier treats raw observations as evidence and recomputes derived fields;
a checksum or stored PASS is not an authority for internal consistency.

| Artifact | Required observations and relationships |
| --- | --- |
| Public/private corpus | Version, mode, shot count, row width/stride/bit order, circuit measurement/detector/observable counts, exact byte lengths, zero padding, file hashes and recomputed dataset identity agree. Masks and answers contain exactly one binary value per shot. |
| Scoring key | Independently parse observable record indices, calculate parity from each public measurement row, and XOR its private input mask. Every resulting bit equals the scoring answer, both during real export checks and standalone archive rescoring. |
| Small-circuit report | Complete Rust/reference histogram counts cover every shot; recompute joint delta, declared tolerance and deterministic known answers. |
| Analytic/distribution reports | Complete named cases and both backends; expected probabilities come from hand-derived probe specifications. Marginals, event rates and tolerances agree with observations and verdicts. |
| Low-probability reports | Both backends have complete joint counts. Marginal/nonidentity counts follow from that joint histogram. Recompute exact binomial intervals from probability and sample size. |
| Real-circuit report | All 196 preselected events and consistent stratum sizes are present. Recompute Fisher p-values from raw counts, the Bonferroni threshold and the verdict; scoring-key coverage equals the full shot count. |
| Summary/timing tables | Every field and row matches underlying counts and phase timings; Wilson intervals and totals are recomputed. |

The report verifier uses a separate standard-library implementation of binomial
quantiles (normalized PMF recurrence) and Fisher tests (hypergeometric sums).
SciPy produces the original statistics; tests compare the independent formulas.
Floating-point comparisons allow numerical roundoff, but acceptance is decided
from recomputed statistics. The standalone download remains Python 3.10+ with
no external dependencies. Its updated script is included in the shot archive;
original corpus and existing prediction bytes remain unchanged; decoder timing JSON is replaced by fresh measurements.

Regression matrices delete fields and observations, corrupt formats, truncate
packed data, alter padding, flip one/all answers and masks with coherent file
checksums, and change derived values/verdicts. Actual exporter answer corruption
must fail the independent check. These are consistency checks, not authenticity
against someone deliberately replacing every source and all evidence together.

## Three experiments

1. **Sampling time:** Mid-SWAP d = 3, 5, 7, rounds = d, Pauli probability
   0.001, operation and pre-measurement loss probabilities both 0.003. Both
   implementations return loss-visible rows and pack b8, in batches of 256.
   Rust parses once; auto-sampler preparation is timed on each call. The Python
   reference includes parsing, grouping and per-history Stim compilation.
   Imports, process startup and file I/O are excluded. Rust uses two warmups;
   the Python reference uses one. The page compares their recorded milliseconds per
   256-shot batch in a table, with medians and min/max over three runs calculated
   directly from `sampling.json`. A prominent note above the table explains
   that the Python reference is unoptimized and includes parsing, loss-history
   circuit construction and Stim compilation, while Rust parsing is excluded.
   These costs have different boundaries: this is **not native Stim performance**
   and no backend speedup ratio is inferred. The separate Rust absolute-throughput
   and reference-cost figures remain available in the evidence bundle.
2. **Loss sweep:** d = 3, 5, 7, rounds = d; Pauli probability 0.001; each loss
   probability in 0.0001, 0.0003, 0.001, 0.003, 0.01. Each point uses 5,000 shared
   blinded shots with seed 20260911. Compare native envelope matching, PyMatching
   with the same envelope-conditioned graph, and PyMatching with fixed base
   weights. The last is a loss-conditioning ablation: it still receives the same
   canonical syndromes, so it does not remove every use of loss flags.
   `timing-sweep.svg` also shows all 15 settings, with three-run medians/ranges,
   individual panel log y-ranges and no omitted timing points. `timing-sweep.csv`
   exposes all 180 decoder repetitions, exact input-pattern counts, graph builds,
   and native streaming cache hits (blank for offline groups). Native offline
   and PyMatching use the same group policy; native streaming remains separate.
   Compare all settings and their repeat ranges, not a universal kernel ranking.
3. **Accuracy / time:** d = 3, rounds = 2, Pauli 0.001, loss 0.003, 5,000 shared
   shots, seed 20260912. Add envelope MLE with a 500 ms per-shot timeout. Plot
   logical failure probability against amortized compilation + decoding time,
   with fresh decoder caches in each of three repetitions. The main PyMatching
   comparators use `from_check_matrix` and `decode_batch`; fixed-weight per-shot calls are also retained
   as an API control and must produce identical predictions to the batch path.
   Backend order rotates each repetition, with serial execution and no concurrent
   build/test workload. Each repetition reruns and remeasures the common Rust compiler and public-row
   transformation. Python decode time includes sparse topology/weight preparation
   once per invocation, array conversion, loss-pattern
   grouping, graph construction, batched decoding, reordering predictions, and b8 output write/flush.
   Startup, scoring and JSON transport/loading are excluded. Native decode time
   includes buffered public-row reads and output packing/flush; the exporter's
   transformation stage also includes public-row reads. Both offline adapters use the same public model exporter and exclude JSON transport/loading; both include output b8 encoding, writing and flush (without fsync). The native streaming CLI remains a separate policy comparison.
   Each batch repetition separately records sparse topology/weight preparation,
   array/group/selection, bulk graph construction,
   decode_batch API calls, output reordering, and remaining adapter/timer overhead.
   The supplementary stage chart uses additive means of these phases plus the
   shared compiler/transform stages. Its graph-construction percentage uses
   Python adapter time as the denominator; native decode remains an aggregate.
   Instrumentation overhead is retained, not subtracted.
   These are adapter/workflow timings, not isolated matching-kernel timings or
   online p99 latency. Do not infer a universal backend speed ranking.

The horizontal coordinate pLoss is the **configured loss parameter**, passed
to both operation loss and pre-measurement loss. Single-qubit and measurement
opportunities use pLoss; each target of a two-qubit operation uses pLoss/2. It is not the total
probability that a wire is lost during an experiment. The exact generated circuit
hash, initial logical-X support derived from its coordinates, public-row hash,
private-answer hash and dataset ID are retained at every decoding point.

Logical error bars are pointwise 95% Wilson intervals. The default loss-sweep
figure shows all 15 settings at d = 3, 5, 7 on logarithmic axes. Zero-event points
use downward arrows at the exact one-sided 95% binomial upper bound
`1 - 0.05**(1/N)` (about 0.000599 for N = 5,000), not a positive measured rate.
Nonzero points retain Wilson intervals. The optional d = 3/5 detail omits
zero-failure points, with no lines joining across those gaps. Both figures use
the same complete raw JSON and summary CSV; display omissions do not change
scoring. Timing ranges are observed min/max, not confidence intervals.
The curves report failure per entire memory experiment; rounds vary with distance.
Timing repeats reuse one corpus; they do not turn 5,000 shots into 15,000
independent accuracy samples. Many low-loss points have only 0–5 failures and
cannot resolve small differences. At the d=3 two-round tradeoff point, conditioned
versus fixed weights have 0 conditioned-only and 13 fixed-only failures; this
paired evidence supports improvement at that setting only. MLE's 1/5,000 has
large relative uncertainty and remains tied to its fault-configuration objective.
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
graph per group with the official `from_check_matrix` interface, calls `decode_batch` and restores input
order. The sparse check/fault matrices and base/conditioned weight arrays are
prepared once inside each measured invocation; each pattern copies and conditions
its weights. Parallel edges use `smallest-weight` and boundary edges use a virtual
boundary node. No topology preparation is amortized across timing repetitions.
Grouping and reordering costs are included; this offline batch policy
requires retaining the batch and differs from the native streaming cache policy.
Only the per-shot Python API control uses the 1,024-entry FIFO cache. Graph builds,
batch calls and available cache statistics are recorded. Backend integer weight
quantization and tie choices can produce prediction differences. The run retains
prediction disagreements and paired native-only / Python-only failure counts;
matching predictions are not assumed identical.

A nonzero exit, timeout or unsupported circuit is recorded as an incomplete run.
No logical error rate is reported from its successful prefix. The publication
step requires all four sweep comparators at all 15 settings; a missing/failed
comparator blocks curve publication rather than silently joining across a gap. The all-shot MLE
success requirement is separate from the matching runs. Any missing graph export
is recorded at case level. This first suite does not benchmark QEC-Playground or
a paper implementation of delayed-erasure decoding.

Primary implementation sources:

- [Stim gates](https://github.com/quantumlib/Stim/blob/main/doc/gates.md#HERALDED_ERASE):
  heralded erasure alone is not persistent absent-wire evolution.
- [Stim DEPOLARIZE2 definition](https://github.com/quantumlib/Stim/blob/main/doc/gates.md#the-depolarize2-instruction):
  each of the 15 nonidentity Pauli pairs has probability p/15; eight flip Z parity.
- [PyMatching bulk graph construction](https://pymatching.readthedocs.io/en/stable/api.html#pymatching.Matching.from_check_matrix):
  sparse parity-check and fault matrices with explicit weights and edge merge policy.
- [PyMatching batch decoding](https://pymatching.readthedocs.io/en/stable/#decoding-stim-circuits):
  official `decode_batch` interface for reducing per-shot Python overhead;
  the pinned package version is in `requirements.txt` and the run provenance.
- `rstim/src/codegen/midswap.rs`, `rstim/src/executor.rs`,
  `rustqec-cli/src/decode/compiler.rs`, `rustqec-cli/src/decode/matching.rs`:
  generated workload and the production semantics being tested.


## Deterministic origin and optimized-mode validation

`validate_dataset` reconstructs the private mask from the declared seed and
batch size, including domain-separated SHA-256 keys, ChaCha12 words, rand 0.8
boolean sampling and the per-batch 64-bit `usize` shuffle. The standalone ZIP
checker implements this independently in Python. All 16 original masks match.
This replay is explicitly tied to rand 0.8 / rand_chacha 0.3 on a 64-bit host;
it does not assume future versions of Rust `StdRng` preserve their stream.
A regression changes masks and answers together to force a chosen decoder to
zero failures, reseals the private manifest, and requires rejection.

`python -m benchmarks.atom_loss.replay` also regenerates the original circuits
and public/private sample bytes using the declared configuration. All 16 match.
The fixed seeds are 20260911 for the sweep and 20260912 for tradeoff. CI repeats
this check. Checksums alone establish internal integrity, not an authenticated
source or immunity to an author changing the generator and the experiment plan.

Every production Python evidence check uses explicit exceptions. CI runs the
verifier both normally and with `python -O`; resealed corrupted chain metadata
must fail in both modes. Tests may still use unittest assertions.

## Matched offline policy and new timing measurements

The additional `envelope-matching-offline` executable is a feature-gated
benchmark adapter using rmatching. Both offline adapters consume the same
exported public graph, group the whole batch in first-appearance pattern order,
build one graph per distinct loss pattern, decode each group in batch, restore
original shot order and write/flush the b8 predictions. They retain the batch
and only one group's matching graph at a time. Preparation and grouping are
inside their measured intervals. JSON transport/loading and process startup
are excluded on both sides; each repetition freshly measures the same compiler
and public-row transformation. There is no output fsync on either side.

The original streaming CLI remains in the charts, explicitly as a different
FIFO/work-budget policy. Its output still includes buffered writes and flush.
Offline native predictions must equal streaming predictions on every retained
corpus; graph build counts must equal the number of distinct input patterns.
The three repetitions on each original corpus are fresh measurements with
rotated serial backend order. Previous prediction values and sample bytes are
unchanged. These are measured adapters, not language or kernel rankings.

## Prospectively fixed multi-seed accuracy check

Before sampling, the additional seeds were fixed to **2026091401, 2026091402,
2026091403**, with 5,000 shots each at all 15 sweep settings and the tradeoff
setting. No seed is replaced or sample count extended based on observed errors.
Each seed compares native envelope matching, conditioned PyMatching and fixed
PyMatching on identical public rows; tradeoff also includes MLE. All 48 corpora
and 147 predictions are downloadable in `accuracy-seeds.zip`, together with
`rescore.py`. Run `python -I -S rescore.py rescore-seeds accuracy-seeds.zip`.

`accuracy-seeds.json` retains each seed's counts, Wilson intervals and paired
discordant counts, plus pooled differences on 15,000 independently generated
shots per setting. The paired difference is native failure probability minus
the comparator's: negative values favor native. Its conservative pointwise 95%
interval subtracts opposite ends of 97.5% Clopper-Pearson intervals for the two
discordant probabilities (Bonferroni). It does not pretend the paired outcomes
are independent. The independent checker inverts binomial CDFs using the Python
standard library; regression values are checked against SciPy beta quantiles.
These are pointwise intervals, not simultaneous coverage across all settings.
Three seeds still do not establish a threshold or universal accuracy ranking.
The old three timing repetitions remain only 5,000 accuracy shots; the new
15,000-shot data are a separate experiment, not pooled with the original corpus.


The clean `evidence_run` entry point above runs the additional accuracy experiment
after timing has finished. Its sources and measured binaries are bound to the
same source commit in `provenance-seeds.json`. To verify the retained seed corpus:

```sh
python -m benchmarks.atom_loss.replay --archive site/static/data/atom-loss/accuracy-seeds.zip
python -m benchmarks.atom_loss.decoder_replay
python -O -m benchmarks.atom_loss.verify
```

Historical timings remain available in Git history. Current figures show the
fresh output-inclusive workflow measurements and the native offline comparator.
