# Issue 322 QEC-Code Random-Window Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a local JSONL benchmark runner for `qec-code code css-distance random-window-upper-bound`.

**Architecture:** Add a focused Python module under `benchmarks/qec_code_random_window/` that validates #321 manifests, runs the local `qec-code` CLI once per case/seed, measures subprocess wall time, and writes JSONL rows. Unit tests use a temporary fake executable so failure handling is verified without slow benchmark work.

**Tech Stack:** Python 3.11+ standard library (`argparse`, `json`, `os`, `pathlib`, `subprocess`, `sys`, `tempfile`, `time`, `tomllib`, `unittest`), existing TOML manifests, Cargo workspace verification.

## Global Constraints

- Create `benchmarks/qec_code_random_window/run_local.py`.
- Consume the manifest format from #321 via the existing validator contract.
- Run only `qec-code code css-distance random-window-upper-bound`; do not run or vendor paper algorithms.
- Pass `--json` to every `qec-code` random-window subprocess.
- Measure elapsed time with `time.perf_counter()` around the subprocess call.
- Write one JSONL row per case and seed.
- Each row must include at least `case_id`, `code_id`, `seed`, `iterations`, `restarts`, `upper_bound`, `elapsed_s`, `status`, and `raw_cli_json`.
- Use `status = "ok"` only when the subprocess exits 0, stdout parses as JSON, parsed JSON has `status = "completed"`, parsed JSON has `method = "random-window-upper-bound"`, and parsed JSON has a positive integer `upper_bound`.
- On failure, write a non-`ok` row with enough stderr/stdout context for diagnosis.
- Command-line overrides must support `--seeds`, `--iterations`, `--restarts`, and `--target-weight`.
- Default qec-code binary resolution is `QEC_CODE_BIN`, then `target/debug/qec-code` when present, then `qec-code` from `PATH`.
- The runner exits 0 only when every emitted row is `ok`; any failed row makes the runner exit nonzero.
- Required verification includes the issue smoke command, `cargo test -p qec-code`, and `cargo test`.

---

### Task 1: Add Local Runner With Subprocess Tests

**Files:**
- Create: `benchmarks/qec_code_random_window/run_local.py`
- Create: `benchmarks/qec_code_random_window/tests/test_run_local.py`

**Interfaces:**
- Consumes: `validate_cases.load_manifest(path: Path) -> dict[str, Any]` and `validate_cases.validate_manifest(manifest: dict[str, Any]) -> list[str]`.
- Produces: `main(argv: list[str] | None = None) -> int`.
- Produces: CLI `python3 -m benchmarks.qec_code_random_window.run_local --cases <path> --out <path> [--seeds ...] [--iterations N] [--restarts N] [--target-weight N] [--qec-code-bin PATH]`.
- Produces: JSONL rows with `raw_cli_json` holding the parsed CLI JSON object on parse success.

- [ ] **Step 1: Write the failing tests**

Create `benchmarks/qec_code_random_window/tests/test_run_local.py`:

