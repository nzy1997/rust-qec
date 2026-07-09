from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator.verify_distributions import (
    _direct_binary_path,
    _outcome_tolerance,
    collect_environment_metadata,
    build_sample_command,
    compare_distribution,
    format_report,
    main,
    parse_01_samples,
    verify_case,
)


def unit_case() -> dict[str, object]:
    return {
        "case_id": "unit_bell",
        "source_url": "https://example.test/source",
        "source_commit": "abc123",
        "source_line_start": 10,
        "source_line_end": 20,
        "circuit": "H 0\nCNOT 0 1\nM 0 1\n",
        "shots": 4,
        "tolerance": 1e-9,
        "expected_distribution": {"00": 0.5, "11": 0.5},
    }


def sha256_text(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


class VerifyDistributionHelpersTest(unittest.TestCase):
    def test_parse_01_samples_requires_rectangular_output(self) -> None:
        self.assertEqual(parse_01_samples("00\n11\n", expected_bits=2, expected_shots=2), ["00", "11"])
        with self.assertRaisesRegex(ValueError, "expected 2 bits"):
            parse_01_samples("0\n11\n", expected_bits=2, expected_shots=2)
        with self.assertRaisesRegex(ValueError, "expected 2 shots"):
            parse_01_samples("00\n", expected_bits=2, expected_shots=2)
        with self.assertRaisesRegex(ValueError, "non-01"):
            parse_01_samples("0x\n11\n", expected_bits=2, expected_shots=2)

    def test_compare_distribution_accepts_five_sigma_frequencies(self) -> None:
        result = compare_distribution(["00"] * 50 + ["11"] * 50, {"00": 0.5, "11": 0.5})

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["sample_count"], 100)
        self.assertEqual(result["observed_counts"], {"00": 50, "11": 50})
        self.assertAlmostEqual(result["observed_frequencies"]["00"], 0.5)

    def test_outcome_tolerance_uses_exact_five_sigma_without_one_over_n_floor(self) -> None:
        self.assertEqual(
            _outcome_tolerance(sample_count=100, expected_probability=0.5, z_score=5.0),
            0.25,
        )

    def test_compare_distribution_flags_unexpected_observed_outcome(self) -> None:
        result = compare_distribution(["00"] * 90 + ["01"] * 10, {"00": 1.0})

        self.assertEqual(result["status"], "statistical_mismatch")
        self.assertGreater(result["max_delta"], result["max_tolerance"])
        self.assertTrue(any("01" in reason for reason in result["failure_reasons"]))

    def test_compare_distribution_keeps_zero_count_expected_rows(self) -> None:
        result = compare_distribution(["00"] * 100, {"00": 0.5, "11": 0.5})

        rows = {row["outcome"]: row for row in result["outcomes"]}
        self.assertEqual(rows["11"]["observed_count"], 0)
        self.assertEqual(rows["11"]["expected_probability"], 0.5)
        self.assertEqual(rows["11"]["observed_frequency"], 0.0)

    def test_build_sample_command_uses_stdin_compatible_cli(self) -> None:
        self.assertEqual(
            build_sample_command(["rstim"], shots=4, seed=7),
            ["rstim", "sample", "--shots", "4", "--seed", "7", "--out_format", "01"],
        )

    def test_collect_environment_metadata_uses_full_stim_command_for_version_probe(self) -> None:
        fake_completed = mock.Mock(returncode=0, stdout="stim 1.2.3\n", stderr="")
        with (
            mock.patch(
                "benchmarks.rstim_vs_stim_simulator.verify_distributions.subprocess.run",
                return_value=fake_completed,
            ) as mocked_run,
            mock.patch(
                "benchmarks.rstim_vs_stim_simulator.verify_distributions.shutil.which",
                return_value="/usr/bin/rstim",
            ),
        ):
            metadata = collect_environment_metadata(
                ["python3", "-m", "stim"],
                ["rstim"],
            )

        mocked_run.assert_any_call(
            ["python3", "-m", "stim", "--version"],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(metadata["stim_version"], "stim 1.2.3")
        self.assertEqual(metadata["stim_version_source"], "stim-cli-stdout")
        self.assertEqual(metadata["stim_version_command"]["command"], ["python3", "-m", "stim", "--version"])
        self.assertEqual(metadata["rstim_binary_path"], "/usr/bin/rstim")

    def test_collect_environment_metadata_falls_back_to_python_stim_version_when_cli_stdout_is_empty(self) -> None:
        def fake_run(command: list[str], **kwargs: object) -> mock.Mock:
            if command == ["stim", "--version"]:
                return mock.Mock(returncode=0, stdout="", stderr="No mode was given.\n")
            if command == ["python3", "-c", "import stim; print(stim.__version__)"]:
                return mock.Mock(returncode=0, stdout="1.15.0\n", stderr="")
            if command == ["rustc", "--version"]:
                return mock.Mock(returncode=0, stdout="rustc 1.93.1\n", stderr="")
            if command == ["cargo", "--version"]:
                return mock.Mock(returncode=0, stdout="cargo 1.93.1\n", stderr="")
            raise AssertionError(f"unexpected command: {command}")

        with (
            mock.patch(
                "benchmarks.rstim_vs_stim_simulator.verify_distributions.subprocess.run",
                side_effect=fake_run,
            ),
            mock.patch(
                "benchmarks.rstim_vs_stim_simulator.verify_distributions.shutil.which",
                return_value="/usr/bin/rstim",
            ),
        ):
            metadata = collect_environment_metadata(["stim"], ["rstim"])

        self.assertEqual(metadata["stim_version"], "stim python package 1.15.0")
        self.assertEqual(metadata["stim_version_source"], "python-stim-module")
        self.assertEqual(metadata["stim_python_version"], "1.15.0")
        self.assertEqual(
            metadata["stim_python_version_command"]["command"],
            ["python3", "-c", "import stim; print(stim.__version__)"],
        )
        self.assertEqual(metadata["stim_version_command"]["stderr"], "No mode was given.")

    def test_direct_binary_path_only_accepts_unwrapped_single_token_commands(self) -> None:
        with mock.patch(
            "benchmarks.rstim_vs_stim_simulator.verify_distributions.shutil.which",
            return_value="/usr/bin/rstim",
        ) as mocked_which:
            self.assertEqual(_direct_binary_path(["rstim"]), "/usr/bin/rstim")
            self.assertIsNone(_direct_binary_path(["cargo"]))
            self.assertIsNone(_direct_binary_path(["python3", "-m", "stim"]))

        mocked_which.assert_called_once_with("rstim")


class VerifyDistributionRunnerTest(unittest.TestCase):
    def test_verify_case_records_expected_observed_tolerance_and_provenance(self) -> None:
        def fake_run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
            return {
                "command": command,
                "exit_code": 0,
                "stderr": "",
                "success": True,
                "stdout": "00\n11\n00\n11\n",
                "stdin_source": "catalog:circuit",
            }

        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_distributions.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                unit_case(),
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=4,
                seeds=[1],
                inject_rstim_bitflip_rate=0.0,
            )

        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["sample_count"], 4)
        self.assertEqual(result["expected_distribution"], {"00": 0.5, "11": 0.5})
        self.assertEqual(result["source_url"], "https://example.test/source")
        self.assertEqual(result["stim"]["observed_counts"], {"00": 2, "11": 2})
        self.assertEqual(result["rstim"]["observed_frequencies"], {"00": 0.5, "11": 0.5})
        self.assertEqual(result["stim"]["runs"][0]["command"][0], "stim")
        self.assertEqual(result["rstim"]["runs"][0]["stderr"], "")
        self.assertNotIn("stdout", result["stim"]["runs"][0])
        self.assertNotIn("elapsed_s", result["stim"]["runs"][0])

    def test_verify_case_reports_rstim_negative_control_mismatch(self) -> None:
        def fake_run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
            tool = command[0]
            if tool == "stim":
                stdout = "00\n00\n00\n00\n"
            else:
                stdout = "00\n00\n00\n00\n"
            return {
                "command": command,
                "exit_code": 0,
                "stderr": "",
                "success": True,
                "stdout": stdout,
                "stdin_source": "catalog:circuit",
            }

        case = unit_case()
        case["expected_distribution"] = {"00": 1.0}
        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_distributions.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                case,
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=4,
                seeds=[1],
                inject_rstim_bitflip_rate=1.0,
            )

        self.assertEqual(result["status"], "statistical_mismatch")
        self.assertEqual(result["stim"]["status"], "pass")
        self.assertEqual(result["rstim"]["status"], "statistical_mismatch")

    def test_verify_case_records_tool_failure_stderr(self) -> None:
        def fake_run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
            success = command[0] == "stim"
            return {
                "command": command,
                "exit_code": 0 if success else 2,
                "stderr": "" if success else "broken rstim",
                "success": success,
                "stdout": "00\n11\n00\n11\n" if success else "",
                "stdin_source": "catalog:circuit",
            }

        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_distributions.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                unit_case(),
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=4,
                seeds=[1],
                inject_rstim_bitflip_rate=0.0,
            )

        self.assertEqual(result["status"], "rstim_failed")
        self.assertIn("broken rstim", result["failure_reasons"][0])
        self.assertEqual(result["rstim"]["runs"][0]["stderr"], "broken rstim")

    def test_verify_case_labels_custom_stim_command_failure_as_stim_failed(self) -> None:
        def fake_run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
            success = command[0] != "/custom/stim"
            return {
                "command": command,
                "exit_code": 0 if success else 2,
                "stderr": "" if success else "broken stim",
                "success": success,
                "stdout": "00\n11\n00\n11\n" if success else "",
                "stdin_source": "catalog:circuit",
            }

        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_distributions.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                unit_case(),
                stim_command=["/custom/stim"],
                rstim_command=["rstim"],
                shots=4,
                seeds=[1],
                inject_rstim_bitflip_rate=0.0,
            )

        self.assertEqual(result["status"], "stim_failed")
        self.assertEqual(result["stim"]["status"], "stim_failed")
        self.assertEqual(result["rstim"]["status"], "pass")

    def test_verify_case_prioritizes_stim_failure_over_partial_sample_mismatch(self) -> None:
        mismatch_stdout = ("00\n" * 100)
        matching_stdout = ("00\n11\n" * 50)

        def fake_run_tool(command: list[str], *, circuit: str) -> dict[str, object]:
            tool = command[0]
            seed = command[command.index("--seed") + 1]
            if tool == "stim" and seed == "1":
                return {
                    "command": command,
                    "exit_code": 0,
                    "stderr": "",
                    "success": True,
                    "stdout": mismatch_stdout,
                    "stdin_source": "catalog:circuit",
                }
            if tool == "stim" and seed == "2":
                return {
                    "command": command,
                    "exit_code": 2,
                    "stderr": "broken stim on seed 2",
                    "success": False,
                    "stdout": "",
                    "stdin_source": "catalog:circuit",
                }
            return {
                "command": command,
                "exit_code": 0,
                "stderr": "",
                "success": True,
                "stdout": matching_stdout,
                "stdin_source": "catalog:circuit",
            }

        with mock.patch("benchmarks.rstim_vs_stim_simulator.verify_distributions.run_tool") as mocked:
            mocked.side_effect = fake_run_tool
            result = verify_case(
                unit_case(),
                stim_command=["stim"],
                rstim_command=["rstim"],
                shots=100,
                seeds=[1, 2],
                inject_rstim_bitflip_rate=0.0,
            )

        self.assertEqual(result["status"], "stim_failed")
        self.assertEqual(result["stim"]["status"], "stim_failed")
        self.assertIn("broken stim on seed 2", result["failure_reasons"][0])
        self.assertEqual(result["stim"]["observed_counts"], {"00": 100})


