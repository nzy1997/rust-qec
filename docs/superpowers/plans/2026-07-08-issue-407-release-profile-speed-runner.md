# Issue 407 Release-Profile Speed Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case` to build/select debug or release `rstim`, run one selected perf case, and write raw, summary, report, and environment artifacts.

**Architecture:** Keep benchmark logic in the existing `rstim perf run`, `rstim perf summarize`, and `rstim perf report` commands. The new Python module is a thin orchestration layer that validates the profile, builds the matching binary, invokes the three perf commands, and records environment metadata in JSON.

**Tech Stack:** Python 3.11 standard library (`argparse`, `dataclasses`, `json`, `subprocess`, `pathlib`, `unittest.mock`), existing Rust `rstim` perf CLI.

## Global Constraints

- CLI interface is exactly `python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case --profile <debug|release> --case <case-label> --warmup-rounds <n> --measure-rounds <n> --out-dir <dir>`.
- Successful output files are exactly `<dir>/raw.jsonl`, `<dir>/summary.json`, `<dir>/report.md`, and `<dir>/environment.json`.
- Artifacts must describe only the selected case label.
- `environment.json` must include at least `profile`, `rustc_version`, `cargo_version`, `rstim_binary_path`, and Stim CLI version or failure status.
- Release runs build with `cargo build --release -p rstim --bin rstim` and use `target/release/rstim`.
- Debug runs build with `cargo build -p rstim --bin rstim` and use `target/debug/rstim`.
- Reuse existing `rstim perf run`, `rstim perf summarize`, and `rstim perf report`; do not duplicate benchmark logic.
- Keep the script under `benchmarks/rstim_vs_stim_simulator/`.
- Do not set a pass/fail wall-clock threshold.
- Do not update the checked #406 artifacts under `benchmarks/rstim_vs_stim_simulator/results/full/`.
- Do not optimize sampler internals.

---

## File Structure

- Create `benchmarks/rstim_vs_stim_simulator/run_speed_case.py`: Python module entrypoint and orchestration helpers.
- Create `benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_case.py`: unit tests for profile build selection, perf command orchestration, environment metadata, and invalid profile behavior.
- Modify `benchmarks/rstim_vs_stim_simulator/README.md`: document the selected-case speed runner command and artifact set.

### Task 1: Python Speed Runner And Tests

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/run_speed_case.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_case.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/README.md`

**Interfaces:**
- Produces: `build_rstim(profile: str, repo_root: Path = REPO_ROOT) -> Path`.
- Produces: `run_speed_case(args: argparse.Namespace, repo_root: Path = REPO_ROOT) -> None`.
- Produces: `main(argv: list[str] | None = None) -> int`.
- Consumes: existing `rstim perf run`, `rstim perf summarize`, and `rstim perf report` CLI subcommands.

- [ ] **Step 1: Write failing tests for profile build selection**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_case.py` with imports and these first tests:

