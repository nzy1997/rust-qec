# Magna reproduction reference

This harness pins the original Magna implementation and paper archive before
an independent Rust backend is compared with them. It inventories 31 packaged
tasks, 57 upstream test modules and every deposited paper file. It does not
implement a simulator or claim reproduction of the paper's precision curves.

## Current coverage

The source-bound ledger currently has 103 verified, 1 partial and 248 unmapped
original methods, with zero architecture-specific core exclusions. It requires
122 distinct Rust witnesses from 124 declarations across 53 targets. Strict
complete parity intentionally exits 2. The detailed per-change sections below
retain their historical inventory snapshots; `coverage.json` is authoritative.

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
pinned 57 core modules (352 cases). The current ledger has 69 verified, zero
partial and 283 unmapped core methods, requiring 82 successful Rust witnesses.
These include all ten original static circuit-file methods, the complete roundtrip and
expansion-budget contracts, the independent
controller binding/config-isolation method, and the complete GF2/quadratic method
(162 packed, 143 factorization and 26 quadratic cases). Other owning mappings
cover the finite native-core, phase-quotient, history, pilot, capped-support,
low-weight, feedback and selection-syntax contracts documented below.
A mapping describes a manually reviewed semantic contract, not a claim that a
similarly named Rust test proves every native assertion.
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

Integration witnesses accept explicit unconditional top-level `#[test]`
functions with ordinary ASCII identifiers in `rmagna/tests/<target>.rs`.
The reserved target `lib` instead uses a fully qualified test name, resolved
from `rmagna/src/lib.rs` through explicit external module declarations. Ordinary
module paths, `#[cfg(test)]`, and literal safe relative `#[path = "file.rs"]`
registrations are supported; the leaf file still requires an unconditional
flat `#[test]` function and matching SHA-256. The complete `source_files` identity
binds all registration files. Build with `cargo test --locked -p rmagna --lib
--no-run` and retain the actual library harness's qualified `--list` and run
observations under target `lib`.

Inline modules, macro-generated declarations, other configurations/attributes,
ambiguous registrations, unsafe paths and symlinks refuse library witnesses.
Raw identifiers (including `r#name`), non-ASCII names and ignored or otherwise
unsupported tests remain unsupported. Identifier prefixes, strings and comments
cannot invent witnesses. Source declarations establish existence; only matching
actual execution establishes a pass.

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
`--rust-run <run.json> --complete-parity` flags documented above. The 82 mapped
Rust witnesses for 69 verified and zero partial upstream cases must execute successfully,
while the overall gate must still return nonzero because 283 core contracts remain
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
The five assertion-specific ledger mappings, together with the other verified contracts, leave 283 cases unmapped and zero partial, so the
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
69 verified, zero partial, 283 unmapped and zero architecture-specific core cases,
backed by 82 Rust witnesses; the strict `--rust-run <run.json>
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
has 69 verified, zero partial, 283 unmapped and zero architecture-specific cases,
with 82 required Rust witnesses. Strict complete parity still exits 2. These
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
It has 69 verified, zero partial, 283 unmapped and zero architecture-specific
core cases, with 82 required witnesses and strict complete-parity exit 2. The
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

The current 82 required witnesses support 69 verified, zero partial and 283
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

All 82 required Rust witnesses support 69 verified, zero partial and 283 unmapped
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

All 82 required witnesses support 69 verified, zero partial and 283 unmapped
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
The complete ledger still has 69 verified, zero partial and 283 unmapped core
methods, requiring 82 successful Rust witnesses and strict complete-parity exit
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

All 82 required witnesses support 69 verified, zero partial and 283 unmapped
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
Including the inherited-record contract below, the ledger has 69 verified, zero partial
and 283 unmapped core cases, with 82 required Rust witnesses and zero
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

The ledger now has 69 verified, zero partial and 283 unmapped core cases, with
82 required Rust witnesses and zero architecture-specific exclusions. Strict
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

The current ledger has 69 verified, zero partial and 283 unmapped core cases,
with 82 required Rust witnesses and zero architecture-specific exclusions.
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
fault order, immutable reuse and targets. The current ledger has 69 verified,
zero partial and 283 unmapped core cases, with 82 required Rust witnesses and
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
against a fresh pinned production extension. The ledger has69 verified,
zero partial and283 unmapped methods, requiring82 successful Rust witnesses;
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
69 verified, zero partial and 283 unmapped core methods, with 82 required Rust
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
The coverage ledger contains69 verified /0 partial /283 unmapped cases, with82
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

The ledger now has69 verified, zero partial and283 unmapped core cases of352,
with82 required Rust witnesses and zero architecture-specific exclusions. Strict
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
not claim Rust sanitizer execution. The complete original combined method is
verified by these packed witnesses and the factor/quadratic witnesses below. The current ledger has
69 verified, zero partial and 283 unmapped core cases, with 82 required successful
Rust witnesses. Strict complete parity still exits 2. These correctness checks
do not establish paper-scale runtime, throughput or memory performance.


## Original factor certificates and quadratic restrictions

`cargo test --locked -p rmagna --test original_factor_kernels --test original_packed_kernels`
executes the complete 162 packed, 143 factor and 26 quadratic case schedule from
the same pinned combined original method. `factor_kernel_inputs.cpp` and both Rust
witnesses retain MPL-2.0 attribution. The new fixture stores only actual C++ inputs,
not expected factors, certificates, solutions or restricted coefficients. Its SHA-256
is `f3f1454a73578e50ac3085e20b130af30b133b4e09de9b5dfd9e5d54f3cb64ef`.
The prior packed fixture and all its inputs remain unchanged.

The remaining cases continue after exactly 708,504 random outputs. Only original
consistent-RHS branches consume seed coordinates. The complete sequence ends at
1,000,450 outputs with word-FNV64 fingerprint 9751379586251010270: start at
14695981039346656037, XOR each full engine word, then multiply by 1099511628211
with wrapping unsigned 64-bit arithmetic. The Rust stream checks every input bit,
coefficient and unconsumed per-case checkpoint. The C++ adapter executes the
unchanged combined method first and checks complete engine-state equality at
both the packed boundary and the final position.

Independent byte elimination, dot products and row combinations establish rank,
complete independent left/right kernels, every actual production reduction
certificate, consistent/inconsistent RHS values, seeded free coordinates and all
small affine solution sets. A reversible variable-coordinate adapter translates
native highest pivots to existing Rust lowest pivots; equation and certificate
coordinates retain their order. The literal equation x0 XOR x1=1 with zero seed
checks the distinct native (0,1) and default Rust (1,0) solutions explicitly.

The explicit full-certificate constructor admits exactly m*ceil(m/64) retained
packed words before reduction; N/N-1 quotas, malformed widths/padding, empty/tall
systems, contradictory equations and unchanged-input controls remain typed. The
default constructor still compresses tall provenance. Independent integer
polynomials check every original affine quadratic substitution, canonical upper
triangle, omission of only the resulting constant and all eight global phases.
Small exact Gauss sums add literal sum checks; phase-preserving defaults remain.

After verifying the pinned checkout with `reference.py verify`, regenerate into a
new output directory using the original-baseline command above and:

```sh
clang++ -std=c++17 -O1 -fsanitize=undefined -fno-sanitize-recover=all \
  -Wall -Wextra -I "$MAGNA/core/magna/native" -I "$MAGNA/tests/native" \
  benchmarks/magna_reproduction/factor_kernel_inputs.cpp -o "$OUT/factor-inputs"
