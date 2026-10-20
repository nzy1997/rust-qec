# Magna reproduction reference

This harness pins the original Magna implementation and paper archive before
an independent Rust backend is compared with them. It inventories 31 packaged
tasks, 57 upstream test modules and every deposited paper file. It does not
implement a simulator or claim reproduction of the paper's precision curves.

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
