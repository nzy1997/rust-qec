# Issue #305 Bravyi BB Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a checked-in Bravyi BB circuit BP-OSD source contract and a validator that catches drift in repository BB compare defaults.

**Architecture:** Store compact JSON and Markdown contract artifacts under `benchmarks/bb_circuit_bposd_compare/reference/`. Add an import-based validator CLI that checks the JSON against `cases.py` and `run_compare.py`, and expose explicit Python `ldpc.BpOsdDecoder` kwargs so `ms_scaling_factor=0` is a real checked run-contract field.

**Tech Stack:** Python 3 standard library, pytest, existing `benchmarks.bb_circuit_bposd_compare` package, Cargo workspace verification.

## Global Constraints

- Upstream repository must be `sbravyi/BivariateBicycleCodes`.
- Upstream commit must be `fa77e3333d3ec44c79d8f914dd24c040d1da471b`.
- Result row columns must be physical error rate, syndrome cycles, Monte Carlo trials, and failed trials.
- `failure_unit` must be `monte_carlo_trial`.
- Decoder settings must be `bp_method = "ms"`, `max_iter = 10000`, `osd_method = "osd_cs"`, `osd_order = 7`, and `ms_scaling_factor = 0`.
- The run convention must append exactly two noiseless syndrome tail cycles after the configured noisy cycles.
- Failure semantics must decode Z first, decode X only if Z succeeds, and count one failed Monte Carlo trial when Z fails or when X fails after Z succeeds.
- Do not vendor upstream source; store only compact derived contract data and pinned provenance URLs.
- The validator must exit 0 with a PASS line naming the commit hash, `osd_cs`, OSD order 7, `ms_scaling_factor=0`, two noiseless tail cycles, and `failure_unit=monte_carlo_trial`.
- The validator must exit nonzero and name the mismatched field when `failure_unit`, `osd_order`, or `ms_scaling_factor` is mutated.

---

## File Structure

- Create: `benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json`
  - Machine-readable source contract and provenance links.
- Create: `benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.md`
  - Reviewer-readable source note.
- Create: `benchmarks/bb_circuit_bposd_compare/verify_bravyi_contract.py`
  - Validator module and CLI.
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py`
  - Positive and negative pytest coverage.
- Modify: `benchmarks/bb_circuit_bposd_compare/run_compare.py`
  - Add explicit `PYTHON_UPSTREAM_MS_SCALING_FACTOR = 0`, decoder kwargs helper, and failure-semantics constants.
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`
  - Extend the existing fake decoder test to assert `ms_scaling_factor=0`.

## Task 1: Contract Tests First

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`

**Interfaces:**
- Consumes: future `validate_contract(contract: dict[str, object]) -> list[str]`, `_load_contract(path: Path) -> dict[str, object]`, and `_python_bposd_decoder_kwargs() -> dict[str, object]`.
- Produces: failing tests that define the validator, explicit scaling, and trial-level failure behavior.

- [ ] **Step 1: Write the failing validator tests**

Create `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py` with tests that:

```python
from __future__ import annotations

import csv
import json
import subprocess
import sys
from pathlib import Path
from types import ModuleType
from unittest import mock

from benchmarks.bb_circuit_bposd_compare import run_compare
from benchmarks.bb_circuit_bposd_compare.cases import SMOKE_CASES
from benchmarks.bb_circuit_bposd_compare.run_compare import _python_row
from benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract import (
    _load_contract,
    validate_contract,
)


CONTRACT_PATH = (
    Path(__file__).resolve().parents[1] / "reference" / "bravyi_contract.json"
)


def test_checked_in_bravyi_contract_matches_repository_defaults() -> None:
    assert validate_contract(_load_contract(CONTRACT_PATH)) == []


def test_contract_negative_controls_name_mismatched_fields() -> None:
    contract = _load_contract(CONTRACT_PATH)

    mutated = json.loads(json.dumps(contract))
    mutated["result_row"]["failure_unit"] = "per_cycle"
    assert any("result_row.failure_unit" in err for err in validate_contract(mutated))

    mutated = json.loads(json.dumps(contract))
    mutated["decoder"]["osd_order"] = 0
    assert any("decoder.osd_order" in err for err in validate_contract(mutated))

    mutated = json.loads(json.dumps(contract))
    mutated["decoder"]["ms_scaling_factor"] = 1
    assert any("decoder.ms_scaling_factor" in err for err in validate_contract(mutated))


def test_verify_bravyi_contract_cli_prints_required_pass_line() -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract",
            str(CONTRACT_PATH),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    stdout = result.stdout
    assert "PASS" in stdout
    assert "fa77e3333d3ec44c79d8f914dd24c040d1da471b" in stdout
    assert "osd_cs" in stdout
    assert "OSD order 7" in stdout
    assert "ms_scaling_factor=0" in stdout
    assert "two noiseless tail cycles" in stdout
    assert "failure_unit=monte_carlo_trial" in stdout