"$OUT/factor-inputs" > "$OUT/original_factor_inputs.json"
cmp "$OUT/original_factor_inputs.json" rmagna/tests/fixtures/original_factor_inputs.json
```

The unchanged original C++ UBSan method checks 646,357 assertions. This remains
separate from safe Rust boundary and mathematical checks; Rust is not described
as C++-instrumented. The ledger has 69 verified, zero partial and 283 unmapped
methods, with 82 required Rust witnesses. Strict complete parity still exits 2.
Full public-surface reproduction and paper benchmarks remain incomplete.


## Complete original static circuit-file contracts

`static_circuit_contracts.rs` preserves every finite input and observable assertion
from the ten static methods in pinned `test_circuit_file.CircuitFileTests`.
Eight previously unmapped methods are promoted; the two existing measurement/CZ
mappings and their witnesses are retained, with the exact-source witnesses added.
The separate binding/config method keeps its existing mapping. The complete
atomic-control and native rational/replaced-view methods are mapped separately
below.

The test header attributes the original MPL-2.0 source. Python grouped
instructions lower to individual typed Rust operations. Resolved `BitExpr`
values replace versioned Python target lookups; the write versions are checked
before resolution. The existing `FileTask` inspection and
`rmagna-provenance-v1` fingerprints establish serializable descriptions and
stable independent Rust identity. They do not claim Python dataclass layout or
native fingerprint-byte identity. The nineteen source-located parser errors use
an explicit wording translation and the original mismatched correlated guard
also rejects. The exact conditional channel mass is 3/10.

The ten unchanged original methods were run against a fresh extension built
from the pinned source, with installed Stim 1.15.0 and zero skips. The portable
`rmagna/tests/fixtures/original_static_stim.json` records that actual Stim result:
the complete repeated circuit has six measurements and three qubits. Its SHA-256
is `c40f38af37d84193baf59cf79c14a45e9accfc704481dfd3c151b1ef60d65146`.
The Rust test reads the same source and checks both counts. To reproduce the
native comparison, use the pinned environment described above, put
`/path/to/pinned-magna/tests/core` on `PYTHONPATH`, and run
`python3.12 -B -m unittest -v test_circuit_file.CircuitFileTests.test_stim_subset_matches_record_count`;
Stim must be installed, and a skip is not a successful compatibility check.

```sh
cargo test --locked -p rmagna --test static_circuit_contracts
```

The full source-bound execution audit must pass all 82 mapped witnesses in
ordinary and optimized ledger modes. Strict complete parity continues to exit 2
because 283 methods remain unmapped. These finite contracts establish neither
complete frontend parity nor paper benchmark performance.

### Exact circuit parser resource stops

The complete original `test_expansion_budget_is_enforced` method is mapped to
`parser_resource_limits::original_expansion_budget_reports_exact_instruction_and_qubit_requirements`.
It preserves all original inputs and checks both repeat budgets, the exact sparse
width failure and source location, an explicitly raised width, and huge empty
repeats. The pinned production extension runs that unchanged method separately.

Eight Rust tests also cover all five owned structural resource payloads, zero
quotas, UTF-8 byte counts, operation-prefix and repeat locations, malformed input,
platform-width multiplication/addition overflow, and counts beyond `u128`.
Instruction admission uses arbitrary-precision complete counts before lowering;
`usize::MAX` is a real limit, never an overflow sentinel. These boundary tests
make no process-memory or paper-performance claim. The current ledger has
69 verified, zero partial and 283 unmapped core methods, requiring 82 successful
Rust witnesses; strict complete parity still exits 2.


### Original lossless circuit roundtrip and captured file identity

`circuit_roundtrip_contracts::lossless_roundtrip_and_resolved_repeat` preserves the
complete original commented source, including both MX(0.1) repeat iterations.
It checks exact text and file bytes, three records/two qubits, all three register
versions, the previous write captured by the later input, repeat locations and
line7, the final three-record affine parity, unique lowered operation IDs,
JSON-roundtrippable inspection and equal file-reload fingerprints. The unchanged
original native method is executed separately in a fresh pinned environment.

Rust resolves versioned register reads into owned `BitExpr` inputs and splits
some grouped source instructions into individual operations. Standard file I/O
writes `Circuit::to_text()` without reformatting; `FileTask` owns captured content
and serializable inspection with its independent `rmagna-provenance-v1` encoding.
These are functional substitutes, without Python dataclass or native hash-byte
identity claims. Releasing source owners and deleting the file leaves the loaded
snapshot's fingerprints and inspection intact. A new load of the deleted path
fails. A second test changes only the comment, then the readout probability,
then corrupts and deletes the path: fresh loads see those changes/refusals while
the original snapshot remains stable. Comment-only changes alter source identity
even when the circuit operations are unchanged.

Run `cargo test --locked -p rmagna --test circuit_roundtrip_contracts` for both
owning tests. Only the complete original roundtrip method is newly mapped. The
full source-bound audit requires82 unique Rust witnesses from84 declarations and
36 owning targets:69 verified, zero partial and283 unmapped original methods.
Strict complete parity still exits2; full public-surface and paper experiments
remain incomplete.

### Original atomic controller outputs and nonlinear observations

`atomic_controller_contracts::atomic_outputs_and_nonlinear_parity_information`
retains the complete original M0/1/2, XOR, detector, two-output external call,
XOR and CH source. It checks the causal nonlinear route, exactly two required
controller observations (rec0 XOR rec1 and rec2), the captured register0/version0
read before its version1 overwrite, distinct atomic outputs and both external
decisions in the final XOR. Parsing and causal preparation invoke no callback.

Rust retains required observations as the external operation's owned `BitExpr`
inputs; registers resolve at each read, and call/bit decision atoms identify
external outputs. `needs_history_sampling()` selects the causal route. These
replace Python analysis/target lookup objects. The Rust callback captures its observation log. The original unused
configuration dictionary has no separately claimed Rust config API; Python
object/config ABI is not claimed.
The unchanged original native method is executed independently in the pinned
environment. Its explicit callback input (0,1) is exercised through Rust's public
`CausalProgram::new_stabilizer`/`resolve` APIs using the original source and the
normalized computational input q2=1, q0=q1=0. All four combinations of compiler
and fixed/cold causal strategies make exactly one call with inputs/outputs0/1,
retain history mass1 and resolve the two decisions and final XOR to0,1,1.
Record parities use requested observations; decisions use `resolve_expression`.

Run `cargo test --locked -p rmagna --test atomic_controller_contracts`. Only this
complete original method is newly mapped. The complete source-bound audit
requires82 unique witnesses from84 declarations and36 targets:69 verified,
zero partial and283 unmapped. Strict complete parity continues to exit2; full
public-surface reproduction and paper benchmarks remain incomplete.

### Original nonuniform weighted Z importance law

`weighted_noise_contracts::deterministic_nonuniform_and_importance` preserves the
complete original probability1/3 event, X/Z conditional weights0/1, proposal
scale2, seed14 and40 draws. Both compiler strategies observe identity and Z,
with exact physical/proposal masses2/3,1/3 and1/3,2/3 respectively and likelihood
ratios2 and1/2. Zero X/Y alternatives are never drawn and their prescribed
physical/proposal masses are exactly zero. There are no report-flip actions.

Rust's canonical `PAULI_CHANNEL_1(0,0,1/3) 4` retains all XYZ alternatives: zero
X, zero Y, and Z mass1/3. The actual compiled Pauli word is Z on physical qubit4.
The native string anchor `original` is represented by an owned typed operation0
address with source label `original`, line1/column1 and no repeat coordinates;
every original channel and chosen Z action retains that address. Canonical Z
is alternative2 rather than the native two-channel index1. SplitMix64 supplies
the independent Rust seed14 stream; the forty native and Rust trajectories are
not claimed equal. The unchanged original weighted-noise method is run
separately against a fresh pinned production native environment.

Run `cargo test --locked -p rmagna --test weighted_noise_contracts`. Only this
complete original method is newly mapped; all remaining weighted-noise gaps
remain explicit. The full source-bound audit requires82 unique witnesses from
84 declarations and36 targets:69 verified, zero partial and283 unmapped.
Strict complete parity still exits2; full interface parity and paper benchmarks
remain incomplete.


### Original unit-scale and effective deterministic endpoint invariance

`weighted_noise_contracts::unit_scale_and_effective_deterministic_endpoints`
retains the base zero-X qubit0 site, forced-Z qubit1 site and record0 report
flip probability1/5. It also retains all three effective physical rates0,1/7,1.
The effective ensembles are explicit parsed X_ERROR/Z_ERROR/M probabilities,
including replacement of the originally zero/one rates. Original source roles
`zero`, `one`, `variable` map to typed operations0/1/2 and their captured
line1/2/3 addresses; native `m0` maps to record0. Metadata checks preserve each
X/Z/report action, rate and source identity. This is an explicit finite input
representation, without a native `physical_rate` keyword or string-ID ABI.

Both compiler strategies compare complete physical and configured scale1 draws,
including their exact probabilities, likelihood and entropy count, after each
of40 seed26 draws for all four ensembles. Their SplitMix64 states must agree
after every draw. Effective rates0/1 also preserve the original seed17 single
draw against scale100, with exact empty/all-selected choices, likelihood1,
zero entropy and unchanged stream state. Genuinely stochastic ensembles reject
scale100 rather than clip their proposals. Rust stream equality is internal;
no equality with Python Random trajectories is claimed. The unchanged pinned
original method is separately run against a fresh native environment.

Run `cargo test --locked -p rmagna --test weighted_noise_contracts`. Only this
complete original method is newly mapped. The ledger requires82 unique Rust
witnesses from84 declarations and36 targets:69 verified, zero partial and283
unmapped. Strict complete parity continues to exit2; full public-interface
parity and paper benchmarks remain incomplete.


### Owned control analysis and provenance-safe circuit views

`Circuit::control_analysis()` returns owned resolved expressions for every
`RegisterVersion { register, version }`, including overwritten writes. Unknown
versions return `None`; a supplied zero `BitExpr` remains present and does not
fall back to the parsed value. `ControlAnalysis::from_expressions` and
`with_expressions` own their maps and leave the parsed circuit unchanged.

`Circuit::view()` creates an owned `CircuitView`; `CircuitView::from(circuit)`
can consume the circuit without cloning its parsed source/operations. An
unchanged view provides `parsed_circuit()` for the existing compiler/export
APIs, plus guarded `format_version()` and `to_text()`. Replacing operations or
analysis unconditionally removes parsed provenance, even for identical values
or a subsequent return to original content. The edited data remain inspectable
and `source_snapshot()` retains the original source. It is explicitly an
original snapshot, not serialization of the edited view. This slice adds no
general programmatic instrument compiler.

`circuit_views::native_rationals_and_replaced_views` retains the complete pinned
method: all five exact rational literals, original M0/NOT source, truncated
operation rejection with the no-parsed-form diagnostic, and explicit zero
ctrl0/version0 analysis replacement. Additional owning regressions cover all
three register versions, unknown versions, caller/source ownership, identical
operation/analysis edits, malformed inspection-only data and source snapshots
after releasing original owners. The unchanged parsed view still compiles under
both strategies. Typed resolved expressions and explicit provenance replace the
native dataclass/string-target representation, without an object-ABI claim.

Run `cargo test --locked -p rmagna --test circuit_views`. Frontend validation
still compares all31 tasks/63 circuit files; `make test-magna-task-json` and
`make test-magna-controller-json` exercise the strict JSON feature matrices.
Only this complete original method is newly mapped; all15 original circuit-file
methods are now mapped. The full source-bound audit requires82 unique witnesses
from84 declarations and36 targets:69 verified, zero partial and283 unmapped.
Strict complete parity still exits2; full public-surface and paper benchmarks
remain incomplete.


### Exact conditional channel laws and shared physical rates

`ConditionalSiteLaw::new(channel_count, occurrence, weights)` owns the exact
occurrence probability and explicit conditional channel weights. Every declared
channel needs one weight in [0,1], and weights must sum exactly to one even when
occurrence is zero or one. Zero weights are valid; weights are never implicitly
renormalized. Typed `ConditionalLawError` values distinguish channel counts,
occurrence/weight/rate errors, nonunit sums and canonical-model resource errors.

`NoiseModel::from_conditional_laws(&laws, Some(rate), &options)` constructs owned
absolute categories with identity followed by the original channel indices,
including zero-weight slots. The shared rate replaces every occurrence, so a law
with original occurrence zero and weights(0,1) activates only channel one under
rate 1/3. `None` uses each
law's original occurrence. The laws remain available for reuse at other rates;
an absolute zero-mass category alone cannot recover their conditional weights.
Denominator limits apply to the resulting canonical categories through the
existing model constructor, and configured importance sampling uses that model.
Channel actions remain the caller's catalog, without a general instrument compiler.

`conditional_noise_laws::native_invalid_weights` retains the complete original
two-channel vectors(1),(1,1),(-1,2) at occurrence one, requiring specific typed and
actionable count/sum/negative-weight errors. The unchanged pinned native method
is separately executed in a fresh verified reference environment. Additional
owning regressions cover empty/zero/mismatched channel counts, raw zero
fraction denominators, below-unit sums, valid zero weights, source ownership,
and zero/one-occurrence conditional distributions under shared rates 0,1/7,1.
They check exact identity/channel masses, scale-two joint likelihood tables,
scale 100 deterministic endpoints with zero entropy and unchanged stream state,
nonendpoint scale rejection, and rejection/recovery at the exact five-bit
canonical denominator 21 boundary. Rust draws use explicit SplitMix64 states;
no Python Random trajectory or native action/object ABI identity is claimed.

Run `cargo test --locked -p rmagna --test conditional_noise_laws`. Only the
complete original invalid-weights method is newly mapped. The complete ledger
requires 82 unique Rust witnesses from 84 declarations and 36 targets: 69 verified,
zero partial and 283 unmapped. Strict complete parity still exits 2; broader M6,
full public-interface and paper benchmark reproduction remain incomplete.


### Immutable reported-condition instrument views

`CompiledInstrument::with_reported_conditions` returns an opt-in owned view
with additional `(BitExpr, bool)` reported-record parity predicates. It reuses
the validated causal condition lowering, appends to existing source
`POSTSELECT` constraints and shares the original query caches and preparation
limits. The original instrument and caller-owned conditions remain independent.
Acceptance, output densities and Pauli moments are unnormalized event masses;
there is no division by event probability or normalized state construction.
Ordinary reported probabilities, support and sampling continue to exclude
selection predicates. Prescribed faults and report flips retain their meaning.

`reported_condition_views::native_entangled_trace_and_postselection_are_distinct`
retains the complete original `SingleTraceTests` method alongside its existing
`original_trace` witness. The original fixture starts in `|+0>`, applies T0 and
CX0,1, destructively measures X1 as record `a`, and retains output0. Its total
mass is exactly one, and unconditional X/Y/Z moments are zero. Each record
sector has mass one-half, signed X and Y moments of `+sqrt(2)/4` and
`-sqrt(2)/4`, and zero Z moment. Native `x=z=1` denotes Hermitian Y, including
its phase. Both sector values are retained and cancel exactly.

Four owning integration tests cover both compiler strategies and Cold, Trace,
FixedFibers, FixedCatalogue and Native query policies. They additionally check
literal analytical densities, condition ownership, inherited acceptance,
constant/dependent/contradictory predicates, invalid records and decisions,
source-postselection equivalence, exact prescribed fault masses and report
flips, plus shared probability/density/moment cache reuse and cap recovery.
No callback or stochastic sampler is invoked to establish these contracts.

Run `cargo test --locked -p rmagna --test reported_condition_views`. The
unchanged original native method also runs against a freshly compiled pinned
production extension, with original and Rust fixture inputs retained before
execution; additional exact native sectors pin the Y convention. This extends
an already verified method and does not promote another unmapped case. The
ledger requires 82 unique witnesses from 84 declarations and 36 targets:
69 verified, zero partial and 283 unmapped methods. Strict complete parity
continues to exit 2; broader single-trace, programmatic IR and paper-result
reproduction remain separate.

### Complete original scientific-ratio rounding loops

`original_ratio_rounding` retains both complete original numerical methods:
350 `Random(20260927)` rational inputs (including both scalar powers) and
237 `Random(5641)`/midpoint input pairs with all three sign/power variants,
giving 711 large-rational comparisons. The 27 midpoint cases retain both tie
parities and one-integer-unit neighbors; coefficients extend to 5000 bits.
Each Rust assertion constructs the exact rational with its full binary exponent
before `approximate_real`, then converts the Rust [1,2) scientific mantissa to
native [0.5,1) by exact division by two and adds one to its exponent. Expected
mantissa bits and exponents must match exactly, including signed extreme inputs.

The portable `rmagna/tests/data/original-ratio-rounding.json` contains all inputs,
expected values, pinned commit and original test-source hashes. Host paths,
runtime/compiler identities and production/helper/generator hashes belong to
the separate generation receipt. The complete unchanged original native methods
are independently executed with their build-capable dependency loader replaced
by a forwarding proxy to the existing read-only helper. Its observed 712 calls
and 1061 input/result pairs match the generator inventory exactly.

Use the Python executable/version/ABI of an already-built matching test helper,
with a separately built production extension in the verified pinned source:

```sh
python -B benchmarks/magna_reproduction/original_ratio_rounding_vectors.py \
  --upstream /path/to/pinned-magna \
  --helper /path/to/native-testing/SOURCE_DIGEST/_native_testing.cpython-312-darwin.so \
  --helper-record /path/to/admitted-helper.json \
  --out /path/outside-input-trees/ratios.json \
  --receipt /path/outside-input-trees/ratios-receipt.json
