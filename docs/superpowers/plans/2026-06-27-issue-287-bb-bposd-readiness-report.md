# Issue 287 BB BP-OSD Readiness Report Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Markdown readiness report writer and validator for BB BP-OSD compare artifacts.

**Architecture:** The writer builds a deterministic report model from a results directory, reusing #286 `ready_for_full` checks for the machine verdict and reading the five required artifact classes for reviewer tables. The report embeds a compact JSON snapshot comment that the validator compares against a freshly rebuilt model from `--results-dir`, including artifact hashes and final verdict.

**Tech Stack:** Python standard library (`argparse`, `csv`, `datetime`, `hashlib`, `json`, `re`, `pathlib`), existing `benchmarks.bb_circuit_bposd_compare` readiness modules, pytest tests, workspace `cargo test`.

## Global Constraints

- Do not launch the full benchmark campaign.
- Do not change benchmark data or decoder behavior.
- Reuse `ready_for_full.check_results_dir()` and `ready_for_full.readiness_verdict()` for the final verdict.
- Generate Markdown at `--out`.
- The validator must compare report contents against source artifacts, not only check that a file exists.
- A report that says `PASS` while #286 would return `FAIL` must be rejected.
- Failure messages must name the stale, missing, or placeholder section.
- Required CLI commands are:
  `python3 -m benchmarks.bb_circuit_bposd_compare.write_readiness_report --results-dir /tmp/rstim-bb-ready --out /tmp/bb-bposd-readiness.md`
  and
  `python3 -m benchmarks.bb_circuit_bposd_compare.validate_readiness_report --results-dir /tmp/rstim-bb-ready --report /tmp/bb-bposd-readiness.md`.

---

## File Structure

- Create `benchmarks/bb_circuit_bposd_compare/write_readiness_report.py`
  for model building, Markdown rendering, snapshot embedding, and writer CLI.
- Create `benchmarks/bb_circuit_bposd_compare/validate_readiness_report.py`
  for parsing report text, rebuilding the model, comparing snapshots, checking
  visible section tokens, and validator CLI.
- Create `benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py`
  for positive and negative controls using the #286 fixture tree helpers.
- Modify `benchmarks/bb_circuit_bposd_compare/README.md` with the report writer
  and validator commands.

---

### Task 1: Report Writer Positive Tests

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py`

**Interfaces:**
- Consumes: `write_ready_tree(results_dir: Path)` from `tests/test_ready_for_full.py`
- Consumes: future `write_readiness_report.main(argv: list[str] | None) -> int`
- Produces: tests that define the required visible Markdown sections and CLI behavior

- [ ] **Step 1: Add tests for generated report content and writer CLI**

Create `benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py`
with these initial tests:

```python
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import write_readiness_report
from benchmarks.bb_circuit_bposd_compare.tests.test_ready_for_full import (
    write_ready_tree,
)


def test_write_readiness_report_includes_required_reviewer_sections(tmp_path) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)

    status = write_readiness_report.main(
        ["--results-dir", str(results_dir), "--out", str(report_path)]
    )

    report = report_path.read_text()
    assert status == 0
    assert "# BB BP-OSD Full-Campaign Readiness Report" in report
    assert "**Final readiness verdict:** PASS" in report
    assert "## Gate Summary" in report
    assert "## Semantic Parity Replay" in report
    assert "bb90-p006-c10-seed12345-order7-hard-syndrome" in report
    assert "## BB90 Hard-Profile Counters" in report
    assert "planned_candidate_count" in report
    assert "4100" in report
    assert "## Setup/Run Split Evidence" in report
    assert "decoder_build_count" in report
    assert "## Diagnostic Rust/Python Compare Rows" in report
    assert "bb144-p0060-c12-t1-seed12345" in report
    assert "## Small-LDPC Case Coverage" in report
    assert "bb288" in report
    assert "unsupported_rust_constructor" in report
    assert "rstim-bb-readiness-snapshot" in report
```

- [ ] **Step 2: Run the focused test and confirm it fails before implementation**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py -q
```

Expected: collection or test failure because `write_readiness_report` is not implemented.

- [ ] **Step 3: Commit the failing test**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py
git commit -m "test: cover bb readiness report writer"
```

---

### Task 2: Markdown Writer And Report Model

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/write_readiness_report.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py`

**Interfaces:**
- Produces: `SNAPSHOT_PREFIX: str`
- Produces: `build_report_model(results_dir: Path) -> dict[str, object]`
- Produces: `snapshot_model(model: dict[str, object]) -> dict[str, object]`
- Produces: `render_markdown(model: dict[str, object]) -> str`
- Produces: `write_report(results_dir: Path, out_path: Path) -> dict[str, object]`
- Produces: `main(argv: list[str] | None = None) -> int`

