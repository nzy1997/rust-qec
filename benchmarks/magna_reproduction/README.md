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
with four owning Rust witnesses. One further method is explicitly `partial`,
with two required finite witnesses and a remaining native range-status gap;
the other 334 cases
remain explicitly `unmapped`. A mapping describes a manually reviewed semantic contract,
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
`--rust-run <run.json> --complete-parity` flags documented above. The 22 mapped
Rust witnesses for 17 verified and one partial upstream case must execute successfully,
while the overall gate must still return nonzero because 334 core contracts remain
unmapped and the partial method has an unresolved compatibility gap. These finite
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
The five assertion-specific ledger mappings, together with the other verified contracts, leave 334 cases unmapped and one partial, so the
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
three more. The separate incomplete-pilot/native floating-range method is
partial, with the numerical gap described below. The current ledger contains
17 verified, one partial, 334 unmapped and zero architecture-specific core cases,
backed by 22 Rust witnesses; the strict `--rust-run <run.json>
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
alongside the existing sixteen and the two partial witnesses below. The ledger
has 17 verified, one partial, 334 unmapped and zero architecture-specific cases,
with 22 required Rust witnesses. Strict complete parity still exits 2. These
finite checks establish no full M6 or paper-performance result.

### Partial original incomplete-pilot and tiny-error contract

`history_incomplete_contracts` witnesses the original `H 0; T 0; MX 0` circuit,
`H 0` target, record 0, four pilot draws and one-query cap. Explicit prepared
families preserve the Rust preparation role. The failed comparison retains both
named pilot receipts, without Gamma, recommendation, automatic switching or a
production run. An `I 0` run of two draws without a target/error metric reports
`no_error_metric` and absent kappa.

The native scientific tuple `(.5, -100000)` represents `2^-100001`. Independent
BigInt rational arithmetic checks this identity and the centered residuals.
Two identical tiny errors have exactly zero residual variance, so Rust reports
`zero_residual_variance`. The native method instead expects
`diagnostic_range_exceeded` from its host-long-double guard. This diagnostic-status
compatibility gap remains explicit in `remaining_gap`; the method is partial,
never verified or architecture-excluded. No host exponent limit or tuple ABI is
guessed and exact statistics are unchanged.

A distinct zero/tiny pair retains strictly positive exact variance coefficient
`2^-200004` and kappa one. This control detects accidental clamping or fabricated
unavailability. Both finite witnesses are required by the source-bound audit;
the unresolved native status still blocks full parity.