cmp /path/outside-input-trees/ratios.json rmagna/tests/data/original-ratio-rounding.json
cargo test --locked -p rmagna --test original_ratio_rounding
```

The external helper record is a local admission of an existing binary, not a
portable fixture or a build command. Required fields are `helper.path` and
`helper.sha256`, `runtime.sha256` (the resolved Python executable), the exact
`recorded_python_version`, `loader_FLAGS`, `source_cpp_hpp` (relative source paths
mapped to objects with `sha256`), `shared_libraries` (objects with `resolved`
and `sha256`), and `source_root` (the helper's original source tree). These must
be obtained from the existing helper's build provenance. The generator checks
every listed native source against the pinned manifest, recomputes the original
loader's source/runtime digest without importing that loader, and checks the
read-only binary identity before loading it. A record asserts build provenance;
the generator cannot authenticate a fabricated record. It never builds a helper
or imports `tests.support.native_testing`. Outputs must be fresh and distinct,
outside the upstream, helper and Rust input trees, including resolved aliases.
It rechecks sources and binary identities even after capture fails.

When these two methods were added, the source-bound ledger reached 67 verified,
zero partial and 285 unmapped methods, requiring 80 unique Rust witnesses from
82 declarations and 35 targets. Strict complete parity still exits 2. Other
numerical failures, general programmatic IR and paper benchmarks remain separate.


### Original residual identity and invalid weighted-noise scales

`weighted_noise_validation::zero_and_identity` retains the complete original
seed3 exclusive vectors(0,1) and(0,0). Native label1 becomes Rust alternative1;
native label-1 becomes the explicit identity slot and no selected fault.
Canonical categories are identity followed by the original alternatives:
(0,0,1) and(1,0,0). Both draws have exact unit physical/proposal mass and
likelihood, zero entropy and unchanged shared Rust SplitMix64 state3.
Exclusive all-zero masses leave residual identity one; they are distinct from
conditional all-zero weights, which correctly fail the unit-sum requirement.
All four literal uniform quarter intervals additionally cover the exact
nontrivial law with exclusive masses1/4,1/2 and residual identity1/4. Assertions
retain every choice, mass and entropy count without statistical tolerance.

`weighted_noise_validation::invalid_scales_and_scaled_support` preserves all
original tokens0,-1,True,nan,inf,1/0. Direct rational/scale checks use typed
errors, while the public CLI `--noise-proposal-scale` reports structured JSON
argument failures for dynamic text. Unsupported `--proposal-rate` rejects as
the Rust configuration counterpart of native `proposal_rate`; no Python
keyword binding is added. Original integer scales3 and4 at occurrence1/3
reject with `NoiseError::ScaledProbability { site: 0 }`, including the exact
support-losing boundary. The original conditional law accepts scale2, then a
genuine `NoiseModel::from_conditional_laws(..., Some(1/2), ...)` rate replacement
rejects scale2. Original occurrence1/3, weights(1) and model remain unchanged.
A valid public CLI scale2 control confirms the ordinary configuration works.

Run `cargo test --locked -p rmagna --test weighted_noise_validation`. Both
unchanged original methods are separately executed with a fresh verified
production extension and the identified existing read-only test oracle, without
the original build-capable loader or reference writes. Rust uses explicit
identity/fault indices, exact typed models and structured CLI errors; no equality
with native Python Random trajectories or object/keyword ABI is claimed.

Only these two complete methods are newly mapped. All five finite original
`WeightedNoiseTests` methods are now mapped; the separate original
`NativeNoiseStreamTests` range protocol is covered separately by the sequential
noise-stream witness below.
The source-bound ledger requires82 unique Rust witnesses from84 declarations
and36 targets:69 verified, zero partial and283 unmapped original methods.
Strict complete parity still exits2. General programmatic IR, broader M6 and
paper-result reproduction remain separate.

## Witness output admission

Witness and receipt destinations must be fresh and distinct after resolving
parent-directory aliases. Existing files, directories and symlinks, including
broken symlinks, are refused before native capture. Destinations inside the
resolved upstream or Rust input tree are also refused. The ratio generator
additionally protects the admitted helper's source tree and binary directory.
Destination parents must already exist; a symlink to an ordinary output parent
is allowed when its resolved destination remains outside every input tree.

The shared contract covers `original_ratio_rounding_vectors.py`,
`original_noise_reference.py`, `native_core_contract_reference.py` and
`native_core_reference.py`. The historical noise and core-contract entrypoints
retain their original numerical loops. Noise writes one JSON file; core-contract
capture writes a fresh directory containing the scenario families, receipt and
upstream license. The latter reads only the literal `dense` and `exact` oracle
definitions, without importing the original test module or its build-capable
loader. Existing mathematical fixtures and coverage mappings are unchanged:
at the admission-only change the ledger retained 72 verified and 280 unmapped methods,
with 85 unique Rust witnesses from 87 declarations across 37 targets.

The native-core fixture retains its original producer hash and numerical values.
`rmagna/tests/fixtures/native_core_producer.py.source` is the authenticated,
data-only snapshot of that historical producer, rather than an executable
entrypoint. Rust verifies its original identity, and a Python regression compares
every numerical function's AST with the current generator. This keeps fixture
provenance intact when CLI destination admission changes.

Run the admission regressions without native extensions:

```sh
python3 -B -m unittest discover -s benchmarks/magna_reproduction -p test_witness_paths.py
python3 -O -B -m unittest discover -s benchmarks/magna_reproduction -p test_witness_paths.py
```

## Original sequential noise stream

`original_noise_stream_vectors.py` captures the complete original
`NativeNoiseStreamTests.test_original_randrange_stream_and_exact_weights`
method using the pinned production native extension. It extracts the
unmodified class AST rather than importing the upstream test module, whose
unrelated imports can access a build-capable test helper. All 960 original
finite assertions run: 480 fault/flip/exact-weight comparisons and 480 full
native/reference Python random-state comparisons. The capture independently
checks all 3,076 ordered range requests/results and each draw's tape position.

Build the pinned production extension first, then use fresh destinations
outside upstream and Rust input trees:

```sh
python3 -I -B benchmarks/magna_reproduction/original_noise_stream_vectors.py \
  --upstream /path/to/magna --out /tmp/original-noise-stream.json \
  --receipt /tmp/original-noise-stream-host.json