class VerifyDistributionCliTest(unittest.TestCase):
    def test_main_writes_json_and_prints_pass_summary(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "summary.json"
            manifest = {"suite": "rstim_vs_stim_simulator", "cases": [unit_case()]}
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.load_manifest",
                    return_value=manifest,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.validate_manifest",
                    return_value=[],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.verify_case"
                ) as mocked,
            ):
                mocked.return_value = {
                    "case_id": "unit_bell",
                    "status": "pass",
                    "sample_count": 4,
                    "failure_reasons": [],
                    "expected_distribution": {"00": 0.5, "11": 0.5},
                    "source_url": "https://example.test/source",
                    "stim": {"status": "pass"},
                    "rstim": {"status": "pass"},
                }
                with mock.patch("sys.stdout.write") as stdout:
                    code = main(
                        [
                            "--cases",
                            "benchmarks/rstim_vs_stim_simulator/distribution_cases.toml",
                            "--shots",
                            "4",
                            "--out",
                            str(out),
                        ]
                    )

            self.assertEqual(code, 0)
            data = json.loads(out.read_text())
            self.assertEqual(data["status"], "pass")
            self.assertEqual(data["case_count"], 1)
            self.assertEqual(data["counts"]["pass"], 1)
            self.assertTrue(
                any(
                    "PASS distribution correctness cases=1 mismatch=0" in call.args[0]
                    for call in stdout.call_args_list
                )
            )

    def test_main_negative_control_bitflip_writes_json_and_reports_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            out = Path(temp_dir) / "summary.json"
            manifest = {"suite": "rstim_vs_stim_simulator", "cases": [unit_case()]}
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.load_manifest",
                    return_value=manifest,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.validate_manifest",
                    return_value=[],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.verify_case"
                ) as mocked,
            ):
                mocked.return_value = {
                    "case_id": "unit_bell",
                    "status": "statistical_mismatch",
                    "sample_count": 4,
                    "failure_reasons": ["outcome 01 exceeds tolerance"],
                    "expected_distribution": {"00": 0.5, "11": 0.5},
                    "source_url": "https://example.test/source",
                    "stim": {"status": "pass"},
                    "rstim": {"status": "statistical_mismatch"},
                }
                with mock.patch("sys.stdout.write") as stdout:
                    code = main(
                        [
                            "--cases",
                            "benchmarks/rstim_vs_stim_simulator/distribution_cases.toml",
                            "--shots",
                            "4",
                            "--inject-rstim-bitflip-rate",
                            "0.20",
                            "--out",
                            str(out),
                        ]
                    )

            self.assertEqual(code, 1)
            data = json.loads(out.read_text())
            self.assertEqual(data["status"], "statistical_mismatch")
            self.assertEqual(data["inject_rstim_bitflip_rate"], 0.2)
            self.assertTrue(
                any(case["status"] == "statistical_mismatch" for case in data["cases"])
            )
            self.assertTrue(
                any("FAIL statistical mismatch" in call.args[0] for call in stdout.call_args_list)
            )

    def test_main_records_catalog_hash_command_line_and_environment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            cases = temp / "cases.toml"
            cases.write_text("manifest_version = 1\nsuite = \"unit\"\n[[cases]]\n", encoding="utf-8")
            out = temp / "summary.json"
            manifest = {"suite": "rstim_vs_stim_simulator", "cases": [unit_case()]}
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.load_manifest",
                    return_value=manifest,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.validate_manifest",
                    return_value=[],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.verify_case"
                ) as mocked_verify,
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.verify_distributions.collect_environment_metadata",
                    return_value={
                        "stim_command": ["stim"],
                        "rstim_command": ["target/debug/rstim"],
                        "rstim_binary_path": "target/debug/rstim",
                        "stim_version": "stim test",
                        "stim_version_source": "stim-cli-stdout",
                        "rustc_version": "rustc test",
                        "cargo_version": "cargo test",
                    },
                ),
            ):
                mocked_verify.return_value = {
                    "case_id": "unit_bell",
                    "status": "pass",
                    "sample_count": 4,
                    "failure_reasons": [],
                    "expected_distribution": {"00": 0.5, "11": 0.5},
                    "source_url": "https://example.test/source",
                    "source_commit": "abc123",
                    "source_line_start": 10,
                    "source_line_end": 20,
                    "stim": {"status": "pass"},
                    "rstim": {"status": "pass"},
                }
                code = main(
                    [
                        "--cases",
                        str(cases),
                        "--rstim",
                        "target/debug/rstim",
                        "--shots",
                        "4",
                        "--out",
                        str(out),
                    ]
                )

            self.assertEqual(code, 0)
            data = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(data["catalog_sha256"], sha256_text(cases))
            self.assertEqual(data["distribution_case_ids"], ["unit_bell"])
            self.assertEqual(data["command_line"][0], "python3")
            self.assertIn("--cases", data["command_line"])
            self.assertEqual(data["environment"]["rstim_binary_path"], "target/debug/rstim")
            self.assertEqual(data["environment"]["stim_version"], "stim test")
            self.assertEqual(data["environment"]["rustc_version"], "rustc test")

    def test_format_report_returns_nonzero_for_mismatch(self) -> None:
        summary = {
            "status": "statistical_mismatch",
            "case_count": 1,
            "counts": {
                "pass": 0,
                "statistical_mismatch": 1,
                "stim_failed": 0,
                "rstim_failed": 0,
            },
        }

        exit_code, report = format_report(summary)

        self.assertEqual(exit_code, 1)
        self.assertEqual(report, "FAIL statistical mismatch cases=1 mismatch=1")


if __name__ == "__main__":
    unittest.main()
