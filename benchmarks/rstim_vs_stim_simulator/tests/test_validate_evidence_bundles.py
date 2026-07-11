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
SCRIPT = PACKAGE_DIR / "validate_evidence_bundles.py"
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


def valid_catalog_fixture() -> dict[str, object]:
    return copy.deepcopy(load_catalog(CATALOG))


class ValidateEvidenceBundlesTest(unittest.TestCase):
    def test_direct_script_help_imports_package(self) -> None:
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--help"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--catalog", result.stdout)

    def test_validate_catalog_accepts_valid_in_memory_catalog(self) -> None:
        self.assertEqual(validate_catalog(valid_catalog_fixture(), CATALOG), [])

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

    def test_runtime_identity_rejects_live_path_fields(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["runtime_identities"][0]["path"] = "/opt/homebrew/bin/stim"

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(any("runtime identity unsupported field(s): path" in error for error in errors), errors)

    def test_checked_command_executable_must_be_logical_role(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["checked_commands"][0]["argv"][0] = "stim"

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(
            any("checked command executable must be declared tool:// role" in error for error in errors),
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

    def test_checked_commands_scan_extra_fields_for_host_paths(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["checked_commands"][0]["cwd"] = "/tmp/build"

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(any("checked command contains host-absolute path" in error for error in errors), errors)

    def test_checked_provenance_scans_extra_fields_for_host_paths(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["checked_provenance"][0]["host_path"] = "/tmp/provenance.json"

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(any("checked provenance contains host-absolute path" in error for error in errors), errors)

    def test_artifacts_must_cover_bundle_files(self) -> None:
        catalog = load_catalog(CATALOG)
        mutated = copy.deepcopy(catalog)
        mutated["bundles"][0]["artifacts"] = [
            artifact for artifact in mutated["bundles"][0]["artifacts"] if artifact["path"] != "summary.json"
        ]

        errors = validate_catalog(mutated, CATALOG)

        self.assertTrue(any("artifact catalog missing bundle file: summary.json" in error for error in errors), errors)

    def test_rejects_windows_and_unc_host_paths(self) -> None:
        catalog = valid_catalog_fixture()
        catalog["bundles"][0]["repository_inputs"][0]["path"] = "C:\\tmp\\fixture.stim"
        catalog["bundles"][1]["checked_provenance"][0]["value"] = "\\\\server\\share\\fixture.stim"

        errors = validate_catalog(catalog, CATALOG)

        self.assertTrue(any("repository path must be relative" in error for error in errors), errors)
        self.assertTrue(any("checked provenance contains host-absolute path" in error for error in errors), errors)

    def test_rejects_artifact_digest_mismatch(self) -> None:
        catalog = valid_catalog_fixture()
        catalog["bundles"][0]["artifacts"][0]["sha256"] = "0" * 64

        errors = validate_catalog(catalog, CATALOG)

        self.assertTrue(any("artifacts[0] sha256 mismatch" in error for error in errors), errors)

    def test_rejects_missing_runtime_identity_fields(self) -> None:
        catalog = valid_catalog_fixture()
        del catalog["bundles"][0]["runtime_identities"][0]["basename"]

        errors = validate_catalog(catalog, CATALOG)

        self.assertTrue(any("missing required field(s): basename" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
