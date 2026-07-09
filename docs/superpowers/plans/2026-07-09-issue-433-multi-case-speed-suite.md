# Issue 433 Multi-Case Speed Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite` to run multiple requested `rstim` perf cases with one selected build profile and one artifact bundle.

**Architecture:** Reuse the existing Python build/probe helpers from `run_speed_case.py` and the Rust `rstim perf run/summarize/report` pipeline. The new suite module builds once, validates labels through the built Rust CLI, appends selected raw records, merges selected per-case summaries, renders one report, and writes one suite environment JSON.

**Tech Stack:** Python 3 standard library (`argparse`, `json`, `subprocess`, `tempfile`, `pathlib`, `unittest.mock`), existing Rust `rstim` perf CLI, existing Python benchmark test package.

## Global Constraints

- CLI interface is `python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite --profile <debug|release> --cases <comma-separated-labels> --warmup-rounds <n> --measure-rounds <n> --out-dir <dir>`.
- Successful output files are exactly `<dir>/raw.jsonl`, `<dir>/summary.json`, `<dir>/report.md`, and `<dir>/environment.json`.
- `raw.jsonl` and `summary.json` must contain exactly the requested case labels, with no omitted requested case and no extra unrequested case.
- Build the selected `rstim` profile once per suite run.
- Reuse `run_speed_case.py` and the existing `rstim perf run/summarize/report` pipeline where possible.
- Do not loop over `run_speed_case.py`.
- Do not run the full perf suite and trust downstream filtering.
- `environment.json` must record the case list, profile, command line, `rstim` binary path, Rust/Cargo versions, and Stim CLI probe metadata.
- `--cases ""` exits nonzero and prints `no benchmark cases requested`.
- `--cases does-not-exist` exits nonzero and prints `unknown benchmark case`.
- Do not publish checked evidence in this issue.
- Do not add performance thresholds against Stim.

---

## File Structure

- Modify `benchmarks/rstim_vs_stim_simulator/run_speed_case.py`: add a suite environment helper while preserving the single-case API.
- Create `benchmarks/rstim_vs_stim_simulator/run_speed_suite.py`: multi-case CLI and orchestration.
- Create `benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_suite.py`: focused TDD coverage for parsing, orchestration, exact summary labels, environment metadata, and negative controls.
- Modify `benchmarks/rstim_vs_stim_simulator/README.md`: document the suite command and exact artifacts.

### Task 1: Suite Tests And Environment Helper

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/run_speed_case.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_suite.py`

**Interfaces:**
- Produces: `collect_suite_environment(profile: str, case_labels: list[str], warmup_rounds: int, measure_rounds: int, rstim_binary_path: Path, command_line: list[str]) -> dict[str, Any]`.
- Keeps: `collect_environment(profile: str, case_label: str, warmup_rounds: int, measure_rounds: int, rstim_binary_path: Path) -> dict[str, Any]`.

- [ ] **Step 1: Write failing environment test**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_suite.py` with:

