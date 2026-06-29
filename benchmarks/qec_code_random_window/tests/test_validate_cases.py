from __future__ import annotations

import subprocess
import sys
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "qec_code_random_window"
SMOKE_MANIFEST = PACKAGE_DIR / "cases.smoke.toml"
FULL_MANIFEST = PACKAGE_DIR / "cases.full.toml"
FIXTURES = PACKAGE_DIR / "tests" / "fixtures"
BB144_CODE_ID = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"


def run_validator(path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.qec_code_random_window.validate_cases",
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


def cases_by_id(path: Path) -> dict[str, dict[str, object]]:
    manifest = load_manifest(path)
    cases = manifest["cases"]
    assert isinstance(cases, list)
    return {case["case_id"]: case for case in cases}


class ValidateCasesTest(unittest.TestCase):
    def test_smoke_manifest_cli_prints_pass(self) -> None:
        result = run_validator(SMOKE_MANIFEST)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS\n")
        self.assertEqual(result.stderr, "")

    def test_full_manifest_cli_prints_pass(self) -> None:
        result = run_validator(FULL_MANIFEST)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS\n")
        self.assertEqual(result.stderr, "")

    def test_duplicate_case_id_fixture_is_rejected_and_names_id(self) -> None:
        result = run_validator(FIXTURES / "duplicate_case_id.toml")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate case_id", result.stderr)
        self.assertIn("duplicate_case", result.stderr)

    def test_strict_baseline_fixture_requires_usable_key(self) -> None:
        result = run_validator(FIXTURES / "strict_baseline_missing_key.toml")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("strict_missing_baseline", result.stderr)
        self.assertIn("baseline_required = true", result.stderr)
        self.assertIn("baseline_key", result.stderr)

    def test_smoke_manifest_pins_required_cases_and_baseline_contract(self) -> None:
        cases = cases_by_id(SMOKE_MANIFEST)

        self.assertEqual(
            tuple(cases),
            (
                "steane_smoke",
                "surface_rotated_d3_smoke",
                "toric_d3_smoke",
                "bb72_smoke",
            ),
        )
        self.assertEqual(cases["steane_smoke"]["code_id"], "steane")
        self.assertEqual(cases["surface_rotated_d3_smoke"]["code_id"], "surface_rotated:d=3")
        self.assertEqual(cases["toric_d3_smoke"]["code_id"], "toric:d=3")
        self.assertEqual(cases["bb72_smoke"]["code_id"], "bb72")
        self.assertFalse(cases["steane_smoke"]["baseline_required"])
        self.assertFalse(cases["surface_rotated_d3_smoke"]["baseline_required"])
        self.assertFalse(cases["toric_d3_smoke"]["baseline_required"])
        self.assertTrue(cases["bb72_smoke"]["baseline_required"])
        self.assertEqual(
            cases["bb72_smoke"]["baseline_key"],
            "codeDistancePYPI:bivariate_bicycle:bb72",
        )

    def test_full_manifest_includes_larger_bb_case(self) -> None:
        cases = cases_by_id(FULL_MANIFEST)

        self.assertIn("bb144_full", cases)
        self.assertEqual(cases["bb144_full"]["code_id"], BB144_CODE_ID)
        self.assertEqual(cases["bb144_full"]["target_upper_bound"], 12)
        self.assertTrue(cases["bb144_full"]["baseline_required"])
        self.assertEqual(
            cases["bb144_full"]["baseline_key"],
            "codeDistancePYPI:bivariate_bicycle:bb144",
        )


if __name__ == "__main__":
    unittest.main()