python3 -I -O -B benchmarks/magna_reproduction/original_noise_stream_vectors.py \
  --upstream /path/to/magna --out /tmp/original-noise-stream-O.json \
  --receipt /tmp/original-noise-stream-O-host.json
cmp /tmp/original-noise-stream.json /tmp/original-noise-stream-O.json
```

The portable fixture retains source identities and the original test source;
the separate host receipt binds the producer, native binary and fixture bytes.
Destination admission precedes reference loading. Source and native hashes are
checked even if capture or an original assertion fails. Offline corruption
controls exercise a wrong oracle with the correct range protocol, existing
files/links/aliases, and changed inputs on failed captures in ordinary and
optimized Python. Rust replays the provided range tape; it makes no Python RNG
implementation claim. Neither dynamic instruments nor paper experiments are
completed by this finite protocol.

The complete original finite stream method is mapped to
`sequential_noise::original_randrange_stream_and_exact_weights`. The current
ledger totals are 73 verified, 279 unmapped, 86 unique Rust witnesses,
88 declarations and 38 targets. Strict complete parity continues to exit 2.


## Original phase injection and raw-resource admission

The complete pinned PhaseInjectionTests.test_exact_native_injection method is
covered by original_phase_injection. The unchanged original C++ test runs with
UBSan; an independent C++ std::mt19937 producer fixes the seed1729 32-bit stream,
48306 draws and all 2720 tensor inputs. Rust reconstructs every selected core
pointwise and checks 37760 exact amplitudes across 800 base tensors and 1920
updates. Prepared boundary expansions also check actual shared-map pullbacks;
certificate, signature-axis and section storage remain frozen across updates.

Controls include n0/n1, the two-T tie, seven dependent axes contracting eight
terms to two, an explicit coefficient-zero raw tensor before resource selection,
scalar/selection failures and recovery. A library unit test calls the real
internal attempt on a valid nonminimal two-coordinate constant/unit certificate;
no fabricated certificate or mock selector is used. Twelve independent C++
MT19937 checkpoints extend through one million draws.

The optional reference scratch-byte cap applies the portable formula described
in rmagna/README.md before pairing and the term-count tie. The original 4096-byte
control has a necessary 11424-byte charge and retains the four-term resource.
The exact 11424 boundary reaches the tie; the previous 512 logical-word cap is
kept as its own accounting model. This finite formula excludes C++ live tables,
entries and ABI overhead and does not measure Rust allocation. The CLI reports
the charge of the actual selected query, whose quotient may be smaller than the
raw circuit. Absent settings retain previous JSON shapes and all three prior
query-family fingerprints; explicit zero and full-u128 caps remain distinct.

Only this complete original method is newly mapped: 77 verified,
275 unmapped, 93 unique Rust witnesses from 95 declarations across
42 targets. The strict complete-parity audit must still exit 2; general
upstream parity and paper-result reproduction remain incomplete.

### Raw Clifford-radical absorption

The production PhasePathTensor::absorb_clifford_radical API now contracts only
directions invisible to both the physical map and actual non-Clifford quotient,
then safely repeats after path reduction. No resource is constructed during
absorption. Arbitrary complex scalars, quadratic/parity phases, noninjective
labels and physical row order are retained. Width, stage, declared-work,
logical-geometry and scalar errors leave the borrowed input unchanged.

The attributed original_global_phase192.json fixture retains every actual input
from seed26091484, n0..7 and24 trials of the pinned original primary method.
Its module SHA256 is
b9d596e34dd777a83ac9aa8feb81e4a246d56bd4fae9db9be663f091927d6b50.
The fixture includes complete complex amplitudes and coherent norms. The Rust
witness checks literal input sums, absorbed amplitudes, actual catalogue and
no-catalogue native resources, all expanded amplitudes and every pairwise norm
cross term. Generic384 complex-coefficient inputs provide additional fresh
coverage; their seed2609148401 does not claim byte identity with an earlier
private scratch prototype. Both fixtures use the upstream MPL-2.0 attribution
and the full license in rmagna/tests/fixtures/original_many_record_input.LICENSE.

Run cargo test --locked -p rmagna --test original_radical_absorption.
Additional production tests cover joint visibility, repeated reductions, exact
zero/empty inputs, 192 packed logical coordinates with cancelling odd axes,
193-bit ordered physical labels, typed quota boundaries, scalar overflow and
immutable retries. The three-path rank-two example reaches one path and a
rank-zero one-term final resource; selecting first rejects the same rank cap.

Only test_actual_absorption_and_decomposition_against_independent_dense_sums is
newly mapped. The separate complete exception/certificate method stays unmapped:
native reports initial needed two, while the current Rust selector reports the
first rejected direction, needed one. Full native certificate dictionaries,
Python ABI, complete native heap accounting, original-IR integration and paper
performance remain separate gaps. This stage recorded78 verified,274 unmapped,
94 unique Rust witnesses from96 declarations across43 integration targets.


### Original raw phase admission

Run `cargo test --locked -p rmagna --test original_raw_phase_admission`.
The fixture `rmagna/tests/fixtures/original_phase_decompositions.json` preserves
all 12426 original bytes of `core/magna/data/phase_decompositions.json` at
commit `56847478c840395735146ef1e1ccdc3d8aedc10f`, SHA256
`81b0487232c624b202c1ecd53123980ec29ada377599395c0c18f92d98ccac76`.
Its adjacent NOTICE identifies the original MPL-2.0 source and bundled license.

The public JSON parser certifies all fifteen original entries; the witness
also reconstructs every certificate independently over all its assignments.
Changing the original first unit coefficient from one to two, or appending
the original first entry again, rejects without publishing staged entries.
Valid catalogues remain usable after refusal. T products on one through six
coordinates use genuine catalogue/product routes and retain every one of the
126 literal normalized, globally phased complex amplitudes
`omega^(3+popcount(y))*sqrt(2)^(-n)`. Every ordered pair of actual emitted
stabilizer terms contributes to the coherent norm, which is exactly one.

The second complete original method retains n2/mask4 padding refusal and the
one-coordinate quadratic upper diagonal refusal through real typed public
constructors. Valid counterparts, unchanged source values, raw zero-phase
absorption/selection/expansion and successful retries are checked.

Only `test_database_exact_reconstruction_and_product_identification` and
`test_malformed_dimensions_and_zero_phase_conventions` are newly mapped.
This stage recorded80 verified,272 unmapped,96 unique Rust witnesses from98
declarations across44 integration targets. The separate complete native
exception/certificate method still reports needed two versus Rust's first
rejected direction, needed one, and remains unmapped. Python ABI, complete
native heap accounting, original-IR integration and paper performance remain
separate gaps; the complete reproduction is still unfinished.

### Original branches after Clifford-radical absorption

Run `cargo test --locked -p rmagna --test original_absorbed_branches`.
The complete pinned original method
`test_original_ir_exceeds_fixed_budget_and_succeeds_after_native_absorption`
is now exercised through real original-IR collection and
`OriginalBranch::compile_after_absorption` under both compiler strategies.
The new certificate projects actual records and absorbs hidden Clifford paths
before selecting its explicit branch resource. Both outcomes retain exact
probability one half with rank zero, one certified/emitted term and one
ordered probability pair. The existing source-before-projection route remains
unchanged and still refuses the same restricted source expansion.

An actual X fault after T2 and before Tdg2 preserves trace probability one half,
but the new branch certificate rejects rank zero and reports rank two under
relaxed bounds. Independent physical gates check all branch amplitudes and full
coherent norms. Further regressions cover impossible records, zero-term/pair
zero certificates, output/environment chronology, reported versus true feedback,
stored decisions without callback replay, global phases and immutable recovery
after path, projection, geometry, stage, work, term, pair and scalar refusals.

Only this whole original method was newly mapped at this step. Its scope was81 verified,
271 unmapped,98 unique Rust witnesses from100 declarations across45 integration
targets. The strict complete-parity audit must still exit2. The separate native
exception/certificate needed-two versus needed-one policy remains unmapped.
General original-IR coverage, full native certificate serialization, Python ABI,
complete heap-byte accounting and paper performance remain separate gaps.

The additional six-wire balanced CCZ/Z circuit has zero all-X event amplitude
and probability but a nonzero absorbed tensor with quotient rank six. Both
compiler strategies and both explicit resource policies retain their actual
resource and nonempty coherent states; the full ordered-pair norm cancels to
zero. A rank-zero compile cap still refuses, and a valid retry succeeds.
Only an explicit zero coefficient after projection/absorption skips resource
selection. This additional frontend regression does not map another upstream
method or introduce an implicit zero-detection/normalization policy.


## Exact retained-record mass draws

Run `cargo test --locked -p rmagna --test original_record_sampling`.
The complete original `test_commuting_kernel_includes_joint_parities` now has
an actual original-IR Rust witness under both compilers. The odd XOR phase
followed by destructive X readouts retains exact masses `(2+sqrt2)/4` at00
and `(2-sqrt2)/4` at11, with zero mass at01/10. Native and Catalogue policies
produce four and two coherent mass terms respectively, with the same law.

`OriginalRecordProbabilities::prepare_affine_sampling` constructs an immutable
conservative affine support bound and relabels canonical mass terms without
changing their coefficients/quadratics. This witness has offset0, independent
direction3 and dimension1. Each draw makes one conditional coordinate query,
returns only00or11, reports exact label/total masses and counts actual Boolean
callback bits. Mass terms are summed, never squared or sampled as a positive
mixture. No record truth table or complete side resource is constructed.

Further tests cover report inversions,129 repeated rows, nonpoint terms,
complex overlaps and canceled labels, variable phases, uniform/deterministic
and zero/invalid laws, immutable preparation/draw quotas, scalar refusal and
valid recovery. A two-coordinate nonuniform law proves that entropy exhaustion
counts the actual prefix across coordinates and never overconsumes the source.
The unchanged original method passes with a fresh production native extension
and identified existing helper in normal and optimized Python; a genuine
returned-mass sign mutant is rejected. Independent rational physical gates
check all four complex amplitudes and coherent norms.

Only this complete original method is newly mapped. Current scope is82 verified,
270 unmapped,99 distinct Rust witnesses from101 declarations across46 targets.
The strict complete-parity audit must still exit2. General native commuting-axis
certificates, Python RNG/class ABI and throughput/paper parity remain incomplete.

### Original scientific scalar-ratio contracts

`original_scalar_ratios` maps exactly the four complete
`NativeFileRuntimeTests.test_scaled_scalar_ratios_*` methods from the pinned
`tests/core/test_file_native.py`. The original file SHA256 is
`b078965d22ae894eb7b1bb9d16000d49ae43f7ccd5727074d05802d7b572e4a1`;
the existing identified helper source SHA256 is
`87aeaf19cfcdfb7223b65a4acb43556bf8a2540f0ca6ca8acdaf5966c5b4a24e`.
The four unchanged methods also run independently through a freshly built
identified helper in normal and optimized Python; a returned-native-mantissa
sign mutant is rejected. The original build-capable Python loader is not used.

The Rust export rounds only the coefficient ratio, then combines original binary
powers in wide arithmetic. Literal native tuples cover both signs of the
underflowing `9*2^-1160`, exponent difference `-2,000,000`, zero and a positive
million-scale exponent. A 512-bit work budget admits these separate powers while
the same cap refuses their expanded rational field representation.

All three native half-even ties retain their exact expected mantissa bits.
The 120-step Pell inputs and independently checked expected bits are pinned in
`rmagna/tests/data/original-scalar-ratio-reference.json`. Its 600-digit Decimal
oracle uses the large unit and reciprocal norm-one identity, avoiding the
original subtractive-cancellation formula. Additional tests preserve negative
numerators, reject nonreal/nonpositive denominators even for zero numerators,
bound coefficients/intermediate work, permit immutable retries and check final
exponent overflow and cancellation across intermediate i64 limits.

Current coverage is 86 verified and 266 unmapped original methods, with zero
partial or architecture-specific exclusions. All 103 distinct witnesses from
105 declarations across 47 targets must actually execute in the source-bound
audit. Strict complete parity still exits 2. Other original file-native methods,
Python tuple/class ABI, binary64 after scaling, defaults, performance and paper
precision curves remain outside this finite mapping.

## Original global signed-mass draws and seed replay

The three complete private-native methods in pinned `test_file_native.py`
(SHA256 `b078965d22ae894eb7b1bb9d16000d49ae43f7ccd5727074d05802d7b572e4a1`)
map to the retained owner's private `original_contracts` library tests:
`test_affine_mixture_big_boundary_and_reproducibility`,
`test_exact_irrational_signed_global_marginal`, and
`test_complex_terms_cancel_before_sampling`.

Run `cargo test --locked -p rmagna --lib original_contracts` for three tests.
The fixtures preserve all 256 wide seeds plus exact replay, all 1024 signed
seeds with the original 0.035 frequency tolerance, and ten deterministic complex
cancellation draws. Every draw checks exact selected/total mass, independent
support dimension, conditional-query count, and actual callback-bit consumption.
The complex control distinguishes the true linear total one from the incorrect
sum of individually squared coefficients three. Raw terms remain private.

The unchanged original methods are separate native-reference checks, using
the pinned production core and identified test helper rather than the original
build-capable loader. Returned-word mutations must fail all three methods.
Rust uses caller-owned SplitMix64 Boolean bits; native MT stream identity,
signed-seed ABI refusal, general input validation and paper throughput remain
outside this finite sampling contract.

This increment has 89 verified and 263 unmapped original methods, zero partial
or architecture-specific exclusions, and 106 distinct witnesses from 108
declarations across 47 integration targets plus the library harness. The strict
source-bound audit must execute all mapped assertions and still exit 2 because
overall parity is incomplete. The library witness parser also has positive
registration and qualified-run controls and explicit refusal cases; synthetic
transcripts in its Python tests are parser fixtures, not execution evidence.

## Original identical-irrational scaled ratios

The complete pinned `NoiseNumericFastpathTests` method
`test_identical_irrational_coefficients_and_invalid_denominators` in
`tests/core/test_noise_numeric_fastpaths.py` (SHA256 `80169c9ceed6deecd8cd81a99d6ccbd9f0aabc0aff13bdfb8f5564ccdb152855`)
maps to `original_identical_irrational_coefficients_preserve_scaled_ratios_and_refusals`
in `original_scalar_ratios`. Run it with:

```sh
cargo test --locked -p rmagna --test original_scalar_ratios \
  original_identical_irrational_coefficients_preserve_scaled_ratios_and_refusals -- --exact