```python
from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]


FAKE_QEC_CODE = """\
#!/usr/bin/env python3
import json
import os
import sys


def value_after(flag):
    try:
        return sys.argv[sys.argv.index(flag) + 1]
    except (ValueError, IndexError):
        return None


record_path = os.environ.get("FAKE_QEC_CODE_INVOCATIONS")
if record_path:
    with open(record_path, "a", encoding="utf-8") as handle:
        handle.write(json.dumps({"argv": sys.argv[1:]}) + "\\n")

mode = os.environ.get("FAKE_QEC_CODE_MODE", "ok")
code_id = value_after("--code-id")
if code_id == "invalid":
    print("unknown built-in CSS code: invalid", file=sys.stderr)
    sys.exit(2)

if mode == "non_json":
    print("not json")
    sys.exit(0)

payload = {
    "status": "completed",
    "method": "random-window-upper-bound",
    "bound_type": "upper",
    "upper_bound": int(value_after("--target-weight") or "3"),
    "options": {
        "iterations": int(value_after("--iterations") or "0"),
        "restarts": int(value_after("--restarts") or "0"),
        "seed": int(value_after("--seed") or "0"),
        "target_weight": int(value_after("--target-weight") or "0"),
    },
}

if mode == "missing_upper_bound":
    del payload["upper_bound"]

print(json.dumps(payload))
"""


def write_manifest(path: Path, code_ids: list[str]) -> None:
    cases = []
    for index, code_id in enumerate(code_ids, start=1):
        cases.append(
            f'''
[[cases]]
case_id = "case_{index}"
code_id = "{code_id}"
distance_side = "any"
iterations = 10
restarts = 2
seed = 5
target_weight = 3
target_upper_bound = 3
baseline_key = "unmapped:case_{index}"
baseline_required = false
'''.strip()
        )

    path.write_text(
        'manifest_version = 1\\n'
        'suite = "qec_code_random_window"\\n\\n'
        + "\\n\\n".join(cases)
        + "\\n",
        encoding="utf-8",
    )


def read_jsonl(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


class RunLocalTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.tmp_path = Path(self.tmp.name)

        self.fake_bin = self.tmp_path / "qec-code"
        self.fake_bin.write_text(FAKE_QEC_CODE, encoding="utf-8")
        self.fake_bin.chmod(self.fake_bin.stat().st_mode | stat.S_IXUSR)

        self.invocations = self.tmp_path / "invocations.jsonl"
        self.manifest = self.tmp_path / "cases.toml"
        self.out = self.tmp_path / "results.jsonl"

    def run_runner(
        self,
        *extra_args: str,
        mode: str = "ok",
        code_ids: list[str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        write_manifest(self.manifest, code_ids or ["steane", "surface_rotated:d=3"])
        env = os.environ.copy()
        env["FAKE_QEC_CODE_INVOCATIONS"] = str(self.invocations)
        env["FAKE_QEC_CODE_MODE"] = mode

        return subprocess.run(
            [
                sys.executable,
                "-m",
                "benchmarks.qec_code_random_window.run_local",
                "--cases",
                str(self.manifest),
                "--out",
                str(self.out),
                "--qec-code-bin",
                str(self.fake_bin),
                *extra_args,
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
            env=env,
        )

    def test_success_writes_one_ok_row_per_case_and_seed_with_overrides(self) -> None:
        result = self.run_runner(
            "--seeds",
            "7",
            "8",
            "--iterations",
            "50",
            "--restarts",
            "1",
            "--target-weight",
            "3",
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")
        rows = read_jsonl(self.out)
        self.assertEqual(len(rows), 4)
        self.assertTrue(all(row["status"] == "ok" for row in rows))
        self.assertEqual([row["seed"] for row in rows], [7, 8, 7, 8])
        self.assertTrue(all(row["iterations"] == 50 for row in rows))
        self.assertTrue(all(row["restarts"] == 1 for row in rows))
        self.assertTrue(all(row["upper_bound"] == 3 for row in rows))
        self.assertTrue(all(row["elapsed_s"] > 0 for row in rows))
        self.assertTrue(
            all(row["raw_cli_json"]["method"] == "random-window-upper-bound" for row in rows)
        )

        invocations = read_jsonl(self.invocations)
        self.assertEqual(len(invocations), 4)
        first_argv = invocations[0]["argv"]
        self.assertEqual(
            first_argv[:3],
            ["code", "css-distance", "random-window-upper-bound"],
        )
        self.assertIn("--json", first_argv)
        self.assertEqual(first_argv[first_argv.index("--code-id") + 1], "steane")
        self.assertEqual(first_argv[first_argv.index("--seed") + 1], "7")
        self.assertEqual(first_argv[first_argv.index("--iterations") + 1], "50")
        self.assertEqual(first_argv[first_argv.index("--restarts") + 1], "1")
        self.assertEqual(first_argv[first_argv.index("--target-weight") + 1], "3")

    def test_invalid_code_id_exits_nonzero_and_does_not_emit_ok_row(self) -> None:
        result = self.run_runner(code_ids=["invalid"])

        self.assertNotEqual(result.returncode, 0)
        rows = read_jsonl(self.out)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["status"], "cli_error")
        self.assertIsNone(rows[0]["upper_bound"])
        self.assertIn("unknown built-in CSS code", rows[0]["stderr_context"])

    def test_non_json_stdout_exits_nonzero_and_does_not_emit_ok_row(self) -> None:
        result = self.run_runner(mode="non_json", code_ids=["steane"])

        self.assertNotEqual(result.returncode, 0)
        rows = read_jsonl(self.out)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["status"], "non_json_stdout")
        self.assertIsNone(rows[0]["raw_cli_json"])
        self.assertIn("not json", rows[0]["stdout_context"])

    def test_missing_upper_bound_exits_nonzero_and_does_not_emit_ok_row(self) -> None:
        result = self.run_runner(mode="missing_upper_bound", code_ids=["steane"])

        self.assertNotEqual(result.returncode, 0)
        rows = read_jsonl(self.out)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["status"], "missing_upper_bound")
        self.assertIsNone(rows[0]["upper_bound"])
        self.assertEqual(rows[0]["raw_cli_json"]["status"], "completed")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run RED verification**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_run_local -q
```

