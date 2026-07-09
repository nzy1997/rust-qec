from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
import tomllib

from benchmarks.rstim_vs_stim_simulator.validate_distribution_cases import validate_manifest


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "rstim_vs_stim_simulator"
FIXTURES = PACKAGE_DIR / "tests" / "fixtures"
DISTRIBUTION_MANIFEST = PACKAGE_DIR / "distribution_cases.toml"
PINNED_COMMIT = "9e225958f9ae1f9c33d1b9a012b7ec4392b43aef"
SOURCE_URL = (
    "https://github.com/quantumlib/Stim/blob/"
    f"{PINNED_COMMIT}/src/stim/cmd/command_sample.test.cc"
)


def minimal_manifest() -> dict[str, object]:
    return {
        "manifest_version": 1,
        "suite": "rstim_vs_stim_simulator",
        "description": "test distribution cases",
        "distribution_tolerance": 1e-9,
        "cases": [
            {
                "case_id": "unit_bell",
                "source_url": SOURCE_URL,
                "source_commit": PINNED_COMMIT,
                "source_line_start": 160,
                "source_line_end": 169,
                "circuit": "H 0\nCNOT 0 1\nM 0 1\n",
                "shots": 10000,
                "tolerance": 1e-9,
                "expected_distribution": {"00": 0.5, "11": 0.5},
            }
        ],
    }


def run_validator(path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.validate_distribution_cases",
            str(path),
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def load_manifest(path: Path) -> dict[str, object]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def distribution_cases_by_id() -> dict[str, dict[str, object]]:
    manifest = load_manifest(DISTRIBUTION_MANIFEST)
    cases = manifest["cases"]
    assert isinstance(cases, list)
    return {case["case_id"]: case for case in cases}


class ValidateDistributionCasesTest(unittest.TestCase):
    def test_validate_manifest_accepts_minimal_source_grounded_case(self) -> None:
        self.assertEqual(validate_manifest(minimal_manifest()), [])

    def test_validate_manifest_rejects_missing_source_commit(self) -> None:
        manifest = minimal_manifest()
        case = manifest["cases"][0]
        assert isinstance(case, dict)
        del case["source_commit"]

        errors = validate_manifest(manifest)

        self.assertTrue(any("source_commit" in error for error in errors), errors)

    def test_validate_manifest_rejects_missing_source_line_metadata(self) -> None:
        manifest = minimal_manifest()
        case = manifest["cases"][0]
        assert isinstance(case, dict)
        del case["source_line_start"]

        errors = validate_manifest(manifest)

        self.assertTrue(any("source_line_start" in error for error in errors), errors)

    def test_bad_distribution_sum_negative_control_cli_fails(self) -> None:
        result = run_validator(FIXTURES / "bad_distribution_sum.toml")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("expected distribution probabilities must sum to 1", result.stderr)

    def test_cli_accepts_single_valid_distribution_case(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "one_case.toml"
            path.write_text(
                f'''\
manifest_version = 1
suite = "rstim_vs_stim_simulator"
description = "one valid case"
distribution_tolerance = 1e-9

[[cases]]
case_id = "unit_bell"
source_url = "{SOURCE_URL}"
source_commit = "{PINNED_COMMIT}"
source_line_start = 160
source_line_end = 169
circuit = """
H 0
CNOT 0 1
M 0 1
"""
shots = 10000
tolerance = 1e-9
expected_distribution = {{ "00" = 0.5, "11" = 0.5 }}
''',
                encoding="utf-8",
            )

            result = run_validator(path)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS 1 distribution cases\n")
        self.assertEqual(result.stderr, "")

    def test_distribution_catalog_cli_prints_case_count(self) -> None:
        result = run_validator(DISTRIBUTION_MANIFEST)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS 8 distribution cases\n")
        self.assertEqual(result.stderr, "")

    def test_distribution_catalog_pins_expected_case_ids(self) -> None:
        cases = distribution_cases_by_id()

        self.assertEqual(
            tuple(cases),
            (
                "stim_bell_pair_basic_distribution",
                "stim_sqrt_x_transformed_pair",
                "stim_sqrt_y_transformed_pair",
                "stim_x_error_two_measured_qubits",
                "stim_z_error_h_conjugated_pair",
                "stim_y_error_two_measured_qubits",
                "stim_depolarize1_two_measured_qubits",
                "stim_depolarize2_two_measured_qubits",
            ),
        )

    def test_distribution_catalog_records_representative_probabilities(self) -> None:
        cases = distribution_cases_by_id()

        bell = cases["stim_bell_pair_basic_distribution"]["expected_distribution"]
        sqrt_x = cases["stim_sqrt_x_transformed_pair"]["expected_distribution"]
        depolarize1 = cases["stim_depolarize1_two_measured_qubits"]["expected_distribution"]
        depolarize2 = cases["stim_depolarize2_two_measured_qubits"]["expected_distribution"]
        self.assertEqual(bell, {"00": 0.5, "11": 0.5})
        self.assertEqual(sqrt_x, {"10": 0.5, "01": 0.5})
        self.assertEqual(depolarize1, {"00": 0.64, "01": 0.16, "10": 0.16, "11": 0.04})
        self.assertAlmostEqual(depolarize2["00"], 0.92)
        self.assertAlmostEqual(depolarize2["01"], 0.1 * 4 / 15)


if __name__ == "__main__":
    unittest.main()
