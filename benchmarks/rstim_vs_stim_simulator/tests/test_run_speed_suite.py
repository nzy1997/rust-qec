from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator import run_speed_case, run_speed_suite


class RunSpeedSuiteEnvironmentTest(unittest.TestCase):
    def test_collect_suite_environment_records_case_list_and_command_line(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            binary = Path(temp_dir) / "target/release/rstim"
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
                    return subprocess.CompletedProcess(command, 0, "stim 1.15.0\n", "")
                raise AssertionError(f"unexpected command: {command}")

            with mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_case.subprocess.run") as mocked:
                mocked.side_effect = fake_run
                env = run_speed_case.collect_suite_environment(
                    profile="release",
                    case_labels=["rep-sample-d13-r13", "surface-detect-d13-r13"],
                    warmup_rounds=0,
                    measure_rounds=1,
                    rstim_binary_path=binary,
                    command_line=["python3", "-m", "benchmarks.rstim_vs_stim_simulator.run_speed_suite"],
                )

            self.assertEqual(env["profile"], "release")
            self.assertEqual(env["case_labels"], ["rep-sample-d13-r13", "surface-detect-d13-r13"])
            self.assertEqual(env["case_count"], 2)
            self.assertEqual(env["command_line"][2], "benchmarks.rstim_vs_stim_simulator.run_speed_suite")
            self.assertEqual(env["rustc_version"], "rustc 1.93.1")
            self.assertEqual(env["cargo_version"], "cargo 1.93.1")
            self.assertEqual(env["rstim_binary_path"], str(binary.resolve()))
            self.assertEqual(env["stim_cli_status"], "ok")
            self.assertEqual(env["stim_cli_version"], "stim 1.15.0")