```python
from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator import run_speed_case, run_speed_suite


class RunSpeedSuiteEnvironmentTest(unittest.TestCase):
    def test_collect_suite_environment_records_case_list_and_command_line(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            binary = Path(temp_dir) / "target/release/rstim"
            binary.parent.mkdir(parents=True)
            binary.write_text("")

            def fake_run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                if command == ["rustc", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "rustc 1.93.1\n", "")
                if command == ["cargo", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "cargo 1.93.1\n", "")
                if command == [str(binary)]:
                    return subprocess.CompletedProcess(command, 0, "rstim 0.1.1\n", "")
                if command == ["stim", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "stim 1.15.0\n", "")
                raise AssertionError(f"unexpected command: {command}")

            with mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_case.subprocess.run") as mocked:
                mocked.side_effect = fake_run
                env = run_speed_case.collect_suite_environment(
                    profile="release",
                    case_labels=["rep-sample-d13-r13", "surface-detect-d13-r13"],
                    warmup_rounds=0,
                    measure_rounds=1,
                    rstim_binary_path=binary,
                    command_line=["python3", "-m", "benchmarks.rstim_vs_stim_simulator.run_speed_suite"],
                )

            self.assertEqual(env["profile"], "release")
            self.assertEqual(env["case_labels"], ["rep-sample-d13-r13", "surface-detect-d13-r13"])
            self.assertEqual(env["case_count"], 2)
            self.assertEqual(env["command_line"][2], "benchmarks.rstim_vs_stim_simulator.run_speed_suite")
            self.assertEqual(env["rustc_version"], "rustc 1.93.1")
            self.assertEqual(env["cargo_version"], "cargo 1.93.1")
            self.assertEqual(env["rstim_binary_path"], str(binary.resolve()))
            self.assertEqual(env["stim_cli_status"], "ok")
            self.assertEqual(env["stim_cli_version"], "stim 1.15.0")
```

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_suite -v`

Expected before implementation: import or attribute failure because `run_speed_suite` or `collect_suite_environment` does not exist.

- [ ] **Step 2: Implement `collect_suite_environment` with shared base helper**

Refactor `benchmarks/rstim_vs_stim_simulator/run_speed_case.py` so common probe fields are created by an internal helper:

```python
def _collect_environment_base(
    *,
    profile: str,
    warmup_rounds: int,
    measure_rounds: int,
    rstim_binary_path: Path,
) -> dict[str, Any]:
    stim = _probe(["stim", "--version"])
    stim_python = _probe_stim_python_version() if stim["status"] == "ok" and not stim.get("version") else None
    stim_version = stim.get("version")
    stim_version_source = "stim-cli-stdout"
    if not stim_version and stim_python is not None and stim_python["status"] == "ok":
        stim_version = stim_python.get("version")
        stim_version_source = "python-stim-module"
    rstim = _probe([str(rstim_binary_path)])
    environment: dict[str, Any] = {
        "profile": profile,
        "warmup_rounds": warmup_rounds,
        "measure_rounds": measure_rounds,
        "rustc_version": _version_string(["rustc", "--version"]),
        "cargo_version": _version_string(["cargo", "--version"]),
        "rstim_binary_path": str(rstim_binary_path.resolve()),
        "rstim_version": rstim.get("version"),
        "rstim_status": rstim["status"],
        "stim_cli": stim,
        "stim_cli_status": stim["status"],
        "stim_cli_version": stim_version,
        "stim_cli_version_source": stim_version_source if stim_version else None,
        "stim_cli_stderr": stim.get("stderr"),
    }
    if stim_python is not None:
        environment["stim_python"] = stim_python
        environment["stim_python_version"] = stim_python.get("version")
    return environment
```

Keep `collect_environment` by adding `case_label`, and add:

```python
def collect_suite_environment(
    *,
    profile: str,
    case_labels: list[str],
    warmup_rounds: int,
    measure_rounds: int,
    rstim_binary_path: Path,
    command_line: list[str],
) -> dict[str, Any]:
    environment = _collect_environment_base(
        profile=profile,
        warmup_rounds=warmup_rounds,
        measure_rounds=measure_rounds,
        rstim_binary_path=rstim_binary_path,
    )
    environment["case_labels"] = list(case_labels)
    environment["case_count"] = len(case_labels)
    environment["command_line"] = list(command_line)
    return environment
```

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_suite -v`

Expected after implementation: environment test passes if the module stub exists; orchestration tests will be added next.