- [ ] **Step 1: Implement artifact readers, hashes, and model builder**

Create `benchmarks/bb_circuit_bposd_compare/write_readiness_report.py` with:

```python
from __future__ import annotations

import argparse
import csv
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Sequence

from benchmarks.bb_circuit_bposd_compare import ready_for_full

SNAPSHOT_PREFIX = "rstim-bb-readiness-snapshot:"


def _artifact_hash(path: Path) -> str:
    if not path.exists() or not path.is_file():
        return "missing"
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _read_csv_rows(path: Path) -> list[dict[str, str]]:
    if not path.exists():
        return []
    with path.open(newline="") as handle:
        return [dict(row) for row in csv.DictReader(handle)]


def _read_json_object(path: Path) -> dict[str, object]:
    if not path.exists():
        return {}
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError:
        return {}
    return data if isinstance(data, dict) else {}
```

Then add `build_report_model()` that calls #286 and stores deterministic
sections:

```python
def build_report_model(results_dir: Path) -> dict[str, object]:
    checks = ready_for_full.check_results_dir(results_dir)
    verdict = ready_for_full.readiness_verdict(checks)
    gate_checks = [
        {
            "name": check.name,
            "status": check.status,
            "artifact": check.artifact,
            "messages": list(check.messages),
        }
        for check in checks
    ]

    hard_replay_path = results_dir / ready_for_full.SEMANTIC_REPLAY_PATH
    hard_profile_path = results_dir / ready_for_full.HARD_PROFILE_PATH
    setup_run_path = results_dir / ready_for_full.SETUP_RUN_PATH
    catalog_path = results_dir / ready_for_full.CATALOG_PATH
    diagnostic_path = results_dir / ready_for_full.DIAGNOSTIC_PATH

    semantic_rows = _select_rows(
        _read_csv_rows(hard_replay_path),
        (
            "case_id",
            "decoder_impl",
            "status",
            "basis",
            "syndrome_weight",
            "logical_prediction",
            "expected_logical",
            "setup_seconds",
            "decode_seconds",
            "run_seconds",
            "logical_error_rate",
        ),
    )
    hard_profile = _select_mapping(
        _read_json_object(hard_profile_path),
        (
            "case_id",
            "basis",
            "osd_planner",
            "osd_order",
            "candidate_limit",
            "planned_candidate_count",
            "ldpc_cs_candidate_bound",
            "osd_candidate_count",
            "bp_iteration_count",
            "osd_use_count",
            "decode_call_count",
            "z_decode_call_count",
            "x_decode_call_count",
            "gf2_solve_count",
            "gf2_full_elimination_count",
            "decode_seconds",
            "bp_seconds",
            "osd_seconds",
        ),
    )
    setup_run = _select_mapping(
        _read_json_object(setup_run_path),
        (
            "code_id",
            "num_trials",
            "sample_count",
            "code_build_count",
            "syndrome_cycle_build_count",
            "effective_model_build_count",
            "decoder_build_count",
            "decode_call_count",
            "z_decode_call_count",
            "x_decode_call_count",
            "setup_seconds",
            "sample_seconds",
            "decode_seconds",
        ),
    )
    diagnostic_rows = _select_rows(
        _read_csv_rows(diagnostic_path),
        (
            "case_id",
            "decoder_impl",
            "status",
            "code_id",
            "p",
            "num_cycles",
            "num_trials",
            "setup_seconds",
            "decode_seconds",
            "run_seconds",
            "logical_error_rate",
            "decode_call_count",
            "bp_iteration_count",
            "osd_use_count",
            "osd_candidate_count",
            "gf2_solve_count",
            "gf2_full_elimination_count",
        ),
    )
    catalog_summary = _catalog_summary(_read_csv_rows(catalog_path))

    return {
        "schema_version": 1,
        "results_dir": str(results_dir),
        "generated_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "verdict": verdict,
        "gate_checks": gate_checks,
        "artifact_hashes": {
            "semantic-replay": _artifact_hash(hard_replay_path),
            "hard-profile": _artifact_hash(hard_profile_path),
            "setup-run-separation": _artifact_hash(setup_run_path),
            "catalog-coverage": _artifact_hash(catalog_path),
            "diagnostic-compare": _artifact_hash(diagnostic_path),
        },
        "sections": {
            "semantic-replay": semantic_rows,
            "hard-profile": hard_profile,
            "setup-run-separation": setup_run,
            "diagnostic-compare": diagnostic_rows,
            "catalog-coverage": catalog_summary,
        },
    }
```

Add helper functions `_select_mapping`, `_select_rows`, and `_catalog_summary`
that convert all values to strings and group catalog rows by `code_id`.