```python
from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator import run_speed_case


class RunSpeedCaseProfileTest(unittest.TestCase):
    def test_build_rstim_debug_builds_debug_binary(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            binary = repo_root / "target/debug/rstim"
            binary.parent.mkdir(parents=True)
            binary.write_text("")
            calls: list[tuple[list[str], dict[str, object]]] = []

            def fake_run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                calls.append((command, kwargs))
                return subprocess.CompletedProcess(command, 0, "", "")

            with mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_case.subprocess.run") as mocked:
                mocked.side_effect = fake_run
                result = run_speed_case.build_rstim("debug", repo_root=repo_root)

            self.assertEqual(result, binary)
            self.assertEqual(calls[0][0], ["cargo", "build", "-p", "rstim", "--bin", "rstim"])
            self.assertEqual(calls[0][1]["cwd"], repo_root)
            self.assertTrue(calls[0][1]["check"])

    def test_build_rstim_release_builds_release_binary(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            binary = repo_root / "target/release/rstim"
            binary.parent.mkdir(parents=True)
            binary.write_text("")
            calls: list[list[str]] = []

            def fake_run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                calls.append(command)
                return subprocess.CompletedProcess(command, 0, "", "")

            with mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_case.subprocess.run") as mocked:
                mocked.side_effect = fake_run
                result = run_speed_case.build_rstim("release", repo_root=repo_root)

            self.assertEqual(result, binary)
            self.assertEqual(
                calls[0],
                ["cargo", "build", "--release", "-p", "rstim", "--bin", "rstim"],
            )
```

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_case -v`

Expected before implementation: import or attribute failure because `run_speed_case` and `build_rstim` do not exist.

- [ ] **Step 2: Write failing tests for orchestration and invalid profile behavior**

Append these tests to the same file:

```python
class RunSpeedCaseWorkflowTest(unittest.TestCase):
    def test_run_speed_case_invokes_perf_pipeline_and_writes_environment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            out_dir = repo_root / "out"
            binary = repo_root / "target/release/rstim"
            binary.parent.mkdir(parents=True)
            binary.write_text("")
            args = argparse.Namespace(
                profile="release",
                case="stim-style-surface-sample-d11-r100-b1024",
                warmup_rounds=0,
                measure_rounds=1,
                out_dir=out_dir,
            )
            commands: list[list[str]] = []

            def fake_run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                commands.append(command)
                if command == ["rustc", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "rustc 1.93.1\n", "")
                if command == ["cargo", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "cargo 1.93.1\n", "")
                if command == [str(binary)]:
                    return subprocess.CompletedProcess(command, 0, "rstim 0.1.1\n", "")
                if command == ["stim", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "stim 1.15.0\n", "")
                return subprocess.CompletedProcess(command, 0, "", "")

            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_speed_case.build_rstim",
                    return_value=binary,
                ),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_case.subprocess.run") as mocked,
            ):
                mocked.side_effect = fake_run
                run_speed_case.run_speed_case(args, repo_root=repo_root)

            self.assertIn(
                [
                    str(binary),
                    "perf",
                    "run",
                    "--case",
                    "stim-style-surface-sample-d11-r100-b1024",
                    "--warmup-rounds",
                    "0",
                    "--measure-rounds",
                    "1",
                    "--out",
                    str(out_dir / "raw.jsonl"),
                ],
                commands,
            )
            self.assertIn(
                [
                    str(binary),
                    "perf",
                    "summarize",
                    "--in",
                    str(out_dir / "raw.jsonl"),
                    "--out",
                    str(out_dir / "summary.json"),
                ],
                commands,
            )
            self.assertIn(
                [
                    str(binary),
                    "perf",
                    "report",
                    "--in",
                    str(out_dir / "summary.json"),
                    "--out",
                    str(out_dir / "report.md"),
                ],
                commands,
            )
            self.assertFalse((out_dir / "summary.json").exists())
            env = json.loads((out_dir / "environment.json").read_text())
            self.assertEqual(env["profile"], "release")
            self.assertEqual(env["case_label"], "stim-style-surface-sample-d11-r100-b1024")
            self.assertEqual(env["rustc_version"], "rustc 1.93.1")
            self.assertEqual(env["cargo_version"], "cargo 1.93.1")
            self.assertEqual(env["rstim_binary_path"], str(binary.resolve()))
            self.assertEqual(env["stim_cli_status"], "ok")
            self.assertEqual(env["stim_cli_version"], "stim 1.15.0")

    def test_run_speed_case_records_stim_version_failure_status(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            binary = repo_root / "target/debug/rstim"
            binary.parent.mkdir(parents=True)
            binary.write_text("")
            env = run_speed_case.collect_environment(
                profile="debug",
                case_label="case-a",
                warmup_rounds=0,
                measure_rounds=1,
                rstim_binary_path=binary,
            )
            self.assertIn("stim_cli_status", env)

    def test_main_rejects_bogus_profile_before_output_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out_dir = Path(temp_dir) / "bogus"
            with self.assertRaises(SystemExit) as raised:
                run_speed_case.main(
                    [
                        "--profile",
                        "bogus",
                        "--case",
                        "stim-style-surface-sample-d11-r100-b1024",
                        "--out-dir",
                        str(out_dir),
                    ]
                )

            self.assertNotEqual(raised.exception.code, 0)
            self.assertFalse((out_dir / "summary.json").exists())
```

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_case -v`

Expected before implementation: failures because orchestration helpers and parser behavior do not exist.

- [ ] **Step 3: Implement `run_speed_case.py` minimally**

Create `benchmarks/rstim_vs_stim_simulator/run_speed_case.py` with:

```python
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


PACKAGE_DIR = Path(__file__).resolve().parent
REPO_ROOT = PACKAGE_DIR.parents[1]


def _run_checked(command: list[str], *, cwd: Path) -> None:
    subprocess.run(command, cwd=cwd, check=True)


def _probe(command: list[str]) -> dict[str, object]:
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        return {
            "command": command,
            "status": "failed",
            "exit_code": None,
            "version": None,
            "stderr": str(error),
        }
    version = completed.stdout.strip()
    status = "ok" if completed.returncode == 0 else "failed"
    return {
        "command": command,
        "status": status,
        "exit_code": completed.returncode,
        "version": version if status == "ok" else None,
        "stderr": completed.stderr.strip(),
    }


def _version_string(command: list[str]) -> str:
    result = _probe(command)
    if result["status"] == "ok" and result["version"]:
        return str(result["version"])
    stderr = str(result.get("stderr") or "")
    return f"failed: {stderr}" if stderr else "failed"


def build_rstim(profile: str, *, repo_root: Path = REPO_ROOT) -> Path:
    if profile == "release":
        command = ["cargo", "build", "--release", "-p", "rstim", "--bin", "rstim"]
        binary = repo_root / "target/release/rstim"
    elif profile == "debug":
        command = ["cargo", "build", "-p", "rstim", "--bin", "rstim"]
        binary = repo_root / "target/debug/rstim"
    else:
        raise ValueError(f"unsupported profile: {profile}")

    _run_checked(command, cwd=repo_root)
    if not binary.exists():
        raise FileNotFoundError(f"expected rstim binary not found: {binary}")
    return binary


def collect_environment(
    *,
    profile: str,
    case_label: str,
    warmup_rounds: int,
    measure_rounds: int,
    rstim_binary_path: Path,
) -> dict[str, Any]:
    stim = _probe(["stim", "--version"])
    rstim = _probe([str(rstim_binary_path)])
    return {
        "profile": profile,
        "case_label": case_label,
        "warmup_rounds": warmup_rounds,
        "measure_rounds": measure_rounds,
        "rustc_version": _version_string(["rustc", "--version"]),
        "cargo_version": _version_string(["cargo", "--version"]),
        "rstim_binary_path": str(rstim_binary_path.resolve()),
        "rstim_version": rstim.get("version"),
        "rstim_status": rstim["status"],
        "stim_cli": stim,
        "stim_cli_status": stim["status"],
        "stim_cli_version": stim.get("version"),
        "stim_cli_stderr": stim.get("stderr"),
    }


def write_environment(path: Path, environment: dict[str, Any]) -> None:
    path.write_text(json.dumps(environment, indent=2, sort_keys=True) + "\n")


def run_speed_case(args: argparse.Namespace, *, repo_root: Path = REPO_ROOT) -> None:
    rstim_binary = build_rstim(args.profile, repo_root=repo_root)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    raw_path = out_dir / "raw.jsonl"
    summary_path = out_dir / "summary.json"
    report_path = out_dir / "report.md"
    environment_path = out_dir / "environment.json"

    _run_checked(
        [
            str(rstim_binary),
            "perf",
            "run",
            "--case",
            args.case,
            "--warmup-rounds",
            str(args.warmup_rounds),
            "--measure-rounds",
            str(args.measure_rounds),
            "--out",
            str(raw_path),
        ],
        cwd=repo_root,
    )
    _run_checked(
        [
            str(rstim_binary),
            "perf",
            "summarize",
            "--in",
            str(raw_path),
            "--out",
            str(summary_path),
        ],
        cwd=repo_root,
    )
    _run_checked(
        [
            str(rstim_binary),
            "perf",
            "report",
            "--in",
            str(summary_path),
            "--out",
            str(report_path),
        ],
        cwd=repo_root,
    )
    write_environment(
        environment_path,
        collect_environment(
            profile=args.profile,
            case_label=args.case,
            warmup_rounds=args.warmup_rounds,
            measure_rounds=args.measure_rounds,
            rstim_binary_path=rstim_binary,
        ),
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run one rstim-vs-Stim speed case with a selected rstim build profile."
    )
    parser.add_argument("--profile", choices=["debug", "release"], required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--warmup-rounds", type=int, default=1)
    parser.add_argument("--measure-rounds", type=int, default=5)
    parser.add_argument("--out-dir", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        run_speed_case(args)
    except (OSError, subprocess.CalledProcessError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run tests and fix only implementation defects**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_speed_case -v`

Expected after implementation: all new Python tests pass.

- [ ] **Step 5: Document the runner in the benchmark README**

Add this section after the Correctness Verification section in
`benchmarks/rstim_vs_stim_simulator/README.md`:

```markdown
## Selected Speed Runner

Run a single public speed case with an explicit `rstim` build profile:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case \
  --profile release \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir /tmp/rstim-speed-release
```

The runner builds `target/release/rstim` for `--profile release` or
`target/debug/rstim` for `--profile debug`, then reuses `rstim perf run`,
`rstim perf summarize`, and `rstim perf report`. It writes `raw.jsonl`,
`summary.json`, `report.md`, and `environment.json` under `--out-dir`.

This runner is for selected-case evidence only. It does not set a timing
threshold, update checked results, or optimize sampler internals.
```

- [ ] **Step 6: Verify issue commands**

Run:

```bash
rm -rf /tmp/rstim-speed-release
python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case \
  --profile release \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir /tmp/rstim-speed-release
python3 - <<'PY'
import json
from pathlib import Path
root = Path('/tmp/rstim-speed-release')
for name in ['raw.jsonl', 'summary.json', 'report.md', 'environment.json']:
    assert (root / name).is_file(), name
summary = json.loads((root / 'summary.json').read_text())
assert [case['case_label'] for case in summary['cases']] == ['stim-style-surface-sample-d11-r100-b1024']
env = json.loads((root / 'environment.json').read_text())
assert env['profile'] == 'release'
assert 'rustc_version' in env and 'cargo_version' in env
report = (root / 'report.md').read_text()
assert 'report-only Stim comparison' in report
print('PASS release selected-case speed artifacts are complete')
PY
```

Expected: `PASS release selected-case speed artifacts are complete`.

Run:

```bash
rm -rf /tmp/rstim-speed-bogus
if python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case \
  --profile bogus \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --out-dir /tmp/rstim-speed-bogus; then
  echo 'unexpected success' >&2
  exit 1
fi
test ! -f /tmp/rstim-speed-bogus/summary.json
```

Expected: command exits successfully because the runner rejected `bogus` and
did not write `summary.json`.

- [ ] **Step 7: Run repository gate and commit**

Run:

```bash
cargo test
```

Expected: all Rust tests pass.

Commit:

```bash
git add benchmarks/rstim_vs_stim_simulator/run_speed_case.py \
  benchmarks/rstim_vs_stim_simulator/tests/test_run_speed_case.py \
  benchmarks/rstim_vs_stim_simulator/README.md \
  docs/superpowers/plans/2026-07-08-issue-407-release-profile-speed-runner.md
git commit -m "feat: add selected speed runner"
```

## Self-Review

- The plan covers every issue output artifact.
- The plan uses the existing perf commands rather than duplicating benchmark logic.
- The invalid profile path is tested before output creation.
- The release and debug binary paths are both tested.
- The real issue verification and negative control are included.