Expected: FAIL because `benchmarks.qec_code_random_window.run_local` does not exist yet.

- [ ] **Step 3: Implement the runner**

Create `benchmarks/qec_code_random_window/run_local.py`:

```python
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from benchmarks.qec_code_random_window.validate_cases import load_manifest, validate_manifest


ROOT = Path(__file__).resolve().parents[2]
METHOD = "random-window-upper-bound"
CONTEXT_LIMIT = 4000


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return parsed


def _nonnegative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be a non-negative integer")
    return parsed


def _clip_context(text: str, limit: int = CONTEXT_LIMIT) -> str:
    if len(text) <= limit:
        return text
    marker = "\n...[truncated]...\n"
    keep = max(0, (limit - len(marker)) // 2)
    return f"{text[:keep]}{marker}{text[-keep:]}"


def _default_qec_code_bin() -> str:
    env_bin = os.environ.get("QEC_CODE_BIN")
    if env_bin:
        return env_bin

    suffix = ".exe" if os.name == "nt" else ""
    workspace_bin = ROOT / "target" / "debug" / f"qec-code{suffix}"
    if workspace_bin.exists():
        return str(workspace_bin)

    return "qec-code"


def _case_int(case: dict[str, Any], field: str) -> int:
    value = case[field]
    if type(value) is not int:
        raise TypeError(f'{case["case_id"]} field "{field}" must be an integer')
    return value


def _case_str(case: dict[str, Any], field: str) -> str:
    value = case[field]
    if not isinstance(value, str):
        raise TypeError(f'{case["case_id"]} field "{field}" must be a string')
    return value


def _build_command(
    qec_code_bin: str,
    code_id: str,
    seed: int,
    iterations: int,
    restarts: int,
    target_weight: int,
) -> list[str]:
    return [
        qec_code_bin,
        "code",
        "css-distance",
        METHOD,
        "--code-id",
        code_id,
        "--iterations",
        str(iterations),
        "--restarts",
        str(restarts),
        "--seed",
        str(seed),
        "--target-weight",
        str(target_weight),
        "--json",
    ]


def _row_prefix(
    case: dict[str, Any],
    command: list[str],
    seed: int,
    iterations: int,
    restarts: int,
    target_weight: int,
    elapsed_s: float,
) -> dict[str, Any]:
    return {
        "case_id": _case_str(case, "case_id"),
        "code_id": _case_str(case, "code_id"),
        "distance_side": _case_str(case, "distance_side"),
        "seed": seed,
        "iterations": iterations,
        "restarts": restarts,
        "target_weight": target_weight,
        "target_upper_bound": case.get("target_upper_bound"),
        "baseline_key": case.get("baseline_key"),
        "baseline_required": case.get("baseline_required"),
        "command": command,
        "elapsed_s": elapsed_s,
        "upper_bound": None,
        "raw_cli_json": None,
    }


def _classify_completed(
    row: dict[str, Any],
    completed: subprocess.CompletedProcess[str],
) -> dict[str, Any]:
    row["returncode"] = completed.returncode

    if completed.returncode != 0:
        row["status"] = "cli_error"
        row["stdout_context"] = _clip_context(completed.stdout)
        row["stderr_context"] = _clip_context(completed.stderr)
        return row

    try:
        parsed = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        row["status"] = "non_json_stdout"
        row["stdout_context"] = _clip_context(completed.stdout)
        row["stderr_context"] = _clip_context(completed.stderr)
        row["error"] = str(error)
        return row

    row["raw_cli_json"] = parsed
    if not isinstance(parsed, dict):
        row["status"] = "invalid_cli_json"
        row["error"] = "parsed CLI JSON must be an object"
        return row

    if parsed.get("status") != "completed":
        row["status"] = "cli_not_completed"
        return row

    if parsed.get("method") != METHOD:
        row["status"] = "unexpected_method"
        return row

    upper_bound = parsed.get("upper_bound")
    if "upper_bound" not in parsed:
        row["status"] = "missing_upper_bound"
        return row

    if type(upper_bound) is not int or upper_bound <= 0:
        row["status"] = "invalid_upper_bound"
        return row

    row["upper_bound"] = upper_bound
    row["status"] = "ok"
    return row


def _run_case_seed(
    case: dict[str, Any],
    qec_code_bin: str,
    seed: int,
    iterations: int,
    restarts: int,
    target_weight: int,
) -> dict[str, Any]:
    command = _build_command(qec_code_bin, _case_str(case, "code_id"), seed, iterations, restarts, target_weight)
    start = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        elapsed_s = time.perf_counter() - start
    except OSError as error:
        elapsed_s = time.perf_counter() - start
        row = _row_prefix(case, command, seed, iterations, restarts, target_weight, elapsed_s)
        row["status"] = "spawn_error"
        row["returncode"] = None
        row["error"] = str(error)
        row["stdout_context"] = ""
        row["stderr_context"] = ""
        return row

    row = _row_prefix(case, command, seed, iterations, restarts, target_weight, elapsed_s)
    return _classify_completed(row, completed)


def run(args: argparse.Namespace) -> int:
    try:
        manifest = load_manifest(args.cases)
    except Exception as error:
        print(f"{args.cases}: {error}", file=sys.stderr)
        return 2

    errors = validate_manifest(manifest)
    if errors:
        for error in errors:
            print(f"{args.cases}: {error}", file=sys.stderr)
        return 2

    cases = manifest["cases"]
    qec_code_bin = args.qec_code_bin or _default_qec_code_bin()
    rows_ok = True

    try:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        with args.out.open("w", encoding="utf-8") as handle:
            for case in cases:
                assert isinstance(case, dict)
                seeds = args.seeds if args.seeds is not None else [_case_int(case, "seed")]
                iterations = args.iterations if args.iterations is not None else _case_int(case, "iterations")
                restarts = args.restarts if args.restarts is not None else _case_int(case, "restarts")
                target_weight = (
                    args.target_weight if args.target_weight is not None else _case_int(case, "target_weight")
                )

                for seed in seeds:
                    row = _run_case_seed(
                        case,
                        qec_code_bin,
                        seed,
                        iterations,
                        restarts,
                        target_weight,
                    )
                    rows_ok = rows_ok and row["status"] == "ok"
                    handle.write(json.dumps(row, sort_keys=True) + "\n")
    except OSError as error:
        print(f"{args.out}: {error}", file=sys.stderr)
        return 2

    return 0 if rows_ok else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run local qec-code random-window upper-bound benchmarks."
    )
    parser.add_argument("--cases", required=True, type=Path, help="Path to a #321 case manifest.")
    parser.add_argument("--out", required=True, type=Path, help="Path to write JSONL results.")
    parser.add_argument("--seeds", nargs="+", type=_nonnegative_int, help="Override manifest seeds.")
    parser.add_argument("--iterations", type=_positive_int, help="Override manifest iterations.")
    parser.add_argument("--restarts", type=_positive_int, help="Override manifest restarts.")
    parser.add_argument("--target-weight", type=_positive_int, help="Override manifest target_weight.")
    parser.add_argument("--qec-code-bin", help="Path to qec-code executable. Defaults to QEC_CODE_BIN, target/debug/qec-code, then PATH.")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run GREEN verification**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_run_local -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases benchmarks.qec_code_random_window.tests.test_run_local -q
```

