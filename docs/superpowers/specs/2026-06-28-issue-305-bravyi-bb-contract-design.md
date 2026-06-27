# Issue #305 Bravyi BB Circuit BP-OSD Contract Design

Date: 2026-06-28
Status: Non-interactive Agent Desk design, auto-approved by standing policy
Scope: GitHub issue #305, evidence infrastructure for the BB circuit BP-OSD comparison

## Context

Issue #305 asks for a checked-in, machine-readable source contract for the
upstream Bravyi bivariate-bicycle circuit BP-OSD run semantics at
`sbravyi/BivariateBicycleCodes@fa77e3333d3ec44c79d8f914dd24c040d1da471b`.
The repository already has `benchmarks/bb_circuit_bposd_compare` with BB
comparison case catalogs, Python `ldpc` replay helpers, hard-replay and
diagnostic verifiers, and README prose that records most upstream defaults.

The pinned upstream checkout is available locally at
`/private/tmp/sbravyi-BivariateBicycleCodes` and is exactly at commit
`fa77e3333d3ec44c79d8f914dd24c040d1da471b`. The relevant source facts are:

- `README.md` lines 16-21 define the result row as physical error rate,
  syndrome cycles, Monte Carlo trials, failed trials, with failures counted per
  trial.
- `decoder_setup.py` lines 50-55 define the 7-round schedule and cycle count,
  and lines 511-618 append two noiseless cycles when building effective
  decoder histories.
- `decoder_run.py` lines 67-72 define `bp_method="ms"`, `max_iter=10000`,
  `osd_method="osd_cs"`, `osd_order=7`, and `ms_scaling_factor=0`.
- `decoder_run.py` lines 329-349 pass those BP/OSD settings into both upstream
  decoders.
- `decoder_run.py` lines 364-415 decode Z first, decode X only when Z
  succeeds, and count one failed Monte Carlo trial otherwise.

## Goals

- Add a compact `bravyi_contract.json` artifact under
  `benchmarks/bb_circuit_bposd_compare/reference/`.
- Add a reviewer-readable `bravyi_contract.md` beside the JSON, with pinned
  upstream provenance links and source line ranges.
- Add `verify_bravyi_contract.py`, a small validator that compares the JSON
  contract against current BB compare defaults and Python replay settings.
- Make `ms_scaling_factor=0` an actively checked field and pass it explicitly
  through the Python `ldpc.BpOsdDecoder` replay path.
- Add pytest coverage for the positive contract, negative controls, explicit
  scaling, and trial-level Z-first/X-if-Z-success semantics.

## Non-Goals

- Do not vendor the upstream Bravyi repository.
- Do not regenerate BB comparison data.
- Do not fix the Rust/Python hard replay mismatch.
- Do not run the upstream 50,000-trial scripts.
- Do not broaden the benchmark schema beyond the fields needed to catch
  contract drift.

## Approach Options

### Recommended: Compact Contract Plus Active Validator

Check in JSON and Markdown reference files, add a validator CLI, and expose the
Python replay decoder kwargs through a helper that the validator can import.
The validator checks the contract, all current BB compare case defaults, the
manifest scaling field, Python `ldpc` replay kwargs, and the trial-level
failure predicate.

This is the best fit because it keeps the deliverable small, reviewable, and
machine-checkable without copying upstream source.

### Alternative: Documentation-Only Contract

Add Markdown prose plus a JSON snapshot but no import-based drift checks. This
would satisfy some reviewer-readability needs but would not catch later changes
to `run_compare.py` or case defaults.

This does not meet the issue's validator requirement.

### Alternative: Vendor Or Fetch Upstream Scripts In Tests

Store or download the pinned upstream scripts and compare repository behavior
against the source files directly. This gives strong provenance but violates
the issue's instruction not to vendor upstream source and makes tests depend on
network or large fixtures.

This is intentionally out of scope.

## Design

### Contract Artifacts

`benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json` records:

- upstream repository, commit hash, and pinned tree URL;
- result row column names and `failure_unit = "monte_carlo_trial"`;
- decoder settings: `bp_method = "ms"`, `max_iter = 10000`,
  `osd_method = "osd_cs"`, `osd_order = 7`, `ms_scaling_factor = 0`;
- cycle convention: configured noisy cycles plus exactly two noiseless tail
  cycles;
- failure predicate: decode Z first, decode X only if Z succeeds, count at most
  one failed Monte Carlo trial per sampled trial;
- source references with file names, line ranges, URLs, and supported fields.

`bravyi_contract.md` explains the same contract in prose for reviewers and
links each claim to the pinned upstream blob line ranges.

### Validator

`benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract` loads the JSON,
validates required fields and exact expected values, then imports:

- `benchmarks.bb_circuit_bposd_compare.cases` to inspect
  `SMALL_LDPC_CASES`, `BB72_BB144_PLOT_SMOKE_CASES`,
  `BB72_BB144_FULL_CASES`, `DIAGNOSTIC_CASES`, `SMOKE_CASES`, and
  `HARD_REPLAY_CASES`;
- `benchmarks.bb_circuit_bposd_compare.run_compare` to inspect
  `PYTHON_UPSTREAM_*` constants, `_python_bposd_decoder_kwargs()`, and explicit
  failure-semantics constants.

On success, the CLI prints a PASS line naming the Bravyi commit hash,
`osd_cs`, OSD order 7, `ms_scaling_factor=0`, two noiseless tail cycles, and
`failure_unit=monte_carlo_trial`. On failure, it exits nonzero and prints each
field mismatch with the field path.

### Python Replay Drift Checks

`run_compare.py` gains:

- `PYTHON_UPSTREAM_MS_SCALING_FACTOR = 0`;
- `_python_bposd_decoder_kwargs()` returning the exact kwargs passed into
  `ldpc.BpOsdDecoder`, including `ms_scaling_factor=0`;
- `PYTHON_FAILURE_UNIT = "monte_carlo_trial"`;
- `PYTHON_FAILURE_PREDICATE = "z_first_x_only_if_z_succeeds"`.

The existing Python replay constructors use the helper for hard replay,
ordinary smoke/diagnostic replay, and batched replay through `_python_row`.
Tests verify fake decoders receive `ms_scaling_factor=0` and that `_python_row`
does not call the X decoder when Z logical prediction already fails.

### Tests

Add `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py` with:

- JSON contract loads and validates with no errors.
- Validator negative controls reject `failure_unit = "per_cycle"`,
  `osd_order = 0`, and `ms_scaling_factor = 1`, naming the mismatched field.
- CLI success output contains the required PASS tokens.
- Python replay kwargs contain explicit `ms_scaling_factor=0`.
- The actual `_python_row` failure loop counts one failed trial when Z fails
  and skips the X decoder for that trial.

## Verification

Required commands:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract \
  benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json
cargo test
```

Negative control:

```bash
cp benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json /tmp/bravyi_contract_bad.json
python3 - <<'PY'
import json
from pathlib import Path
path = Path('/tmp/bravyi_contract_bad.json')
data = json.loads(path.read_text())
data['decoder']['ms_scaling_factor'] = 1
path.write_text(json.dumps(data, indent=2) + '\n')
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract /tmp/bravyi_contract_bad.json
```

Expected: verifier exits nonzero and names `decoder.ms_scaling_factor`.

## Approval

The run is non-interactive. The standing answer policy chooses the recommended
compact contract plus active validator because it satisfies the issue's
machine-readable and source-backed requirements while keeping the change scoped
to evidence infrastructure.
