from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator.validate_distribution_cases import validate_manifest


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "rstim_vs_stim_simulator"
FIXTURES = PACKAGE_DIR / "tests" / "fixtures"
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


if __name__ == "__main__":
    unittest.main()