Expected: both commands pass with no warnings or stderr.

- [ ] **Step 5: Run the issue smoke command**

Run:

```bash
python3 -m benchmarks.qec_code_random_window.run_local \
  --cases benchmarks/qec_code_random_window/cases.smoke.toml \
  --out /tmp/qec-rw-smoke.jsonl \
  --seeds 7 \
  --iterations 50 \
  --restarts 1
```

Expected: exit 0. `/tmp/qec-rw-smoke.jsonl` contains four rows, all with
`status = "ok"`, `seed = 7`, positive `elapsed_s`, positive integer
`upper_bound`, and `raw_cli_json.method = "random-window-upper-bound"`.

- [ ] **Step 6: Commit**

Run:

```bash
git add benchmarks/qec_code_random_window/run_local.py benchmarks/qec_code_random_window/tests/test_run_local.py
git commit -m "benchmarks: add qec random-window local runner"
```

---

### Task 2: Final Verification And PR Readiness

**Files:**
- Read: `benchmarks/qec_code_random_window/run_local.py`
- Read: `benchmarks/qec_code_random_window/tests/test_run_local.py`
- Read: `/tmp/qec-rw-smoke.jsonl`

**Interfaces:**
- Consumes: committed runner and tests.
- Produces: verification evidence for the pull request.

