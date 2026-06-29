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
        'manifest_version = 1\n'
        'suite = "qec_code_random_window"\n\n'
        + "\n\n".join(cases)
        + "\n",
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
