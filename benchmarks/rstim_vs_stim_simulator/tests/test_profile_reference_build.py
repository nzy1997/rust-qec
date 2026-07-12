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
DEFAULT_COUNTERS = {
    "measurement_reset_batches": 103,
    "canonical_materializations": 0,
    "canonical_writebacks": 0,
    "direct_inverse_batches": 103,
    "transposed_collapse_batches": 2,
    "collapse_pivots": 120,
    "expanded_repeat_iterations": 99,
    "executed_repeat_iterations": 1,
    "skipped_repeat_iterations": 98,
    "measurement_bits": 12121,
}


class ProfileReferenceBuildTest(unittest.TestCase):
    def _write_worker(
        self,
        directory: Path,
        *,
        include_counters: bool = True,
        phase_counters: object = DEFAULT_COUNTERS,
        response_overrides: dict[str, object] | None = None,
    ) -> Path:
        worker = directory / "worker.py"
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
                    response["phase_counters"] = {phase_counters!r}
                response.update({(response_overrides or {})!r})
                print(json.dumps(response), flush=True)
                """
            ),
            encoding="utf-8",
        )
        worker.chmod(0o755)
        return worker

    def _run_profile(self, directory: Path, worker: Path) -> subprocess.CompletedProcess[str]:
        fixture = directory / "fixture.stim"
        fixture.write_text("M 0\n", encoding="utf-8")
        return subprocess.run(
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

    def test_profile_command_writes_json_and_pass_line(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            worker = self._write_worker(directory)

            result = self._run_profile(directory, worker)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                result.stdout.strip(),
                "PASS reference phase profile batches=103 canonical=0 transposed=2 pivots=120 executed_repeats=1 skipped_repeats=98 bits=12121",
            )
            payload = json.loads((directory / "profile.json").read_text(encoding="utf-8"))
            self.assertEqual(payload["protocol"], PROTOCOL)
            self.assertEqual(payload["backend"], "packed_inverse")
            self.assertEqual(payload["phase_counters"]["measurement_reset_batches"], 103)
            self.assertEqual(payload["phase_counters"]["canonical_writebacks"], 0)

    def test_profile_command_rejects_missing_phase_counters(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            worker = self._write_worker(directory, include_counters=False)
            result = self._run_profile(directory, worker)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("phase_counters", result.stderr)

    def test_profile_command_rejects_malformed_build_response_identity(self) -> None:
        cases = (
            ({"protocol": "wrong-protocol"}, "build response protocol"),
            ({"type": "loaded"}, "build response type"),
            ({"request_id": 1}, "build response request_id"),
            ({"request_id": False}, "build response request_id"),
        )
        for overrides, message in cases:
            with self.subTest(overrides=overrides):
                with tempfile.TemporaryDirectory() as temp_dir:
                    directory = Path(temp_dir)
                    worker = self._write_worker(
                        directory, response_overrides=overrides
                    )
                    result = self._run_profile(directory, worker)

                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn(message, result.stderr)

    def test_profile_command_rejects_malformed_phase_counter_payloads(self) -> None:
        missing_key = dict(DEFAULT_COUNTERS)
        del missing_key["canonical_writebacks"]
        cases = (
            ([], "phase_counters must be a dictionary"),
            (missing_key, "phase_counters missing 'canonical_writebacks'"),
            (
                {**DEFAULT_COUNTERS, "measurement_bits": True},
                "phase_counters['measurement_bits'] must be a nonnegative integer",
            ),
            (
                {**DEFAULT_COUNTERS, "collapse_pivots": -1},
                "phase_counters['collapse_pivots'] must be a nonnegative integer",
            ),
            (
                {**DEFAULT_COUNTERS, "direct_inverse_batches": "0"},
                "phase_counters['direct_inverse_batches'] must be a nonnegative integer",
            ),
        )
        for counters, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as temp_dir:
                    directory = Path(temp_dir)
                    worker = self._write_worker(directory, phase_counters=counters)
                    result = self._run_profile(directory, worker)

                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn(message, result.stderr)


if __name__ == "__main__":
    unittest.main()
