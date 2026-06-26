# BB Full-Campaign Readiness Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a non-running readiness gate that refuses BB small-LDPC full-campaign readiness unless all required prerequisite artifacts are present and passing.

**Architecture:** Implement a focused Python CLI in `benchmarks/bb_circuit_bposd_compare/ready_for_full.py`. It reads a results directory, validates fixed artifact paths, delegates replay and diagnostic CSV semantics to existing verifier modules, validates profile JSON counters directly, validates catalog manifests against `SMALL_LDPC_CASES`, and prints a PASS/WARN/FAIL summary.

**Tech Stack:** Python standard library (`argparse`, `csv`, `json`, `math`, `dataclasses`, `pathlib`), existing BB compare verifier modules, existing `pytest` benchmark tests, workspace `cargo test`.

## Global Constraints

- Do not launch the full campaign, run decoders, compute plots, or auto-download Python dependencies.
- Required artifacts are exactly `hard-replay/results.csv`, `hard-profile/profile.json`, `setup-run/profile.json`, `small-ldpc-catalog/manifest.csv`, and `diagnostic/results.csv` under `--results-dir`.
- Optional provenance is `provenance.json`; missing or incomplete provenance is WARN only.
- Exit status is `0` for PASS or WARN, and nonzero for FAIL.
- Any missing, malformed, stale, or failing required prerequisite is FAIL and must name the prerequisite and artifact path.
- Do not rely on wall-clock age thresholds; staleness comes from schema/content mismatches.
- Missing or skipped Python `ldpc_bposd` rows are readiness failures.
- Preserve existing verifier contracts from `verify_replay.py`, `verify_diagnostic.py`, and `cases.py`.

---

## File Structure

- Create `benchmarks/bb_circuit_bposd_compare/ready_for_full.py`: readiness check dataclasses, artifact readers, semantic replay check, hard profile check, setup/run check, catalog check, diagnostic check, provenance warning, CLI output and exit status.
- Create `benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py`: unit and CLI tests with temporary artifact trees, positive and negative controls from issue #286.
- Modify `benchmarks/bb_circuit_bposd_compare/README.md`: readiness gate command, artifact layout, and PASS/WARN/FAIL semantics.

---

### Task 1: Core Readiness Gate And Required Artifact Tests

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/ready_for_full.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py`

**Interfaces:**
- Produces: `CheckResult(name: str, status: str, artifact: str, messages: tuple[str, ...])`
- Produces: `check_results_dir(results_dir: Path) -> list[CheckResult]`
- Produces: `readiness_verdict(results: Sequence[CheckResult]) -> str`
- Produces: `main(argv: list[str] | None = None) -> int`

- [ ] **Step 1: Write failing tests for a complete tree, missing hard replay, and missing setup/run artifact**

Create `benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py` with this content:

```python
import csv
import json
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import ready_for_full
from benchmarks.bb_circuit_bposd_compare.cases import (
    CATALOG_HEADER,
    CSV_HEADER,
    DIAGNOSTIC_CASES,
    small_ldpc_manifest_rows,
)


HARD_CASE_ID = "bb90-p006-c10-seed12345-order7-hard-syndrome"
HARD_PREDICTION = "[false,true,false,true,false,false,false,true]"
HARD_SUPPORT = "[5,8,14]"


