from __future__ import annotations

import copy
import subprocess
import sys
import unittest
from pathlib import Path

from benchmarks.rstim_vs_stim_simulator.portable_provenance import (
    EXPECTED_BUNDLE_IDS,
    load_catalog,
    validate_catalog,
)


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "rstim_vs_stim_simulator"
CATALOG = PACKAGE_DIR / "evidence_bundles.toml"
REQUIRED_BUNDLE_IDS = (
    "fair-cli-release",
    "compiled-steady-release",
    "reference-build-release",
    "frame-instruction-wide-release",
)


def run_validator(path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles",
            "--catalog",
            str(path),
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


class ValidateEvidenceBundlesTest(unittest.TestCase):
    def test_cli_accepts_committed_catalog(self) -> None:
        result = run_validator(CATALOG)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS portable evidence catalog bundles=4 schema=2\n")
        self.assertEqual(result.stderr, "")

    def test_catalog_pins_exact_schema_and_bundle_ids(self) -> None:
        catalog = load_catalog(CATALOG)
        bundles = catalog["bundles"]

        self.assertEqual(catalog["schema"], 2)
        self.assertEqual(tuple(bundle["id"] for bundle in bundles), REQUIRED_BUNDLE_IDS)
        self.assertEqual(EXPECTED_BUNDLE_IDS, REQUIRED_BUNDLE_IDS)

    def test_repository_inputs_reject_host_absolute_paths(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["repository_inputs"][0]["path"] = "/tmp/fixture.stim"

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(any("repository path must be relative" in error for error in errors), errors)

    def test_runtime_identity_rejects_required_live_path(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["runtime_identities"][0]["required_live_path"] = True

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(
            any("checked evidence must not require a live runtime path" in error for error in errors),
            errors,
        )

    def test_checked_commands_reject_host_absolute_paths(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["checked_commands"][0]["argv"] = [
            "tool://stim",
            "sample",
            "--in",
            "/tmp/fixture.stim",
        ]

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(any("checked command contains host-absolute path" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