- [ ] **Step 2: Implement Markdown rendering and writer CLI**

Render deterministic Markdown tables and append the snapshot comment:

```python
def render_markdown(model: dict[str, object]) -> str:
    lines = [
        "# BB BP-OSD Full-Campaign Readiness Report",
        "",
        f"**Source results directory:** {model['results_dir']}",
        f"**Generated at:** {model['generated_at']}",
        f"**Final readiness verdict:** {model['verdict']}",
        "",
        "## Gate Summary",
        "",
    ]
    lines.extend(_markdown_table(["check", "status", "artifact", "messages"], _gate_rows(model)))
    lines.extend(["", "## Semantic Parity Replay", ""])
    lines.extend(_markdown_table(_semantic_columns(), model["sections"]["semantic-replay"]))
    lines.extend(["", "## BB90 Hard-Profile Counters", ""])
    lines.extend(_key_value_table(model["sections"]["hard-profile"]))
    lines.extend(["", "## Setup/Run Split Evidence", ""])
    lines.extend(_key_value_table(model["sections"]["setup-run-separation"]))
    lines.extend(["", "## Diagnostic Rust/Python Compare Rows", ""])
    lines.extend(_markdown_table(_diagnostic_columns(), model["sections"]["diagnostic-compare"]))
    lines.extend(["", "## Small-LDPC Case Coverage", ""])
    lines.extend(_markdown_table(["code_id", "cycles", "case_count", "p_values", "catalog_status"], model["sections"]["catalog-coverage"]))
    snapshot = snapshot_model(model)
    lines.extend(
        [
            "",
            f"<!-- {SNAPSHOT_PREFIX} {json.dumps(snapshot, sort_keys=True, separators=(',', ':'))} -->",
            "",
        ]
    )
    return "\n".join(lines)
```

`snapshot_model()` must remove `generated_at` and `results_dir` so validation
is stable across runs against the same artifacts.

- [ ] **Step 3: Run the focused writer test**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py -q
```

Expected: the writer content test passes.

- [ ] **Step 4: Commit the writer implementation**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/write_readiness_report.py benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py
git commit -m "feat: write bb readiness report"
```

---

### Task 3: Validator And Negative Controls

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/validate_readiness_report.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py`

**Interfaces:**
- Consumes: `write_readiness_report.build_report_model(results_dir: Path) -> dict[str, object]`
- Consumes: `write_readiness_report.snapshot_model(model: dict[str, object]) -> dict[str, object]`
- Produces: `validate_report(results_dir: Path, report_path: Path) -> list[str]`
- Produces: `main(argv: list[str] | None = None) -> int`

- [ ] **Step 1: Add validator tests**

Append these tests:

```python
from benchmarks.bb_circuit_bposd_compare import validate_readiness_report


def test_validate_readiness_report_accepts_generated_report(tmp_path, capsys) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)
    assert write_readiness_report.main(["--results-dir", str(results_dir), "--out", str(report_path)]) == 0

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 0
    assert "readiness report validated" in captured.out


def test_validate_readiness_report_rejects_stale_catalog_section(tmp_path, capsys) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)
    assert write_readiness_report.main(["--results-dir", str(results_dir), "--out", str(report_path)]) == 0
    (results_dir / "small-ldpc-catalog" / "manifest.csv").unlink()

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "catalog-coverage" in captured.err


def test_validate_readiness_report_rejects_visible_pass_when_gate_fails(tmp_path, capsys) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)
    (results_dir / "hard-profile" / "profile.json").unlink()
    assert write_readiness_report.main(["--results-dir", str(results_dir), "--out", str(report_path)]) == 0
    report_path.write_text(report_path.read_text().replace("**Final readiness verdict:** FAIL", "**Final readiness verdict:** PASS"))

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "final readiness verdict" in captured.err


def test_validate_readiness_report_rejects_placeholder_report(tmp_path, capsys) -> None:
    results_dir = tmp_path / "rstim-bb-ready"
    report_path = tmp_path / "bb-bposd-readiness.md"
    write_ready_tree(results_dir)
    report_path.write_text(
        "# BB BP-OSD Full-Campaign Readiness Report\n\n"
        "**Final readiness verdict:** PASS\n\n"
        "## Gate Summary\n\n"
        "## Semantic Parity Replay\n\n"
        "## BB90 Hard-Profile Counters\n\n"
        "## Setup/Run Split Evidence\n\n"
        "## Diagnostic Rust/Python Compare Rows\n\n"
        "## Small-LDPC Case Coverage\n"
    )

    status = validate_readiness_report.main(
        ["--results-dir", str(results_dir), "--report", str(report_path)]
    )

    captured = capsys.readouterr()
    assert status == 1
    assert "snapshot" in captured.err