def test_python_decoder_kwargs_expose_upstream_scaling() -> None:
    kwargs = run_compare._python_bposd_decoder_kwargs()
    assert kwargs["bp_method"] == "ms"
    assert kwargs["max_iter"] == 10000
    assert kwargs["osd_method"] == "osd_cs"
    assert kwargs["osd_order"] == 7
    assert kwargs["ms_scaling_factor"] == 0
    assert kwargs["input_vector_type"] == "syndrome"
```

Add the fake `numpy` and fake `ldpc.BpOsdDecoder` classes from
`test_run_compare.py` or local equivalents, then add a test named
`test_python_row_counts_trial_failure_once_and_skips_x_when_z_fails`. The fake
Z decoder should return a correction whose predicted logical does not match
`trial["z_logical"]`; the fake X decoder must raise if its `decode()` is called.
Assert `_python_row(...)` returns `logical_error_rate == "1.0"` for one trial
and that only one Z decode call happened.

- [ ] **Step 2: Extend the existing pinned settings test**

In `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`, inside
`test_python_row_uses_pinned_upstream_settings`, add:

```python
self.assertEqual(decoder.kwargs["ms_scaling_factor"], 0)
```

- [ ] **Step 3: Run the new tests and confirm they fail for missing interfaces**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py::RunCompareTest::test_python_row_uses_pinned_upstream_settings
```

Expected: FAIL because `verify_bravyi_contract.py`, `bravyi_contract.json`, and `_python_bposd_decoder_kwargs()` do not exist yet.

- [ ] **Step 4: Commit only if the task is intentionally isolated**

Do not commit a permanently failing test-only task unless the controller asks
for red-test commits. Leave tests staged/unstaged for Task 2 to make green.

## Task 2: Explicit Python Replay Scaling Contract

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`
- Test: `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py`

**Interfaces:**
- Produces: `PYTHON_UPSTREAM_MS_SCALING_FACTOR: int`, `PYTHON_FAILURE_UNIT: str`, `PYTHON_FAILURE_PREDICATE: str`, and `_python_bposd_decoder_kwargs() -> dict[str, object]`.
- Produces: all Python replay `BpOsdDecoder` constructors passing `ms_scaling_factor=0` explicitly.

- [ ] **Step 1: Add run-contract constants and kwargs helper**

In `run_compare.py`, near the existing `PYTHON_UPSTREAM_*` constants, add:

```python
PYTHON_UPSTREAM_MS_SCALING_FACTOR = 0
PYTHON_FAILURE_UNIT = "monte_carlo_trial"
PYTHON_FAILURE_PREDICATE = "z_first_x_only_if_z_succeeds"
```

Replace `_python_upstream_settings()` with an unchanged CSV settings helper,
then add:

```python
def _python_bposd_decoder_kwargs() -> dict[str, object]:
    return {
        "max_iter": PYTHON_UPSTREAM_MAX_ITER,
        "bp_method": PYTHON_UPSTREAM_BP_METHOD,
        "ms_scaling_factor": PYTHON_UPSTREAM_MS_SCALING_FACTOR,
        "osd_method": PYTHON_UPSTREAM_OSD_METHOD,
        "osd_order": PYTHON_UPSTREAM_OSD_ORDER,
        "input_vector_type": "syndrome",
    }
```

- [ ] **Step 2: Pass the helper into every Python replay decoder**

In `_python_hard_replay_row()` and `_python_row()`, replace repeated keyword
arguments to `BpOsdDecoder(...)` with:

```python
**_python_bposd_decoder_kwargs(),
```

Keep matrix and `error_channel=...` arguments exactly as they are.

- [ ] **Step 3: Run focused settings tests**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py::RunCompareTest::test_python_row_uses_pinned_upstream_settings benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py::test_python_decoder_kwargs_expose_upstream_scaling benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py::test_python_row_counts_trial_failure_once_and_skips_x_when_z_fails
```

Expected: PASS after Task 2 and Task 3 validator imports exist; before Task 3,
the contract test module may still fail at import time.

- [ ] **Step 4: Commit Task 2**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/run_compare.py benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py
git commit -m "fix: pin bravyi ldpc scaling replay"
```

## Task 3: Contract Artifacts And Validator

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json`
- Create: `benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.md`
- Create: `benchmarks/bb_circuit_bposd_compare/verify_bravyi_contract.py`
- Test: `benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py`

**Interfaces:**
- Produces: `_load_contract(path: Path) -> dict[str, object]`, `validate_contract(contract: dict[str, object]) -> list[str]`, and CLI `main(argv: list[str] | None = None) -> int`.

- [ ] **Step 1: Add the JSON contract**

Create `bravyi_contract.json` with this structure and exact binding values:

