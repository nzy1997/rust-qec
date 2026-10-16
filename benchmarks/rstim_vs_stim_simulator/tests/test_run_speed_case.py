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
            self.assertEqual(calls[0][0], ["cargo", "build", "--locked", "-p", "rstim", "--bin", "rstim"])
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
                ["cargo", "build", "--locked", "--release", "-p", "rstim", "--bin", "rstim"],
            )


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
                if command[:3] == [str(binary), "perf", "run"]:
                    (out_dir / "raw.jsonl").write_text('{"ok":true}\n')
                elif command[:3] == [str(binary), "perf", "summarize"]:
                    (out_dir / "summary.json").write_text('{"summary":"ok"}\n')
                elif command[:3] == [str(binary), "perf", "report"]:
                    (out_dir / "report.md").write_text("# report\n")
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
                    "--case",
                    "stim-style-surface-sample-d11-r100-b1024",
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
            self.assertTrue((out_dir / "raw.jsonl").exists())
            self.assertTrue((out_dir / "summary.json").exists())
            self.assertTrue((out_dir / "report.md").exists())
            env = json.loads((out_dir / "environment.json").read_text())
            self.assertEqual(env["profile"], "release")
            self.assertEqual(env["case_label"], "stim-style-surface-sample-d11-r100-b1024")
            self.assertEqual(env["rustc_version"], "rustc 1.93.1")
            self.assertEqual(env["cargo_version"], "cargo 1.93.1")
            self.assertEqual(env["rstim_binary_path"], str(binary.resolve()))
            self.assertEqual(env["stim_cli_status"], "ok")
            self.assertEqual(env["stim_cli_version"], "stim 1.15.0")

    def test_run_speed_case_raises_when_successful_pipeline_misses_artifact(self) -> None:
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

            def fake_run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                if command[:3] == [str(binary), "perf", "run"]:
                    (out_dir / "raw.jsonl").write_text('{"ok":true}\n')
                elif command[:3] == [str(binary), "perf", "report"]:
                    (out_dir / "report.md").write_text("# report\n")
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
                with self.assertRaisesRegex(RuntimeError, "missing expected artifact: .*summary\\.json"):
                    run_speed_case.run_speed_case(args, repo_root=repo_root)

            self.assertFalse((out_dir / "environment.json").exists())

    def test_collect_environment_records_stim_version_failure_status(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            binary = Path(temp_dir) / "target/debug/rstim"
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
                    return subprocess.CompletedProcess(
                        command,
                        127,
                        "",
                        "stim: command not found\n",
                    )
                raise AssertionError(f"unexpected command: {command}")

            with mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_case.subprocess.run") as mocked:
                mocked.side_effect = fake_run
                env = run_speed_case.collect_environment(
                    profile="debug",
                    case_label="case-a",
                    warmup_rounds=0,
                    measure_rounds=1,
                    rstim_binary_path=binary,
                )

            self.assertEqual(env["stim_cli_status"], "failed")
            self.assertIsNone(env["stim_cli_version"])
            self.assertEqual(env["stim_cli"]["status"], "failed")
            self.assertIsNone(env["stim_cli"]["version"])
            self.assertEqual(env["stim_cli_stderr"], "stim: command not found")
            self.assertEqual(env["stim_cli"]["stderr"], "stim: command not found")

    def test_collect_environment_uses_python_stim_version_when_cli_version_is_empty(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            binary = Path(temp_dir) / "target/debug/rstim"
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
                    return subprocess.CompletedProcess(command, 0, "", "No mode was given.\n")
                if command == [
                    "python3",
                    "-c",
                    "import stim; print(stim.__version__)",
                ]:
                    return subprocess.CompletedProcess(command, 0, "1.15.0\n", "")
                raise AssertionError(f"unexpected command: {command}")

            with mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_case.subprocess.run") as mocked:
                mocked.side_effect = fake_run
                env = run_speed_case.collect_environment(
                    profile="debug",
                    case_label="case-a",
                    warmup_rounds=0,
                    measure_rounds=1,
                    rstim_binary_path=binary,
                )

            self.assertEqual(env["stim_cli_status"], "ok")
            self.assertEqual(env["stim_cli_version"], "1.15.0")
            self.assertEqual(env["stim_python_version"], "1.15.0")
            self.assertEqual(env["stim_cli"]["version"], "")
            self.assertEqual(env["stim_cli_stderr"], "No mode was given.")

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