```

- [ ] **Step 2: Run validator tests and confirm they fail before implementation**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py -q
```

Expected: validator import or test failures.

- [ ] **Step 3: Implement validator module**

Create `benchmarks/bb_circuit_bposd_compare/validate_readiness_report.py` with:

```python
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare import write_readiness_report

REQUIRED_HEADINGS = (
    "# BB BP-OSD Full-Campaign Readiness Report",
    "## Gate Summary",
    "## Semantic Parity Replay",
    "## BB90 Hard-Profile Counters",
    "## Setup/Run Split Evidence",
    "## Diagnostic Rust/Python Compare Rows",
    "## Small-LDPC Case Coverage",
)


def validate_report(results_dir: Path, report_path: Path) -> list[str]:
    errors: list[str] = []
    try:
        report = report_path.read_text()
    except OSError as error:
        return [f"cannot read report: {error}"]

    for heading in REQUIRED_HEADINGS:
        if heading not in report:
            errors.append(f"missing report section: {heading}")

    expected = write_readiness_report.snapshot_model(
        write_readiness_report.build_report_model(results_dir)
    )
    visible_verdict = _visible_verdict(report)
    if visible_verdict is None:
        errors.append("missing final readiness verdict")
    elif visible_verdict != expected["verdict"]:
        errors.append(
            "final readiness verdict mismatch: "
            f"report says {visible_verdict}, source gate says {expected['verdict']}"
        )

    snapshot = _report_snapshot(report, errors)
    if snapshot is not None and snapshot != expected:
        _append_snapshot_errors(snapshot, expected, errors)

    _check_visible_tokens(report, expected, errors)
    return errors
```

Implement `_visible_verdict`, `_report_snapshot`, `_append_snapshot_errors`,
and `_check_visible_tokens`. Snapshot errors must name changed sections using
`artifact_hashes` keys and `sections` keys, for example
`stale or missing section: catalog-coverage`.

- [ ] **Step 4: Run validator tests**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py -q
```

Expected: all readiness report tests pass.

- [ ] **Step 5: Commit validator implementation**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/validate_readiness_report.py benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py
git commit -m "feat: validate bb readiness report"
```

---

### Task 4: Documentation And End-To-End Verification

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/README.md`

**Interfaces:**
- Consumes: writer and validator CLIs from Tasks 2 and 3
- Produces: documented reviewer workflow and full verification evidence

- [ ] **Step 1: Add README section**

Insert after `## Full-Campaign Readiness Gate`:

```markdown
## Reviewer Readiness Report

After collecting a `/tmp/rstim-bb-ready` artifact tree accepted by the
readiness gate, generate the reviewer-readable Markdown report with:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.write_readiness_report \
  --results-dir /tmp/rstim-bb-ready \
  --out /tmp/bb-bposd-readiness.md
python3 -m benchmarks.bb_circuit_bposd_compare.validate_readiness_report \
  --results-dir /tmp/rstim-bb-ready \
  --report /tmp/bb-bposd-readiness.md
```

The report includes semantic parity replay status, BB90 hard-profile counters,
setup/run split evidence, high-p diagnostic Rust/Python compare rows, complete
small-LDPC catalog coverage, and the final verdict from `ready_for_full`.

The validator rebuilds the same readiness model from source artifacts and
compares it to the report snapshot and visible section content. It rejects
stale reports, missing source sections, placeholder headings, and reports whose
visible final verdict does not match the #286 readiness gate.
```

- [ ] **Step 2: Run focused tests**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests/test_readiness_report.py -q
```

Expected: all tests pass.

- [ ] **Step 3: Run the full BB compare pytest set**

Run:

```bash
python3 -m pytest benchmarks/bb_circuit_bposd_compare/tests -q
```

Expected: all tests pass.

- [ ] **Step 4: Run required CLI commands on a generated fixture tree**

Create the fixture tree with the existing test helper and run both required
commands:

```bash
python3 - <<'PY'
from pathlib import Path
from benchmarks.bb_circuit_bposd_compare.tests.test_ready_for_full import write_ready_tree
write_ready_tree(Path("/tmp/rstim-bb-ready"))
PY
python3 -m benchmarks.bb_circuit_bposd_compare.write_readiness_report --results-dir /tmp/rstim-bb-ready --out /tmp/bb-bposd-readiness.md
python3 -m benchmarks.bb_circuit_bposd_compare.validate_readiness_report --results-dir /tmp/rstim-bb-ready --report /tmp/bb-bposd-readiness.md
```

Expected: writer exits 0 and validator prints `readiness report validated`.

- [ ] **Step 5: Run Rust workspace verification**

Run:

```bash
cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 6: Commit documentation and final polish**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/README.md
git commit -m "docs: document bb readiness report"
```
