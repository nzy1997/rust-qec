from __future__ import annotations

import json
import contextlib
import hashlib
import io
import subprocess
import sys
import tempfile
import textwrap
import unittest
import unittest.mock
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator import run_compiled_steady


ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "fair_cli_cases.toml"


class RunCompiledSteadyTest(unittest.TestCase):
    def test_build_rstim_worker_uses_locked_cargo_resolution(self) -> None:
        completed = subprocess.CompletedProcess(["cargo"], 0, "", "")
        with unittest.mock.patch(
            "benchmarks.rstim_vs_stim_simulator.run_compiled_steady.subprocess.run",
            return_value=completed,
        ) as run:
            self.assertEqual(
                run_compiled_steady.build_rstim_worker("debug"),
                ["target/debug/rstim_compiled_steady_worker"],
            )

        self.assertEqual(
            run.call_args.args[0],
            ["cargo", "build", "--locked", "-p", "rstim", "--bin", "rstim_compiled_steady_worker"],
        )

    def test_rstim_worker_returns_packed_known_answer_and_lifecycle_telemetry(self) -> None:
        build = subprocess.run(
            ["cargo", "build", "--locked", "-p", "rstim", "--bin", "rstim_compiled_steady_worker"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(build.returncode, 0, build.stderr)
        version = subprocess.run(
            [*run_compiled_steady.default_rstim_worker_command("debug"), "--version"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(version.returncode, 0, version.stderr)
        self.assertRegex(version.stdout.strip(), r"^rstim 0\.2\.0")

        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = Path(temp_dir) / "known_answer.stim"
            fixture.write_text("X 0\nM 0\n", encoding="utf-8")
            session = run_compiled_steady.WorkerSession(
                [
                    *run_compiled_steady.default_rstim_worker_command("debug"),
                    "--variant",
                    "rstim-precompiled",
                ],
                input_path=fixture,
                seed=0,
            )
            try:
                ready = session.read_ready()
                self.assertEqual(ready["variant"], "rstim-precompiled")
                self.assertGreater(ready["precompile_elapsed_ns"], 0)
                self.assertEqual(ready["compile_count"], 1)
                self.assertEqual(ready["reference_build_count"], 1)
                self.assertEqual(ready["sample_call_count"], 0)
                self.assertEqual(ready["fixture_sha256"], hashlib.sha256(fixture.read_bytes()).hexdigest())
                self.assertEqual(ready["measurement_count"], 1)
                self.assertEqual(ready["bytes_per_shot"], 1)

                call_count, data, sample_elapsed_ns, b8_elapsed_ns = session.sample(0, 1)
                self.assertEqual(call_count, 1)
                self.assertEqual(data, b"\x01")
                self.assertGreaterEqual(sample_elapsed_ns, 0)
                self.assertGreaterEqual(b8_elapsed_ns, 0)

                final = session.stop()
                self.assertEqual(final["compile_count"], 1)
                self.assertEqual(final["reference_build_count"], 1)
                self.assertEqual(final["sample_call_count"], 1)
            except BaseException:
                session.abort()
                raise
            finally:
                session.stdout.close()
                session.stderr.close()

    def test_rstim_worker_emits_error_frame_for_missing_input(self) -> None:
        build = subprocess.run(
            ["cargo", "build", "--locked", "-p", "rstim", "--bin", "rstim_compiled_steady_worker"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(build.returncode, 0, build.stderr)

        with tempfile.TemporaryDirectory() as temp_dir:
            missing_fixture = Path(temp_dir) / "missing.stim"
            process = subprocess.Popen(
                [
                    *run_compiled_steady.default_rstim_worker_command("debug"),
                    "--variant",
                    "rstim-precompiled",
                    "--input",
                    str(missing_fixture),
                    "--seed",
                    "0",
                ],
                cwd=ROOT,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            assert process.stdout is not None
            frame_type, payload = run_compiled_steady.read_frame(process.stdout)
            stdout_remainder, stderr = process.communicate(timeout=10)

        self.assertEqual(frame_type, run_compiled_steady.ERROR)
        self.assertIn("missing.stim", payload.decode(errors="replace"))
        self.assertEqual(stdout_remainder, b"")
        self.assertNotEqual(process.returncode, 0)
        self.assertIn("missing.stim", stderr.decode(errors="replace"))

    def test_rstim_worker_emits_error_frame_for_missing_required_args(self) -> None:
        build = subprocess.run(
            ["cargo", "build", "--locked", "-p", "rstim", "--bin", "rstim_compiled_steady_worker"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(build.returncode, 0, build.stderr)

        process = subprocess.Popen(
            [
                *run_compiled_steady.default_rstim_worker_command("debug"),
                "--variant",
                "rstim-precompiled",
            ],
            cwd=ROOT,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        assert process.stdout is not None
        frame_type, payload = run_compiled_steady.read_frame(process.stdout)
        stdout_remainder, stderr = process.communicate(timeout=10)

        self.assertEqual(frame_type, run_compiled_steady.ERROR)
        self.assertIn("--input", payload.decode(errors="replace"))
        self.assertEqual(stdout_remainder, b"")
        self.assertNotEqual(process.returncode, 0)
        self.assertIn("--input", stderr.decode(errors="replace"))

    def test_stim_worker_returns_packed_known_answer_when_stim_1_15_0_is_available(self) -> None:
        try:
            import stim
        except ImportError:
            self.skipTest("stim is unavailable")
        if stim.__version__ != "1.15.0":
            self.skipTest(f"requires stim==1.15.0, got {stim.__version__}")

        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = Path(temp_dir) / "known_answer.stim"
            fixture.write_text("X 0\nM 0\n", encoding="utf-8")
            session = run_compiled_steady.WorkerSession(
                [
                    *run_compiled_steady.default_stim_worker_command(),
                    "--variant",
                    "stim-precompiled",
                ],
                input_path=fixture,
                seed=0,
            )
            try:
                ready = session.read_ready()
                self.assertEqual(ready["variant"], "stim-precompiled")
                self.assertGreater(ready["precompile_elapsed_ns"], 0)
                self.assertEqual(ready["compile_count"], 1)
                self.assertEqual(ready["reference_build_count"], 1)
                self.assertEqual(ready["sample_call_count"], 0)
                self.assertEqual(ready["fixture_sha256"], hashlib.sha256(fixture.read_bytes()).hexdigest())
                self.assertEqual(ready["measurement_count"], 1)
                self.assertEqual(ready["bytes_per_shot"], 1)

                call_count, data, sample_elapsed_ns, b8_elapsed_ns = session.sample(0, 1)
                self.assertEqual(call_count, 1)
                self.assertEqual(data, b"\x01")
                self.assertGreaterEqual(sample_elapsed_ns, 0)
                self.assertGreaterEqual(b8_elapsed_ns, 0)

                final = session.stop()
                self.assertEqual(final["sample_call_count"], 1)
            except BaseException:
                session.abort()
                raise
            finally:
                session.stdout.close()
                session.stderr.close()

    def test_stim_worker_emits_error_frame_for_invalid_sample_json(self) -> None:
        try:
            import stim
        except ImportError:
            self.skipTest("stim is unavailable")
        if stim.__version__ != "1.15.0":
            self.skipTest(f"requires stim==1.15.0, got {stim.__version__}")

        with tempfile.TemporaryDirectory() as temp_dir:
            fixture = Path(temp_dir) / "known_answer.stim"
            fixture.write_text("X 0\nM 0\n", encoding="utf-8")
            session = run_compiled_steady.WorkerSession(
                [
                    *run_compiled_steady.default_stim_worker_command(),
                    "--variant",
                    "stim-precompiled",
                ],
                input_path=fixture,
                seed=0,
            )
            try:
                ready = session.read_ready()
                self.assertEqual(ready["variant"], "stim-precompiled")
                run_compiled_steady.write_frame(session.stdin, run_compiled_steady.SAMPLE, b"{")
                frame_type, payload = run_compiled_steady.read_frame(session.stdout)
                self.assertEqual(frame_type, run_compiled_steady.ERROR)
                self.assertIn("invalid SAMPLE JSON", payload.decode(errors="replace"))
            finally:
                session.abort()
                session.stdin.close()
                session.stdout.close()
                session.stderr.close()

    def test_stim_worker_emits_error_frame_for_missing_required_args(self) -> None:
        process = subprocess.Popen(
            [
                *run_compiled_steady.default_stim_worker_command(),
                "--variant",
                "stim-precompiled",
            ],
            cwd=ROOT,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        assert process.stdout is not None
        frame_type, payload = run_compiled_steady.read_frame(process.stdout)
        stdout_remainder, stderr = process.communicate(timeout=10)

        self.assertEqual(frame_type, run_compiled_steady.ERROR)
        self.assertIn("--input", payload.decode(errors="replace"))
        self.assertEqual(stdout_remainder, b"")
        self.assertNotEqual(process.returncode, 0)
        self.assertIn("--input", stderr.decode(errors="replace"))

    def _write_fake_workers(self, directory: Path, *, mode: str) -> dict[str, Path]:
        paths: dict[str, Path] = {}
        for variant in ("stim", "rstim"):
            path = directory / f"{variant}_worker.py"
            path.write_text(
                textwrap.dedent(
                    f"""
                    import argparse
                    import sys

                    if "--version" in sys.argv:
                        print("rstim 0.1.1" if "{variant}" == "rstim" else "stim fake")
                        raise SystemExit(0)

                    import hashlib
                    import json
                    import struct
                    import time

                    from benchmarks.rstim_vs_stim_simulator import run_compiled_steady as protocol

                    parser = argparse.ArgumentParser()
                    parser.add_argument("--variant", required=True)
                    parser.add_argument("--input", required=True)
                    parser.add_argument("--seed", required=True)
                    args = parser.parse_args()
                    input_bytes = open(args.input, "rb").read()
                    is_preflight = input_bytes in (b"X 0\\nM 0\\n", b"X 0\\nLOSS(0) 0\\nM 0\\n")
                    telemetry_variant = "wrong-variant" if {mode!r} == "wrong-variant" else args.variant
                    compile_count, reference_build_count = protocol._expected_lifecycle_counts(args.variant, 0)
                    telemetry = {{
                        "variant": telemetry_variant,
                        "precompile_elapsed_ns": 7 if args.variant in protocol.PRECOMPILED_VARIANTS else 0,
                        "compile_count": compile_count,
                        "reference_build_count": reference_build_count,
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
                            compile_count, reference_build_count = protocol._expected_lifecycle_counts(args.variant, calls)
                            telemetry["compile_count"] = compile_count
                            telemetry["reference_build_count"] = reference_build_count
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
                        result = struct.pack("<QQQQ", request["request_id"], calls, 1, 2) + data
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
            "PASS split precompile/sample/b8 lifecycle variants=5 calls=9 measured=35",
            result.stdout,
        )
        raw = [json.loads(line) for line in (out_dir / "raw.jsonl").read_text().splitlines()]
        self.assertEqual(sum(1 for record in raw if record["record_type"] == "ready"), 5)
        self.assertEqual(sum(1 for record in raw if record["record_type"] == "sample"), 45)
        self.assertEqual(sum(1 for record in raw if record["record_type"] == "final"), 5)
        for record in raw:
            if record["record_type"] == "sample":
                self.assertEqual(record["shots"], 1024)
                self.assertEqual(record["output_format"], "b8")
                self.assertEqual(record["sample_elapsed_ns"], 1)
                self.assertEqual(record["b8_elapsed_ns"], 2)
                self.assertEqual(record["worker_total_elapsed_ns"], 3)
        summary = json.loads((out_dir / "summary.json").read_text())
        self.assertEqual(summary["measured_records"], 35)
        self.assertEqual({variant["sample_count"] for variant in summary["variants"]}, {7})
        report = (out_dir / "report.md").read_text(encoding="utf-8")
        self.assertIn(
            "| variant | sample_count | precompile_elapsed_ns | median_call_sample_elapsed_ns | median_call_b8_elapsed_ns | median_worker_total_elapsed_ns |",
            report,
        )
        for variant in summary["variants"]:
            self.assertIn(
                f"| {variant['variant']} | {variant['sample_count']} | "
                f"{variant['precompile_elapsed_ns']} | {variant['median_call_sample_elapsed_ns']} | "
                f"{variant['median_call_b8_elapsed_ns']} | {variant['median_worker_total_elapsed_ns']} |",
                report,
            )
        environment = json.loads((out_dir / "environment.json").read_text())
        for key in (
            "git_commit",
            "os",
            "cpu_model",
            "profile",
            "timer_scope",
            "seed_policy",
            "stim_version",
            "rstim_version",
            "rustc_version",
            "fair_manifest_path",
            "fair_manifest_sha256",
            "source_manifest_path",
            "source_manifest_sha256",
            "fixture_path",
            "fixture_sha256",
            "atom_loss_fixture_path",
            "atom_loss_fixture_sha256",
            "worker_argv",
            "stim_worker_module_path",
            "stim_worker_module_sha256",
            "runtime_identities",
            "protocol_version",
            "known_answer_preflight",
        ):
            self.assertIn(key, environment)

        manifest = run_compiled_steady.fair_cli_contract.load_manifest(MANIFEST)
        case = run_compiled_steady.fair_cli_contract.find_case(manifest, "stim_surface_d11_r100")
        fixture = (ROOT / case["canonical_input_path"]).resolve()
        source_manifest = (ROOT / case["source_manifest_path"]).resolve()
        stim_worker_module = (ROOT / "benchmarks/rstim_vs_stim_simulator/workers/stim_compiled_steady.py").resolve()
        atom_loss_fixture = ROOT / run_compiled_steady.ATOM_LOSS_FIXTURE_PATH
        expected_worker_argv = {
            variant: run_compiled_steady._portable_worker_argv(
                variant,
                (
                    atom_loss_fixture.relative_to(ROOT).as_posix()
                    if variant == run_compiled_steady.ATOM_LOSS_VARIANT
                    else case["canonical_input_path"]
                ),
                seed=0,
            )
            for variant in run_compiled_steady.VARIANTS
        }
        known_answer_sha = hashlib.sha256(b"X 0\nM 0\n").hexdigest()
        atom_loss_known_answer_sha = hashlib.sha256(b"X 0\nLOSS(0) 0\nM 0\n").hexdigest()

        self.assertEqual(environment["profile"], "release")
        self.assertEqual(environment["timer_scope"], run_compiled_steady.TIMER_SCOPE)
        self.assertNotIn("secondary_timer_scope", environment)
        self.assertEqual(environment["stim_version"], "1.15.0")
        self.assertEqual(environment["rstim_version"], "rstim 0.1.1")
        self.assertEqual(environment["seed"], 0)
        self.assertEqual(environment["warmup_rounds"], 2)
        self.assertEqual(environment["measure_rounds"], 7)
        self.assertEqual(environment["protocol_version"], run_compiled_steady.PROTOCOL_VERSION)
        self.assertEqual(
            environment["seed_policy"],
            "precompiled_and_rstim_interpreted_seed_once;stim_direct_seed_per_call",
        )
        self.assertEqual(environment["fair_manifest_path"], MANIFEST.relative_to(ROOT).as_posix())
        self.assertEqual(environment["fair_manifest_sha256"], hashlib.sha256(MANIFEST.read_bytes()).hexdigest())
        self.assertEqual(environment["source_manifest_path"], source_manifest.relative_to(ROOT).as_posix())
        self.assertEqual(environment["source_manifest_sha256"], hashlib.sha256(source_manifest.read_bytes()).hexdigest())
        self.assertEqual(environment["fixture_path"], fixture.relative_to(ROOT).as_posix())
        self.assertEqual(environment["fixture_sha256"], hashlib.sha256(fixture.read_bytes()).hexdigest())
        self.assertEqual(environment["fixture_sha256"], case["canonical_input_sha256"])
        self.assertEqual(
            environment["atom_loss_fixture_path"], atom_loss_fixture.relative_to(ROOT).as_posix()
        )
        self.assertEqual(
            environment["atom_loss_fixture_sha256"], hashlib.sha256(atom_loss_fixture.read_bytes()).hexdigest()
        )
        self.assertEqual(environment["worker_argv"], expected_worker_argv)
        self.assertEqual(environment["canonical_worker_argv"], expected_worker_argv)
        self.assertEqual(
            environment["stim_worker_module_path"],
            stim_worker_module.relative_to(ROOT).as_posix(),
        )
        self.assertEqual(
            environment["stim_worker_module_sha256"],
            hashlib.sha256(stim_worker_module.read_bytes()).hexdigest(),
        )
        self.assertEqual(
            {identity["role"] for identity in environment["runtime_identities"]},
            {"tool://python", "tool://stim-extension", "tool://stim-worker", "tool://rstim-worker"},
        )
        preflight = environment["known_answer_preflight"]
        self.assertEqual([record["variant"] for record in preflight], list(run_compiled_steady.VARIANTS))
        self.assertEqual([record["result_hex"] for record in preflight], ["01"] * 5)
        for record in preflight:
            expected_known_answer_sha = (
                atom_loss_known_answer_sha
                if record["variant"] == run_compiled_steady.ATOM_LOSS_VARIANT
                else known_answer_sha
            )
            self.assertEqual(record["ready"]["fixture_sha256"], expected_known_answer_sha)
            expected_precompile = 7 if record["variant"] in run_compiled_steady.PRECOMPILED_VARIANTS else 0
            self.assertEqual(record["ready"]["precompile_elapsed_ns"], expected_precompile)
            compile_count, reference_build_count = run_compiled_steady._expected_lifecycle_counts(
                record["variant"], 0
            )
            self.assertEqual(record["ready"]["compile_count"], compile_count)
            self.assertEqual(record["ready"]["reference_build_count"], reference_build_count)
            self.assertEqual(record["ready"]["sample_call_count"], 0)
            self.assertEqual(record["ready"]["measurement_count"], 1)
            self.assertEqual(record["ready"]["bytes_per_shot"], 1)
            self.assertEqual(record["final"]["fixture_sha256"], expected_known_answer_sha)
            self.assertEqual(record["final"]["sample_call_count"], 1)

    def test_worker_timings_exclude_delayed_pipe_byte(self) -> None:
        result, out_dir, temp_dir = self._run_fake_mode("delay-last-byte")
        self.addCleanup(temp_dir.cleanup)

        self.assertEqual(result.returncode, 0, result.stderr)
        raw = [json.loads(line) for line in (out_dir / "raw.jsonl").read_text().splitlines()]
        samples = [record for record in raw if record["record_type"] == "sample"]
        self.assertEqual({record["sample_elapsed_ns"] for record in samples}, {1})
        self.assertEqual({record["b8_elapsed_ns"] for record in samples}, {2})
        self.assertEqual({record["worker_total_elapsed_ns"] for record in samples}, {3})
        self.assertTrue(all("end_to_end_elapsed_ns" not in record for record in samples))

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

    def test_worker_variant_telemetry_is_validated(self) -> None:
        result, out_dir, temp_dir = self._run_fake_mode("wrong-variant")
        self.addCleanup(temp_dir.cleanup)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("worker telemetry variant", result.stderr)
        self.assertFalse((out_dir / "raw.jsonl").exists())

    def test_default_stim_worker_command_is_canonical_python3_argv(self) -> None:
        self.assertEqual(
            run_compiled_steady.default_stim_worker_command(),
            [
                "python3",
                "-m",
                "benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady",
            ],
        )

    def test_default_rstim_worker_command_is_canonical_relative_argv(self) -> None:
        self.assertEqual(
            run_compiled_steady.default_rstim_worker_command("release"),
            ["target/release/rstim_compiled_steady_worker"],
        )

    def test_stim_version_requirement_is_checked_with_worker_overrides(self) -> None:
        with unittest.mock.patch.object(
            run_compiled_steady,
            "_probe_stim_python",
            return_value={
                "status": "ok",
                "version": "1.14.0",
                "path": None,
                "sha256": None,
                "stderr": "",
            },
        ):
            with tempfile.TemporaryDirectory() as temp_dir:
                paths = self._write_fake_workers(Path(temp_dir), mode="ok")
                out_dir = Path(temp_dir) / "out"
                stderr = io.StringIO()
                with contextlib.redirect_stderr(stderr):
                    exit_code = run_compiled_steady.main(
                        [
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
                            json.dumps([sys.executable, str(paths["stim"])]),
                            "--rstim-worker-command",
                            json.dumps([sys.executable, str(paths["rstim"])]),
                        ]
                    )

        self.assertNotEqual(exit_code, 0)
        self.assertIn("requires stim==1.15.0", stderr.getvalue())
        self.assertFalse((out_dir / "raw.jsonl").exists())

    def test_profile_argument_accepts_release_and_debug(self) -> None:
        parser = run_compiled_steady.build_parser()
        args = parser.parse_args(
            [
                "--manifest",
                str(MANIFEST),
                "--case",
                "stim_surface_d11_r100",
                "--profile",
                "debug",
                "--out-dir",
                "/tmp/out",
            ]
        )
        self.assertEqual(args.profile, "debug")

    def test_debug_profile_records_portable_worker_argv(self) -> None:
        parser = run_compiled_steady.build_parser()
        args = parser.parse_args(
            [
                "--manifest",
                str(MANIFEST),
                "--case",
                "stim_surface_d11_r100",
                "--profile",
                "debug",
                "--out-dir",
                "/tmp/out",
            ]
        )
        manifest = run_compiled_steady.fair_cli_contract.load_manifest(MANIFEST)
        case = run_compiled_steady.fair_cli_contract.find_case(manifest, "stim_surface_d11_r100")
        fixture = (ROOT / case["canonical_input_path"]).resolve()
        worker_details = []

        with (
            unittest.mock.patch.object(run_compiled_steady, "_version_string", return_value=None),
            unittest.mock.patch.object(
                run_compiled_steady,
                "_resolve_executable",
                return_value=Path(sys.executable).resolve(),
            ),
        ):
            environment = run_compiled_steady._collect_environment(
                args=args,
                case=case,
                input_path=fixture,
                atom_loss_input_path=(ROOT / run_compiled_steady.ATOM_LOSS_FIXTURE_PATH).resolve(),
                rstim_command=run_compiled_steady.default_rstim_worker_command("debug"),
                worker_details=worker_details,
                preflight_results=[],
                stim_probe={"path": str(Path(sys.executable).resolve()), "version": "1.15.0"},
            )

        fixture_path = fixture.relative_to(ROOT).as_posix()
        self.assertEqual(environment["worker_argv"]["rstim-precompiled"][0], "tool://rstim-worker")
        self.assertEqual(
            environment["canonical_worker_argv"]["rstim-precompiled"],
            [
                "tool://rstim-worker",
                "--variant",
                "rstim-precompiled",
                "--input",
                fixture_path,
                "--seed",
                "0",
            ],
        )


if __name__ == "__main__":
    unittest.main()