def _write_csv(path: Path, fieldnames: list[str], rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        for row in rows:
            writer.writerow({field: row.get(field, "") for field in fieldnames})


def _hard_replay_row(decoder_impl: str, **overrides: str) -> dict[str, str]:
    row = {
        "case_id": HARD_CASE_ID,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": "bb90",
        "p": "0.006",
        "num_cycles": "10",
        "num_trials": "1",
        "seed": "12345",
        "bp_method": "ms",
        "max_iter": "10000",
        "osd_method": "osd_cs",
        "osd_order": "7",
        "basis": "Z",
        "syndrome_weight": "3",
        "syndrome_support": HARD_SUPPORT,
        "logical_prediction": HARD_PREDICTION,
        "expected_logical": HARD_PREDICTION,
        "setup_seconds": "0.1",
        "decode_seconds": "0.2",
        "run_seconds": "0.3",
        "logical_error_rate": "0.0",
        "bp_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "osd_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "decode_call_count": "1" if decoder_impl == "rbposd" else "",
        "bp_iteration_count": "10000" if decoder_impl == "rbposd" else "",
        "osd_use_count": "1" if decoder_impl == "rbposd" else "",
        "osd_candidate_count": "16" if decoder_impl == "rbposd" else "",
        "gf2_solve_count": "1" if decoder_impl == "rbposd" else "",
        "gf2_full_elimination_count": "1" if decoder_impl == "rbposd" else "",
        "status": "ok",
        "error": "",
    }
    row.update(overrides)
    return row


def _diagnostic_row(case, decoder_impl: str, **overrides: str) -> dict[str, str]:
    row = {
        "case_id": case.case_id,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": case.code_id,
        "p": str(case.p),
        "num_cycles": str(case.num_cycles),
        "num_trials": str(case.num_trials),
        "seed": str(case.seed),
        "bp_method": case.bp_method,
        "max_iter": str(case.max_iter),
        "osd_method": case.osd_method,
        "osd_order": str(case.osd_order),
        "basis": "",
        "syndrome_weight": "",
        "syndrome_support": "",
        "logical_prediction": "",
        "expected_logical": "",
        "setup_seconds": "0.1",
        "decode_seconds": "0.2",
        "run_seconds": "0.3",
        "logical_error_rate": "0.0",
        "bp_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "osd_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "decode_call_count": "2" if decoder_impl == "rbposd" else "",
        "bp_iteration_count": "20000" if decoder_impl == "rbposd" else "",
        "osd_use_count": "1" if decoder_impl == "rbposd" else "",
        "osd_candidate_count": "16" if decoder_impl == "rbposd" else "",
        "gf2_solve_count": "1" if decoder_impl == "rbposd" else "",
        "gf2_full_elimination_count": "1" if decoder_impl == "rbposd" else "",
        "status": "ok",
        "error": "",
    }
    row.update(overrides)
    return row


def _write_json(path: Path, data: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, sort_keys=True))


def _hard_profile() -> dict[str, object]:
    return {
        "case_id": HARD_CASE_ID,
        "basis": "Z",
        "osd_planner": "ldpc_osd_cs",
        "osd_order": 7,
        "candidate_limit": 16,
        "planned_candidate_count": 4100,
        "ldpc_cs_candidate_bound": 4100,
        "decode_seconds": 0.3,
        "bp_seconds": 0.2,
        "osd_seconds": 0.1,
        "decode_call_count": 1,
        "z_decode_call_count": 1,
        "x_decode_call_count": 0,
        "bp_iteration_count": 10000,
        "osd_use_count": 1,
        "osd_candidate_count": 16,
        "gf2_solve_count": 1,
        "gf2_full_elimination_count": 1,
    }


def _setup_profile() -> dict[str, object]:
    return {
        "code_id": "bb72",
        "num_trials": 8,
        "setup_seconds": 0.1,
        "sample_seconds": 0.2,
        "decode_seconds": 0.3,
        "code_build_count": 1,
        "syndrome_cycle_build_count": 1,
        "effective_model_build_count": 1,
        "decoder_build_count": 1,
        "sample_count": 8,
        "decode_call_count": 16,
        "z_decode_call_count": 8,
        "x_decode_call_count": 8,
    }


