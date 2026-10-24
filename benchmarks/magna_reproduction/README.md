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