```json
{
  "contract_version": 1,
  "upstream": {
    "repository": "sbravyi/BivariateBicycleCodes",
    "commit": "fa77e3333d3ec44c79d8f914dd24c040d1da471b",
    "tree_url": "https://github.com/sbravyi/BivariateBicycleCodes/tree/fa77e3333d3ec44c79d8f914dd24c040d1da471b"
  },
  "result_row": {
    "columns": [
      "physical_error_rate",
      "num_syndrome_cycles",
      "num_monte_carlo_trials",
      "num_failed_trials"
    ],
    "failure_unit": "monte_carlo_trial"
  },
  "decoder": {
    "bp_method": "ms",
    "max_iter": 10000,
    "osd_method": "osd_cs",
    "osd_order": 7,
    "ms_scaling_factor": 0
  },
  "cycle_convention": {
    "configured_noisy_cycles_field": "num_cycles",
    "noiseless_tail_cycles": 2
  },
  "failure_predicate": {
    "decode_order": ["Z", "X"],
    "x_decode_condition": "only_if_z_succeeds",
    "failed_trial_condition": "z_fails_or_x_fails_after_z_succeeds"
  },
  "sources": [
    {
      "file": "README.md",
      "lines": "16-21",
      "url": "https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/README.md#L16-L21",
      "supports": ["result_row", "failure_unit"]
    },
    {
      "file": "decoder_setup.py",
      "lines": "511-618",
      "url": "https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_setup.py#L511-L618",
      "supports": ["noiseless_tail_cycles", "effective_decoder_histories"]
    },
    {
      "file": "decoder_run.py",
      "lines": "67-72,329-349",
      "url": "https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_run.py#L67-L72",
      "supports": ["decoder"]
    },
    {
      "file": "decoder_run.py",
      "lines": "364-415",
      "url": "https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_run.py#L364-L415",
      "supports": ["failure_predicate"]
    }
  ]
}
```

- [ ] **Step 2: Add the Markdown note**

Create `bravyi_contract.md` with sections:

- Upstream Pin
- Result Row And Failure Unit
- BP/OSD Parameters
- Cycle Convention
- Failure Predicate
- Source References

Each section must cite the same pinned URLs and line ranges from the JSON.

- [ ] **Step 3: Add the validator implementation**

Create `verify_bravyi_contract.py` that:

- loads JSON using `json.loads(path.read_text())`;
- appends errors through helper checks that name paths such as
  `decoder.osd_order`;
- compares contract values to exact constants from `cases.py` and
  `run_compare.py`;
- checks every current compare case in `SMALL_LDPC_CASES`,
  `BB72_BB144_PLOT_SMOKE_CASES`, `BB72_BB144_FULL_CASES`,
  `DIAGNOSTIC_CASES`, `SMOKE_CASES`, and `HARD_REPLAY_CASES`;
- checks `small_ldpc_manifest_rows()` emits `scaling == "0"`;
- checks `_python_bposd_decoder_kwargs()["ms_scaling_factor"] == 0`;
- checks `PYTHON_FAILURE_UNIT == "monte_carlo_trial"` and
  `PYTHON_FAILURE_PREDICATE == "z_first_x_only_if_z_succeeds"`;
- prints the required PASS line on success and errors to stderr on failure.

- [ ] **Step 4: Run validator tests**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py
```

Expected: PASS.

- [ ] **Step 5: Run the validator CLI**

Run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json
```

Expected: PASS line with commit hash, `osd_cs`, OSD order 7,
`ms_scaling_factor=0`, two noiseless tail cycles, and
`failure_unit=monte_carlo_trial`.

- [ ] **Step 6: Commit Task 3**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.md benchmarks/bb_circuit_bposd_compare/verify_bravyi_contract.py benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py
git commit -m "feat: verify bravyi bb contract"
```

## Task 4: Final Verification And Negative Control

**Files:**
- No planned source edits unless verification reveals a bug.

**Interfaces:**
- Consumes: completed Tasks 1-3.
- Produces: verification evidence for issue #305 and PR body.

- [ ] **Step 1: Run required pytest**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_bravyi_contract.py
```

Expected: PASS.

- [ ] **Step 2: Run required validator CLI**

Run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json
```

Expected: PASS line naming all required contract values.

- [ ] **Step 3: Run negative control**

Run:

```bash
cp benchmarks/bb_circuit_bposd_compare/reference/bravyi_contract.json /tmp/bravyi_contract_bad.json
python3 - <<'PY'
import json
from pathlib import Path
path = Path("/tmp/bravyi_contract_bad.json")
data = json.loads(path.read_text())
data["decoder"]["ms_scaling_factor"] = 1
path.write_text(json.dumps(data, indent=2) + "\n")
PY
python3 -m benchmarks.bb_circuit_bposd_compare.verify_bravyi_contract /tmp/bravyi_contract_bad.json
```

Expected: nonzero exit and an error mentioning `decoder.ms_scaling_factor`.

- [ ] **Step 4: Run cargo test**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 5: Commit any verification fixes**

If verification required edits, commit them with a scoped imperative message.
If no edits were needed, do not create an empty commit.
