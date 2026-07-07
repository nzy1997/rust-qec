from __future__ import annotations

import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator.validate_cases import validate_manifest


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "rstim_vs_stim_simulator"
SMOKE_MANIFEST = PACKAGE_DIR / "cases.smoke.toml"
FULL_MANIFEST = PACKAGE_DIR / "cases.full.toml"
FIXTURES = PACKAGE_DIR / "tests" / "fixtures"


def run_validator(path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.validate_cases",
            str(path),
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def run_validator_with_cwd(path: Path, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.validate_cases",
            str(path),
        ],
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
    )


def load_manifest(path: Path) -> dict[str, object]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def cases_by_id(path: Path) -> dict[str, dict[str, object]]:
    manifest = load_manifest(path)
    cases = manifest["cases"]
    assert isinstance(cases, list)
    return {case["case_id"]: case for case in cases}


class ValidateCasesTest(unittest.TestCase):
    def test_smoke_manifest_cli_prints_case_count(self) -> None:
        result = run_validator(SMOKE_MANIFEST)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS 3 fixture cases\n")
        self.assertEqual(result.stderr, "")

    def test_full_manifest_cli_prints_case_count(self) -> None:
        result = run_validator(FULL_MANIFEST)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS 1 fixture cases\n")
        self.assertEqual(result.stderr, "")

    def test_uniform_noise_negative_control_rejects_round_data_depolarization(self) -> None:
        result = run_validator(FIXTURES / "bad_uniform_noise.toml")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "before_round_data_depolarization must be 0 for stim_surface_d11_r100",
            result.stderr,
        )
        self.assertNotIn("canonical_input_path", result.stderr)

    def test_validate_manifest_accepts_manifest_base_dir_contract(self) -> None:
        errors = validate_manifest(load_manifest(FIXTURES / "bad_uniform_noise.toml"), FIXTURES)

        self.assertIn(
            "before_round_data_depolarization must be 0 for stim_surface_d11_r100",
            errors,
        )
        self.assertFalse(
            any("canonical_input_path" in error for error in errors),
            errors,
        )

    def test_nested_manifest_falls_back_to_benchmark_root_fixture_path(self) -> None:
        manifest_text = """\
manifest_version = 1
suite = "rstim_vs_stim_simulator"
generated_outputs_root = "benchmarks/out/rstim_vs_stim_simulator"

[[cases]]
case_id = "stim_surface_d11_r100"
tier = "full"
source = "stim"
workload = "surface_code:rotated_memory_z"
code = "surface_code"
task = "rotated_memory_z"
distance = 11
rounds = 100
canonical_input_path = "fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
generation_command = "stim gen --code surface_code --task rotated_memory_z --distance 11 --rounds 100 --after_clifford_depolarization 0.001 --after_reset_flip_probability 0.001 --before_measure_flip_probability 0.001 --before_round_data_depolarization 0 --out benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
after_clifford_depolarization = 0.001
after_reset_flip_probability = 0.001
before_measure_flip_probability = 0.001
before_round_data_depolarization = 0
shots = 1024
expected_qubits = 274
expected_measurements = 12121
expected_detectors = 12000
expected_observables = 1
stim_version = "1.15.0"
provenance = "Canonical full Stim-generated surface_code:rotated_memory_z fixture for later simulator speed and correctness comparisons."
"""

        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = Path(temp_dir)
            nested_dir = temp_root / "nested" / "manifests"
            nested_dir.mkdir(parents=True)
            manifest_path = nested_dir / "fixture.toml"
            manifest_path.write_text(manifest_text)

            result = run_validator_with_cwd(manifest_path, ROOT)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS 1 fixture cases\n")
        self.assertEqual(result.stderr, "")

    def test_smoke_manifest_pins_three_catalog_entries(self) -> None:
        cases = cases_by_id(SMOKE_MANIFEST)

        self.assertEqual(
            tuple(cases),
            (
                "stim_repetition_d3_r3_smoke",
                "stim_surface_d3_r3_smoke",
                "stim_surface_d11_r100",
            ),
        )
        self.assertEqual(cases["stim_repetition_d3_r3_smoke"]["tier"], "smoke")
        self.assertEqual(cases["stim_surface_d3_r3_smoke"]["tier"], "smoke")
        self.assertEqual(cases["stim_surface_d11_r100"]["tier"], "documentation-only")

    def test_full_case_pins_stim_noise_contract_and_counts(self) -> None:
        cases = cases_by_id(FULL_MANIFEST)
        case = cases["stim_surface_d11_r100"]

        self.assertEqual(case["workload"], "surface_code:rotated_memory_z")
        self.assertEqual(case["distance"], 11)
        self.assertEqual(case["rounds"], 100)
        self.assertEqual(case["shots"], 1024)
        self.assertEqual(case["after_clifford_depolarization"], 0.001)
        self.assertEqual(case["after_reset_flip_probability"], 0.001)
        self.assertEqual(case["before_measure_flip_probability"], 0.001)
        self.assertEqual(case["before_round_data_depolarization"], 0)
        self.assertEqual(case["expected_qubits"], 274)
        self.assertEqual(case["expected_measurements"], 12121)
        self.assertEqual(case["expected_detectors"], 12000)
        self.assertEqual(case["expected_observables"], 1)


if __name__ == "__main__":
    unittest.main()
