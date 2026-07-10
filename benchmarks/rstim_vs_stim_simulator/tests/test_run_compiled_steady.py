from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "fair_cli_cases.toml"


class RunCompiledSteadyTest(unittest.TestCase):
    def _write_fake_workers(self, directory: Path, *, mode: str) -> dict[str, Path]:
        paths: dict[str, Path] = {}
        for variant in ("stim", "rstim"):
            path = directory / f"{variant}_worker.py"
            path.write_text(
                textwrap.dedent(
                    f"""
                    import argparse
                    import hashlib
                    import json
                    import struct
                    import sys
                    import time

                    from benchmarks.rstim_vs_stim_simulator import run_compiled_steady as protocol

                    parser = argparse.ArgumentParser()
                    parser.add_argument("--input", required=True)
                    parser.add_argument("--seed", required=True)
                    args = parser.parse_args()
                    input_bytes = open(args.input, "rb").read()
                    is_preflight = input_bytes == b"X 0\\nM 0\\n"
                    telemetry = {{
                        "variant": "{variant}",
                        "compile_count": 1,
                        "reference_build_count": 1,
                        "sample_call_count": 0,
                        "fixture_sha256": hashlib.sha256(input_bytes).hexdigest(),
                        "measurement_count": 1 if is_preflight else 12121,
                        "bytes_per_shot": 1 if is_preflight else 1516,
                    }}
                    protocol.write_frame(sys.stdout.buffer, protocol.READY, json.dumps(telemetry).encode())
                    calls = 0
                    while True:
                        frame_type, payload = protocol.read_frame(sys.stdin.buffer)
                        if frame_type == protocol.STOP:
                            telemetry["sample_call_count"] = calls
                            protocol.write_frame(sys.stdout.buffer, protocol.FINAL, json.dumps(telemetry).encode())
                            sys.stdout.buffer.flush()
                            if {mode!r} == "final-then-nonzero":
                                sys.exit(7)
                            break
                        if frame_type != protocol.SAMPLE:
                            raise RuntimeError(f"unexpected frame: {{frame_type!r}}")
                        request = json.loads(payload)
                        calls += 1
                        data = b"\\x00" if {mode!r} == "bad-known-answer" and is_preflight else (
                            b"\\x01" if is_preflight else b"\\x00" * 1552384
                        )
                        result = struct.pack("<QQ", request["request_id"], calls) + data
                        if {mode!r} == "delay-last-byte" and not is_preflight:
                            header = protocol.RESULT + struct.pack("<Q", len(result))
                            sys.stdout.buffer.write(header + result[:-1])
                            sys.stdout.buffer.flush()
                            time.sleep(0.15)
                            sys.stdout.buffer.write(result[-1:])
                            sys.stdout.buffer.flush()
                        else:
                            protocol.write_frame(sys.stdout.buffer, protocol.RESULT, result)
                            sys.stdout.buffer.flush()
                    """
                ),
                encoding="utf-8",
            )
            paths[variant] = path
        return paths

    def _run_runner(
        self,
        out_dir: Path,
        *,
        stim_worker: list[str],
        rstim_worker: list[str],
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                "-m",
                "benchmarks.rstim_vs_stim_simulator.run_compiled_steady",
                "--manifest",
                str(MANIFEST),
                "--case",
                "stim_surface_d11_r100",
                "--profile",
                "release",
                "--warmup-rounds",
                "2",
                "--measure-rounds",
                "7",
                "--seed",
                "0",
                "--out-dir",
                str(out_dir),
                "--stim-worker-command",
                json.dumps(stim_worker),
                "--rstim-worker-command",
                json.dumps(rstim_worker),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def _run_fake_mode(self, mode: str) -> tuple[subprocess.CompletedProcess[str], Path, tempfile.TemporaryDirectory[str]]:
        temp_dir = tempfile.TemporaryDirectory()
        directory = Path(temp_dir.name)
        paths = self._write_fake_workers(directory, mode=mode)
        out_dir = directory / "out"
        result = self._run_runner(
            out_dir,
            stim_worker=[sys.executable, str(paths["stim"])],
            rstim_worker=[sys.executable, str(paths["rstim"])],
        )
        return result, out_dir, temp_dir

    def test_fake_workers_emit_required_lifecycle_and_summary(self) -> None:
        result, out_dir, temp_dir = self._run_fake_mode("ok")
        self.addCleanup(temp_dir.cleanup)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "PASS compiled steady-state lifecycle variants=2 compile=1 reference=1 calls=9 measured=14",
            result.stdout,
        )
        raw = [json.loads(line) for line in (out_dir / "raw.jsonl").read_text().splitlines()]
        self.assertEqual(sum(1 for record in raw if record["record_type"] == "ready"), 2)
        self.assertEqual(sum(1 for record in raw if record["record_type"] == "sample"), 18)
        self.assertEqual(sum(1 for record in raw if record["record_type"] == "final"), 2)
        summary = json.loads((out_dir / "summary.json").read_text())
        self.assertEqual(summary["measured_records"], 14)
        self.assertEqual({variant["sample_count"] for variant in summary["variants"]}, {7})

    def test_sample_timing_includes_delayed_final_result_byte(self) -> None:
        result, out_dir, temp_dir = self._run_fake_mode("delay-last-byte")
        self.addCleanup(temp_dir.cleanup)

        self.assertEqual(result.returncode, 0, result.stderr)
        raw = [json.loads(line) for line in (out_dir / "raw.jsonl").read_text().splitlines()]
        elapsed = [record["elapsed_ns"] for record in raw if record["record_type"] == "sample"]
        self.assertGreaterEqual(max(elapsed), 140_000_000)

    def test_nonzero_exit_after_final_rejects_before_summary(self) -> None:
        result, out_dir, temp_dir = self._run_fake_mode("final-then-nonzero")
        self.addCleanup(temp_dir.cleanup)

        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((out_dir / "summary.json").exists())

    def test_known_answer_zero_fails_before_canonical_timing(self) -> None:
        result, out_dir, temp_dir = self._run_fake_mode("bad-known-answer")
        self.addCleanup(temp_dir.cleanup)

        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((out_dir / "raw.jsonl").exists())


if __name__ == "__main__":
    unittest.main()
