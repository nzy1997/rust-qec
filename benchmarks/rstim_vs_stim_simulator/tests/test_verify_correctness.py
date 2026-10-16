from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator.verify_correctness import (
    build_sample_command,
    compare_sample_sets,
    default_rstim_command,
    format_report,
    inject_bitflip,
    main,
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

    def test_default_rstim_command_prefers_release_binary(self) -> None:
        with mock.patch(
            "benchmarks.rstim_vs_stim_simulator.verify_correctness.Path.exists",
            side_effect=[True],
        ):
            self.assertEqual(default_rstim_command(), ["target/release/rstim"])

    def test_default_rstim_command_falls_back_to_debug_binary(self) -> None:
        with mock.patch(
            "benchmarks.rstim_vs_stim_simulator.verify_correctness.Path.exists",
            side_effect=[False, True],
        ):
            self.assertEqual(default_rstim_command(), ["target/debug/rstim"])

    def test_default_rstim_command_uses_offline_cargo_when_binaries_are_absent(self) -> None:
        with mock.patch(
            "benchmarks.rstim_vs_stim_simulator.verify_correctness.Path.exists",
            side_effect=[False, False],
        ):
            self.assertEqual(
                default_rstim_command(),
                ["cargo", "run", "--locked", "--offline", "--quiet", "-p", "rstim", "--bin", "rstim", "--"],
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
            ["1", "2", "3"],
        )
        expected_input_path = (
            Path("benchmarks/rstim_vs_stim_simulator") / self.sample_case["canonical_input_path"]
        ).resolve()
        invoked_seed_pairs = [
            (call.kwargs["input_path"], call.args[0][0], call.args[0][call.args[0].index("--seed") + 1])
            for call in mocked.call_args_list
        ]
        self.assertIn(
            (
                expected_input_path,
                "rstim",
                "2",
            ),
            invoked_seed_pairs,
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


class VerifyCorrectnessCliTest(unittest.TestCase):
    def test_main_writes_json_and_keeps_documentation_only_skip_out_of_warn_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "summary.json"
            manifest = {"suite": "rstim_vs_stim_simulator", "cases": [{}, {}]}
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.load_manifest",
                    return_value=manifest,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.validate_manifest",
                    return_value=[],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.verify_case"
                ) as mocked,
            ):
                mocked.side_effect = [
                    {
                        "case_id": "case_a",
                        "tier": "smoke",
                        "status": "pass",
                        "sample_count": 4,
                        "max_delta": 0.0,
                        "max_tolerance": 0.01,
                        "failure_reasons": [],
                        "selected_columns": [0],
                        "selected_pairs": [],
                        "marginals": [],
                        "pairs": [],
                        "stim_runs": [{"success": True}, {"success": True}],
                        "rstim_runs": [{"success": True}, {"success": True}],
                    },
                    {
                        "case_id": "doc_case",
                        "tier": "documentation-only",
                        "status": "skipped",
                        "sample_count": 0,
                        "failure_reasons": ["documentation-only"],
                        "selected_columns": [0],
                        "selected_pairs": [],
                        "stim_runs": [],
                        "rstim_runs": [],
                    },
                ]
                with mock.patch("sys.stdout.write") as stdout:
                    code = main(
                        [
                            "--cases",
                            "benchmarks/rstim_vs_stim_simulator/cases.smoke.toml",
                            "--shots",
                            "4",
                            "--out",
                            str(out),
                        ]
                    )
            self.assertEqual(code, 0)
            data = json.loads(out.read_text())
            self.assertEqual(data["status"], "pass")
            self.assertEqual(data["case_count"], 2)
            self.assertTrue(any("PASS correctness smoke" in call.args[0] for call in stdout.call_args_list))

    def test_main_warns_when_all_cases_are_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "summary.json"
            manifest = {"suite": "rstim_vs_stim_simulator", "cases": [{}]}
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.load_manifest",
                    return_value=manifest,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.validate_manifest",
                    return_value=[],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.verify_case"
                ) as mocked,
            ):
                mocked.return_value = {
                    "case_id": "doc_case",
                    "tier": "documentation-only",
                    "status": "skipped",
                    "sample_count": 0,
                    "failure_reasons": ["documentation-only"],
                    "selected_columns": [],
                    "selected_pairs": [],
                    "stim_runs": [],
                    "rstim_runs": [],
                }
                with mock.patch("sys.stdout.write") as stdout:
                    code = main(
                        [
                            "--cases",
                            "benchmarks/rstim_vs_stim_simulator/cases.smoke.toml",
                            "--shots",
                            "4",
                            "--out",
                            str(out),
                        ]
                    )

            self.assertEqual(code, 0)
            data = json.loads(out.read_text())
            self.assertEqual(data["status"], "warn")
            self.assertTrue(any("WARN correctness smoke" in call.args[0] for call in stdout.call_args_list))

    def test_main_rejects_invalid_bitflip_rate_before_running_tools(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "summary.json"
            manifest = {"suite": "rstim_vs_stim_simulator", "cases": [{}]}
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.load_manifest",
                    return_value=manifest,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.validate_manifest",
                    return_value=[],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.verify_case"
                ) as mocked,
                mock.patch("sys.stderr.write"),
            ):
                code = main(
                    [
                        "--cases",
                        "benchmarks/rstim_vs_stim_simulator/cases.smoke.toml",
                        "--shots",
                        "4",
                        "--inject-rstim-bitflip-rate",
                        "1.5",
                        "--out",
                        str(out),
                    ]
                )

            self.assertEqual(code, 1)
            self.assertFalse(out.exists())
            mocked.assert_not_called()

    def test_main_returns_nonzero_for_statistical_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "summary.json"
            manifest = {"suite": "rstim_vs_stim_simulator", "cases": [{}]}
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.load_manifest",
                    return_value=manifest,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.validate_manifest",
                    return_value=[],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_correctness.verify_case"
                ) as mocked,
            ):
                mocked.return_value = {
                    "case_id": "case_a",
                    "tier": "smoke",
                    "status": "statistical_mismatch",
                    "sample_count": 4,
                    "max_delta": 0.2,
                    "max_tolerance": 0.01,
                    "failure_reasons": ["marginal c0 delta 0.2 > tolerance 0.01"],
                    "selected_columns": [0],
                    "selected_pairs": [],
                }
                with mock.patch("sys.stdout.write") as stdout:
                    code = main(
                        [
                            "--cases",
                            "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
                            "--shots",
                            "4",
                            "--out",
                            str(out),
                        ]
                    )
            self.assertEqual(code, 1)
            data = json.loads(out.read_text())
            self.assertEqual(data["status"], "statistical_mismatch")
            self.assertTrue(
                any("FAIL statistical mismatch" in call.args[0] for call in stdout.call_args_list)
            )

    def test_format_report_prefers_tool_failure_headline_over_warn(self) -> None:
        summary = {
            "status": "stim_failed",
            "manifest_path": "benchmarks/rstim_vs_stim_simulator/cases.smoke.toml",
            "case_count": 2,
            "shots": 4,
            "seeds": [1, 2],
            "counts": {
                "pass": 0,
                "statistical_mismatch": 0,
                "stim_failed": 1,
                "rstim_failed": 0,
                "skipped": 1,
            },
            "cases": [
                {
                    "case_id": "case_a",
                    "status": "stim_failed",
                    "sample_count": 2,
                    "selected_columns": [0],
                    "selected_pairs": [],
                    "max_delta": 0.0,
                    "max_tolerance": 0.01,
                    "failure_reasons": ["seed 2: stim failed"],
                    "marginals": [],
                    "pairs": [],
                    "stim_runs": [{"success": True}, {"success": False}],
                    "rstim_runs": [{"success": True}, {"success": True}],
                },
                {
                    "case_id": "doc_case",
                    "status": "skipped",
                    "sample_count": 0,
                    "selected_columns": [],
                    "selected_pairs": [],
                    "failure_reasons": ["documentation-only"],
                    "stim_runs": [],
                    "rstim_runs": [],
                },
            ],
        }

        exit_code, report = format_report(summary)

        self.assertEqual(exit_code, 1)
        self.assertTrue(report.startswith("FAIL tool failure"))
        self.assertIn("summary status=stim_failed", report)

    def test_format_report_includes_rates_tolerance_and_tool_status_fields(self) -> None:
        summary = {
            "status": "pass",
            "manifest_path": "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
            "case_count": 1,
            "shots": 8,
            "seeds": [11, 12],
            "counts": {
                "pass": 1,
                "statistical_mismatch": 0,
                "stim_failed": 0,
                "rstim_failed": 0,
                "skipped": 0,
            },
            "cases": [
                {
                    "case_id": "case_a",
                    "status": "pass",
                    "sample_count": 16,
                    "selected_columns": [0, 3],
                    "selected_pairs": [[0, 3]],
                    "max_delta": 0.05,
                    "max_tolerance": 0.08,
                    "failure_reasons": [],
                    "marginals": [
                        {
                            "column": 0,
                            "stim_rate": 0.125,
                            "rstim_rate": 0.1875,
                            "delta": 0.0625,
                            "tolerance": 0.08,
                        }
                    ],
                    "pairs": [
                        {
                            "pair": [0, 3],
                            "stim_rate": 0.0,
                            "rstim_rate": 0.0625,
                            "delta": 0.0625,
                            "tolerance": 0.09,
                        }
                    ],
                    "stim_runs": [{"success": True}, {"success": True}],
                    "rstim_runs": [{"success": True}, {"success": False}],
                }
            ],
        }

        _, report = format_report(summary)

        self.assertIn("PASS correctness full", report)
        self.assertIn("marginal c0 stim=0.125000 rstim=0.187500 delta=0.062500 tol=0.080000", report)
        self.assertIn("pair 0,3 stim=0.000000 rstim=0.062500 delta=0.062500 tol=0.090000", report)
        self.assertIn("stim_runs=2/2_ok", report)
        self.assertIn("rstim_runs=1/2_ok", report)


if __name__ == "__main__":
    unittest.main()
