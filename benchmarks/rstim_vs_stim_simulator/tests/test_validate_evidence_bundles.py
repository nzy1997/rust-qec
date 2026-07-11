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
FIXTURE = PACKAGE_DIR / "fixtures" / "stim_surface_code_rotated_memory_z_d11_r100.stim"
FAIR_RESULTS = PACKAGE_DIR / "results" / "fair-cli-release"
REQUIRED_BUNDLE_IDS = (
    "fair-cli-release",
    "compiled-steady-release",
    "reference-build-release",
    "frame-instruction-wide-release",
)
FIXTURE_SHA256 = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
FAIR_RAW_SHA256 = "7548768a65425c55ca51480a4bdbb45b0a7e2462810f05498a195c02a8318a48"


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


def minimal_valid_catalog() -> dict[str, object]:
    return {
        "schema": 2,
        "suite": "rstim_vs_stim_simulator",
        "bundles": [
            {
                "id": "fair-cli-release",
                "bundle_path": "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release",
                "repository_inputs": [
                    {
                        "path": "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
                        "sha256": FIXTURE_SHA256,
                    },
                ],
                "artifacts": [
                    {"path": "raw.jsonl", "sha256": FAIR_RAW_SHA256},
                ],
                "logical_executables": [
                    {"role": "tool://stim"},
                    {"role": "tool://rstim"},
                ],
                "runtime_identities": [
                    {
                        "role": "tool://stim",
                        "version": "1.15.0",
                        "basename": "stim",
                        "sha256": "e7f31b9ac1780080161b3992e70644ade97dbe97369a9464997645c437a29323",
                    },
                ],
                "checked_commands": [
                    {
                        "name": "sample",
                        "argv": [
                            "tool://stim",
                            "sample",
                            "--in",
                            "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
                        ],
                    },
                ],
                "checked_provenance": [
                    {
                        "name": "fixture",
                        "value": "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
                    },
                ],
            },
            {
                "id": "compiled-steady-release",
                "bundle_path": "benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release",
                "repository_inputs": [],
                "artifacts": [],
                "logical_executables": [{"role": "tool://python"}],
                "runtime_identities": [
                    {
                        "role": "tool://python",
                        "version": "3.14.3",
                        "basename": "python3.14",
                        "sha256": "cbf84109626aa1013bbe408fbb9590bd0f1c1548f038b2221c6b8b87de26ca43",
                    },
                ],
                "checked_commands": [{"name": "worker", "argv": ["tool://python", "-m", "worker"]}],
                "checked_provenance": [{"name": "timer_scope", "value": "cli_end_to_end"}],
            },
            {
                "id": "reference-build-release",
                "bundle_path": "benchmarks/rstim_vs_stim_simulator/results/reference-build-release",
                "repository_inputs": [],
                "artifacts": [],
                "logical_executables": [{"role": "tool://rstim"}],
                "runtime_identities": [
                    {
                        "role": "tool://rstim",
                        "version": "rstim 0.1.1",
                        "basename": "rstim_reference_build_worker",
                        "sha256": "82d395176ebe76d6890bb9e747771fac46a019867287f69b0da8d6d5075e1265",
                    },
                ],
                "checked_commands": [{"name": "reference", "argv": ["tool://rstim", "--protocol", "reference-build-v1"]}],
                "checked_provenance": [{"name": "protocol", "value": "reference-build-v1"}],
            },
            {
                "id": "frame-instruction-wide-release",
                "bundle_path": "benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release",
                "repository_inputs": [],
                "artifacts": [],
                "logical_executables": [{"role": "tool://rstim"}],
                "runtime_identities": [
                    {
                        "role": "tool://rstim",
                        "version": "rstim 0.1.1",
                        "basename": "rstim",
                        "sha256": "336ab36864ba884314507d39378628aa653f16f9c51693512da510cbf3982568",
                    },
                ],
                "checked_commands": [{"name": "sample", "argv": ["tool://rstim", "sample"]}],
                "checked_provenance": [{"name": "timer_scope", "value": "process_spawn_stdout_stderr_drain_exit"}],
            },
        ],
    }


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

    def test_validate_catalog_accepts_minimal_valid_shape(self) -> None:
        self.assertEqual(validate_catalog(minimal_valid_catalog(), CATALOG), [])

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

    def test_rejects_windows_and_unc_host_paths(self) -> None:
        catalog = minimal_valid_catalog()
        catalog["bundles"][0]["repository_inputs"][0]["path"] = "C:\\tmp\\fixture.stim"
        catalog["bundles"][1]["checked_provenance"][0]["value"] = "\\\\server\\share\\fixture.stim"

        errors = validate_catalog(catalog, CATALOG)

        self.assertTrue(any("repository path must be relative" in error for error in errors), errors)
        self.assertTrue(any("checked provenance contains host-absolute path" in error for error in errors), errors)

    def test_rejects_artifact_digest_mismatch(self) -> None:
        catalog = minimal_valid_catalog()
        catalog["bundles"][0]["artifacts"][0]["sha256"] = "0" * 64

        errors = validate_catalog(catalog, CATALOG)

        self.assertTrue(any("artifacts[0] sha256 mismatch" in error for error in errors), errors)

    def test_rejects_missing_runtime_identity_fields(self) -> None:
        catalog = minimal_valid_catalog()
        del catalog["bundles"][0]["runtime_identities"][0]["basename"]

        errors = validate_catalog(catalog, CATALOG)

        self.assertTrue(any("missing required field(s): basename" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