```

Both original pairs use `c = 3 - 2*sqrt(2)`, whose positivity and nonzero
value follow from `9 > 8` and the conjugate norm one. The exact Rust outputs
`(1.0, -2000000)` and `(-1.0, 100)` match the original frexp tuples
`(0.5, -1999999)` and `(-0.5, 101)`. The test retains both original
zero-numerator refusals for negative `c` and complex `1+i` denominators,
with distinct typed errors. Inputs remain unchanged and sufficient-budget
retry succeeds after an 8-bit work budget refuses. The identical 128-bit
limit admits compact export and refuses expanded million-scale scalars.

The whole unchanged original method also runs through a freshly built pinned
core and identified native helper in normal and optimized Python, with no
original build-capable loader import. A returned-mantissa sign mutant must
fail the original assertions. This promotes exactly one original method;
four existing mappings receive only the owning file hash refresh. Current
coverage is 90 verified and 262 unmapped, with zero partial or architecture-
specific exclusions, and 107 distinct witnesses from 109 declarations across
48 targets. Strict complete parity still exits 2. Python ABI, numerical
performance and paper precision curves remain outside this finite contract.

## Original categorical noise distribution laws

`original_noise_distributions` maps the complete pinned
`NoiseNumericFastpathTests.test_depolarizing_and_nonuniform_channels_match_declared_law`
method in `tests/core/test_noise_numeric_fastpaths.py` (SHA256
`80169c9ceed6deecd8cd81a99d6ccbd9f0aabc0aff13bdfb8f5564ccdb152855`). Run:

```sh
cargo test --locked -p rmagna --test original_noise_distributions -- --nocapture
```

The four original fault groups retain fifteen alternatives of probability
1/30, `[0,1/10,1/5,0,3/10]`, `[1,0]`, and no alternatives; their residual
identity probabilities are 1/2, 2/5, 0 and 1. A finite 30-by-10 oracle visits
all 300 accepted integer positions and checks all 96 joint count cells,
including forbidden alternatives, with 64 positive joint outcomes. Each actual
draw checks ordered nonduplicated faults, exact physical/proposal joint mass,
likelihood one, forced/empty endpoints and exactly two consumed entropy words.
A one-word entropy cap refuses with typed `EntropyLimit`; retrying the same
model with words `[29,9]` and a two-word cap recovers final alternatives and
exact mass 1/100. Original input category vectors remain unchanged.

All 32,768 original-size trials retain the original marginal tolerances
`6*sqrt(shots*p*(1-p))+2`. The joint fault count retains the stricter original
bound `6*sqrt(shots*0.3*0.7)` with no additive slack. Rejection sampling can consume more than
two words for these stochastic trials; actual provider calls must match the
reported count. The first 41 complete draws equal separate per-seed calls of
17 and 24, including weights and entropy. Rust uses independent SplitMix64
streams; the original native MT/Python Random trajectory is not an ABI claim.
The complete unchanged original method also passes through a fresh pinned
core and identified helper in normal and optimized Python; an actual returned-
weight mutant is rejected, with no original build-capable loader import.

Only this one complete original method is newly mapped. Current coverage is
91 verified and 261 unmapped, with zero partial or architecture-specific core
exclusions. All 108 distinct witnesses from 110 declarations across 49 targets
must execute in the strict source-bound audit, which still exits 2 because
overall parity is incomplete. Existing fixtures, generators and mappings,
production sampling defaults, Python/native RNG ABI and paper performance
claims remain unchanged.

## Lazy finite occurrence draws

`OccurrenceThreshold::draw_with` and `draw_bits_with` provide opt-in lazy
finite-resolution occurrence draws. Run:

```sh
cargo test --locked -p rmagna --test original_finite_occurrence
```

The two owning tests borrow every literal finite cutoff and endpoint from
pinned `tests/native/rational_random.cpp`: third, half, zero, certain, the
reachable 2^-64 bucket, positive but unreachable 2^-65, near-one, the wide
64-bit denominator, and rates 1/1000 and 1/10000. Scalar equality rejects;
zero/certain/public zero cutoffs and empty live masks never invoke a provider.
Preparing or drawing a rounded threshold preserves the exact stored rational.

All 560 original bit-plane trials retain seed 20260927 and the independently
checked MT19937-64 engine. They perform 35,840 ordinary scalar comparisons and
reach exactly 36,393 generated input words. Expected masks use direct scalar
`<`; expected provider calls use shrinking scalar prefix intervals, rather
than the implementation lane masks. The original native loop stops when
pending lanes or remaining cutoff bits become zero. In particular, half
needs one plane, while the wide cutoff rejects an equal prefix after 63.
Partial masks suppress inactive lanes, and explicit no-extra-plane controls
reject correct answers that consume unnecessary entropy.

Only the finite occurrence portion of
`NoiseNumericFastpathTests.test_finite_noise_thresholds_and_general_conversion_in_native_module`
is mapped. Its complete pinned Python source SHA256 is
`80169c9ceed6deecd8cd81a99d6ccbd9f0aabc0aff13bdfb8f5564ccdb152855`.
The complete unchanged original method separately compiles and runs its
standalone CPP probe at O2 with assertions enabled, without a native/helper
extension rebuild. Rust still lacks this method's separate `uniform_index`
rejection/count-one cases, 512 hardware/general exact ratio comparisons and
FE_UPWARD fallback/rounding restoration assertions. The ledger therefore
records this whole method as partial, with these gaps explicit.

Current coverage is 91 verified, 1 partial and 260 unmapped original methods,
with zero architecture-specific core exclusions. All 110 distinct witnesses
from 112 declarations across 50 targets must execute; strict complete parity
still exits 2. Every previous mapping, fixture and generator is preserved.
These APIs do not change categorical/default sampling, add a native RNG ABI
layer or establish benchmark/paper performance.

## Original importance proposal channel laws

Run the two complete original-method Rust witnesses:

```sh
cargo test --locked -p rmagna --test original_importance_channels
```

They retain the exact source circuits, rates and scales from pinned
`tests/core/test_importance_sampling.py` (SHA256
`fc24b087ed08c9dcd35a8f6befe373d6b5a23c5709749dae207f2b1a0fe75caf`).
Actual source metadata preserves Pauli XYZ order, unconditional correlated
probabilities 1/10 and 1/5, report-flip record zero, guard expressions and
zero/one endpoints. At scale two, the three tilted nonidentity channel lists
are [1/10,1/5,0], [1/5,2/5] and [1/50]; guarded and endpoint categories remain
unchanged. Every identity and alternative satisfies an independently supplied
exact likelihood identity. Literal identity factors are 17/14, 7/4 and 99/98,
with common fault factor 1/2 even for a zero alternative. No 0/0 is evaluated.

The support test rejects both the original boundary and boundary plus one:
X and Pauli total 1/5 at scales 5 and 6, correlated total 3/10 at 10/3 and 13/3,
and measurement 1/4 at 4 and 5. Errors identify model category zero. Scale-two
retries match literal probabilities and leave physical inputs unchanged. The
final noiseless-M/guarded-X/zero/one case stays unchanged at scale 100. Its
source internal sites [1,2,3] differ from dense model rows [0,1,2]; excluding
model row zero protects the guard, while reusing source site one is rejected.

Both complete unchanged original Python methods separately passed with fresh
production and identified-helper extensions in both Python modes. The needed
`weight_factors` hook is extracted unchanged, without importing its original
build-capable module. A mutation of the actual returned native identity
factor is rejected by the unchanged first-method assertion.

Only these two complete ledger rows are promoted. Current coverage is 93
verified, 1 partial and 258 unmapped, with zero architecture-specific core
exclusions. All 112 distinct witnesses from 114 declarations across 51 targets
must execute; strict complete parity still exits 2. The numerical method
partially mapped by `original_finite_occurrence` retains its explicit gaps.
Existing mappings, fixtures, generators and sampling defaults are preserved;
these finite laws do not establish native random-trajectory identity, Python
keyword compatibility or paper performance.

## Original weighted task outcome identities

Run the complete original-method Rust witness:

```sh
cargo test --locked -p rmagna --test original_importance_outcomes
```

The exact source circuit has X error 1/10 on wire zero, a noiseless measurement,
X error 1/5 on wire one, a detector on the measurement and postselection on
that detector. The task compares output wire one with the zero target.
It retains the complete original importance method from pinned
`tests/core/test_importance_sampling.py` (SHA256
`fc24b087ed08c9dcd35a8f6befe373d6b5a23c5709749dae207f2b1a0fe75caf`).

Both compiler strategies sample all four configurations at scales 1/2, 1
and 2: 24 actual draws through `TaskEvaluator::sample`. Independent literal
physical probabilities are [18/25,9/50,2/25,1/50], ordered by faults
[(false,false),(false,true),(true,false),(true,true)]. Each proposal table and
likelihood table is independently supplied. Exact admitted fault bucket words
are [19,9], [9,4] and [4,3] for the three scales; each identity uses word zero.
Every draw consumes exactly two actual source words.

Per-draw checks precede every aggregate: physical probability and likelihood,
raw postselection/error/target masses, unit target normalization and history
factors, and weighted acceptance/error/target contributions. Rejected
configurations retain their independently checked physical probabilities and
likelihoods while contributing zero accepted/error/target mass. Finite sums
weighted by proposal probabilities give normalization 1, acceptance 9/10,
error 9/50, target success 18/25 and conditional error 1/5.

The complete unchanged original Python method separately passed with the fresh
production extension in normal and optimized modes. Its twelve prescribed
draws per mode were observed without extra replay or importing the original
build-capable support modules. Mutating the actual first returned accepted
contribution to one is rejected by that unchanged method.

Two private Rust controls separately compiled against fresh own libraries,
then changed the actual first accepted producer output 16/19 to one (ignored
weight) or 256/361 (double weight). Both failed the same unmodified known-answer
assertion for the intended exact-value reason. Controls remain outside
production and repository source; the final public candidate is byte-identical.

Only this one complete ledger row is promoted. Current coverage is 94 verified,
1 partial and 257 unmapped, with zero architecture-specific core exclusions.
All 113 distinct witnesses from 115 declarations across 52 targets must execute;
strict complete parity still exits 2. Prior mappings, fixtures and generators
remain intact, including the explicit numerical-method partial gaps.
These exact finite identities do not establish native random-stream ABI,
Python keyword/serialization compatibility, weighted-summary standard errors,
Monte Carlo convergence or paper performance.

## Original nonlinear postselection truth tables

Run the three complete original-method witnesses:

```sh
cargo test --locked -p rmagna --test original_nonlinear_postselection
```

The pinned source is tests/core/test_nonlinear_postselection.py, SHA256
76becb6b2146a1c7bcee326b3a7daab1aac5de12390f81887175d417d07351e8.
The owning tests retain the complete noncommuting readout, mixed affine
selection and joint duplicate/conflicting-condition methods. These three
rows are covered by this preceding stage. The dense instrument below adds a
fourth complete method; thirteen methods in that original module remain unresolved.

Both compiler strategies and both ColdPrefix and FixedTemplate causal runtimes
run the exact source circuits. The native implicit idle-wire zero target is
represented by an explicit Rust I 0 target. Typed stateless AND returns one
bit; echo returns its input vector. Noncommuting readout has acceptance 3/4,
error 1/4, target success 1/2, accepted Z moment 1/4 and conditional error 1/3.
Mixed selection keeps the third record unobserved: every sample has captured
history mass 1/4 and acceptance 1/2, while physical enumeration gives error
1/8, target success 3/8 and conditional error 1/4. The native count of two
independent sampled parities is not the Rust count of probability queries.

All four prescribed AND-input histories and eight actual sampling trials
are checked for each compiler/runtime combination in the first two tests.
The third preserves the original duplicated-detector/echo prefix and all seven
suffixes in source order: acceptance 1/4, 1/4, 1/2, 0, 1/4, 0, 1/4.
Every physical enumeration has covered probability 1 and omitted probability
0, with identical lower and upper acceptance/error bounds. Conditional
intervals are inspected only when defined.

The complete unchanged three original methods separately passed in normal
and optimized Python against fresh production native builds, with nine actual
enumeration calls and eight sample trials per mode. Only complete unchanged
registry/exact helper AST was extracted; original build-capable support
modules were not imported. Changing the actual first native acceptance from
3/4 to one is rejected by the unchanged original assertion.

Three private Rust semantic controls compiled against fresh own libraries and
failed the unchanged literal witnesses after confirming their actual altered
results: replacing the second measurement-basis rotation correlates the two
measurements, an always-zero final selector accepts every branch, and asking
to observe the third record changes captured history mass to 1/8. All controls
stay outside repository source; the tested public candidate is unchanged.

This stage has 97 verified, 1 partial and 254 unmapped methods, with no
architecture-specific core exclusions. All 116 distinct witnesses from 118
declarations across 53 targets must run in the source-bound audit; strict
complete parity still exits 2. Prior mappings, fixtures and generators remain
intact, including the numerical-method partial gaps. This finite validation
does not establish Python keyword/serialization/private-handle compatibility,
native random trajectories, new sampling defaults, Monte Carlo convergence
or paper performance.

## Original dense nonlinear readout instrument

Run the complete original-method witness, then the complete four-test target:

```sh
cargo test --locked -p rmagna --test original_nonlinear_postselection original_dense_noncommuting_magic_instrument_with_report_noise -- --exact
cargo test --locked -p rmagna --test original_nonlinear_postselection
```

This extends the same pinned nonlinear suite and preserves the preceding
three tests and helpers byte for byte. Only the complete dense original
method is newly verified; the three existing witnesses receive the updated
SHA256 for their extended source file. Other contracts, original module
hashes, fixtures, generators and numerical partial gaps remain intact.

For H/T/MX/S/T/M(1/3)/H/T/MY, the opaque AND observes the first and last
readouts. XOR/NOT selection uses the middle reported bit. Wire 0 is compared
against the ideal H then S target. Both compiler strategies and both
ColdPrefix/FixedTemplate causal runtimes enumerate two physical noise
assignments and eight observed histories. Coverage is one, omitted mass
zero, physical sector probabilities are 2/3 and 1/3, and target norm is one.

The independent two-dimensional amplitude
P_y(c) T H P_z(b) T S P_x(a) T H |0> is summed over all sixteen
(a,b,c,report-flip) choices with selection (a AND c) XOR (b XOR flip).
Exact rational coefficients in Q(sqrt(2), i) give total norm one, accepted
mass (13-sqrt(2))/24, error 7/24 and target success (6-sqrt(2))/24.
Conditional error is (91+7sqrt(2))/167. The owning Rust assertions compare
exact cyclotomic values and all lower/upper interval endpoints. A separate
NumPy branch calculation agrees within 1e-12; that tolerance applies only
to the numerical cross-check.

The unchanged complete original method and registry helper AST also pass
in normal and optimized Python against a fresh native production build.
The original module top level and build-capable support loaders are not
imported. Actual input controls replace M(1/3) by X_ERROR(1/3) then ideal M,
with unchanged known answers: native and Rust acceptance changes to
(5-sqrt(2))/8. The private Rust control compiles and fails the same exact
acceptance assertion after confirming that altered result. These controls
stay outside repository source; the public candidate remains unchanged.

The ledger now has 98 verified, 1 partial and 253 unmapped methods.
All 117 distinct witnesses from 119 declarations across 53 targets must run
in the source-bound audit. Strict complete parity still exits 2. This finite
validation does not claim native RNG ABI or Python object compatibility,
Monte Carlo convergence, new defaults or paper performance.

## Original weighted nonlinear aggregation

Run the complete original-method witness, then the complete five-test target:

~~~sh
cargo test --locked -p rmagna --test original_nonlinear_postselection original_importance_weights_rejected_draws_and_aggregate_ratio -- --exact
cargo test --locked -p rmagna --test original_nonlinear_postselection
~~~

This extends the same pinned nonlinear suite. The preceding four-test and
helper source prefix remains byte-identical. Only this weighted aggregation
method is newly verified; the four existing witnesses receive a mechanical
source SHA256 refresh. Every other contract, fixture, generator, original
module hash and partial numerical gap remains unchanged.

Physical independent X-error rates are 1/10 and 1/5. Proposal scale two gives
rates 1/5 and 2/5. Indexing bitmasks as a + 2*b gives multiplicities [12,3,8,2],
physical masses [18/25,2/25,9/50,1/50], proposal masses
[12/25,3/25,8/25,2/25] and likelihoods [3/2,2/3,9/16,1/4].
An opaque version-one AND rejects only a=b=1; wire zero is compared with a
zero-state target. Both compilers and both ColdPrefix/FixedTemplate causal
runtimes take all 25 actual typed samples using deterministic words admitted
to the exact categorical buckets. Every draw checks two consumed noise words,
its physical mass and likelihood, controller transcript, history mass and
weight one, target normalization one, raw acceptance/error/target success
and the corresponding once-weighted contributions.

The streaming accumulator retains both rejected draws in count 25. Exact
means are acceptance 49/50, error 2/25 and target success 9/10. The aggregate
conditional ratio is 4/49, different from the wrong per-draw ratio average
3/25. Mean diagnostic weight is one and weight-only ESS is 2400/119.
A separate Fraction truth-table oracle checks these finite sums.

Before the first Rust run, the complete unchanged original method, registry
helper and file_hooks.summary AST pass in normal and optimized Python.
Four actual supplied NoiseDraw samples form the original 25-entry mixture,
which the freshly compiled, identified native testing helper summarizes.
A recorded loader returns that already-built helper; original build-capable
Python loaders and support-module top levels are not executed. Actual native
input controls drop the two rejected entries before summarization, producing
23 entries and acceptance 49/46; the unchanged original assertion rejects it.
Two private Rust variants of the final owning source compile successfully,
then fail the unchanged count or exact mean assertion when rejected pushes
are omitted or already weighted contributions are multiplied again. The
controls also confirm the actual wrong count or mean before that failure.

The ledger now has 99 verified, 1 partial and 252 unmapped methods.
All 118 distinct witnesses from 120 declarations across 53 targets must run
in the source-bound audit. Strict complete parity still exits 2. These
deterministic finite-mixture checks do not claim native RNG trajectory or
Python private-representation compatibility, Monte Carlo uncertainty,
new sampling defaults or paper performance.

## Original full-history controller readout

Run the complete original-method witness, then the complete six-test target:

~~~sh
cargo test --locked -p rmagna --test original_nonlinear_postselection original_full_history_final_readout_and_explicit_state -- --exact
cargo test --locked -p rmagna --test original_nonlinear_postselection
~~~

This appends the one pinned full-history method while preserving the preceding
five-test/helper source prefix byte for byte. Only this original-method row is
newly verified; the five earlier witnesses receive mechanical source SHA256
refreshes. Every other contract, status, module hash, fixture, generator and
partial numerical gap remains unchanged. All earlier per-change inventories
retain their historical values.

The physical sequence initializes ctrl[0] to one, then executes X/MR, X/MR
and X/M(1) on wire zero, declaring each reported outcome as a detector. The
opaque version-one decode callback reads detector 0, detector 1, the last
detector and the old ctrl[0], computes (a AND b) XOR final XOR prior, and writes
the same decision register. Postselection accepts zero; the task explicitly
requests an empty output list. The first two reports are true because MR
resets the state after each report. The last report is false because M(1)
flips the report of the final true measurement.

Both compiler strategies and both ColdPrefix/FixedTemplate causal runtimes
assert preparation performs no callback. Each then takes three actual draws,
with one opaque invocation per draw: inputs [true,true,false,true], output
false, exact acceptance one and error zero, unit raw acceptance/history mass,
zero raw error, unit history weight and physical-noise probability/likelihood.
There is no ideal target, target normalization, requested record or moment.
The captured side-effect list is exactly three copies of the input vector,
showing the prior decision was read before its overwrite.

The complete unchanged original method also passes normal and optimized
Python via an actual fresh-native production run with three shots. The
identified testing helper is freshly built but is not imported or used for
this method. Original test-module and build-capable support top levels do
not execute. An independent deterministic binary-state oracle checks both
reset models. Actual native input controls replace only the first MR by M,
observe acceptance zero and fail the unchanged original acceptance assertion.
The matching private final-source Rust variant compiles successfully and
fails the same exact acceptance assertion (exit 101) after confirming zero.
That known-answer assertion precedes transcript assertions, so the control
demonstrates the intended acceptance failure.

The ledger now has 100 verified, 1 partial and 251 unmapped methods.
All 119 distinct witnesses from 121 declarations across 53 targets must run
in the source-bound audit. Strict complete parity still exits 2. The separate
report-flip method stays unmapped: this deliverable does not normalize or
promote its explicit-argument noise-category metadata. These finite checks
make no native RNG trajectory, private Python representation, new-default,
Monte Carlo uncertainty or paper-performance claim.

## Original report flips and true output

Run the complete original-method witness, then the complete seven-test target:

~~~sh
cargo test --locked -p rmagna --test original_nonlinear_postselection original_report_flips_select_reported_bits_and_preserve_true_output -- --exact
cargo test --locked -p rmagna --test original_nonlinear_postselection
~~~

Only this complete original method is newly verified. The preceding 703-line
six-test/helper prefix stays byte-identical; the six earlier witnesses receive
mechanical source SHA256 refreshes. Every other status, contract, fixture,
generator and partial numerical gap remains unchanged. Earlier per-change
inventories retain their historical values.

The original circuit prepares wires zero and one in one, reports the first
measurement with probability-one classical inversion, and measures the second
ideally. Two detectors feed the opaque AND controller and zero postselection.
Both compiler strategies and both ColdPrefix/FixedTemplate causal runtimes
sample that program and its original mixed AND/XOR selector. Requested records
are [false,true]. Acceptance and joint error are one, target success is zero
against a unit zero-state target on output zero, and history mass/weight and
physical-noise probability/likelihood are one. Every draw has one opaque AND
transcript with those inputs and output false; unused moments are empty.

The public original_noise_categories API reports exactly one original category:
ReportFlip on record zero, probability one and identity probability zero.
Separate actual causal resolution exposes two internal compiler sites, including
the implicit zero-rate site for the argumentless measurement. The original
expected category count of one is preserved. No filtering, renumbering or
normalization changes production behavior.

The complete unchanged original method and its used registry helper pass normal
and optimized Python through fresh production-native compile/sample and mixed
run calls. Production and identified helper modules are freshly built; this
method never imports or uses the testing helper. Original module/support top
levels do not execute. An independent two-qubit binary-state oracle distinguishes
classical report inversion from physical X while preserving both reported bits.
The actual native input counterexample replaces only M(1)0 with X_ERROR(1)0
followed by ideal M0. Reports and acceptance remain unchanged, but joint error
becomes zero and the unchanged original tuple assertion fails. A matching
private final-source Rust variant compiles successfully and fails the unchanged
exact joint-error assertion (exit101) after confirming reports, acceptance and
zero error. The public candidate remains unchanged.

Coverage is now 101 verified, 1 partial and 250 unmapped methods, with zero
architecture-specific exclusions. All 120 distinct witnesses from 122 declarations
across 53 targets must be covered by source-bound execution audits. Strict
complete parity intentionally exits2. These finite contracts make no private
Python representation, native RNG trajectory, new-default or paper-performance
claim.

## Original decision versions across repeats and overwrites

Run the complete original-method witness and then the complete eight-test target:

~~~sh
cargo test --locked -p rmagna --test original_nonlinear_postselection original_versions_overwrites_repeat_and_explicit_prior_decisions -- --exact
cargo test --locked -p rmagna --test original_nonlinear_postselection
~~~

Only this complete original method is newly verified. The preceding 809-line
seven-test/helper prefix stays byte-identical; seven prior witnesses receive
mechanical owning-file SHA256 refreshes. Other statuses, contracts, fixtures,
generators and partial numerical gaps remain unchanged. Earlier per-change
sections retain their historical counts.

The repeated circuit initializes ctrl[0] to false, then writes separate echo
decisions in two reset/H/measurement iterations. Its two selected expressions
equal public control_analysis values for `RegisterVersion::new(0, 1)` and `RegisterVersion::new(0, 2)`, and
are distinct. Version zero is false and the final version three is true.
The original explicit-prior circuit reads the earlier echo decision before an
AND overwrites ctrl[0]; its inverted selection also survives the final write.

Both compiler strategies and both ColdPrefix/FixedTemplate runtimes enumerate
both original circuits. Accepted mass is exactly 1/4 and error mass zero.
An explicit empty-output task has no target-success or target-normalization
value; the within-sector conditional error is undefined. One physical pattern
and four joint histories cover probability one, with no omitted physical mass.

Before Rust, the complete unchanged original method and both used registry
and exact helpers pass normal and optimized Python through fresh production
native parsing and two complete enumerate_errors calls per mode. The original
parsed targets retain versions [1,2], and both native accepted masses are 1/4.
The original untargeted native error field is recorded as null; Rust follows
its existing empty-output zero-error-mass convention. This is not a private
Python representation identity claim. The identified helper is freshly built
but never imported or used, and original test/support top levels do not run.

A native negative input removes only the per-iteration postselection at the
repeated evaluation parse boundary, after the untouched original version
assertions. Actual acceptance one fails the original quarter assertion.
An independent fair-bit/version-snapshot oracle enumerates all four assignments
for both correct circuits and the missing-selection model. The final-source
private Rust control similarly changes only repeated evaluation input, leaving
the complete original parsed checks intact; it compiles successfully, confirms
accepted mass one and fails the unchanged exact quarter assertion (exit101).
The public candidate is unchanged by the control.

Coverage is now 102 verified, 1 partial and 249 unmapped original methods, with
zero architecture-specific exclusions. Source-bound execution audits require
121 distinct witnesses from 123 declarations across 53 targets. Strict complete
parity intentionally exits2. These finite contracts make no private Python ABI,
native RNG trajectory, new-default, Monte Carlo precision or paper-performance
claim.

## Original selection independent of error definitions

Run the complete original-method witness and the complete nine-test target:

~~~sh
cargo test --locked -p rmagna --test original_nonlinear_postselection original_selection_distinct_from_each_error_definition -- --exact
cargo test --locked -p rmagna --test original_nonlinear_postselection
~~~

Only this complete original method is newly verified. The preceding 875-line,
35,499-byte eight-test/helper prefix remains byte-identical; eight earlier
witness rows receive mechanical owning-file SHA256 refreshes. Other statuses,
contracts, fixtures, generators and historical per-change counts are retained.

Two separately measured H-prepared bits feed an opaque AND controller and
zero-postselection. A conditional X copies the first bit to output qubit two.
The classical task echoes the first detector into a separate decision for
observable zero; the quantum task uses output two and a unit-norm zero.magna
target. Both compiler strategies and ColdPrefix/FixedTemplate runtimes give
accepted mass 3/4, joint error 1/4 and conditional error 1/3. Only the quantum
target has target-success mass 1/2 and normalization one. The classical task
has neither target field. One physical pattern and four joint histories cover
probability one without omitted mass.

Before Rust, the complete unchanged original method and both registry/exact
helpers pass normal and optimized Python through fresh production native code.
Each mode makes three public parses, including the unchanged I 0 ideal-state
parse, and two complete enumerate_errors calls with accepted mass 3/4 and
joint error 1/4. The identified helper is built fresh but never imported or
used. Original test/support top levels and build-capable loaders are not run.
An independent fair-bit/basis-state oracle enumerates four assignments for
three models, including the wrong-error counterexample.

The native counterexample changes only the classical evaluation input's
OBSERVABLE_INCLUDE decision from ctrl[1] to the selected ctrl[0]. Acceptance
stays 3/4, but the accepted zero-selection decision has error mass zero and
fails the unchanged original joint-quarter assertion. The private final-source
Rust counterexample preserves the original circuit constants and known
assertions, changes only that evaluation input, compiles successfully, confirms
actual accepted mass 3/4 and error zero, and fails the same exact error assertion
(exit101). Public Source is unchanged by either control.

Coverage is now 103 verified, 1 partial and 248 unmapped original methods,
with zero architecture-specific exclusions. Source-bound audits require actual
execution of 122 distinct witnesses from 124 declarations across 53 targets.
Strict complete parity intentionally exits2. These finite contracts make no
private Python representation/ABI, native RNG trajectory, new-default,
Monte Carlo precision or paper-performance claim.