- [ ] **Step 1: Re-run focused Python coverage**

Run:

```bash
python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases benchmarks.qec_code_random_window.tests.test_run_local -q
```

Expected: all tests pass.

- [ ] **Step 2: Re-run and inspect the smoke JSONL**

Run:

```bash
python3 -m benchmarks.qec_code_random_window.run_local \
  --cases benchmarks/qec_code_random_window/cases.smoke.toml \
  --out /tmp/qec-rw-smoke.jsonl \
  --seeds 7 \
  --iterations 50 \
  --restarts 1
python3 - <<'PY'
import json
from pathlib import Path

rows = [json.loads(line) for line in Path("/tmp/qec-rw-smoke.jsonl").read_text().splitlines()]
assert len(rows) == 4, len(rows)
for row in rows:
    assert row["status"] == "ok", row
    assert row["seed"] == 7, row
    assert row["elapsed_s"] > 0, row
    assert type(row["upper_bound"]) is int and row["upper_bound"] > 0, row
    assert row["raw_cli_json"]["method"] == "random-window-upper-bound", row
print("PASS")
PY
```

Expected: runner exits 0 and the inspection script prints `PASS`.

- [ ] **Step 3: Run qec-code crate tests**

Run:

```bash
cargo test -p qec-code
```

Expected: tests pass. If Cargo attempts network access and fails because the
sandbox proxy blocks crates.io, rerun with:

```bash
CARGO_NET_OFFLINE=true cargo test -p qec-code
```

Report both commands and outcomes.

- [ ] **Step 4: Run required full workspace tests**

Run:

```bash
cargo test
```

Expected: tests pass. If Cargo attempts network access and fails because the
sandbox proxy blocks crates.io, rerun with:

```bash
CARGO_NET_OFFLINE=true cargo test
```

Report both commands and outcomes.

- [ ] **Step 5: Inspect final diff and status**

Run:

```bash
git diff --stat origin/master..HEAD
git status --short
```

Expected: branch contains only the issue #322 design, plan, runner, and runner
tests; working tree is clean before PR creation.
