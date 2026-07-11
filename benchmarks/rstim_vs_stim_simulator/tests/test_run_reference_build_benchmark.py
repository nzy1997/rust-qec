from __future__ import annotations

import base64
import hashlib
import json
import platform
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from statistics import median

from benchmarks.rstim_vs_stim_simulator import run_reference_build_benchmark


ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
MANIFEST = ROOT / "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
PROTOCOL = "reference-build-v1"
TIMER_SCOPE = "reference_build_only"
REFERENCE_DIGEST = "d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d"
MANIFEST_DIGEST = "9fc35393f362f709e90bfd64ab08eda5140844974a7e685fd1e5614f67e0c921"
MEASUREMENT_BITS = 12121
PACKED_BYTES = 1516
STIM_VARIANT = "stim-reference-b8"
RSTIM_VARIANT = "rstim-packed-reference-b8"


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_jsonl(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def command_stdout(command: list[str]) -> str:
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise AssertionError(f"{command!r} failed: {result.stderr}")
    return result.stdout.strip()


class RunReferenceBuildBenchmarkTest(unittest.TestCase):
    def _write_fake_worker(
        self,
        directory: Path,
        *,
        backend: str,
        packed: bytes | None = None,
        launched_marker: Path | None = None,
    ) -> Path:
        if packed is None:
            packed = b"\x00" * PACKED_BYTES
        marker_value = str(launched_marker) if launched_marker is not None else None
        path = directory / f"{backend}_worker.py"
        path.write_text(
            textwrap.dedent(
                f"""\
                #!{sys.executable}
                import argparse
                import base64
                import json
                import sys
                from pathlib import Path

                PROTOCOL = {PROTOCOL!r}
                DIGEST = {REFERENCE_DIGEST!r}
                PACKED = {packed!r}
                LAUNCHED_MARKER = {marker_value!r}

                parser = argparse.ArgumentParser()
                parser.add_argument("--protocol", required=True)
                args = parser.parse_args()
                if args.protocol != PROTOCOL:
                    raise SystemExit(f"wrong protocol: {{args.protocol}}")
                if LAUNCHED_MARKER is not None:
                    Path(LAUNCHED_MARKER).write_text("launched", encoding="utf-8")

                load = json.loads(sys.stdin.readline())
                if load.get("protocol") != PROTOCOL or load.get("type") != "load":
                    raise SystemExit(f"unexpected load request: {{load!r}}")
                with open(load["fixture_path"], "rb") as handle:
                    handle.read()
                print(json.dumps({{
                    "protocol": PROTOCOL,
                    "type": "loaded",
                    "parse_count": 1,
                    "measurement_bits": {MEASUREMENT_BITS},
                }}), flush=True)

                reference_build_count = 0
                for expected_request_id in range(9):
                    request = json.loads(sys.stdin.readline())
                    if request.get("protocol") != PROTOCOL or request.get("type") != "build_reference":
                        raise SystemExit(f"unexpected build request: {{request!r}}")
                    if request.get("request_id") != expected_request_id:
                        raise SystemExit(f"wrong request id: {{request!r}}")
                    reference_build_count += 1
                    print(json.dumps({{
                        "protocol": PROTOCOL,
                        "type": "reference_built",
                        "request_id": expected_request_id,
                        "backend": {backend!r},
                        "parse_count": 1,
                        "reference_build_count": reference_build_count,
                        "measurement_bits": {MEASUREMENT_BITS},
                        "packed_bytes": {PACKED_BYTES},
                        "packed_base64": base64.b64encode(PACKED).decode("ascii"),
                        "byte_sha256": DIGEST,
                        "timer_scope": {TIMER_SCOPE!r},
                        "elapsed_ns": 1000 + expected_request_id,
                    }}), flush=True)
                """
            ),
            encoding="utf-8",
        )
        path.chmod(0o755)
        return path

    def _write_stim_python_launcher(
        self,
        directory: Path,
        worker: Path,
        *,
        stim_version: str = "1.15.0",
    ) -> Path:
        launcher = directory / "stim_python.py"
        launcher.write_text(
            textwrap.dedent(
                f"""\
                #!{sys.executable}
                import os
                import sys

                argv = sys.argv[1:]
                if argv and argv[0] == "-c":
                    print({stim_version!r})
                    raise SystemExit(0)
                if len(argv) >= 2 and argv[0] == "-m":
                    argv = argv[2:]
                os.execv(sys.executable, [sys.executable, {str(worker)!r}, *argv])
                """
            ),
            encoding="utf-8",
        )
        launcher.chmod(0o755)
        return launcher

    def _run_runner(
        self,
        out_dir: Path,
        *,
        manifest: Path,
        stim_python: Path,
        rstim_worker: Path,
        warmup_rounds: int = 2,
        measure_rounds: int = 7,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                "-m",
                "benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark",
                "--fixture",
                str(FIXTURE),
                "--manifest",
                str(manifest),
                "--stim-python",
                str(stim_python),
                "--rstim-worker",
                str(rstim_worker),
                "--warmup-rounds",
                str(warmup_rounds),
                "--measure-rounds",
                str(measure_rounds),
                "--out-dir",
                str(out_dir),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_fake_workers_emit_required_artifacts_and_hash_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            stim_worker = self._write_fake_worker(directory, backend="stim_reference")
            rstim_worker = self._write_fake_worker(directory, backend="packed_inverse")
            stim_python = self._write_stim_python_launcher(directory, stim_worker)
            out_dir = directory / "out"

            expected_git_commit = command_stdout(["git", "rev-parse", "HEAD"])
            expected_git_dirty = bool(command_stdout(["git", "status", "--porcelain"]))
            result = self._run_runner(
                out_dir,
                manifest=MANIFEST,
                stim_python=stim_python,
                rstim_worker=rstim_worker,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            expected_files = {
                "raw.jsonl",
                "summary.json",
                "report.md",
                "environment.json",
                "artifact-sha256.json",
            }
            self.assertEqual({path.name for path in out_dir.iterdir()}, expected_files)

            hash_manifest = json.loads((out_dir / "artifact-sha256.json").read_text(encoding="utf-8"))
            self.assertEqual(set(hash_manifest), expected_files - {"artifact-sha256.json"})
            for name, digest in hash_manifest.items():
                self.assertRegex(digest, r"^[0-9a-f]{64}$")
                self.assertEqual(digest, sha256_file(out_dir / name))

            raw = load_jsonl(out_dir / "raw.jsonl")
            self.assertEqual(len(raw), 18)
            self.assertEqual({record["variant"] for record in raw}, {STIM_VARIANT, RSTIM_VARIANT})
            packed = b"\x00" * PACKED_BYTES
            for variant in (STIM_VARIANT, RSTIM_VARIANT):
                records = [record for record in raw if record["variant"] == variant]
                self.assertEqual([record["round"] for record in records], list(range(9)))
                self.assertEqual(
                    [record["phase"] for record in records],
                    ["warmup", "warmup", *["measured"] * 7],
                )
                self.assertEqual(
                    [record["reference_build_count"] for record in records],
                    list(range(1, 10)),
                )
                for record in records:
                    self.assertGreater(record["elapsed_ns"], 0)
                    self.assertEqual(record["protocol"], PROTOCOL)
                    self.assertEqual(record["timer_scope"], TIMER_SCOPE)
                    self.assertEqual(record["parse_count"], 1)
                    self.assertEqual(record["measurement_bits"], MEASUREMENT_BITS)
                    self.assertEqual(record["packed_bytes"], PACKED_BYTES)
                    self.assertEqual(record["byte_sha256"], REFERENCE_DIGEST)
                    self.assertEqual(base64.b64decode(record["packed_base64"]), packed)
            self.assertEqual(
                {record["variant"]: record["backend"] for record in raw if record["round"] == 0},
                {
                    STIM_VARIANT: "stim_reference",
                    RSTIM_VARIANT: "packed_inverse",
                },
            )

            summary = json.loads((out_dir / "summary.json").read_text(encoding="utf-8"))
            self.assertEqual(summary["protocol"], PROTOCOL)
            self.assertEqual(summary["timer_scope"], TIMER_SCOPE)
            self.assertEqual(summary["measured_records"], 14)
            summary_by_variant = {variant["variant"]: variant for variant in summary["variants"]}
            self.assertEqual(set(summary_by_variant), {STIM_VARIANT, RSTIM_VARIANT})
            expected_backends = {
                STIM_VARIANT: "stim_reference",
                RSTIM_VARIANT: "packed_inverse",
            }
            for variant_name, expected_backend in expected_backends.items():
                variant = summary_by_variant[variant_name]
                measured_rows = [
                    record
                    for record in raw
                    if record["variant"] == variant_name and record["phase"] == "measured"
                ]
                measured_elapsed_ns = [record["elapsed_ns"] for record in measured_rows]
                self.assertEqual(len(measured_rows), 7)
                self.assertEqual(variant["count"], len(measured_rows))
                self.assertEqual(variant["min_elapsed_ns"], min(measured_elapsed_ns))
                self.assertEqual(variant["median_elapsed_ns"], int(median(measured_elapsed_ns)))
                self.assertEqual(variant["max_elapsed_ns"], max(measured_elapsed_ns))
                self.assertEqual(variant["backend"], expected_backend)
                self.assertEqual(variant["measurement_bits"], MEASUREMENT_BITS)
                self.assertEqual(variant["packed_bytes"], PACKED_BYTES)
                self.assertEqual(variant["byte_sha256"], REFERENCE_DIGEST)
                self.assertEqual(variant["parse_count"], 1)
                self.assertEqual(variant["final_reference_build_count"], 9)

            report = (out_dir / "report.md").read_text(encoding="utf-8")
            self.assertIn(
                "| variant | count | min_elapsed_ns | median_elapsed_ns | max_elapsed_ns | backend | parse_count | final_reference_build_count | byte_sha256 |",
                report,
            )
            self.assertIn(REFERENCE_DIGEST, report)
            for variant in summary["variants"]:
                report_row = (
                    f"| {variant['variant']} | {variant['count']} | {variant['min_elapsed_ns']} | "
                    f"{variant['median_elapsed_ns']} | {variant['max_elapsed_ns']} | {variant['backend']} | "
                    f"{variant['parse_count']} | {variant['final_reference_build_count']} | {variant['byte_sha256']} |"
                )
                self.assertIn(report_row, report)

            environment = json.loads((out_dir / "environment.json").read_text(encoding="utf-8"))
            for key in (
                "profile",
                "protocol",
                "timer_scope",
                "seed_policy",
                "fixture_path",
                "fixture_sha256",
                "manifest_path",
                "manifest_sha256",
                "stim_version",
                "worker_argv",
                "canonical_worker_argv",
                "runner_argv",
                "runner_python_executable",
                "runner_python_executable_sha256",
                "warmup_rounds",
                "measure_rounds",
                "git_commit",
                "git_dirty",
                "os",
                "cpu_model",
                "python_executable",
                "python_executable_sha256",
                "rstim_worker_binary_path",
                "rstim_worker_binary_sha256",
                "rustc_version",
                "cargo_version",
                "python_version",
            ):
                self.assertIn(key, environment)
            expected_runner_argv = [
                sys.executable,
                "-m",
                "benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark",
                "--fixture",
                str(FIXTURE),
                "--manifest",
                str(MANIFEST),
                "--stim-python",
                str(stim_python),
                "--rstim-worker",
                str(rstim_worker),
                "--warmup-rounds",
                "2",
                "--measure-rounds",
                "7",
                "--out-dir",
                str(out_dir),
            ]
            expected_worker_argv = {
                STIM_VARIANT: [
                    str(stim_python),
                    "-m",
                    "benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build",
                    "--protocol",
                    PROTOCOL,
                ],
                RSTIM_VARIANT: [str(rstim_worker), "--protocol", PROTOCOL],
            }
            expected_canonical_worker_argv = {
                STIM_VARIANT: [
                    "python3",
                    "-m",
                    "benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build",
                    "--protocol",
                    PROTOCOL,
                ],
                RSTIM_VARIANT: [
                    "target/release/rstim_reference_build_worker",
                    "--protocol",
                    PROTOCOL,
                ],
            }
            self.assertEqual(environment["profile"], "release")
            self.assertEqual(environment["protocol"], PROTOCOL)
            self.assertEqual(environment["timer_scope"], TIMER_SCOPE)
            self.assertEqual(environment["seed_policy"], "deterministic_no_seed_reference_builds")
            self.assertEqual(environment["fixture_path"], str(FIXTURE.resolve()))
            self.assertEqual(environment["fixture_sha256"], sha256_file(FIXTURE))
            self.assertEqual(environment["manifest_path"], str(MANIFEST.resolve()))
            self.assertEqual(environment["manifest_sha256"], MANIFEST_DIGEST)
            self.assertEqual(environment["stim_version"], "1.15.0")
            self.assertEqual(environment["runner_argv"], expected_runner_argv)
            expected_runner_python = Path(sys.executable).resolve()
            self.assertEqual(environment["runner_python_executable"], str(expected_runner_python))
            self.assertEqual(environment["runner_python_executable_sha256"], sha256_file(expected_runner_python))
            self.assertEqual(environment["worker_argv"], expected_worker_argv)
            self.assertEqual(environment["canonical_worker_argv"], expected_canonical_worker_argv)
            self.assertEqual(environment["python_executable"], str(stim_python.resolve()))
            self.assertEqual(environment["python_executable_sha256"], sha256_file(stim_python))
            self.assertEqual(environment["rstim_worker_binary_path"], str(rstim_worker.resolve()))
            self.assertEqual(environment["rstim_worker_binary_sha256"], sha256_file(rstim_worker))
            self.assertEqual(environment["rustc_version"], command_stdout(["rustc", "--version"]))
            self.assertEqual(environment["cargo_version"], command_stdout(["cargo", "--version"]))
            self.assertEqual(environment["python_version"], platform.python_version())
            self.assertEqual(environment["os"], platform.platform())
            self.assertIsInstance(environment["cpu_model"], str)
            self.assertTrue(environment["cpu_model"].strip())
            self.assertIsInstance(environment["git_commit"], str)
            self.assertRegex(environment["git_commit"], r"^[0-9a-f]{40}$")
            self.assertEqual(environment["git_commit"], expected_git_commit)
            self.assertIsInstance(environment["git_dirty"], bool)
            self.assertEqual(environment["git_dirty"], expected_git_dirty)
            self.assertEqual(environment["warmup_rounds"], 2)
            self.assertEqual(environment["measure_rounds"], 7)

    def test_runner_rejects_bad_decoded_packed_payload_before_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            bad_payload = b"\x01" + (b"\x00" * (PACKED_BYTES - 1))
            stim_worker = self._write_fake_worker(
                directory,
                backend="stim_reference",
                packed=bad_payload,
            )
            rstim_worker = self._write_fake_worker(directory, backend="packed_inverse")
            stim_python = self._write_stim_python_launcher(directory, stim_worker)
            out_dir = directory / "out"

            result = self._run_runner(
                out_dir,
                manifest=MANIFEST,
                stim_python=stim_python,
                rstim_worker=rstim_worker,
            )

            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("decoded packed bytes SHA-256", result.stderr)
            self.assertFalse(out_dir.exists(), "runner wrote artifacts after rejecting packed bytes")

    def test_runner_rejects_noncanonical_round_counts(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            stim_worker = self._write_fake_worker(directory, backend="stim_reference")
            rstim_worker = self._write_fake_worker(directory, backend="packed_inverse")
            stim_python = self._write_stim_python_launcher(directory, stim_worker)

            for warmup_rounds, measure_rounds in ((1, 7), (2, 6)):
                with self.subTest(warmup_rounds=warmup_rounds, measure_rounds=measure_rounds):
                    out_dir = directory / f"out-{warmup_rounds}-{measure_rounds}"
                    result = self._run_runner(
                        out_dir,
                        manifest=MANIFEST,
                        stim_python=stim_python,
                        rstim_worker=rstim_worker,
                        warmup_rounds=warmup_rounds,
                        measure_rounds=measure_rounds,
                    )

                    self.assertNotEqual(result.returncode, 0, result.stdout)
                    self.assertIn("requires --warmup-rounds 2 --measure-rounds 7", result.stderr)
                    self.assertFalse(out_dir.exists(), "runner wrote artifacts for noncanonical counts")

    def test_runner_rejects_wrong_stim_version_before_launching_workers(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            launched = directory / "worker-launched"
            stim_worker = self._write_fake_worker(
                directory,
                backend="stim_reference",
                launched_marker=launched,
            )
            rstim_worker = self._write_fake_worker(
                directory,
                backend="packed_inverse",
                launched_marker=launched,
            )
            stim_python = self._write_stim_python_launcher(
                directory,
                stim_worker,
                stim_version="1.14.0",
            )
            out_dir = directory / "out"

            result = self._run_runner(
                out_dir,
                manifest=MANIFEST,
                stim_python=stim_python,
                rstim_worker=rstim_worker,
            )

            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("requires stim==1.15.0, got 1.14.0", result.stderr)
            self.assertFalse(launched.exists(), "runner launched workers before rejecting Stim version")
            self.assertFalse(out_dir.exists(), "runner wrote artifacts after rejecting Stim version")

    def test_default_worker_argvs_match_reference_build_protocol(self) -> None:
        self.assertEqual(run_reference_build_benchmark.PROTOCOL, PROTOCOL)
        self.assertEqual(run_reference_build_benchmark.TIMER_SCOPE, TIMER_SCOPE)
        self.assertEqual(run_reference_build_benchmark.EXPECTED_MANIFEST_SHA256, MANIFEST_DIGEST)
        self.assertEqual(run_reference_build_benchmark.EXPECTED_REFERENCE_SHA256, REFERENCE_DIGEST)
        self.assertEqual(
            run_reference_build_benchmark.default_stim_worker_argv("python3"),
            [
                "python3",
                "-m",
                "benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build",
                "--protocol",
                PROTOCOL,
            ],
        )
        self.assertEqual(
            run_reference_build_benchmark.default_rstim_worker_argv("target/release/rstim_reference_build_worker"),
            [
                "target/release/rstim_reference_build_worker",
                "--protocol",
                PROTOCOL,
            ],
        )

    def test_runner_rejects_wrong_manifest_hash_before_launching_workers(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            directory = Path(temp_dir)
            bad_manifest = directory / "cases.full.toml"
            bad_manifest.write_text(MANIFEST.read_text(encoding="utf-8") + "\n# wrong digest\n", encoding="utf-8")
            launched = directory / "worker-launched"
            worker = directory / "worker.py"
            worker.write_text(
                textwrap.dedent(
                    f"""\
                    #!{sys.executable}
                    from pathlib import Path
                    Path({str(launched)!r}).write_text("launched", encoding="utf-8")
                    raise SystemExit(17)
                    """
                ),
                encoding="utf-8",
            )
            worker.chmod(0o755)
            stim_python = self._write_stim_python_launcher(directory, worker)

            result = self._run_runner(
                directory / "out",
                manifest=bad_manifest,
                stim_python=stim_python,
                rstim_worker=worker,
            )

            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("manifest SHA-256", result.stderr)
            self.assertIn(MANIFEST_DIGEST, result.stderr)
            self.assertFalse(launched.exists(), "runner launched workers before rejecting manifest hash")


if __name__ == "__main__":
    unittest.main()