### Task 2: Suite Orchestration And CLI

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/run_speed_suite.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_suite.py`

**Interfaces:**
- Produces: `parse_case_labels(raw_cases: str) -> list[str]`.
- Produces: `run_speed_suite(args: argparse.Namespace, repo_root: Path = REPO_ROOT, command_line: list[str] | None = None) -> None`.
- Produces: `main(argv: list[str] | None = None) -> int`.
- Consumes: `run_speed_case.build_rstim`, `run_speed_case._run_checked`, `run_speed_case._require_artifact`, `run_speed_case.write_environment`, `run_speed_case.collect_suite_environment`.

- [ ] **Step 1: Write failing parser and negative-control tests**

Append to `test_run_speed_suite.py`:

```python
class RunSpeedSuiteParserTest(unittest.TestCase):
    def test_parse_case_labels_strips_blanks_and_rejects_empty(self) -> None:
        self.assertEqual(
            run_speed_suite.parse_case_labels(" rep-sample-d13-r13, surface-detect-d13-r13 "),
            ["rep-sample-d13-r13", "surface-detect-d13-r13"],
        )
        with self.assertRaisesRegex(ValueError, "no benchmark cases requested"):
            run_speed_suite.parse_case_labels("")
        with self.assertRaisesRegex(ValueError, "no benchmark cases requested"):
            run_speed_suite.parse_case_labels(" , ")

    def test_parse_case_labels_rejects_duplicates(self) -> None:
        with self.assertRaisesRegex(ValueError, "duplicate benchmark case: rep-sample-d13-r13"):
            run_speed_suite.parse_case_labels("rep-sample-d13-r13,rep-sample-d13-r13")

    def test_main_empty_cases_prints_required_message(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out_dir = Path(temp_dir) / "out"
            with (
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_suite.print") as mocked_print,
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_suite.run_speed_case.build_rstim") as mocked_build,
            ):
                code = run_speed_suite.main(
                    [
                        "--profile",
                        "release",
                        "--cases",
                        "",
                        "--warmup-rounds",
                        "0",
                        "--measure-rounds",
                        "1",
                        "--out-dir",
                        str(out_dir),
                    ]
                )

            self.assertEqual(code, 1)
            self.assertFalse(mocked_build.called)
            self.assertIn("no benchmark cases requested", str(mocked_print.call_args))
```

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_suite -v`

Expected before implementation: parser and main attribute failures.

- [ ] **Step 2: Write failing suite orchestration test**

Append to `test_run_speed_suite.py`:

```python
class RunSpeedSuiteWorkflowTest(unittest.TestCase):
    def test_run_speed_suite_builds_once_and_writes_exact_requested_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            out_dir = repo_root / "suite"
            binary = repo_root / "target/release/rstim"
            binary.parent.mkdir(parents=True)
            binary.write_text("")
            args = argparse.Namespace(
                profile="release",
                cases="rep-sample-d13-r13,surface-detect-d13-r13,stim-style-surface-sample-d11-r100-b1024",
                warmup_rounds=0,
                measure_rounds=1,
                out_dir=out_dir,
            )
            commands: list[list[str]] = []

            def fake_run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                commands.append(command)
                if command[:3] == [str(binary), "perf", "run"]:
                    label = command[command.index("--case") + 1]
                    out_path = Path(command[command.index("--out") + 1])
                    out_path.write_text(f'{{"case_label":"{label}","tool_variant":"stim-cli"}}\n')
                elif command[:3] == [str(binary), "perf", "summarize"]:
                    label = command[command.index("--case") + 1]
                    out_path = Path(command[command.index("--out") + 1])
                    out_path.write_text(
                        json.dumps(
                            {
                                "cases": [{"case_label": label, "variants": []}],
                                "issues": [],
                            }
                        )
                        + "\n"
                    )
                elif command[:3] == [str(binary), "perf", "report"]:
                    out_path = Path(command[command.index("--out") + 1])
                    out_path.write_text("# suite report\n")
                elif command == ["rustc", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "rustc 1.93.1\n", "")
                elif command == ["cargo", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "cargo 1.93.1\n", "")
                elif command == [str(binary)]:
                    return subprocess.CompletedProcess(command, 0, "rstim 0.1.1\n", "")
                elif command == ["stim", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "stim 1.15.0\n", "")
                return subprocess.CompletedProcess(command, 0, "", "")

            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_speed_suite.run_speed_case.build_rstim",
                    return_value=binary,
                ) as build,
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_case.subprocess.run") as mocked,
            ):
                mocked.side_effect = fake_run
                run_speed_suite.run_speed_suite(
                    args,
                    repo_root=repo_root,
                    command_line=[
                        "python3",
                        "-m",
                        "benchmarks.rstim_vs_stim_simulator.run_speed_suite",
                    ],
                )

            self.assertEqual(build.call_count, 1)
            run_commands = [command for command in commands if command[:3] == [str(binary), "perf", "run"]]
            self.assertEqual(len(run_commands), 3)
            self.assertEqual(
                [command[command.index("--case") + 1] for command in run_commands],
                [
                    "rep-sample-d13-r13",
                    "surface-detect-d13-r13",
                    "stim-style-surface-sample-d11-r100-b1024",
                ],
            )
            self.assertEqual(
                [json.loads(line)["case_label"] for line in (out_dir / "raw.jsonl").read_text().splitlines()],
                [
                    "rep-sample-d13-r13",
                    "surface-detect-d13-r13",
                    "stim-style-surface-sample-d11-r100-b1024",
                ],
            )
            summary = json.loads((out_dir / "summary.json").read_text())
            self.assertEqual(
                [case["case_label"] for case in summary["cases"]],
                [
                    "rep-sample-d13-r13",
                    "surface-detect-d13-r13",
                    "stim-style-surface-sample-d11-r100-b1024",
                ],
            )
            self.assertEqual(summary["issues"], [])
            self.assertEqual((out_dir / "report.md").read_text(), "# suite report\n")
            environment = json.loads((out_dir / "environment.json").read_text())
            self.assertEqual(environment["profile"], "release")
            self.assertEqual(
                environment["case_labels"],
                [
                    "rep-sample-d13-r13",
                    "surface-detect-d13-r13",
                    "stim-style-surface-sample-d11-r100-b1024",
                ],
            )
            self.assertEqual(environment["command_line"][2], "benchmarks.rstim_vs_stim_simulator.run_speed_suite")
```

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_suite -v`

Expected before implementation: `run_speed_suite` attribute failure.

- [ ] **Step 3: Implement `run_speed_suite.py`**

Create `benchmarks/rstim_vs_stim_simulator/run_speed_suite.py` with:

```python
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from benchmarks.rstim_vs_stim_simulator import run_speed_case


PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]
```

Implement these functions:

```python
def parse_case_labels(raw_cases: str) -> list[str]:
    labels = [label.strip() for label in raw_cases.split(",") if label.strip()]
    if not labels:
        raise ValueError("no benchmark cases requested")
    seen: set[str] = set()
    for label in labels:
        if label in seen:
            raise ValueError(f"duplicate benchmark case: {label}")
        seen.add(label)
    return labels
