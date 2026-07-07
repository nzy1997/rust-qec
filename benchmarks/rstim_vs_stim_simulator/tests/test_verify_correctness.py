from __future__ import annotations

import subprocess
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator.verify_correctness import (
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

    def test_verify_case_records_stim_failure_before_statistics(self) -> None:
        case = {
            "case_id": "case_a",
            "tier": "smoke",
            "canonical_input_path": "fixtures/example.stim",
            "expected_measurements": 2,
            "expected_detectors": 0,
            "expected_observables": 0,
        }
        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_correctness.run_tool") as mocked:
            mocked.return_value = {
                "command": ["stim"],
                "exit_code": 1,
                "stdout": "",
                "stderr": "stim failed",
                "elapsed_s": 0.01,
                "success": False,
            }
            result = verify_case(
                case,
                base_dir=Path("benchmarks/rstim_vs_stim_simulator"),
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=4,
                seeds=[1],
                inject_rstim_bitflip_rate=0.0,
            )
        self.assertEqual(result["status"], "stim_failed")
        self.assertIn("stim failed", result["failure_reasons"][0])


if __name__ == "__main__":
    unittest.main()