def write_ready_tree(results_dir: Path, *, provenance: bool = True) -> None:
    _write_csv(
        results_dir / "hard-replay" / "results.csv",
        CSV_HEADER,
        [_hard_replay_row("rbposd"), _hard_replay_row("ldpc_bposd")],
    )
    _write_json(results_dir / "hard-profile" / "profile.json", _hard_profile())
    _write_json(results_dir / "setup-run" / "profile.json", _setup_profile())
    _write_csv(
        results_dir / "small-ldpc-catalog" / "manifest.csv",
        CATALOG_HEADER,
        small_ldpc_manifest_rows(),
    )
    diagnostic_rows = []
    for case in DIAGNOSTIC_CASES:
        diagnostic_rows.append(_diagnostic_row(case, "rbposd"))
        diagnostic_rows.append(_diagnostic_row(case, "ldpc_bposd"))
    _write_csv(results_dir / "diagnostic" / "results.csv", CSV_HEADER, diagnostic_rows)
    if provenance:
        _write_json(
            results_dir / "provenance.json",
            {
                "artifact_hash": "sha256:example",
                "command": "agent desk test fixture",
                "timestamp": "2026-06-27T00:00:00+08:00",
            },
        )


def test_ready_for_full_passes_complete_artifact_tree(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 0
    assert "PASS semantic-replay" in output
    assert "PASS hard-profile" in output
    assert "PASS setup-run-separation" in output
    assert "PASS catalog-coverage" in output
    assert "PASS diagnostic-compare" in output
    assert "PASS readiness verdict" in output


def test_ready_for_full_fails_missing_hard_replay(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    (tmp_path / "hard-replay" / "results.csv").unlink()

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL semantic-replay" in output
    assert "hard-replay/results.csv" in output
    assert "FAIL readiness verdict" in output


def test_ready_for_full_fails_without_setup_run_artifact(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    (tmp_path / "setup-run" / "profile.json").unlink()

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL setup-run-separation" in output
    assert "setup-run/profile.json" in output
```

- [ ] **Step 2: Run the focused tests and confirm RED**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py -q
```

Expected: FAIL because `benchmarks.bb_circuit_bposd_compare.ready_for_full` does not exist.

- [ ] **Step 3: Implement the minimal readiness module**

Create `benchmarks/bb_circuit_bposd_compare/ready_for_full.py` with:

```python
from __future__ import annotations

import argparse
import csv
import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

from benchmarks.bb_circuit_bposd_compare.cases import (
    CATALOG_HEADER,
    CSV_HEADER,
    CompareCase,
    validate_small_ldpc_catalog,
)
from benchmarks.bb_circuit_bposd_compare.verify_diagnostic import (
    verify_rows as verify_diagnostic_rows,
)
from benchmarks.bb_circuit_bposd_compare.verify_replay import (
    verify_rows as verify_replay_rows,
)

PASS = "PASS"
WARN = "WARN"
FAIL = "FAIL"

SEMANTIC_REPLAY_PATH = Path("hard-replay/results.csv")
HARD_PROFILE_PATH = Path("hard-profile/profile.json")
SETUP_RUN_PATH = Path("setup-run/profile.json")
CATALOG_PATH = Path("small-ldpc-catalog/manifest.csv")
DIAGNOSTIC_PATH = Path("diagnostic/results.csv")
PROVENANCE_PATH = Path("provenance.json")


@dataclass(frozen=True)
class CheckResult:
    name: str
    status: str
    artifact: str
    messages: tuple[str, ...] = ()


def _pass(name: str, artifact: Path, message: str = "ok") -> CheckResult:
    return CheckResult(name, PASS, artifact.as_posix(), (message,))


def _warn(name: str, artifact: Path, message: str) -> CheckResult:
    return CheckResult(name, WARN, artifact.as_posix(), (message,))


def _fail(name: str, artifact: Path, message: str) -> CheckResult:
    return CheckResult(name, FAIL, artifact.as_posix(), (message,))


def _load_csv(results_dir: Path, relative_path: Path, header: Sequence[str]) -> tuple[list[dict[str, str]] | None, list[str]]:
    path = results_dir / relative_path
    if not path.exists():
        return None, [f"missing artifact: {relative_path.as_posix()}"]
    try:
        with path.open(newline="") as handle:
            reader = csv.DictReader(handle)
            fieldnames = list(reader.fieldnames or [])
            rows = list(reader)
    except csv.Error as error:
        return None, [f"malformed CSV {relative_path.as_posix()}: {error}"]
    except OSError as error:
        return None, [f"cannot read {relative_path.as_posix()}: {error}"]
    missing = [column for column in header if column not in fieldnames]
    if missing:
        return rows, [
            f"{relative_path.as_posix()} is missing required CSV column(s): "
            + ", ".join(missing)
        ]
    return rows, []


def _load_json_object(results_dir: Path, relative_path: Path) -> tuple[dict[str, object] | None, list[str]]:
    path = results_dir / relative_path
    if not path.exists():
        return None, [f"missing artifact: {relative_path.as_posix()}"]
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError as error:
        return None, [f"malformed JSON {relative_path.as_posix()}: {error}"]
    except OSError as error:
        return None, [f"cannot read {relative_path.as_posix()}: {error}"]
    if not isinstance(data, dict):
        return None, [f"{relative_path.as_posix()} must contain a JSON object"]
    return data, []


def _as_int(data: dict[str, object], field: str, errors: list[str]) -> int | None:
    value = data.get(field)
    if isinstance(value, bool) or value is None:
        errors.append(f"{field} must be an integer")
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, float) and math.isfinite(value) and value.is_integer():
        return int(value)
    if isinstance(value, str):
        try:
            parsed = float(value)
        except ValueError:
            errors.append(f"{field} must be an integer")
            return None
        if math.isfinite(parsed) and parsed.is_integer():
            return int(parsed)
    errors.append(f"{field} must be an integer")
    return None


def _as_seconds(data: dict[str, object], field: str, errors: list[str]) -> float | None:
    value = data.get(field)
    if isinstance(value, bool) or value is None:
        errors.append(f"{field} must be finite and non-negative")
        return None
    try:
        seconds = float(value)
    except (TypeError, ValueError):
        errors.append(f"{field} must be finite and non-negative")
        return None
    if not math.isfinite(seconds) or seconds < 0.0:
        errors.append(f"{field} must be finite and non-negative")
        return None
    return seconds


def _check_semantic_replay(results_dir: Path) -> CheckResult:
    rows, errors = _load_csv(results_dir, SEMANTIC_REPLAY_PATH, CSV_HEADER)
    if rows is not None:
        errors.extend(verify_replay_rows(rows, allow_missing_python=False))
    if errors:
        return _fail("semantic-replay", SEMANTIC_REPLAY_PATH, "; ".join(errors))
    return _pass("semantic-replay", SEMANTIC_REPLAY_PATH, "hard replay rows verified")


def _check_hard_profile(results_dir: Path) -> CheckResult:
    data, errors = _load_json_object(results_dir, HARD_PROFILE_PATH)
    if data is None:
        return _fail("hard-profile", HARD_PROFILE_PATH, "; ".join(errors))
    if data.get("osd_planner") != "ldpc_osd_cs":
        errors.append(
            "osd_planner must be ldpc_osd_cs, got "
            + str(data.get("osd_planner", ""))
        )
    candidate_limit = _as_int(data, "candidate_limit", errors)
    planned = _as_int(data, "planned_candidate_count", errors)
    bound = data.get("ldpc_cs_candidate_bound")
    if bound is not None:
        parsed_bound = _as_int(data, "ldpc_cs_candidate_bound", errors)
        if parsed_bound is not None and planned is not None and parsed_bound != planned:
            errors.append("ldpc_cs_candidate_bound must match planned_candidate_count")
    osd_candidates = _as_int(data, "osd_candidate_count", errors)
    gf2_solves = _as_int(data, "gf2_solve_count", errors)
    gf2_eliminations = _as_int(data, "gf2_full_elimination_count", errors)
    decode_calls = _as_int(data, "decode_call_count", errors)
    z_calls = _as_int(data, "z_decode_call_count", errors)
    x_calls = _as_int(data, "x_decode_call_count", errors)
    for field in ("decode_seconds", "bp_seconds", "osd_seconds"):
        _as_seconds(data, field, errors)
    if candidate_limit != 16:
        errors.append(f"candidate_limit must be 16, got {candidate_limit}")
    if planned is not None and planned <= 0:
        errors.append("planned_candidate_count must be positive")
    if osd_candidates is not None and osd_candidates <= 0:
        errors.append("osd_candidate_count must be positive")
    if (
        osd_candidates is not None
        and candidate_limit is not None
        and planned is not None
        and osd_candidates > min(candidate_limit, planned)
    ):
        errors.append(
            "osd_candidate_count exceeds candidate_limit/planned_candidate_count"
        )
    if gf2_solves != 1:
        errors.append(f"gf2_solve_count must be 1, got {gf2_solves}")
    if gf2_eliminations != 1:
        errors.append(
            f"gf2_full_elimination_count must be 1, got {gf2_eliminations}"
        )
    if (
        decode_calls is not None
        and z_calls is not None
        and x_calls is not None
        and decode_calls != z_calls + x_calls
    ):
        errors.append("decode_call_count must equal z_decode_call_count + x_decode_call_count")
    if errors:
        return _fail("hard-profile", HARD_PROFILE_PATH, "; ".join(errors))
    return _pass("hard-profile", HARD_PROFILE_PATH, "counter-bounded profile verified")


def _check_setup_run(results_dir: Path) -> CheckResult:
    data, errors = _load_json_object(results_dir, SETUP_RUN_PATH)
    if data is None:
        return _fail("setup-run-separation", SETUP_RUN_PATH, "; ".join(errors))
    for field in (
        "code_build_count",
        "syndrome_cycle_build_count",
        "effective_model_build_count",
        "decoder_build_count",
    ):
        if _as_int(data, field, errors) != 1:
            errors.append(f"{field} must be 1")
    num_trials = _as_int(data, "num_trials", errors)
    sample_count = _as_int(data, "sample_count", errors)
    decode_calls = _as_int(data, "decode_call_count", errors)
    z_calls = _as_int(data, "z_decode_call_count", errors)
    x_calls = _as_int(data, "x_decode_call_count", errors)
    for field in ("setup_seconds", "sample_seconds", "decode_seconds"):
        _as_seconds(data, field, errors)
    if num_trials is not None and sample_count is not None and sample_count != num_trials:
        errors.append("sample_count must equal num_trials")
    if (
        decode_calls is not None
        and z_calls is not None
        and x_calls is not None
        and decode_calls != z_calls + x_calls
    ):
        errors.append("decode_call_count must equal z_decode_call_count + x_decode_call_count")
    if errors:
        return _fail("setup-run-separation", SETUP_RUN_PATH, "; ".join(errors))
    return _pass("setup-run-separation", SETUP_RUN_PATH, "setup counters verified")


def _case_from_manifest_row(row: dict[str, str], row_number: int, errors: list[str]) -> CompareCase | None:
    try:
        return CompareCase(
            case_id=row["case_id"],
            code_id=row["code_id"],
            p=float(row["p"]),
            num_cycles=int(row["num_cycles"]),
            num_trials=int(row["num_trials"]),
            seed=int(row["seed"]),
            bp_method=row["bp_method"],
            max_iter=int(row["max_iter"]),
            osd_method=row["osd_method"],
            osd_order=int(row["osd_order"]),
            scaling=int(row["scaling"]),
            catalog_status=row["catalog_status"],
            catalog_note=row["catalog_note"],
        )
    except (KeyError, TypeError, ValueError) as error:
        errors.append(f"manifest row {row_number} is malformed: {error}")
        return None


def _check_catalog(results_dir: Path) -> CheckResult:
    rows, errors = _load_csv(results_dir, CATALOG_PATH, CATALOG_HEADER)
    cases: list[CompareCase] = []
    if rows is not None:
        for index, row in enumerate(rows, start=2):
            case = _case_from_manifest_row(row, index, errors)
            if case is not None:
                cases.append(case)
        if not errors:
            errors.extend(validate_small_ldpc_catalog(tuple(cases)))
    if errors:
        return _fail("catalog-coverage", CATALOG_PATH, "; ".join(errors))
    return _pass("catalog-coverage", CATALOG_PATH, "small-LDPC catalog verified")


def _check_diagnostic(results_dir: Path) -> CheckResult:
    rows, errors = _load_csv(results_dir, DIAGNOSTIC_PATH, CSV_HEADER)
    if rows is not None:
        errors.extend(verify_diagnostic_rows(rows, allow_missing_python=False))
    if errors:
        return _fail("diagnostic-compare", DIAGNOSTIC_PATH, "; ".join(errors))
    return _pass("diagnostic-compare", DIAGNOSTIC_PATH, "diagnostic rows verified")


def _check_provenance(results_dir: Path) -> CheckResult:
    path = results_dir / PROVENANCE_PATH
    if not path.exists():
        return _warn("provenance", PROVENANCE_PATH, "optional provenance.json is missing")
    data, errors = _load_json_object(results_dir, PROVENANCE_PATH)
    if data is None:
        return _warn("provenance", PROVENANCE_PATH, "; ".join(errors))
    recognized = [
        f"{field}={data[field]}"
        for field in ("artifact_hash", "command", "timestamp")
        if data.get(field)
    ]
    if not recognized:
        return _warn("provenance", PROVENANCE_PATH, "no recognized provenance fields")
    return _pass("provenance", PROVENANCE_PATH, ", ".join(recognized))


def check_results_dir(results_dir: Path) -> list[CheckResult]:
    return [
        _check_semantic_replay(results_dir),
        _check_hard_profile(results_dir),
        _check_setup_run(results_dir),
        _check_catalog(results_dir),
        _check_diagnostic(results_dir),
        _check_provenance(results_dir),
    ]


def readiness_verdict(results: Sequence[CheckResult]) -> str:
    if any(result.status == FAIL for result in results):
        return FAIL
    if any(result.status == WARN for result in results):
        return WARN
    return PASS


def _print_summary(results: Sequence[CheckResult], verdict: str) -> None:
    for result in results:
        detail = "; ".join(result.messages)
        print(f"{result.status} {result.name}: {result.artifact} - {detail}")
    if verdict == FAIL:
        print("FAIL readiness verdict: required prerequisites failed")
    elif verdict == WARN:
        print("WARN readiness verdict: all required prerequisites passed with warnings")
    else:
        print("PASS readiness verdict: all prerequisites passed")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    results = check_results_dir(args.results_dir)
    verdict = readiness_verdict(results)
    _print_summary(results, verdict)
    return 1 if verdict == FAIL else 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run focused tests and confirm GREEN**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py -q
```

Expected: PASS for the three tests.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/ready_for_full.py benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py
git commit -m "feat: add bb readiness gate core"
```

Expected: commit succeeds.

---

### Task 2: Stale Artifact Negative Controls And Provenance Warnings

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/ready_for_full.py`

**Interfaces:**
- Consumes: `ready_for_full.main(argv) -> int`
- Consumes: `write_ready_tree(results_dir: Path, provenance: bool = True) -> None` from the test module
- Produces: stale catalog, hard profile, skipped diagnostic, and provenance warning coverage

- [ ] **Step 1: Add failing tests for stale/malformed artifacts and WARN provenance**

Append these tests to `benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py`:

```python

def test_ready_for_full_fails_stale_catalog_manifest(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    manifest_path = tmp_path / "small-ldpc-catalog" / "manifest.csv"
    rows = small_ldpc_manifest_rows()
    rows[0] = {**rows[0], "p": "0.0099"}
    _write_csv(manifest_path, CATALOG_HEADER, rows)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL catalog-coverage" in output
    assert "small-ldpc-catalog/manifest.csv" in output
    assert "unexpected target" in output


def test_ready_for_full_fails_malformed_catalog_csv(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    manifest_path = tmp_path / "small-ldpc-catalog" / "manifest.csv"
    manifest_path.write_text("case_id,code_id,p\nonly,three,columns\n")

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL catalog-coverage" in output
    assert "missing required CSV column(s)" in output
    assert "small-ldpc-catalog/manifest.csv" in output


def test_ready_for_full_fails_hard_profile_counter_regression(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    profile = _hard_profile()
    profile["gf2_solve_count"] = 4101
    _write_json(tmp_path / "hard-profile" / "profile.json", profile)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL hard-profile" in output
    assert "gf2_solve_count" in output
    assert "hard-profile/profile.json" in output


def test_ready_for_full_fails_skipped_diagnostic_python_row(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path)
    rows = []
    for case in DIAGNOSTIC_CASES:
        rows.append(_diagnostic_row(case, "rbposd"))
        rows.append(
            _diagnostic_row(
                case,
                "ldpc_bposd",
                status="skipped",
                setup_seconds="",
                decode_seconds="",
                run_seconds="",
                logical_error_rate="",
                error="python dependency unavailable for ldpc_bposd replay: No module named 'ldpc'",
            )
        )
    _write_csv(tmp_path / "diagnostic" / "results.csv", CSV_HEADER, rows)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 1
    assert "FAIL diagnostic-compare" in output
    assert "Python ldpc_bposd diagnostic row is skipped" in output


def test_ready_for_full_warns_without_optional_provenance(tmp_path, capsys) -> None:
    write_ready_tree(tmp_path, provenance=False)

    status = ready_for_full.main(["--results-dir", str(tmp_path)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 0
    assert "WARN provenance" in output
    assert "provenance.json" in output
    assert "WARN readiness verdict" in output
```

- [ ] **Step 2: Run the focused tests and confirm RED**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py -q
```

Expected: at least one newly added test fails if Task 1 implementation did not already cover every stale-artifact case. If all tests pass because Task 1 implemented the full behavior, record that the negative controls were already green against the completed implementation.

- [ ] **Step 3: Tighten implementation only where tests expose gaps**

If any Task 2 test fails, update `ready_for_full.py` in the smallest way that makes the test pass:

- ensure header errors from `_load_csv()` are returned before calling downstream validators,
- ensure `_check_catalog()` reports `validate_small_ldpc_catalog()` messages,
- ensure `_check_hard_profile()` requires exact GF(2) counts,
- ensure `_check_diagnostic()` calls `verify_diagnostic_rows(..., allow_missing_python=False)`,
- ensure missing provenance is WARN and does not change the exit status.

The expected implementation after this step is the same interface produced by Task 1; do not add new CLI options or artifact names.

- [ ] **Step 4: Run focused tests and confirm GREEN**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py -q
```

Expected: PASS for all readiness tests.

- [ ] **Step 5: Run package-level Python tests**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests -q
```

Expected: PASS for the BB compare Python test package.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/ready_for_full.py benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py
git commit -m "test: cover bb readiness failure modes"
```

Expected: commit succeeds. If Task 2 introduced only tests and Task 1 implementation already passed them, commit just the test additions.

---

### Task 3: README Instructions And Verification Fixture Command

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/README.md`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py`

**Interfaces:**
- Consumes: readiness artifact layout from `ready_for_full.py`
- Produces: README command and a test-created `/tmp/rstim-bb-ready` fixture for the issue verification command

- [ ] **Step 1: Add a test that can build the `/tmp/rstim-bb-ready` fixture used by the issue command**

Append this test to `benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py`:

```python

def test_ready_for_full_verification_command_accepts_named_tmp_fixture(tmp_path, monkeypatch, capsys) -> None:
    fixture_dir = tmp_path / "rstim-bb-ready"
    write_ready_tree(fixture_dir)

    status = ready_for_full.main(["--results-dir", str(fixture_dir)])

    captured = capsys.readouterr()
    output = captured.out + captured.err
    assert status == 0
    assert "PASS readiness verdict" in output
```

- [ ] **Step 2: Run the focused test and confirm GREEN**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py::test_ready_for_full_verification_command_accepts_named_tmp_fixture -q
```

Expected: PASS.

- [ ] **Step 3: Update README with readiness gate instructions**

In `benchmarks/bb_circuit_bposd_compare/README.md`, add this section after the `Diagnostic Tier` section and before `BB90 Hard-Syndrome Replay`:

````markdown
## Full-Campaign Readiness Gate

Before launching the full BB small-LDPC campaign, collect the prerequisite
artifacts under one results directory and run:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.ready_for_full --results-dir /tmp/rstim-bb-ready
```

The gate validates these required artifacts:

- `hard-replay/results.csv`: paired BB90 hard-syndrome replay rows accepted by
  `verify_replay`.
- `hard-profile/profile.json`: release hard-profile JSON with
  `osd_planner=ldpc_osd_cs`, `candidate_limit=16`, bounded OSD candidates, one
  optimized GF(2) solve, one full elimination, and consistent basis decode
  counters.
- `setup-run/profile.json`: BB p-point profile evidence with one code,
  syndrome-cycle, effective-model, and decoder build; `sample_count` equal to
  `num_trials`; and consistent Z/X decode-call counters.
- `small-ldpc-catalog/manifest.csv`: the complete 31-row small-LDPC manifest
  accepted by `validate_small_ldpc_catalog`.
- `diagnostic/results.csv`: paired high-p BB90 and BB144 diagnostic rows
  accepted by `verify_diagnostic`.

Optional `provenance.json` may include `artifact_hash`, `command`, or
`timestamp`. Missing provenance produces `WARN`, but missing, stale, malformed,
skipped, or failing required artifacts produce `FAIL` and a nonzero exit. The
gate does not use wall-clock age thresholds and does not run the full campaign.
````

- [ ] **Step 4: Run focused tests and Python package tests**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py -q
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests -q
```

Expected: both commands PASS.

- [ ] **Step 5: Create the issue verification fixture and run the documented command**

Run:

```bash
python3 - <<'PY'
from pathlib import Path
from benchmarks.bb_circuit_bposd_compare.tests.test_ready_for_full import write_ready_tree
root = Path("/tmp/rstim-bb-ready")
if root.exists():
    import shutil
    shutil.rmtree(root)
write_ready_tree(root)
PY
python3 -m benchmarks.bb_circuit_bposd_compare.ready_for_full --results-dir /tmp/rstim-bb-ready
```

Expected: exit `0`, output names semantic replay, hard profile, setup/run separation, catalog coverage, diagnostic compare, provenance, and final `PASS readiness verdict`.

- [ ] **Step 6: Commit Task 3**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/README.md benchmarks/bb_circuit_bposd_compare/tests/test_ready_for_full.py
git commit -m "docs: document bb readiness gate"
```

Expected: commit succeeds.

---

## Final Verification

After all tasks complete, run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests -q
python3 -m benchmarks.bb_circuit_bposd_compare.ready_for_full --results-dir /tmp/rstim-bb-ready
cargo test
```

Expected:

- Python package tests pass.
- The readiness command exits `0` against the valid `/tmp/rstim-bb-ready` fixture and prints a PASS/WARN/FAIL summary naming each prerequisite.
- `cargo test` passes.
