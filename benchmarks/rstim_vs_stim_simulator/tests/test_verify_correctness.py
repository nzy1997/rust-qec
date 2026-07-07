from __future__ import annotations

import subprocess
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator.verify_correctness import (
    build_sample_command,
    compare_sample_sets,
    default_rstim_command,
    inject_bitflip,
    parse_01_samples,
    run_tool,
    select_columns,
    select_pairs,
    verify_case,
)


class VerifyCorrectnessHelpersTest(unittest.TestCase):
    def test_parse_01_samples_requires_rectangular_output(self) -> None:
        self.assertEqual(
            parse_01_samples("01\n10\n", expected_bits=2, expected_shots=2),
            [[0, 1], [1, 0]],
        )
        with self.assertRaisesRegex(ValueError, "expected 2 bits"):
            parse_01_samples("0\n11\n", expected_bits=2, expected_shots=2)
        with self.assertRaisesRegex(ValueError, "expected 2 shots"):
            parse_01_samples("01\n", expected_bits=2, expected_shots=2)

    def test_selectors_include_observable_tail_even_when_limited(self) -> None:
        columns = select_columns(8, observable_count=2, limit=1)
        self.assertEqual(columns, [0, 6, 7])

    def test_selectors_include_observable_tail_and_pairs(self) -> None:
        columns = select_columns(25, observable_count=2, limit=10)
        self.assertIn(0, columns)
        self.assertIn(23, columns)
        self.assertIn(24, columns)
        pairs = select_pairs(columns, bit_count=25, observable_count=2, limit=10)
        self.assertTrue(any(pair[1] >= 23 for pair in pairs))

    def test_compare_sample_sets_accepts_close_rates(self) -> None:
        stim = [[0, 1], [1, 1], [0, 0], [1, 0]]
        rstim = [[0, 1], [1, 1], [0, 0], [1, 0]]
        result = compare_sample_sets(stim, rstim, columns=[0, 1], pairs=[(0, 1)])
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["sample_count"], 4)

    def test_compare_sample_sets_flags_large_mismatch(self) -> None:
        stim = [[0] for _ in range(100)]
        rstim = [[1] for _ in range(100)]
        result = compare_sample_sets(stim, rstim, columns=[0], pairs=[])
        self.assertEqual(result["status"], "statistical_mismatch")
        self.assertGreater(result["max_delta"], result["max_tolerance"])

    def test_inject_bitflip_is_deterministic_and_changes_bits(self) -> None:
        samples = [[0, 0], [1, 1]]
        self.assertEqual(
            inject_bitflip(samples, rate=1.0, seed=7),
            [[1, 1], [0, 0]],
        )
        self.assertEqual(samples, [[0, 0], [1, 1]])


class VerifyCorrectnessRunnerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.sample_case = {
            "case_id": "case_a",
            "tier": "smoke",
            "canonical_input_path": "fixtures/example.stim",
            "expected_measurements": 2,
            "expected_detectors": 0,
            "expected_observables": 0,
        }

    def test_default_rstim_command_uses_cargo_when_binary_is_absent(self) -> None:
        with mock.patch(
            "benchmarks.rstim_vs_stim_simulator.verify_correctness.Path.exists",
            return_value=False,
        ):
            self.assertEqual(
                default_rstim_command(),
                ["cargo", "run", "--quiet", "-p", "rstim", "--bin", "rstim", "--"],
            )

    def test_run_tool_records_failure_stderr(self) -> None:
        completed = subprocess.CompletedProcess(["bad"], 2, "", "broken")
        with mock.patch(
            "benchmarks.rstim_vs_stim_simulator.verify_correctness.subprocess.run",
            return_value=completed,
        ):
            result = run_tool(["bad"], input_path=Path("case.stim"))
        self.assertEqual(result["exit_code"], 2)
        self.assertEqual(result["stderr"], "broken")
        self.assertFalse(result["success"])

    def test_build_sample_command_detector_mode_appends_observables(self) -> None:
        command = build_sample_command(
            ["stim"],
            mode="detect",
            shots=4,
            seed=9,
            input_path=Path("case.stim"),
        )
        self.assertEqual(
            command,
            [
                "stim",
                "detect",
                "--append_observables",
                "--shots",
                "4",
                "--seed",
                "9",
                "--out_format",
                "01",
                "--in",
                "case.stim",
            ],
        )

    def test_verify_case_continues_after_stim_failure_and_preserves_partial_samples(self) -> None:
        def fake_run_tool(command: list[str], *, input_path: Path) -> dict[str, object]:
            seed = int(command[command.index("--seed") + 1])
            tool = command[0]
            if tool == "stim" and seed == 2:
                return {
                    "command": command,
                    "exit_code": 1,
                    "stdout": "",
                    "stderr": "stim failed on seed 2",
                    "elapsed_s": 0.01,
                    "success": False,
                }
            return {
                "command": command,
                "exit_code": 0,
                "stdout": "01\n10\n",
                "stderr": "",
                "elapsed_s": 0.01,
                "success": True,
            }

        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_correctness.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                self.sample_case,
                base_dir=Path("benchmarks/rstim_vs_stim_simulator"),
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=2,
                seeds=[1, 2, 3],
                inject_rstim_bitflip_rate=0.0,
            )
        self.assertEqual(result["status"], "stim_failed")
        self.assertEqual([run["command"][0] for run in result["stim_runs"]], ["stim", "stim", "stim"])
        self.assertEqual(
            [run["command"][run["command"].index("--seed") + 1] for run in result["stim_runs"]],
            ["1", "2", "3"],
        )
        self.assertEqual(
            [run["command"][run["command"].index("--seed") + 1] for run in result["rstim_runs"]],
            ["1", "3"],
        )
        self.assertEqual(result["sample_count"], 4)
        self.assertIn("stim failed on seed 2", result["failure_reasons"][0])
        self.assertIn("max_delta", result)
        self.assertIn("max_tolerance", result)
        self.assertTrue(result["marginals"])
        self.assertTrue(result["pairs"])

    def test_verify_case_records_partial_statistics_for_rstim_parse_failure(self) -> None:
        def fake_run_tool(command: list[str], *, input_path: Path) -> dict[str, object]:
            seed = int(command[command.index("--seed") + 1])
            tool = command[0]
            if tool == "rstim" and seed == 2:
                stdout = "0\n1\n"
            else:
                stdout = "01\n10\n"
            return {
                "command": command,
                "exit_code": 0,
                "stdout": stdout,
                "stderr": "",
                "elapsed_s": 0.01,
                "success": True,
            }

        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_correctness.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                self.sample_case,
                base_dir=Path("benchmarks/rstim_vs_stim_simulator"),
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=2,
                seeds=[1, 2, 3],
                inject_rstim_bitflip_rate=0.0,
            )

        self.assertEqual(result["status"], "rstim_failed")
        self.assertEqual(
            [run["command"][run["command"].index("--seed") + 1] for run in result["stim_runs"]],
            ["1", "2", "3"],
        )
        self.assertEqual(
            [run["command"][run["command"].index("--seed") + 1] for run in result["rstim_runs"]],
            ["1", "2", "3"],
        )
        self.assertEqual(result["sample_count"], 4)
        self.assertIn("failed to parse rstim output", result["failure_reasons"][0])
        self.assertIn("max_delta", result)
        self.assertIn("max_tolerance", result)
        self.assertTrue(result["marginals"])


if __name__ == "__main__":
    unittest.main()