```

```python
def _append_file(source: Path, destination: Path) -> None:
    with source.open("r", encoding="utf-8") as src, destination.open("a", encoding="utf-8") as dst:
        dst.write(src.read())
```

```python
def _merge_case_summary(summary_path: Path, merged: dict[str, list[Any]]) -> None:
    summary = json.loads(summary_path.read_text())
    merged["cases"].extend(summary.get("cases", []))
    merged["issues"].extend(summary.get("issues", []))
```

`run_speed_suite` should:

1. Parse and validate labels before building.
2. Build once with `run_speed_case.build_rstim(args.profile, repo_root=repo_root)`.
3. Create `out_dir` and use `tempfile.TemporaryDirectory()` for per-case raw and summary files.
4. Validate every label before timing by running:
   `rstim perf summarize --case <label> --in <empty-jsonl> --out <temp-summary>`.
5. Run each selected case with:
   `rstim perf run --case <label> --warmup-rounds <n> --measure-rounds <n> --out <temp-raw>`.
6. Append each temp raw file to `<out_dir>/raw.jsonl`.
7. Summarize each requested label from the suite raw file with:
   `rstim perf summarize --case <label> --in <raw.jsonl> --out <temp-summary>`.
8. Merge the selected case summaries and write pretty JSON plus newline to
   `<out_dir>/summary.json`.
9. Run `rstim perf report --in <summary.json> --out <report.md>`.
10. Write `environment.json` with `collect_suite_environment`.

`main` should catch `OSError`, `RuntimeError`, `subprocess.CalledProcessError`,
and `ValueError`, print the error to stderr, and return `1`.

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_suite -v`

Expected after implementation: all suite unit tests pass.

### Task 3: Documentation And Verification

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md`

**Interfaces:**
- Documents: selected-case and multi-case speed runner commands.

- [ ] **Step 1: Update README**

Add a "Multi-Case Speed Suite" subsection after the selected speed runner:

```markdown
## Multi-Case Speed Suite

Run a release-profile suite over exactly the requested perf cases:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite \
  --profile release \
  --cases rep-sample-d13-r13,surface-detect-d13-r13,stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir /tmp/rstim-speed-suite
```

The suite runner builds the selected `rstim` profile once, writes one
`raw.jsonl`, one `summary.json`, one `report.md`, and one `environment.json`,
and keeps the raw and summary artifacts scoped to exactly the comma-separated
case list.
```

- [ ] **Step 2: Run focused verification**

Run:

```bash
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_suite -q
```

Expected: pass.

- [ ] **Step 3: Run required issue command**

Run:

```bash
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite \
  --profile release \
  --cases rep-sample-d13-r13,surface-detect-d13-r13,stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir /tmp/rstim-speed-suite
```

Expected: exit 0. Then inspect `/tmp/rstim-speed-suite/summary.json` and
`/tmp/rstim-speed-suite/environment.json`; both list exactly:

```text
rep-sample-d13-r13
surface-detect-d13-r13
stim-style-surface-sample-d11-r100-b1024
```

and `environment.json` has `profile = "release"`.

- [ ] **Step 4: Run negative controls**

Run:

```bash
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite --profile release --cases "" --warmup-rounds 0 --measure-rounds 1 --out-dir /tmp/rstim-speed-suite-empty
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_suite --profile release --cases does-not-exist --warmup-rounds 0 --measure-rounds 1 --out-dir /tmp/rstim-speed-suite-unknown
```

Expected: both exit nonzero. The first prints `no benchmark cases requested`.
The second prints `unknown benchmark case`.

- [ ] **Step 5: Run repository gate**

Run:

```bash
cargo test
```

Expected: pass.

- [ ] **Step 6: Commit**

Commit implementation, tests, docs, spec, and plan:

```bash
git add benchmarks/rstim_vs_stim_simulator/run_speed_case.py benchmarks/rstim_vs_stim_simulator/run_speed_suite.py benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_suite.py benchmarks/rstim_vs_stim_simulator/README.md docs/superpowers/specs/2026-07-09-issue-433-multi-case-speed-suite-design.md docs/superpowers/plans/2026-07-09-issue-433-multi-case-speed-suite.md
git commit -m "feat: add multi-case rstim speed suite"
```

## Self-Review

- Every requirement in the issue maps to a task above.
- No placeholder text remains.
- Function names used by tests and implementation steps match.
- The plan uses TDD before production code.
- Verification includes the exact issue command, the focused unit test command, negative controls, and `cargo test`.
