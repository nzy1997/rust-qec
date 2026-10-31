# Magna reproduction reference

This harness pins the original Magna implementation and paper archive before
an independent Rust backend is compared with them. It inventories 31 packaged
tasks, 57 upstream test modules and every deposited paper file. It does not
implement a simulator or claim reproduction of the paper's precision curves.

The capped-support witness generator captures complete small observation sets,
not Monte Carlo precision estimates. Regenerate into a new path and compare the
result with `rmagna/tests/data/capped-support-reference.json`:

```sh
python3 benchmarks/magna_reproduction/capped_support_vectors.py \
  --upstream /absolute/path/to/magna --out /absolute/path/to/new-capped-vectors.json
```

Use the pinned reference Python environment, then repeat with `-O`. The two
captures must agree. Each artifact binds the native extension and source-manifest
hashes. The committed witnesses contain all 31 distinct proposals for four cases,
including affine-certified cancellation, biased T measurements, genuine
nonaffine zeros and a flipped adaptive observation.

The public implementation is
[`56847478c840395735146ef1e1ccdc3d8aedc10f`](https://github.com/itpyi/magna/tree/56847478c840395735146ef1e1ccdc3d8aedc10f).
The [paper data deposit](https://doi.org/10.5281/zenodo.23386778) records an older
development kernel, `853fa2369817e1be65a710b42ff80606e219e464`. Its `.stimx` inputs
were converted to the public `.magna` files. Historical and public input hashes
are different; do not claim byte-identical replay of the historical kernel.

## Verify the reference

Clone upstream outside the repository (or in ignored `drafts/`), check out the
pinned commit, and extract `magna-data-deposit.zip` from the deposit. Verify its
SHA-256 against `paper.archive_sha256` in `reference.json` before extraction.
The data-directory argument is the extracted `magna-data-deposit` directory.

```sh
python3 benchmarks/magna_reproduction/reference.py verify \
  --upstream /absolute/path/to/magna \
  --paper-data /absolute/path/to/magna-data-deposit
python3 -I -m unittest discover -s benchmarks/magna_reproduction -p 'test_*.py'
python3 -I -O -m unittest discover -s benchmarks/magna_reproduction -p 'test_*.py'
```

Verification rejects missing, altered or symlinked inputs and unlisted source
files in executable source directories. Standard `__pycache__` bytecode and
the native extension/build receipts in `core/magna` are allowed; unlisted
sourceless bytecode and other compiled modules are rejected. Downloaded TQD decoder modules
must match the pinned author provenance, and all required modules must exist
before capturing any TQD task.

## Capture upstream smoke results

Use a dedicated Python environment with NumPy, Stim, PyMatching and SciPy.
`reference-requirements.txt` pins the tested CPython 3.12 runtime dependencies;
install it with that environment's package manager before building. Each result
records the installed versions, interpreter, platform and actual native hash.
Build the extension with upstream's `scripts/build_native_core.py` using that
environment's interpreter. Download TQD sources with upstream's
`scripts/fetch_author_decoder.py`; it verifies the authors' pinned hashes.

```sh
python3 benchmarks/magna_reproduction/reference.py run \
  --upstream /absolute/path/to/magna \
  --python /absolute/path/to/reference-env/bin/python \
  --out /absolute/path/to/new-results-directory
python3 benchmarks/magna_reproduction/reference.py check \
  /absolute/path/to/new-results-directory --all-tasks
```

The default captures all 31 tasks with 1024 draws and seed 20261008. Use repeated
`--case bh2-T --case color3` options for a subset, `--shots` for a smaller smoke
run and `--timeout` for the per-task process bound. Results remain marked
`scope: smoke`; 1024 draws do not establish rare logical-error accuracy.

Each task runs in a new isolated Python process and temporary working directory,
with a fresh cache prefix so existing source-tree bytecode is never consumed.
The child imports only the explicitly selected source and decoder paths, hashes
the actual native extension before and after execution, and retains complete
result JSON and stderr. Source files are checked before and after the capture.
The offline `check` command binds result bytes, seeds, source-verification
evidence and task coverage to the receipt; completed receipts with failure
metadata are rejected. It runs
without importing the simulator. Omit `--all-tasks` only for a declared subset.
The output directory must be new. Failures retain a failed receipt and completed
task results rather than overwriting earlier evidence. Store output in ignored
`benchmarks/out/` or outside the repository.

## Historical experiment coverage

`paper_items` maps Tables I-II, Figs. 3-5 and the deposited code-switching and TQD
extensions to their bundles. Redrawing these bundles verifies the deposited
plotting inputs; independent simulation must generate new results instead.
Some historical experiment drivers and specialized importance proposals are
identified only by hashes. Recover or independently reconstruct and validate
those proposals before claiming independent precision-curve reproduction.

The accepted/error masses are integrated over unrequested measurement outcomes.
RNG stream equality with another language is not required: differential
correctness checks must prescribe faults, reports and feedback histories instead.
Keep accepted probability, accepted error mass and conditional error distinct.

## Observed upstream test boundary

### Source-bound Rust coverage ledger

`coverage.json` enumerates every canonical `module.Class.test_method` from the
pinned 57 core modules (352 cases). Its current assertion mappings cover two
native circuit cases with three Rust witness tests and all four native-core
primitive cases with four additional Rust witness tests and all five phase-quotient
optimization cases with five owning Rust witnesses, plus two finite history-efficiency
arithmetic cases with two owning Rust witnesses and one original progress case
with two owning Rust witnesses, plus three original pilot/CLI entry-point cases
with four owning Rust witnesses, plus two capped recovery/guarded-feedback
methods with two owning Rust witnesses, plus the original report-noise and
postselection method with one owning Rust witness, plus three original
low-weight categorical/mandatory-tail methods with three owning Rust witnesses,
plus three original feedback/failure methods with three owning Rust witnesses,
plus the two remaining original endpoint/budget-CLI methods with two witnesses,
plus all seven original selection-syntax methods with seven owning witnesses,
plus the original controller binding/config-isolation method with one real-process witness.
The original incomplete-pilot/unavailable-metrics method is verified with three
required witnesses, including an explicitly declared scientific diagnostic policy;
the combined GF2/quadratic method has a partial mapping for its 162 packed
cases, while the other 302 cases remain explicitly `unmapped`. A mapping describes a manually reviewed semantic contract,
not a claim that a similarly named Rust test proves every native assertion.
The three RM15 example-fixture tests are listed separately with their source
hashes and reasons; they never reduce the core denominator.

```sh
python3 -I benchmarks/magna_reproduction/coverage_ledger.py \
  --upstream /absolute/path/to/pinned/magna \
  --ledger benchmarks/magna_reproduction/coverage.json \
  --out /absolute/path/to/new-coverage-report.json
```

Inventory mode returns zero for this honestly incomplete ledger. The report
separates `inventory_valid` from `parity_complete`, includes per-module and
per-status counts, source hashes and declaration lines, and lists exact missing,
duplicate or unknown IDs on an inventory error. It verifies all pinned source
files before enumerating declarations without importing Magna. Unknown input
fields, duplicate JSON keys, changed hashes and invented completion flags are
rejected, including with Python `-O`. Reports require a new output path.

Use `--complete-parity` as a strict gate. It returns nonzero until every core case
is mapped and all referenced Rust tests have successful source-bound execution
evidence. `architecture_specific` entries require a platform, concrete contract
and reviewed reason and still block full parity. Skipped and ignored cases do
not count as executed passes. The original 12 installed-wheel skips remain
unmapped requirements, not successful reproduction evidence.

`partial` entries require a concrete `contract`, nonempty `rust_tests`, and a
nonempty `remaining_gap`. Their source-bound witnesses must execute just like
verified witnesses, but a partial entry never counts as verified or complete
parity. Missing, failed or ignored partial witnesses remain execution failures.

Optional `--rust-run /absolute/path/to/run.json` accepts schema
`rstim.magna-rust-tests.v1` with exactly these fields:

```json
{
  "schema": "rstim.magna-rust-tests.v1",
  "source_files": {"Cargo.lock": "sha256", "Cargo.toml": "sha256", "rmagna/...": "sha256"},
  "targets": [{
    "package": "rmagna",
    "target": "circuit",
    "binary": {"path": "/absolute/path/to/retained-test-binary", "sha256": "sha256"},
    "list": {"stdout": "complete --list output", "stderr": "", "exit_code": 0},
    "run": {"stdout": "complete --format pretty output", "stderr": "", "exit_code": 0}
  }]
}
```

`source_files` must equal `coverage_rust.source_identity(repository_root)`: all
files under `rmagna`, the root Cargo manifest/lockfile and any `.cargo` files.
Capture it before compilation, verify it again after running, and retain the
compiler and process receipts with the artifact. Use default features, build
with `cargo test --locked -p rmagna --test <target> --no-run`, then run the actual
compiler-reported binary with `--list` and `--format pretty`; store the complete
raw streams and actual exit codes. The validator checks retained binary bytes,
source identity, test names, summary counts and outcomes. It preserves supplied
raw evidence and diagnostics for failed or missing executions. Evidence is an
auditable producer attestation: JSON and hashes cannot authenticate that a
process actually ran, so original compiler/run receipts require independent
review. The synthetic run strings in unit tests are parser fixtures only.

Current declaration validation deliberately accepts only explicit unconditional
top-level `#[test]` functions with ordinary ASCII identifiers in
`rmagna/tests/<target>.rs`. Raw identifiers (including `r#name`), non-ASCII names,
nested, macro-generated, configured, ignored or otherwise unsupported declarations
need an explicit validator extension before becoming coverage witnesses.
Unsupported identifier prefixes never qualify as shorter test names. This restriction avoids
inferring test existence from a string or comment. Source declarations establish
existence; only matching actual execution establishes a pass.

At the pinned revision the source-tree runner discovers 352 tests: 340 pass and
12 installed-wheel tests are skipped without `MAGNA_INSTALLED_PYTHON`. Running
those 12 against a separately built wheel gives 9 passes and 3 failures:
`test_installed_external_decoder_binding`,
`test_installed_nonlinear_postselection_api_and_console`, and
`test_three_file_evaluation_with_installed_native_engine`. They request the
absent `magna_example_decoders` module through legacy fixture bindings.
The portable task-local decoder test passes. Retain this upstream discrepancy
in the later compatibility ledger; do not treat it as a Rust regression or
silently edit the reference to turn it green.

## Compare prepared insertion execution

After building `cargo build --release --locked -p rmagna --example evaluate_task`,
the bounded differential drivers can explicitly select prepared noise and
controller insertions:

```sh
python3 benchmarks/magna_reproduction/affine_task_compare.py \
  --upstream /absolute/path/to/magna \
  --rust-example /absolute/path/to/protected/evaluate_task \
  --compiler shared --probability-preparation fixed --density-preparation fixed \
  --insertion-preparation prepared --max-insertion-table-bytes 268435456 \
  --warm-replays 2 --out /absolute/path/to/new-affine-results.json
python3 benchmarks/magna_reproduction/controller_task_compare.py \
  --upstream /absolute/path/to/magna \
  --rust-example /absolute/path/to/protected/evaluate_task \
  --compiler shared --probability-preparation fixed --density-preparation fixed \
  --insertion-preparation prepared --max-insertion-table-bytes 1073741824 \
  --warm-replays 2 --timeout 600 --out /absolute/path/to/new-controller-results.json
```

The controller driver also accepts `native` and `native_injection` independently
for `--probability-preparation`, `--density-preparation` and
`--moment-preparation`. For a bounded diagnostic of all five packaged tasks:

```sh
python3 benchmarks/magna_reproduction/controller_task_compare.py \
  --upstream /absolute/path/to/pinned/magna \
  --rust-example /absolute/path/to/fresh/evaluate_task \
  --probability-preparation native --density-preparation native \
  --moment-preparation native --timeout 20 \
  --out /absolute/path/to/new-native-controller-results.json
```

Use `--native-transport prepared` to request retained boundary geometry. Its
`--native-transport-max-terms`, `--native-transport-max-geometry-words` and
`--native-transport-max-construction-work` bounds can independently force an
optional preparation skip; exact evaluation then recomputes the boundary.
Selection/construction/core limits use the same `--native-*` flags as the main
CLI; `--injection-*` limits require at least one `native_injection` family.
`--help` lists the complete set. Orphan limits are rejected before decoder import.

The diagnostic reports each family's effective policy in
`configuration.<family>_preparation_options`. Work limits wider than 64 bits are
decimal strings. Native caches report actual retained `native_routes`
(direct/product/Walsh/injected and injection budget skips) and
`native_transports` (prepared/budget skipped). Warm replay checks retained moment
caches along with probability, density and insertion caches. These are preparation
counts, not counts of transport queries or speedup measurements.

The two native uint64 seed endpoints may produce the same history. Each actual
controller history is replayed explicitly in Rust, without assuming identical
random trajectories. Both cases and source provenance survive result-validation
failures; failed or timed-out children retain stdout/stderr. Budget failures and
timeouts produce partial receipts and nonzero exit status. A 20-second capture
does not require every large task to finish and does not establish paper-scale
Monte Carlo or performance results.

Use the pinned reference Python environment to run these commands. Keep the
binary unchanged during a capture; its receipt binds its content hash. The
drivers validate the applied mode, effective limits and retained catalog size;
custom byte/entry limits require prepared mode. Defaults remain direct
propagation. These prescribed no-fault comparisons validate conditional task
results and decoder histories. They do not establish noisy Monte Carlo
throughput or reproduce paper precision curves. Warm replays exclude first-query
preparation and report the same prescribed history separately.

The controller command allows a 1 GiB insertion-table estimate and a 600-second
per-Rust-history bound so it can check the larger TQD5 example. The table cap
excludes the existing program, temporary input catalog and query caches; it is
not a process-memory cap. Use `--tasks tqd3` to select one controller example.
Preparation refusals and timed-out histories remain failed cases in the receipt.

## Native sampler-decision witnesses

`rmagna/tests/fixtures/sampler_decision_native.json` contains 24 tiny binary64
witnesses captured from the pinned native `decide_file_sampler` binding. It
records source revision and native-extension SHA-256, input/output float bits,
and rejected nonfinite/range cases. These are decision-contract fixtures, not
performance or paper-precision results. Regenerate with the reference environment:

```sh
python3 benchmarks/magna_reproduction/sampler_decision_reference.py \
  --magna-root /absolute/path/to/magna --output /absolute/path/to/new-decisions.json
```

Repeat under `python3 -O` and compare the two files byte for byte. The largest
count witness stays within the native Python binding's signed-size input range;
Rust pilot shot counts remain `u64`. The capture checks the strict `Gamma > 1`
exact-marginal recommendation, including the affine-support tie rule and the
native multiplication/division order at floating-point limits.

## Native core primitive contracts

`rmagna/tests/fixtures/native_core.json` binds the pinned source/manifest,
production native extension and capture producer identities. Regenerate it with
the pinned Python environment:

```sh
python3 -I -B benchmarks/magna_reproduction/native_core_reference.py \
  --upstream /absolute/path/to/pinned/magna \
  --manifest benchmarks/magna_reproduction/reference.json \
  --out /absolute/path/to/new-native-core.json
cargo test --locked -p rmagna --test native_core
```

Repeated captures, including Python `-O`, must be byte-identical for the same
source/native environment. The capture reproduces the original Python seeds
26091461/26091462/26091463, including all early breaks before continuing the
second stream into the 300-bit scalar inputs. It retains 162 contraction cases,
92 overlap pairs, eight explicit zero-contraction skips from 100 attempts,
28 closed-product Gaussian sums, 100 scalar pairs and 90 packed-width cases.
All small-boundary amplitudes and every finite large-boundary image are retained
with exact cyclotomic coefficients and binary powers. Gaussian expectations use
a finite closed product through 257 paths, never a 2^257 enumeration.

The expected values come from the pinned independent Python dense/scalar oracle;
production native contraction, norm and scalar APIs cross-check them. The
internal test-only overlap and scalar helpers are not called. Their mathematical
assertion roles are translated to Rust with literal expected values. Malformed
column/diagonal/dimension roles become Rust constructor failures; a negative
Python tuple base has no representation in unsigned Rust words and does not
justify a tuple compatibility API.

Run the source-frozen coverage gate with real binary evidence using the current
`--rust-run <run.json> --complete-parity` flags documented above. The 58 mapped
Rust witnesses for 49 verified and one partial upstream cases must execute successfully,
while the overall gate must still return nonzero because 302 core contracts remain
unmapped. These finite
primitive witnesses establish neither full interface parity nor paper precision
or performance.

## Original phase-quotient assertion contracts

`rmagna/tests/phase_quotient_contracts.rs` translates the five methods in pinned
`tests/core/test_phase_quotient_optimization.py`. Its literal fixture is produced
by compiling only that hash-checked module's pure integer `phase` and `normalize`
functions, with independent finite differences for the small radical ranks:

```sh
python3 -I -B benchmarks/magna_reproduction/phase_quotient_reference.py \
  --upstream /absolute/path/to/pinned/magna \
  --out /absolute/path/to/new-phase-quotient-literals.json
cargo test --locked -p rmagna --test phase_quotient_contracts
```

The producer never imports the upstream module or invokes its source-mutating
test-only native builder. Repeat with `-O` into a new path: the bytes must agree.
Changed module bytes and existing output paths are rejected in both modes.
The committed fixture binds the module, producer and source revision identities.

Every quotient-basis image is checked to form the complete physical input domain,
so reconstruction covers every physical input.

The tests reconstruct all values for 139 small signatures and both original
passes over 16 four-variable sign updates, including even and cancelling axes.
They check exact prepared sums, the entire finite radical sets, all 70 singleton
axes/lifts and their Clifford remainder without exponential expansion, and
retention after invalid inputs. Original low-rank terms have sum
`(1+i)*2^(n-2)`; dimensions 3 through 14, then 3 again, and concurrent shared
queries at widths 3,4,65,129 check dimension separation and owned immutability.

Rust's quotient coordinate convention and immutable preparations replace the
native mutable eight-entry cache; these witnesses do not assert native tuple ABI,
cache hits or evictions. Only the singleton identity gauge is compared directly.
The five assertion-specific ledger mappings, together with the other verified contracts, leave 302 cases unmapped and one partial, so the
actual source-bound `--rust-run <run.json> --complete-parity` audit must retain its
nonzero incomplete result. These finite contracts establish no paper precision
or performance result.

### Finite history-efficiency arithmetic contracts

The two source-bound witnesses in `rmagna/tests/history_efficiency_contracts.rs`
map only `test_weighted_residuals_and_noise_correction` and
`test_extreme_common_scale_and_zero_cases`. The four original weighted rows are
checked with independent centered `BigRational` sums, including the noise
likelihood correction `L/W`, literal exact results and a negative control that
omits `L`. The original two error/no-error rows use direct binary scales
`2^-10000`, `1` and `2^10000`; they retain `kappa = 1` and both variance
coefficients `1/4`. Two zero draws retain the original unavailable diagnostics.
The broader existing scale-invariance test also doubles the scalar sqrt(2)
exponent to reach those actual binary endpoints.

These tests import no upstream test module and invoke no native test-only builder.
The progress contract below adds one mapping and the pilot/CLI contracts add
three more. The separate incomplete-pilot/unavailable-metrics method is
verified with the explicitly declared diagnostic policy described below. The current ledger contains
49 verified, one partial, 302 unmapped and zero architecture-specific core cases,
backed by 58 Rust witnesses; the strict `--rust-run <run.json>
--complete-parity` gate must still exit 2 for incomplete parity. These mappings
establish finite arithmetic contracts, without changing runtime statistics,
claiming native floating-range behavior or reproducing paper performance.

### Original progress and record-sampling semantics

The owning `history_progress_contracts` witness maps only
`test_default_progress_preserves_draws_and_manual_born`. It uses the original
`H 0; T 0; MX 0` circuit, `H 0` target, sampled record 0, 21 draws, seed 42,
and progress interval 8. Fresh, reused and prepared paths retain identical
complete statistics, records, efficiency and operation counts; events occur
at exactly 8, 16 and 21 completed draws. The file default is `affine_support`,
whose present `sampling.exact_marginal_records` value is null. Explicit
`exact_marginal` reports `true conditional distribution` and kappa 1.
Independent first/second moments of the affine proposal weights additionally
detect an unreported change to unit-weight conditional draws.

The same mapping also includes the owning `cli_progress` witness
`original_twenty_one_draws_keep_one_stdout_report_and_three_manual_hint_lines`.
It emits one final stdout JSON and three stderr events containing
the manual sampler flag and variance/cost caveat. Both witnesses are required:
the typed file observer alone does not exercise printed CLI progress.
Existing callback and malformed
argument controls remain in place. Exact enumeration retains its integrated
history description and has no sampled-record claim. These finite contracts
add no pilot, native floating-range or paper-performance coverage.

### Original sampler pilot and CLI entry-point contracts

`history_pilot_contracts` and `history_pilot_cli_contracts` map only the three
original pilot/configuration, CLI JSON/progress, and invalid-count/native-decision
methods. The `X_ERROR(1/2) 0` circuit with `I 0` target uses 32 draws and seed 13
for the immutable caller test. Both independent channels complete with kappa 1,
positive preparation time, and Gamma matching their per-draw sampling-time ratio.
The file task explicitly uses fixed scalar families for the prepared-family
role; this does not equate the two implementations' planner defaults. Identity
pilots of two draws are inconclusive, without Gamma or a recommendation.

The CLI witness checks eight draws with progress every four and a sixteen-draw
comparison. Complete stdout JSON and stderr progress are separately asserted.
Invalid counts, boolean/fractional strings, missing payloads, and retired sampler
names fail before decoder imports; a valid-argument canary proves the decoder
would otherwise import. An absent payload represents Python `None`; omitting the
entire CLI option retains Rust's documented two-draw default. Rust static
integer/nonzero types and early typed count admission replace Python object
validation. A zero-preparation-budget canary separates count rejection from
quantum preparation and callback failures.

The invalid-input mapping also executes `SamplerDecision::from_pilot_totals`
against the original literal native examples: `(4,2,4,10)` gives Gamma 2 and
exact marginal; `(1,2,4,10)` gives Gamma 1/2 and affine support. Both have per-draw
point times 1/5 and 2/5. This is the existing stateless calculation, exposed for
observed totals, and makes no automatic switch or confidence guarantee. The
unchanged library comparison fixture additionally checks all 24 pinned native
binary64 decision cases.

All four pilot/CLI witnesses are required by the source-bound coverage gate,
alongside the existing sixteen, the three diagnostic-domain witnesses below and the two
capped recovery/feedback witnesses, the report-noise/postselection witness and
the three categorical/mandatory-tail and three feedback/failure witnesses below. The ledger
has 49 verified, one partial, 302 unmapped and zero architecture-specific cases,
with 58 required Rust witnesses. Strict complete parity still exits 2. These
finite checks establish no full M6 or paper-performance result.

### Original incomplete-pilot and declared tiny-error diagnostic contract

`history_incomplete_contracts` witnesses the original `H 0; T 0; MX 0` circuit,
`H 0` target, record 0, four pilot draws and one-query cap. Explicit prepared
families preserve the Rust preparation role. The failed comparison retains both
named pilot receipts, without Gamma, recommendation, automatic switching or a
production run. An `I 0` run of two draws without a target/error metric reports
`no_error_metric` and absent kappa.

The native scientific tuple `(.5, -100000)` represents `2^-100001`. Independent
BigInt rational arithmetic checks this identity and the centered residuals.
Two identical tiny errors have exactly zero residual variance, so Rust reports
`zero_residual_variance`. The native method expects
`diagnostic_range_exceeded`. The optional `ScientificEfficiency` wrapper now
reports this status under an explicit binary64 scientific-mantissa ratio policy
with inclusive frexp exponent floor -510, observed on the pinned Apple ARM
reference. `history_scientific_domains::original_tiny_error_reports_declared_range_without_clamping_exact_moments`
completes this finite original method with the two existing witnesses. The
embedded exact summary still reports zero residual variance. The wrapper makes
no arbitrary host-long-double, native residual bit-identity or tuple ABI claim.

A distinct zero/tiny pair retains strictly positive exact variance coefficient
`2^-200004` and kappa one. This control detects accidental clamping or fabricated
unavailability. Both finite witnesses are required by the source-bound audit;
the unresolved native status still blocks full parity.

### Original capped recovery and guarded-feedback contracts

`capped_affine_contracts` maps only the original terminal-recovery and conditional
noise/Pauli-feedback methods from `test_capped_affine.py`; its other seven methods
remain unmapped. Both compiler strategies and fixed-template/cold-prefix causal
configurations exercise the finite contracts.

For physical `H 0`, the original `M 0; DETECTOR rec[-1]; CONTROL[echo] det[-1] ->
ctrl[0]; CX ctrl[0] 0` recovery and `I 0` target retain acceptance one, error zero,
one actual recovery controller call, proposal probability one half and exact
target normalization one on all sixteen draws with seed nine. Both prescribed
measurement branches independently have mass one half and recover to the target.

The original guarded-feedback circuit uses output 3, records 0/1/2 and an `I 0`
target. The independent Clifford/conditional-noise calculation gives:

| Records [M0, MX1, M2] | Physical probability | Output error |
| --- | --- | --- |
| [0, 0, 0] | 1/2 | 0 |
| [1, 0, 0] | 1/4 | 0 |
| [1, 1, 0] | 1/6 | 0 |
| [1, 1, 1] | 1/12 | 1 |

Prescribed evaluations reproduce all four masses and actual echo inputs. Exact
enumeration independently gives acceptance one, error 1/12 and target success
11/12, with no omitted physical probability. Each 512-draw seed-100 batch has
acceptance one and error within the original 0.04 tolerance; retained records
check controller inputs and the exact empirical error contribution. Rust uses
its own SplitMix64 stream, without a Python Random trajectory claim.

The audit requires these two witnesses alongside the prior twenty-two and the
report-noise/postselection witness and three categorical/mandatory-tail plus three feedback/failure witnesses below.
It has 49 verified, one partial, 302 unmapped and zero architecture-specific
core cases, with 58 required witnesses and strict complete-parity exit 2. The
separate declared diagnostic-domain contract is described above. No native tensor/cache
counter ABI, full M6 completion or paper-performance result is claimed.

### Original report-noise and covered-sector conditional error

`enumeration_conditional_point` maps only the complete original
`test_report_noise_and_postselection_are_joint_contributions` method. Its literal
`X_ERROR(1/10) 0; M(1/5) 0; DETECTOR rec[-1]; POSTSELECT det[0]` circuit, `I 0`
target and Z observable have accepted contributions 36/50 and 1/50. Their sum is
37/50, the joint target-error contribution is 1/50, and the signed moment is
35/50 = 7/10. The separately retained exact
`conditional_error_within_enumerated_sector` is therefore 1/37. Fresh and reused
owned reports and the actual CLI retain its ordinary value, scientific value
and exact coefficients.

The typed point is calculated once with the evaluator's existing field budget.
Undefined error metrics and zero covered acceptance return None/null; defined
zero error remains an explicit zero. Covered masses and rigorous full-distribution
bounds retain their existing meaning. Two X-noise sites with probabilities 1/10
and 1/5 at weight one have covered mass 49/50, error mass 13/50 and covered-sector
point 13/49; the full conditional bounds remain [13/50,7/25]. Separate controls
retain the [0,1] full bounds for an empty mandatory-error sector, distinguish
missing metrics from defined zero error, and preserve an observable-only error
2^-1200 exactly and scientifically when its ordinary float is null.

The current 58 required witnesses support 49 verified, one partial and 302
unmapped core methods, with zero architecture-specific exclusions. The actual
source-bound strict coverage audit must still exit 2. The other original
low-weight methods remain outside this mapping; no complete M6 or paper-performance claim is made.

### Original low-weight categories and mandatory suffix

`low_weight_categories_contracts` maps three complete original methods, each to
one owning Rust test. Literal repeated X errors 1/10 and 1/5 on the same output
remain two distinct physical categories. At weight one the independent pattern
masses are 18/25, 2/25 and 9/50: acceptance is 49/50, error is 13/50 and full error
bounds are [13/50,7/25]. Both single-site patterns remain retained; full weight
two adds the double flip without changing the error mass. Original category
definitions, exact first X action and a content-sensitive 64-hex circuit
fingerprint are checked on fresh and reused owned reports.

The literal Pauli channel has exclusive masses [2/5,1/10,1/5,3/10]; only X and Y
flip the zero target, giving error 3/10. The correlated X/else-Z chain instead has
exclusive masses [7/10,1/10,1/5]. Its second supplied probability 2/9 is conditional:
(1-1/10)*(2/9)=1/5. Exact raw pattern responses and exported channel metadata
independently match these values; they never include simultaneous alternatives.

The final witness uses the original 200 optional [9/10,1/10] categories followed
by four mandatory [0,1] categories. With max_patterns=1, weight three is empty;
weight four retains exactly choices [(200,0),(201,0),(202,0),(203,0)]. Its mass is
9^200/10^200, with the exact complementary omitted mass. The cap bounds retained
final patterns, permitting mandatory-suffix pruning before optional prefixes can
exhaust the cap. No native binding ABI or new enumeration algorithm is claimed.

All 58 required Rust witnesses support 49 verified, one partial and 302 unmapped
core cases, with zero architecture-specific exclusions. The actual source-bound
strict gate remains incomplete and must exit 2. Other low-weight methods remain explicitly unverified;
these finite checks establish neither complete M6 parity nor paper performance.

### Original low-weight feedback and failure contracts

`low_weight_feedback_contracts` maps exactly three complete original methods.
The literal H/T/H, CX, two-measurement and AND-controller circuit has two
positive branches with inputs 00 and 11. Their independent masses are
(2+sqrt(2))/4 and (2-sqrt(2))/4. Controlled X on output two gives acceptance one,
error (2-sqrt(2))/4 and complementary target success, with matching ordinary
and exact JSON on fresh and repeatedly reused owned plans.

Physical X_ERROR(1/10) on output zero followed by the literal terminal H1,
measurement, echo controller and controlled X0 has error 1/2. The separate
literal H1, M1, guarded X_ERROR(1/5) rec[-1] 0 input has error 1/10. Every latent
noise assignment retains both positive measurement histories; the guarded
case invokes no decoder callback. Pattern probabilities, exact responses and
original physical category/channel/action/guard/source metadata are checked.
Both compiler strategies and fixed-template/cold-prefix execution are exercised.

Two X_ERROR(1/10) sites with weight two and max_patterns=3 fail; the echo
feedback circuit with max_branches=1 also fails. Fresh and reused failure
reports have a typed error and JSON result null, never complete masses or a
partial pattern estimate. Rust's no-result contract represents the native
zero-completed-pattern/totals-none failure without claiming a native cursor ABI.
The same prepared plans subsequently succeed at adequate budgets: four noise
patterns give error 9/50 and success 41/50, while both echo histories recover
to zero with error zero and success one.

All 58 required witnesses support 49 verified, one partial and 302 unmapped
core methods, with zero architecture-specific exclusions. Actual source-bound
strict complete parity must still exit 2. The separate declared diagnostic policy is described above; these checks establish no complete M6 or paper result.

### Original low-weight endpoints and CLI boundaries

`low_weight_endpoints_contracts` completes the two remaining methods in the
pinned nine-method low-weight module. Literal I0 and X_ERROR(0)0 at weight zero
have one supported pattern and zero error. Forced X_ERROR(1)0 at weight zero
has no supported patterns, zero covered probability, no covered conditional
point and full conditional bounds [0,1]. Weight one and the literal original
1<<64 decimal cutoff have exact error one. Fresh and reused owned plans agree
for both compilers and both causal strategies.

The existing CLI parser already accepts arbitrary nonnegative ASCII decimal
cutoffs, an optional plus and leading zeros. This change preserves that parser
and documents its effective ceiling: reported options and their fingerprints
contain min(input cutoff, usize::MAX). Ordinary in-range cutoffs retain their
values; numerically equivalent large spellings share the ceiling fingerprint.
A cutoff larger than the finite site count does not bypass pattern, site or
stored fault-entry quotas. A 4,096-digit token is exercised without allocating
a correspondingly large sector.

Rust's typed usize API cannot represent negative, Boolean, fractional or null
cutoffs. Equivalent CLI inputs are rejected before loading missing circuits or
tasks, importing a configured decoder or creating output. Empty/missing values,
non-ASCII digits, whitespace and a malformed suffix after overflow also reject.
The literal original X_ERROR(1/10)0, M0 command with outputs[0], an owned I0
zero target, weight one and JSON succeeds with exact error 1/10.

All nine original low-weight methods now have source-bound semantic witnesses.
The complete ledger still has 49 verified, one partial and 302 unmapped core
methods, requiring 56 successful Rust witnesses and strict complete-parity exit
2. This does not establish native object-ABI identity, M6 completion or any
paper-performance result.

### Original selection syntax and migration contracts

`selection_syntax_contracts` maps all seven complete original selection methods.
The literal H/T/H plus X_ERROR(1/10) detector selections have exact complementary
masses 1/2 +/- sqrt(2)/5. Three noisy repeated measurements with a majority
controller give equal acceptance 1/2 for a complemented register selection and
its explicit NOT spelling. A separate literal X_ERROR(1/5) plus three independent
readout flips of probability 1/10 has majority-observable error 271/1250, since
the report-majority error is 7/250. Format magna-0.1, exact/ordinary JSON and
fresh/reused plans are checked under both compilers and causal strategies.

Observable register values are captured at their declaration line: overwriting
one to zero does not change an earlier constant-one observable. A record XOR
its copied register cancels to zero. DETECTOR accepts only non-complemented
record targets, and OBSERVABLE_INCLUDE only non-complemented records/registers.
POSTSELECT accepts detector/register targets with optional complements, with
specific numeric-argument migration guidance. External CONTROL rejects raw
records and names detector/register alternatives; built-in XOR/NOT and Pauli
controls still admit records. Invalid-target diagnostics retain source line,
column and repeat context, and invalid input does not contaminate valid reuse.

Removed root task fields detectors, postselection, outcomes, error_checks and
decoded_error identify their names and supported circuit spelling before loading
ideal/recovery modules. Typed parsing, owned FileTask loading and the actual CLI
are checked; nested ideal_state.postselection remains valid. Strict task JSON
feature combinations retain their raw-token validation. Two existing fixtures
are aligned with the grammar: the preparation fixture reads the final record
instead of its identical detector alias in its observable, and module tests
expect earlier grammar diagnostics for forbidden detector-observable inputs.

All 58 required witnesses support 49 verified, one partial and 302 unmapped
core methods. Strict complete parity still exits 2. These finite contracts do
not establish native object-ABI identity, full M6 or paper performance.


### Original controller binding and nested configuration isolation

`controller_binding_contracts` reproduces the complete original
`CircuitFileTests.test_bindings_are_versioned_and_config_isolated` method.
A persistent independent Python decoder captures the literal `{"a":[1]}`
configuration. After capture, the caller value and configuration file are
changed to zero. The callback also mutates its own nested list, but both
invocations still return one because every invocation receives a deep copy.

The captured `r1` and `r2` versions produce distinct `controllers`
fingerprints while identical circuit text keeps the same `circuit`
fingerprint. This is the existing independent Rust fingerprint encoding;
it is not the native circuit hash or Python object-layout identity. Duplicate
registration rejects the decoder name and preserves the original version.
All four original malformed returns, `(2,)`, `(0.0,)`, `()` and
`None`, fail through a causal one-output binding and name `broken`.
The worker declares its quantum backend unavailable and imports no Magna
quantum backend.

Run the actual process contracts with:

```sh
cargo test --locked -p rmagna --test controller_binding_contracts --test controller_process
```

The unmodified original method was separately executed in ordinary and
optimized CPython 3.12 using a freshly compiled pinned native extension.
Including the inherited-record contract below, the ledger has 49 verified, one partial
and 302 unmapped core cases, with 58 required Rust witnesses and zero
architecture-specific exclusions. Strict complete parity still exits 2.
They establish neither complete M6 parity nor paper precision or performance.


### Owned inherited record distributions (#1000)

`CompiledUnitary::stabilizer_expansion` owns the actual compiled view's exact
Clifford-fiber side kets, including normalized stabilizer input, frozen phase
constants and Pauli insertions. `retained_records` forms the ordered conjugate
product, enforces equality on every physical wire and physically traces those
wires before returning classical mass contributions. Ordered dependent parity
rows, report flips and unnormalized signed/complex cross terms remain exact.
Diagnostics report actual side entries, pair work and retained contributions;
the retained count cannot exceed the square of the actual side-entry count.

The one newly mapped original method is
`SingleTraceTests.test_retained_labels_preserve_inherited_rank_squared_bound`.
Its CCZ/three-plus/X-record example has two side entries, four visited pairs,
at most four retained contributions, total one, and all eight exact 9/16 or
1/16 probabilities, both without flips and with r1 flipped. Both Rust compiler
strategies run the complete witness. The unchanged original native method also
passes normal and optimized Python using a freshly built pinned production
extension, with the test-only extension loader denied.

Separate independent three-qubit literal matrix tests check noncommuting
H/T/CS/CCZ gates, a complex normalized input, all eight common global phases and
an inserted Y. Additional tests cover raw complex and negative cross terms,
physical trace, redundant/inconsistent events, no records, vacuum, physical and
label widths across 64-bit word boundaries, scalar errors and finite path,
geometry, side, record, pair and event bounds. A factory-injection unit test
requires pair/geometry rejection before any pair matrices are constructed.

The ledger now has 49 verified, one partial and 302 unmapped core cases, with
58 required Rust witnesses and zero architecture-specific exclusions. Strict
complete parity still exits 2. This explicit unitary expansion is not full
NativeInstrumentIR/environment parity, arbitrary quantum output density,
automatic runtime integration or complete single-trace caching; event queries
currently rebuild their constraint factorizations. No paper timing or rare-event
precision result follows from these finite correctness checks.


### Completed coherent record marginals (#1002)

`CompletedRecordSpec` owns ordered physical record and event rows. Given
caller-supplied exact coherent `AffineKet` terms, it returns an owned
`CompletedProbability` or a distinct signed `CompletedMoment`. It preserves
relative complex phases, common global phases, subnormalization and coherent
cancellation. Output moments require a Hermitian Pauli commuting with every
record and physical event. Only probabilities expose sampling.

Both completed and inherited distributions share the same pair trace. Physical
wire equality and event equations are formed directly in joint path coordinates;
there is no synthetic n-by-2n wire-equality mask. A 16,385-wire fixed-support test
fits a declared geometry budget below the former square-mask payload while
checking exact probabilities, event mass and signed output moments. Caller row
masks still occupy O(n*(records+events)) bits, and retained/query geometry,
scalar arithmetic and structural construction work have separate admission
checks. Work bounds are logical estimates, not wall-time or process-RSS limits.

The three newly mapped `NativeRecordTests` methods are
`test_dense_events_and_hermitian_moments`,
`test_sampling_completed_marginal` and `test_reject_noncommuting_pauli`.
The first retains every original event, record and I/Z/X/Y assertion against
independently enumerated literal complex amplitudes. Sampling uses all 2048
original draws, seed739 and tolerance0.04, together with exact analytic masses;
Rust declares SplitMix64 words and exact conditional-ratio refinement rather
than PythonRandom stream identity. Each result reports its joint selected mass,
conditioning mass, entropy words, mass-query count and actual Gaussian sums.
The unchanged original three methods separately pass ordinary and optimized
CPython 3.12 with a freshly compiled pinned production extension. Their
build-capable scalar-test loader is replaced only by a read-only exact Gaussian
constructor; no test assertion, source or expected value is changed.

Additional exact-vector checks cover all eight common phases, complex relative
weights, dependent/zero labels, physical conditioning, cancellation, vacuum and
empty source sums. Other checks cover negative Hermitian moments, noncommuting
physical events, finite dimensions/storage/work/query/entropy budgets,
adversarial entropy exhaustion, deterministic zero-entropy draws, owned-result
lifetime and continuation of caller-owned random streams after failures.

```sh
cargo test --locked -p rmagna --test completed_records --test inherited_record_expansion
```

The current ledger has 49 verified, one partial and 302 unmapped core cases,
with 58 required Rust witnesses and zero architecture-specific exclusions.
Strict complete parity still exits 2. Caller-supplied expansions receive no
phase-rank certificate. Event queries still rebuild Gaussian factorizations;
this change does not establish prepared-query caching, original InstrumentIR,
automatic runtime routing, complete M6 parity or paper timing/precision results.


### Complete explicit original-operation branches (#1004)

`OriginalInstrument` owns signed wire labels, named operations/records, a
normalized independent initial stabilizer state and declared callback metadata.
Construction and complete assignment admission precede callback invocation.
True records and XOR report flips remain distinct; callbacks receive only
ordered declared earlier reported parents, including duplicate declarations.
Each extraction invokes the actual callbacks again, while previously compiled
branches retain their own immutable operations, decisions and exact phases.

The explicit route lowers the actual branch into the existing coherent compiler.
Callbacks may emit measurement-free preparation, reset and discard operations.
Outputs follow declared wire order; old reset/discard wires become chronological
environment legs, and destructive X/Z readouts are contracted. Signed Pauli
projectors, parity phases in pi/4 units, original fault anchors and same-anchor
fault order preserve global phases. Typed origins keep generated emissions and
faults distinct from literal user operation IDs. Identity faults retain order
without requiring a live wire. Source/control metadata, emitted operations,
physical slots, lowered gates and exact query geometry have finite admission
limits; arbitrary callback computation and its returned allocation are caller
work, outside those limits.

`OriginalBranch` exposes direct coherent-network amplitudes and independently
projected owned side kets, branch probability, physical output Pauli moments and
coherent target projection weights with environments traced. Diagnostics come
from the actual underlying compiler; no cache or resource certificate is added
to arbitrary completed terms. Target terms can cancel or be subnormalized.

```sh
cargo test --locked -p rmagna --test original_instrument
```

The two mapped original `UnifiedTests` methods cover all eight record assignments
and every output/environment amplitude of the complex small instrument, plus
the Bell/reset output moments and target projection. Literal exact vectors are
independent of GF(2), Gaussian and path kernels. Both original methods separately
pass ordinary and optimized Python using the freshly built pinned native module.
Additional tests cover metadata admission, callbacks, lifetimes, phases, quotas,
fault order, immutable reuse and targets. The current ledger has 49 verified,
one partial and 302 unmapped core cases, with 58 required Rust witnesses and
zero architecture-specific exclusions. Strict complete parity still exits 2.
Dynamic PhaseFeedback template reuse, online causal sampling, automatic Gaussian
caching, Python bindings, runtime integration and paper benchmarks remain
separate work.


### Static original-operation coherent trace (#1005)

`OriginalInstrument::compile_trace` retains physical measurement records instead
of fabricating a complete zero assignment. It rejects unresolved controls before
any callback. `OriginalStaticTrace::with_assignment` replaces the immutable
reference with named faults and XOR report flips. Sparse wire-lifetime histories
resolve reset/preparation/dead-wire semantics. Original anchor order survives
zero-gate operations; same-anchor faults retain their supplied order and phases.

Views accept ordered `OriginalRecordParity` rows and `OriginalRecordEvent`
conditions, including repeated names, constants and contradictory events.
`probability` returns a `CompletedProbability`; `output_moment` returns a signed
`CompletedMoment` and cannot be sampled. Only declared physical outputs carry
the requested Hermitian Pauli; records and chronological reset/discard
environments are traced. Original signed measurement offsets and report flips
are applied independently to retained parities and event outcomes.

The actual owned side expansion supplies `OriginalTraceResult::side`; completed
statistics count the real inherited ordered pairs. CCZ followed by all three
X readouts retains two side entries and four pairs for every original mass and
report flip. `OriginalTraceOptions` selects a supplied catalogue explicitly;
absence or a missing key uses the existing deterministic fiber policy. No
stabilizer simulator or Gaussian cache is introduced. Requested name bytes are
admitted before lookup or copying unknown identities into errors. Row/occurrence/physical
slot/gate/path/geometry/side/pair/query limits fail before their corresponding
allocations, leaving immutable references reusable.

```sh
cargo test --locked -p rmagna --test original_trace
```

The complete original12-trial seeded static method preserves both equivalent
input bases, all original operations, faults/flips, a retained under a XOR b,
and I/X/Y/Z output moments. Source-bound native tables are also checked with
independent literal exact matrices and every output/environment amplitude.
The three unchanged original methods passed both ordinary and optimized Python
against a fresh pinned production extension. The ledger has49 verified,
one partial and302 unmapped methods, requiring58 successful Rust witnesses;
strict complete parity still exits2. Automatic Gaussian caching, optional dynamic
feedback, online causal integration, Python bindings and paper timing/precision
benchmarks remain separate work.


## Explicit prepared original-record geometry

`OriginalTraceView::prepare_records` owns one actual source expansion and every
ordered conjugate pair's support projection, reported boundary map and affine
Gaussian contraction program. Preparation fixes the fault/report assignment,
retained parities and event parities. Supply a complete reported `BitVector` of
event outcomes to each prepared probability or output-moment call. Event values
used at preparation do not select a branch. Changing event RHS or a compatible
commuting Hermitian output Pauli reuses these actual plans; no scalar result is
memoized. Duplicate, dependent, inverted and constant rows retain their order.
Zero coefficients, zero Gaussian references and inconsistent initial events do
not replace structural preparation with an empty or fabricated certificate.

`statistics()` reports actual successful support and pairing constructions.
Each result's `work` reports actual retained-plan applications and zero new
support/pairing factorizations in that pair evaluation. Unit controls instrument
the real `LinearFactorization::new` entry point: preparation makes positive
calls, retained evaluation makes zero calls, and the cold-route negative control
makes positive calls. Subsequent mass/sampling queries on the returned completed
distribution retain their existing API and are outside these preparation counters.

Cold source-entry, raw joint-path, pair, geometry and construction-work limits
remain in force before pair allocation. `PreparedRecordLimits` additionally
admits total retained storage before allocation and separately caps the path
kernel remaining after trace/event equations. This reduced-path cap is not the
cold raw-path cap or the native deferred engine's coordinate dimension.
`prepare_records_with_limits` can also bound the actual restricted source phase
quotient before constructing its resource expansion. The certificate and actual
inherited ordered-pair count remain those of the retained source, including the
two-entry/four-pair T and CCZ examples. Geometry scales with physical width times
path dimension; there is no square physical-wire equality mask.

The unchanged original support-cache method is compared with exact T|+> MX
masses, duplicate events (0,0)/(1,1)/(0,0), inconsistent events, typed invalid
shapes/noncommutation, genuine source-quotient and reduced-path failures, and the
complete-assignment comparator. All twelve original fixtures also compare
prepared/cold/dense/native values for both complex input bases and compilers,
all original reported events and I/X/Y/Z output moments. Additional controls
cover report offsets across 65 bits, complex fault phases, fixed wide support,
zero/cancelling sources, and reuse after quota failures.

This is explicit per-assignment preparation. Automatic eviction/caching,
cross-assignment decomposition reuse, runtime/Python integration, performance
campaigns and paper reproduction remain separate work. The ledger now contains
49 verified, one partial and 302 unmapped core methods, with 58 required Rust
witnesses. Strict full parity remains incomplete and exits 2.

## Original static scalar network queries (#1009)

`OriginalStaticTrace::prepare_network_probability` and `prepare_network_moment`
close the physical trace and event geometry before selecting a final phase
resource. Evaluate an explicit retained family against immutable
`OriginalTraceView` assignments. Fault/report replacements use the original
insertion program; reported event outcomes remain separate ordered inputs.
The result is a scalar `NetworkValue` with its actual retained query-resource
quotient rank and term count. Preparation does not construct a coherent side
expansion or attach a side-entry certificate. Existing side and completed-record
APIs retain their own semantics.

The complete original twenty unused T/H/destructive-Z subcircuits plus the
unused deterministic record return exact probability one with native rank zero
and one term, including the original Y fault after t3 and flips m4/fixed. The
explicit side route still rejects that source at source quotient rank zero.
All twelve original static fixtures compare this scalar route with literal
matrices, complete branches and side/prepared-record traces under both initial
bases and compilers, every original event sector and output I/X/Y/Z moment.

Select the resource policy explicitly with `OriginalNetworkOptions::resource`.
`Catalogue` follows the existing certified-catalogue/fiber policy; `Native`
retains native selection and returns its failures directly. No catalogue is
implicit. For the original T/readout two-term contract, supply
`catalogue::builtin_phase_catalogue()` explicitly with native rank two and term
budget two. Selecting no catalogue requires a four-term Walsh certificate for
that same scalar; a two-term native budget is rejected rather than silently
switching to fibers. Empty events on the unused-T fixture need no catalogue:

```rust
let mut native = rmagna::phase::NativePhaseOptions::default();
native.selection.max_quotient_rank = 0;
let options = rmagna::instrument::OriginalNetworkOptions {
    resource: rmagna::instrument::OriginalNetworkResource::Native(native),
    max_terms: 1,
    ..Default::default()
};
let query = trace.prepare_network_probability(&[], &options, &arithmetic)?;
let view = trace.with_assignment(&assignment, &arithmetic)?;
let result = query.evaluate(&view, &rmagna::gf2::BitVector::zeros(0)?, &arithmetic)?;
assert_eq!((result.quotient_rank, result.terms), (0, 1));
```

Moment words use declared outputs first, then all named records in `records()`
order, with width `network_width()`. Other physical lifetimes carry identity.
Words must be Hermitian and commute with each event. Original record inversions
and assignment report flips change Z/Y signs, including joint complex
output/record correlations. Probability and signed moment families remain
distinct types.

`OriginalNetworkLimits` bounds event rows, actual joint reference paths and a
conservative packed-word estimate before the global-query constructors. Existing
metadata, path and event limits remain effective. The current scalar kernels
materialize packed physical-equality rows: their storage is quadratic in physical
width, explicitly admitted rather than described as path-linear. `geometry_words()`
reports the admitted upper estimate, not measured allocation or RSS. Source-bound
constructor instrumentation checks refusal before actual GF2 factorization;
65/129-bit output order, a 4097-wire storage refusal, vacuum, contradictory events,
strict native refusals and recovery are owning regressions.

These scalar primitives also support the explicit static boundary service below.
The coverage ledger contains49 verified /1 partial /302 unmapped cases, with58
required witnesses. Overall interface parity and paper throughput remain incomplete.

## Original direct static boundary tasks (#1010)

`OriginalStaticTrace::prepare_boundary_task` retains an `OriginalBoundaryTask`
with ordered reported-record postselection events and Hermitian physical-output
Pauli observables. Its service evaluates the actual global scalar queries against
compatible immutable `OriginalTraceView` assignments. `mass` is unconditional;
`acceptance` and the ordered signed `moments` include postselection. No implicit
normalization, coherent-side expansion, callback or assignment cache is used.

`OriginalRecordLayout` provides an admitted invertible change of the complete
reported-record space. Its independent postselection basis precedes a deterministic
trace complement. `transform()`, `inverse_transform()`, `to_virtual()` and
`to_records()` expose the actual maps; `postselection_values()` includes arbitrary
inverted-parity offsets. Dependent equations retain their RHS compatibility check.
Static feedback rank is zero. Classification does not inspect whether a record
is random: the original unused deterministic readout still contributes to trace
rank21 alongside the twenty unused T readouts. Four records with one independent
postselection row have rank tuple(1,0,3) and16 distinct coordinate images.

The direct `sample(view, rng, arithmetic)` adapter requires no sampled reports,
returns an empty reports map and consumes zero caller random words on success or
error. It computes exact joint masses rather than generating a trajectory.
Panicking random sources and unchanged SplitMix64 state exercise this contract;
the fresh original production reference also preserves Python Random state on
an actual T/readout task. Source controls must be rejected by `compile_trace`
before any callback is invoked.

`OriginalBoundaryOptions::network` preserves explicit catalogue/native selection
and the independent scalar query's physical path/event/storage caps. The original
T/readout requires an explicitly supplied builtin database for native rank2 and
two terms; native rank0 refuses that postselection. The unused-T task admits
rank0/one term after physical tracing. `final_resources` contains only actual
evaluated families. Incompatible redundant postselection returns exactzero
conditional values with only the real unconditional query receipt. An inconsistent
closed Gaussian returns its existing zero-work receipt(rank0,terms0), rather than
claiming a one-term expansion that was not evaluated.

`OriginalBoundaryLimits` separately admits records, original postselection rows,
named occurrences, observables, prospective query count, metadata, layout packed
words and aggregate task geometry before allocation. Prospective query admission
reserves mass, acceptance for nonempty postselection and each observable, including
families that incompatibility can later skip. Layout storage includes the full
coordinate transform/inverse and GF2 provenance. Task admission uses the same
real packed physical-equality geometry as the scalar kernels; that storage remains
quadratic in physical width. These are conservative upper estimates, not measured
RSS or substitutes for the explicit phase-resource limits. Real GF2-entry controls
prove refusal before either the layout or global-query constructor. Every new cap
has refusal/recovery controls, including exact word limits and65/129-bit boundaries.

```sh
cargo test --locked -p rmagna --test original_boundary
cargo test --locked -p rmagna --test original_network
cargo test --locked -p rmagna --test original_trace
```

All three original static BoundarySampling methods are mapped only after their
complete original assertions execute: virtual coordinates/redundancy, scalar
residual/zero postselection, and unused-T trace-before-resource. Both Rust compiler
strategies preserve them. All12 original static scenarios additionally compare
every sector and ordered I/X/Y/Z task moment with unchanged native tables, literal
exact matrices and complete branches under both equivalent complex input bases;
all1536 boundary amplitude checks remain. Fresh production native executes those
three original methods and four unchanged SingleTrace methods in normal/optimized
Python, with complete fixture equality and identity checks in finally on success
and deliberate failure. Only read-only public production scalar/layout adapters
replace build-capable testing helpers; no private testing extension is loaded.

The ledger now has49 verified, one partial and302 unmapped core cases of352,
with58 required Rust witnesses and zero architecture-specific exclusions. Strict
complete parity still exits2. Structured/opaque feedback, retained-label
trace-before-resource tensors, cross-assignment Gaussian caching, Python bindings,
full original coverage and paper benchmarks remain separate work.


## Original packed GF2 byte oracles

`cargo test --locked -p rmagna --test original_packed_kernels` runs all 162
shape/density cases from `tests/native/gf2_kernel.cpp` at the pinned upstream
commit. The input fixture records the actual C++ `std::mt19937_64` stream with
seed 2026100409, LSB-first byte hex and explicit row widths. Rust regenerates
all input bits using the standard engine parameters and checks every case's
unconsumed next-word checkpoint. Duplicate prefix bounds consume fresh original
destinations and are deliberately retained. Expectations use independent
unpacked byte dot products, column XORs and transpose entries. Additional
checks cover width/padding errors, invalid selections, mutation atomicity,
65-bit rows in 72-bit destinations, ragged zero-extended parity, empty matrices,
and finite `usize::MAX` clipping/allocation rejection.

`packed_kernel_inputs.cpp` and the Rust case schedule are adaptations of that
upstream test under [MPL-2.0](https://www.mozilla.org/MPL/2.0/); the public Rust
implementation is independent. The fixture contains inputs only. To regenerate,
first verify the entire checkout with `reference.py verify` and use a new output
path. The pinned original C++ source SHA-256 is
`4e090ba87d4ac7d0b560096b6642d0eeeab8c8f43341b351021eeaba3dce408c`.
Compile the unchanged original baseline and the input adapter separately:

```sh
clang++ -std=c++17 -O1 -fsanitize=undefined -fno-sanitize-recover=all \
  -Wall -Wextra -I "$MAGNA/core/magna/native" \
  "$MAGNA/tests/native/gf2_kernel.cpp" -o "$OUT/gf2-original"
"$OUT/gf2-original"
clang++ -std=c++17 -O1 -fsanitize=undefined -fno-sanitize-recover=all \
  -Wall -Wextra -I "$MAGNA/core/magna/native" -I "$MAGNA/tests/native" \
  benchmarks/magna_reproduction/packed_kernel_inputs.cpp -o "$OUT/packed-inputs"
"$OUT/packed-inputs" > "$OUT/original_packed_inputs.json"
cmp "$OUT/original_packed_inputs.json" rmagna/tests/fixtures/original_packed_inputs.json
```

The adapter runs the unchanged original packed method and compares the complete
engine state after the input-only traversal. Its 576,873 packed checks and the
unchanged baseline's 646,357 combined checks are C++ UBSan observations; they do
not claim Rust sanitizer execution. The original combined method remains
`partial`: its 143 factor/certificate/seeded-solve cases and 26 quadratic
restriction cases have no complete Rust mapping yet. The current ledger has
49 verified, one partial and 302 unmapped core cases, with 58 required successful
Rust witnesses. Strict complete parity still exits 2. These correctness checks
do not establish paper-scale runtime, throughput or memory performance.
