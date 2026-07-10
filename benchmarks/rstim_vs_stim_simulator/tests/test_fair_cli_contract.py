from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
import copy
import hashlib
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator import fair_cli_contract


ROOT = Path(__file__).resolve().parents[3]
FAIR_MANIFEST = ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "fair_cli_cases.toml"


def run_contract(path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.fair_cli_contract",
            "--manifest",
            str(path),
            "--case",
            "stim_surface_d11_r100",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


class FairCliContractTest(unittest.TestCase):
    def test_canonical_manifest_passes_with_required_output(self) -> None:
        result = run_contract(FAIR_MANIFEST)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout,
            "PASS fair CLI contract case=stim_surface_d11_r100 shots=1024 "
            "measurements=12121 format=b8 bytes_per_shot=1516 bytes=1552384 "
            "timer=cli_end_to_end\n",
        )
        self.assertEqual(result.stderr, "")

    def test_byte_count_is_independently_recomputed(self) -> None:
        bytes_per_shot = (12121 + 7) // 8

        self.assertEqual(bytes_per_shot, 1516)
        self.assertEqual(bytes_per_shot * 1024, 1552384)

    def test_expanded_argv_is_identical_except_for_binary(self) -> None:
        manifest = fair_cli_contract.load_manifest(FAIR_MANIFEST)
        case = fair_cli_contract.find_case(manifest, "stim_surface_d11_r100")

        stim_argv = fair_cli_contract.expand_argv(
            case["argv"]["stim-cli-b8"],
            case,
            seed=0,
            rstim_binary="target/release/rstim",
        )
        rstim_argv = fair_cli_contract.expand_argv(
            case["argv"]["rstim-cli-b8"],
            case,
            seed=0,
            rstim_binary="target/release/rstim",
        )

        self.assertEqual(
            stim_argv,
            [
                "stim",
                "sample",
                "--shots",
                "1024",
                "--seed",
                "0",
                "--out_format",
                "b8",
                "--in",
                "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
            ],
        )
        self.assertEqual(
            rstim_argv,
            [
                "target/release/rstim",
                "sample",
                "--shots",
                "1024",
                "--seed",
                "0",
                "--out_format",
                "b8",
                "--in",
                "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
            ],
        )

    def _assert_mutation_rejected(self, old: str, new: str, diagnostic: str) -> None:
        manifest_text = FAIR_MANIFEST.read_text(encoding="utf-8")
        self.assertIn(old, manifest_text)
        mutated = manifest_text.replace(old, new, 1)

        with tempfile.TemporaryDirectory() as temp_dir:
            manifest_path = Path(temp_dir) / "fair_cli_cases.toml"
            manifest_path.write_text(mutated, encoding="utf-8")
            result = run_contract(manifest_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(diagnostic, result.stderr)

    def test_rejects_asymmetric_output_format(self) -> None:
        self._assert_mutation_rejected(
            'rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{shots}", "--seed", "{seed}", "--out_format", "b8",',
            'rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{shots}", "--seed", "{seed}", "--out_format", "01",',
            "asymmetric output_format: expected b8",
        )

    def test_rejects_unknown_argv_placeholder_without_traceback(self) -> None:
        manifest_text = FAIR_MANIFEST.read_text(encoding="utf-8")
        old = 'rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{shots}",'
        new = 'rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{bad}",'
        self.assertIn(old, manifest_text)
        mutated = manifest_text.replace(old, new, 1)

        with tempfile.TemporaryDirectory() as temp_dir:
            manifest_path = Path(temp_dir) / "fair_cli_cases.toml"
            manifest_path.write_text(mutated, encoding="utf-8")
            result = run_contract(manifest_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("argv", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_rejects_indexed_argv_placeholder_without_traceback(self) -> None:
        manifest_text = FAIR_MANIFEST.read_text(encoding="utf-8")
        old = 'rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{shots}",'
        new = 'rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{shots[0]}",'
        self.assertIn(old, manifest_text)
        mutated = manifest_text.replace(old, new, 1)

        with tempfile.TemporaryDirectory() as temp_dir:
            manifest_path = Path(temp_dir) / "fair_cli_cases.toml"
            manifest_path.write_text(mutated, encoding="utf-8")
            result = run_contract(manifest_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("argv.rstim-cli-b8", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_rejects_malformed_argv_placeholder_without_traceback(self) -> None:
        manifest_text = FAIR_MANIFEST.read_text(encoding="utf-8")
        old = 'rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{shots}",'
        new = 'rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{shots",'
        self.assertIn(old, manifest_text)
        mutated = manifest_text.replace(old, new, 1)

        with tempfile.TemporaryDirectory() as temp_dir:
            manifest_path = Path(temp_dir) / "fair_cli_cases.toml"
            manifest_path.write_text(mutated, encoding="utf-8")
            result = run_contract(manifest_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("argv.rstim-cli-b8", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_rejects_wrong_timer_scope(self) -> None:
        self._assert_mutation_rejected(
            'timer_scope = "cli_end_to_end"',
            'timer_scope = "sample_only"',
            "timer_scope",
        )

    def test_rejects_wrong_canonical_input_path(self) -> None:
        self._assert_mutation_rejected(
            'canonical_input_path = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"',
            'canonical_input_path = "benchmarks/rstim_vs_stim_simulator/fixtures/missing.stim"',
            "canonical_input_path",
        )

    def test_rejects_wrong_canonical_input_sha256(self) -> None:
        self._assert_mutation_rejected(
            'canonical_input_sha256 = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"',
            'canonical_input_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"',
            "canonical_input_sha256",
        )

    def test_rejects_non_integer_measurement_count_with_diagnostic(self) -> None:
        self._assert_mutation_rejected(
            "measurement_count = 12121",
            'measurement_count = "bad"',
            "measurement_count: expected integer",
        )

    def test_rejects_missing_or_non_string_source_canonical_input_path(self) -> None:
        manifest = fair_cli_contract.load_manifest(FAIR_MANIFEST)
        case = copy.deepcopy(fair_cli_contract.find_case(manifest, "stim_surface_d11_r100"))
        fixture_bytes = b"temporary fixture\n"
        fixture_digest = hashlib.sha256(fixture_bytes).hexdigest()

        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            fixture_path = repo_root / "fixture.stim"
            fixture_path.write_bytes(fixture_bytes)
            case["canonical_input_path"] = "fixture.stim"
            case["canonical_input_sha256"] = fixture_digest
            case["source_manifest_path"] = "source.toml"

            for source_path_value in (None, 123):
                source_manifest = {
                    "cases": [
                        {
                            "case_id": case["source_manifest_case_id"],
                            "canonical_input_path": source_path_value,
                            "expected_measurements": case["measurement_count"],
                            "shots": case["shots"],
                            "stim_version": case["stim_version"],
                        }
                    ]
                }
                source_lines = [
                    "[[cases]]",
                    f'case_id = "{source_manifest["cases"][0]["case_id"]}"',
                    f'expected_measurements = {case["measurement_count"]}',
                    f'shots = {case["shots"]}',
                    f'stim_version = "{case["stim_version"]}"',
                ]
                if source_path_value is not None:
                    source_lines.append(f"canonical_input_path = {source_path_value}")
                (repo_root / "source.toml").write_text("\n".join(source_lines) + "\n", encoding="utf-8")

                errors = fair_cli_contract.validate_case(
                    case,
                    manifest_path=repo_root / "fair.toml",
                    repo_root=repo_root,
                )

                self.assertTrue(
                    any("canonical_input_path: source manifest" in error for error in errors),
                    errors,
                )


if __name__ == "__main__":
    unittest.main()
