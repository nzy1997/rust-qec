from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
PROTOCOL = "reference-build-v1"


class ProfileReferenceBuildTest(unittest.TestCase):
    def _write_worker(self, directory: Path, *, include_counters: bool = True) -> Path:
        worker = directory / "worker.py"
        counters_literal = {
            "measurement_reset_batches": 103,
            "canonical_materializations": 103,
            "canonical_writebacks": 2,
            "direct_inverse_batches": 0,
            "transposed_collapse_batches": 0,
            "collapse_pivots": 120,
            "expanded_repeat_iterations": 99,
            "measurement_bits": 12121,
        }
        worker.write_text(
            textwrap.dedent(
                f"""\
                #!{sys.executable}
                import argparse
                import json
                import sys

                parser = argparse.ArgumentParser()
                parser.add_argument("--protocol", required=True)
                args = parser.parse_args()
                assert args.protocol == {PROTOCOL!r}
                load = json.loads(sys.stdin.readline())
                print(json.dumps({{"protocol": {PROTOCOL!r}, "type": "loaded", "parse_count": 1, "measurement_bits": 12121}}), flush=True)
                build = json.loads(sys.stdin.readline())
                if build.get("include_phase_counters") is not True:
                    raise SystemExit("missing opt-in")
                response = {{
                    "protocol": {PROTOCOL!r},
                    "type": "reference_built",
                    "request_id": 0,
                    "backend": "packed_inverse",
                    "parse_count": 1,
                    "reference_build_count": 1,
                    "measurement_bits": 12121,
                    "packed_bytes": 1516,
                    "packed_base64": "AA==",
                    "byte_sha256": "0" * 64,
                    "timer_scope": "reference_build_only",
                    "elapsed_ns": 1,
                }}
                if {include_counters!r}:
                    response["phase_counters"] = {counters_literal!r}
                print(json.dumps(response), flush=True)
                """
            ),
            encoding="utf-8",
        )
        worker.chmod(0o755)
        return worker

    def test_profile_command_writes_json_and_pass_line(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            fixture = directory / "fixture.stim"
            fixture.write_text("M 0\n", encoding="utf-8")
            worker = self._write_worker(directory)
            out = directory / "profile.json"

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "benchmarks.rstim_vs_stim_simulator.profile_reference_build",
                    "--fixture",
                    str(fixture),
                    "--worker",
                    str(worker),
                    "--out",
                    str(out),
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                result.stdout.strip(),
                "PASS reference phase profile batches=103 canonical=103 writebacks=2 repeats=99 bits=12121",
            )
            payload = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(payload["protocol"], PROTOCOL)
            self.assertEqual(payload["backend"], "packed_inverse")
            self.assertEqual(payload["phase_counters"]["measurement_reset_batches"], 103)
            self.assertEqual(payload["phase_counters"]["canonical_writebacks"], 2)

    def test_profile_command_rejects_missing_phase_counters(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            fixture = directory / "fixture.stim"
            fixture.write_text("M 0\n", encoding="utf-8")
            worker = self._write_worker(directory, include_counters=False)
            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "benchmarks.rstim_vs_stim_simulator.profile_reference_build",
                    "--fixture",
                    str(fixture),
                    "--worker",
                    str(worker),
                    "--out",
                    str(directory / "profile.json"),
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("phase_counters", result.stderr)


if __name__ == "__main__":
    unittest.main()
