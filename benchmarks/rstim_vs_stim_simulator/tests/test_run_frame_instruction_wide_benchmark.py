from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
CASE_ID = "stim_surface_d11_r100"
SHOTS = 1024
SEED = 7
MEASUREMENT_BYTES = 1_552_384
DETECT_BITS = 12_000 + 1


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_fake_cli(path: Path, *, emit_telemetry: bool) -> Path:
    path.write_text(
        textwrap.dedent(
            f"""\
            #!{sys.executable}
            import json
            import sys
            from pathlib import Path

            EMIT_TELEMETRY = {emit_telemetry!r}
            MEASUREMENT_BYTES = {MEASUREMENT_BYTES}
            DETECT_BITS = {DETECT_BITS}

            def fail(message):
                print(message, file=sys.stderr)
                sys.exit(3)

            if sys.argv[1:] == ["--version"]:
                print("stim 1.15.0")
                sys.exit(0)
            if not sys.argv[1:]:
                print("rstim 0.1.1-test")
                sys.exit(0)

            argv = sys.argv[1:]
            telemetry_path = None
            if "--benchmark-telemetry-json" in argv:
                idx = argv.index("--benchmark-telemetry-json")
                telemetry_path = Path(argv[idx + 1])
                del argv[idx:idx + 2]

            if not argv:
                fail("missing command")
            command = argv[0]
            for flag in ("--shots", "--seed", "--out_format", "--in"):
                if flag not in argv:
                    fail(f"missing {{flag}}")
            if argv[argv.index("--shots") + 1] != "1024":
                fail("expected 1024 shots")
            if argv[argv.index("--seed") + 1] != "7":
                fail("expected seed 7")

            if command == "sample":
                if argv[argv.index("--out_format") + 1] != "b8":
                    fail("sample must use b8")
                if EMIT_TELEMETRY and telemetry_path is not None:
                    operations = []
                    for index in range(203):
                        targets = 122 if index == 202 else 120
                        operations.append({{
                            "operation": "X_ERROR",
                            "sampling_path": "sparse",
                            "targets": targets,
                            "iterator_builds": 1,
                            "attempt_count": targets * 1024,
                        }})
                    for _ in range(200):
                        operations.append({{
                            "operation": "DEPOLARIZE1",
                            "sampling_path": "sparse",
                            "targets": 60,
                            "iterator_builds": 1,
                            "attempt_count": 60 * 1024,
                        }})
                    for _ in range(400):
                        operations.append({{
                            "operation": "DEPOLARIZE2",
                            "sampling_path": "sparse",
                            "pairs": 110,
                            "iterator_builds": 1,
                            "attempt_count": 110 * 1024,
                        }})
                    telemetry_path.write_text(
                        json.dumps({{"operations": operations}}, sort_keys=True),
                        encoding="utf-8",
                    )
                sys.stdout.buffer.write(bytes(range(256)) * (MEASUREMENT_BYTES // 256))
                sys.exit(0)

            if command == "detect":
                if "--append_observables" not in argv:
                    fail("detect must append observables")
                if argv[argv.index("--out_format") + 1] != "01":
                    fail("detect must use 01")
                line = ("0" * DETECT_BITS + "\\n").encode("ascii")
                sys.stdout.buffer.write(line * 1024)
                sys.exit(0)

            fail(f"unsupported command: {{command}}")
            """
        ),
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path


class RunFrameInstructionWideBenchmarkTest(unittest.TestCase):
    def run_runner(
        self,
        out_dir: Path,
        *,
        rstim: Path,
        stim: Path,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                "-m",
                "benchmarks.rstim_vs_stim_simulator.run_frame_instruction_wide_benchmark",
                "--case",
                CASE_ID,
                "--manifest",
                str(MANIFEST),
                "--rstim",
                str(rstim),
                "--stim",
                str(stim),
                "--profile",
                "release",
                "--shots",
                str(SHOTS),
                "--seed",
                str(SEED),
                "--warmup-rounds",
                "0",
                "--measure-rounds",
                "1",
                "--out-dir",
                str(out_dir),
            ],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_fake_binary_without_telemetry_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            rstim = write_fake_cli(directory / "rstim", emit_telemetry=False)
            stim = write_fake_cli(directory / "stim", emit_telemetry=False)
            result = self.run_runner(directory / "out", rstim=rstim, stim=stim)

            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("missing benchmark telemetry", result.stderr)

    def test_fake_binary_with_telemetry_writes_checked_bundle_shape(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            rstim = write_fake_cli(directory / "rstim", emit_telemetry=True)
            stim = write_fake_cli(directory / "stim", emit_telemetry=False)
            out_dir = directory / "out"

            result = self.run_runner(out_dir, rstim=rstim, stim=stim)

            self.assertEqual(result.returncode, 0, result.stderr)
            expected = {
                "raw.jsonl",
                "summary.json",
                "report.md",
                "environment.json",
                "fixture-load.json",
                "correctness-summary.json",
                "artifact-sha256.json",
            }
            self.assertEqual({path.name for path in out_dir.iterdir()}, expected)

            raw = [json.loads(line) for line in (out_dir / "raw.jsonl").read_text().splitlines()]
            self.assertEqual([record["operation"] for record in raw], ["X_ERROR", "DEPOLARIZE1", "DEPOLARIZE2"])
            self.assertEqual(sum(record["iterator_builds"] for record in raw), 803)
            self.assertEqual(sum(record["attempt_count"] for record in raw), 82_290_688)
            self.assertEqual(raw[0]["targets"], 24_362)
            self.assertEqual(raw[1]["targets"], 12_000)
            self.assertEqual(raw[2]["pairs"], 44_000)

            summary = json.loads((out_dir / "summary.json").read_text(encoding="utf-8"))
            self.assertEqual(summary["totals"]["iterator_builds"], 803)
            self.assertEqual(summary["totals"]["attempt_count"], 82_290_688)
            self.assertEqual(summary["measurement"]["actual_output_bytes"], MEASUREMENT_BYTES)

            correctness = json.loads((out_dir / "correctness-summary.json").read_text(encoding="utf-8"))
            self.assertEqual(correctness["status"], "pass")
            self.assertEqual(correctness["mode"], "detect")
            self.assertEqual(correctness["detectors"], 12_000)
            self.assertEqual(correctness["observables"], 1)

            hash_manifest = json.loads((out_dir / "artifact-sha256.json").read_text(encoding="utf-8"))
            self.assertEqual(set(hash_manifest), expected - {"artifact-sha256.json"})
            for name, digest in hash_manifest.items():
                self.assertRegex(digest, r"^[0-9a-f]{64}$")
                self.assertEqual(digest, sha256_file(out_dir / name))


if __name__ == "__main__":
    unittest.main()
