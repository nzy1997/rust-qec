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


class RunSpeedSuiteParserTest(unittest.TestCase):
    def test_parse_case_labels_strips_blanks_and_rejects_empty(self) -> None:
        self.assertEqual(
            run_speed_suite.parse_case_labels(" rep-sample-d13-r13, surface-detect-d13-r13 "),
            ["rep-sample-d13-r13", "surface-detect-d13-r13"],
        )
        with self.assertRaisesRegex(ValueError, "no benchmark cases requested"):
            run_speed_suite.parse_case_labels("")
        with self.assertRaisesRegex(ValueError, "no benchmark cases requested"):
            run_speed_suite.parse_case_labels(" , ")

    def test_parse_case_labels_rejects_duplicates(self) -> None:
        with self.assertRaisesRegex(ValueError, "duplicate benchmark case: rep-sample-d13-r13"):
            run_speed_suite.parse_case_labels("rep-sample-d13-r13,rep-sample-d13-r13")

    def test_main_empty_cases_prints_required_message(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out_dir = Path(temp_dir) / "out"
            with (
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_suite.print") as mocked_print,
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_suite.run_speed_case.build_rstim") as mocked_build,
            ):
                code = run_speed_suite.main(
                    [
                        "--profile",
                        "release",
                        "--cases",
                        "",
                        "--warmup-rounds",
                        "0",
                        "--measure-rounds",
                        "1",
                        "--out-dir",
                        str(out_dir),
                    ]
                )

            self.assertEqual(code, 1)
            self.assertFalse(mocked_build.called)
            self.assertIn("no benchmark cases requested", str(mocked_print.call_args))


class RunSpeedSuiteWorkflowTest(unittest.TestCase):
    def test_run_speed_suite_builds_once_and_writes_exact_requested_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            out_dir = repo_root / "suite"
            binary = repo_root / "target/release/rstim"
            binary.parent.mkdir(parents=True)
            binary.write_text("")
            args = argparse.Namespace(
                profile="release",
                cases="rep-sample-d13-r13,surface-detect-d13-r13,stim-style-surface-sample-d11-r100-b1024",
                warmup_rounds=0,
                measure_rounds=1,
                out_dir=out_dir,
            )
            commands: list[list[str]] = []

            def fake_run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                commands.append(command)
                if command[:3] == [str(binary), "perf", "run"]:
                    label = command[command.index("--case") + 1]
                    out_path = Path(command[command.index("--out") + 1])
                    out_path.write_text(f'{{"case_label":"{label}","tool_variant":"stim-cli"}}\n')
                elif command[:3] == [str(binary), "perf", "summarize"]:
                    label = command[command.index("--case") + 1]
                    out_path = Path(command[command.index("--out") + 1])
                    out_path.write_text(
                        json.dumps(
                            {
                                "cases": [{"case_label": label, "variants": []}],
                                "issues": [],
                            }
                        )
                        + "\n"
                    )
                elif command[:3] == [str(binary), "perf", "report"]:
                    out_path = Path(command[command.index("--out") + 1])
                    out_path.write_text("# suite report\n")
                elif command == ["rustc", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "rustc 1.93.1\n", "")
                elif command == ["cargo", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "cargo 1.93.1\n", "")
                elif command == [str(binary)]:
                    return subprocess.CompletedProcess(command, 0, "rstim 0.1.1\n", "")
                elif command == ["stim", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "stim 1.15.0\n", "")
                return subprocess.CompletedProcess(command, 0, "", "")

            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_speed_suite.run_speed_case.build_rstim",
                    return_value=binary,
                ) as build,
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_speed_case.subprocess.run") as mocked,
            ):
                mocked.side_effect = fake_run
                run_speed_suite.run_speed_suite(
                    args,
                    repo_root=repo_root,
                    command_line=[
                        "python3",
                        "-m",
                        "benchmarks.rstim_vs_stim_simulator.run_speed_suite",
                    ],
                )

            self.assertEqual(build.call_count, 1)
            run_commands = [command for command in commands if command[:3] == [str(binary), "perf", "run"]]
            self.assertEqual(len(run_commands), 3)
            self.assertEqual(
                [command[command.index("--case") + 1] for command in run_commands],
                [
                    "rep-sample-d13-r13",
                    "surface-detect-d13-r13",
                    "stim-style-surface-sample-d11-r100-b1024",
                ],
            )
            self.assertEqual(
                [json.loads(line)["case_label"] for line in (out_dir / "raw.jsonl").read_text().splitlines()],
                [
                    "rep-sample-d13-r13",
                    "surface-detect-d13-r13",
                    "stim-style-surface-sample-d11-r100-b1024",
                ],
            )
            summary = json.loads((out_dir / "summary.json").read_text())
            self.assertEqual(
                [case["case_label"] for case in summary["cases"]],
                [
                    "rep-sample-d13-r13",
                    "surface-detect-d13-r13",
                    "stim-style-surface-sample-d11-r100-b1024",
                ],
            )
            self.assertEqual(summary["issues"], [])
            self.assertEqual((out_dir / "report.md").read_text(), "# suite report\n")
            environment = json.loads((out_dir / "environment.json").read_text())
            self.assertEqual(environment["profile"], "release")
            self.assertEqual(
                environment["case_labels"],
                [
                    "rep-sample-d13-r13",
                    "surface-detect-d13-r13",
                    "stim-style-surface-sample-d11-r100-b1024",
                ],
            )
            self.assertEqual(environment["command_line"][2], "benchmarks.rstim_vs_stim_simulator.run_speed_suite")
