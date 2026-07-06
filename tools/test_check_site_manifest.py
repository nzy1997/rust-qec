#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

import tools.check_site_manifest as check_site_manifest
import tools.copy_site_benchmark_data as copy_site_benchmark_data


VALID_MANIFEST = {
    "schema_version": 1,
    "families": [
        {
            "id": "surface-decoder-comparison",
            "title": "Surface Decoder Comparison",
            "status": "existing",
            "source_docs": ["docs/showcases/benchmark-evidence.md"],
            "claims_limit": "Checked full artifacts are committed-run evidence, not a general decoder ordering claim.",
            "evidence_items": [
                {
                    "id": "surface-decoder-full",
                    "title": "Checked surface-decoder full artifacts",
                    "status": "existing",
                    "tier": "full",
                    "artifacts": [
                        {
                            "path": "benchmarks/surface_decoder_compare/results/full/results.csv",
                            "kind": "csv",
                            "checked": True,
                        },
                        {
                            "path": "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
                            "kind": "image",
                            "checked": True,
                        },
                    ],
                    "commands": ["make surface-decoder-compare-full"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                    "claims_limit": "Fixture claim limit.",
                }
            ],
        },
        {
            "id": "bb-circuit-bposd-comparison",
            "title": "BB Circuit BP-OSD Comparison",
            "status": "partial",
            "source_docs": ["docs/showcases/benchmark-evidence.md"],
            "claims_limit": "BB72/BB144 only.",
            "evidence_items": [
                {
                    "id": "bb-circuit-full",
                    "title": "Checked BB full artifacts",
                    "status": "existing",
                    "tier": "full",
                    "artifacts": [],
                    "commands": ["make bb-circuit-bposd-compare-full"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                    "claims_limit": "Fixture claim limit.",
                }
            ],
        },
        {
            "id": "qec-code-random-window",
            "title": "qec-code Random Window",
            "status": "local-only",
            "source_docs": ["benchmarks/qec_code_random_window/README.md"],
            "claims_limit": "Generated outputs are ignored local evidence.",
            "evidence_items": [
                {
                    "id": "qec-code-smoke",
                    "title": "Local smoke command",
                    "status": "local-only",
                    "tier": "smoke",
                    "artifacts": [],
                    "commands": ["make qec-code-random-window-bench-smoke"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["benchmarks/qec_code_random_window/README.md"],
                    "claims_limit": "Local wiring check only.",
                }
            ],
        },
        {
            "id": "rstim-vs-stim-simulator",
            "title": "rstim versus Stim Simulator",
            "status": "future",
            "source_docs": ["docs/showcases/benchmark-evidence.md"],
            "claims_limit": "No current site-facing benchmark artifacts.",
            "evidence_items": [
                {
                    "id": "rstim-stim-future",
                    "title": "Future simulator benchmark",
                    "status": "future",
                    "tier": "future",
                    "artifacts": [],
                    "commands": [],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": ["docs/showcases/benchmark-evidence.md"],
                    "claims_limit": "Planning entry only.",
                }
            ],
        },
        {
            "id": "internal-regression-evidence",
            "title": "Internal Regression Evidence",
            "status": "partial",
            "source_docs": [".github/workflows/ci.yml"],
            "claims_limit": "Regression gate evidence only.",
            "evidence_items": [
                {
                    "id": "rstim-perf-ci",
                    "title": "rstim perf CI",
                    "status": "partial",
                    "tier": "regression-gate",
                    "artifacts": [],
                    "commands": ["cargo run -p rstim --bin rstim -- perf ci --out-dir perf-artifacts"],
                    "provenance_requirements": ["command line", "date"],
                    "provenance_sources": [".github/workflows/ci.yml"],
                    "claims_limit": "Regression gate evidence only.",
                }
            ],
        },
    ],
}


class SiteManifestValidatorTest(unittest.TestCase):
    def write_fixture_manifest(self, remove_family: str | None = None, mutation: str | None = None):
        tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(tmpdir.cleanup)
        root = Path(tmpdir.name)

        (root / ".gitignore").write_text("/benchmarks/out/\n", encoding="utf-8")
        (root / "docs/showcases").mkdir(parents=True)
        (root / "benchmarks/surface_decoder_compare/results/full").mkdir(parents=True)
        (root / "benchmarks/qec_code_random_window").mkdir(parents=True)
        (root / ".github/workflows").mkdir(parents=True)
        (root / "site").mkdir(parents=True)
        (root / "benchmarks/out").mkdir(parents=True)

        (root / "docs/showcases/benchmark-evidence.md").write_text("# Benchmark Evidence\n", encoding="utf-8")
        (root / "benchmarks/surface_decoder_compare/results/full/results.csv").write_text("distance,shots\n", encoding="utf-8")
        (root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").write_text("png\n", encoding="utf-8")
        (root / "benchmarks/surface_decoder_compare/results/full/unchecked.csv").write_text("unchecked\n", encoding="utf-8")
        (root / "benchmarks/qec_code_random_window/README.md").write_text("# Random Window\n", encoding="utf-8")
        (root / ".github/workflows/ci.yml").write_text("name: ci\n", encoding="utf-8")
        (root / "benchmarks/out/ignored.csv").write_text("ignored\n", encoding="utf-8")
        (root / "benchmarks/out/local-only.csv").write_text("local\n", encoding="utf-8")

        manifest = json.loads(json.dumps(VALID_MANIFEST))
        if remove_family is not None:
            manifest["families"] = [family for family in manifest["families"] if family["id"] != remove_family]
        if mutation == "missing_artifact":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = "benchmarks/missing/results.csv"
        elif mutation == "missing_claims_limit":
            del manifest["families"][0]["evidence_items"][0]["claims_limit"]
        elif mutation == "ignored_artifact":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = "benchmarks/out/ignored.csv"
        elif mutation == "force_tracked_ignored_artifact":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = "benchmarks/out/ignored.csv"
        elif mutation == "unchecked_tracked_artifact":
            manifest["families"][0]["evidence_items"][0]["artifacts"].append(
                {
                    "path": "benchmarks/surface_decoder_compare/results/full/unchecked.csv",
                    "kind": "csv",
                    "checked": False,
                }
            )
        elif mutation == "bad_artifact_path_type":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["path"] = 42
        elif mutation == "bad_artifact_kind_type":
            manifest["families"][0]["evidence_items"][0]["artifacts"][0]["kind"] = []
        elif mutation == "bad_commands_type":
            manifest["families"][0]["evidence_items"][0]["commands"] = "make surface-decoder-compare-full"
        elif mutation == "bad_provenance_requirements_type":
            manifest["families"][0]["evidence_items"][0]["provenance_requirements"] = ["command line", 123]
        elif mutation == "duplicate_item_id":
            duplicate = json.loads(json.dumps(manifest["families"][0]["evidence_items"][0]))
            manifest["families"][0]["evidence_items"].append(duplicate)
        elif mutation == "cross_family_duplicate_item_id":
            manifest["families"][1]["evidence_items"][0]["id"] = "surface-decoder-full"
        elif mutation == "empty_source_docs":
            manifest["families"][0]["source_docs"] = []
        elif mutation == "empty_provenance_sources":
            manifest["families"][0]["evidence_items"][0]["provenance_sources"] = []

        manifest_path = root / "site/benchmark-site.json"
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

        subprocess.run(["git", "init", "-q"], cwd=root, check=True)
        subprocess.run(
            [
                "git",
                "add",
                ".gitignore",
                "docs/showcases/benchmark-evidence.md",
                "benchmarks/surface_decoder_compare/results/full/results.csv",
                "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
                "benchmarks/surface_decoder_compare/results/full/unchecked.csv",
                "benchmarks/qec_code_random_window/README.md",
                ".github/workflows/ci.yml",
                "site/benchmark-site.json",
            ],
            cwd=root,
            check=True,
        )
        if mutation == "force_tracked_ignored_artifact":
            subprocess.run(["git", "add", "-f", "benchmarks/out/ignored.csv"], cwd=root, check=True)
        return root, manifest_path

    def test_accepts_valid_fixture_and_reports_families(self) -> None:
        repo, manifest_path = self.write_fixture_manifest()
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertEqual(errors, [])

    def test_rejects_missing_required_family(self) -> None:
        repo, manifest_path = self.write_fixture_manifest(remove_family="rstim-vs-stim-simulator")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("manifest" in error and "rstim-vs-stim-simulator" in error for error in errors),
            errors,
        )

    def test_rejects_negative_control_mutations(self) -> None:
        for mutation, entry_id, rule in [
            ("missing_artifact", "surface-decoder-full", "does not exist"),
            ("missing_claims_limit", "surface-decoder-full", "claims_limit"),
            ("ignored_artifact", "surface-decoder-full", "ignored"),
        ]:
            repo, manifest_path = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(any(entry_id in error and rule in error for error in errors), errors)

    def test_self_test_exercises_negative_controls(self) -> None:
        self.assertEqual(check_site_manifest.run_self_test(), [])

    def test_rejects_force_tracked_ignored_artifact(self) -> None:
        repo, manifest_path = self.write_fixture_manifest(mutation="force_tracked_ignored_artifact")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("surface-decoder-full" in error and "ignored" in error for error in errors),
            errors,
        )

    def test_rejects_malformed_metadata_without_crashing(self) -> None:
        for mutation, entry_id, rule in [
            ("bad_artifact_path_type", "surface-decoder-full", "artifact path must be a non-empty string"),
            ("bad_artifact_kind_type", "surface-decoder-full", "artifact kind must be a non-empty string"),
            ("bad_commands_type", "surface-decoder-full", "commands must be a list"),
            ("bad_provenance_requirements_type", "surface-decoder-full", "provenance_requirements entries must be strings"),
        ]:
            repo, manifest_path = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(any(entry_id in error and rule in error for error in errors), errors)

    def test_rejects_duplicate_evidence_item_ids(self) -> None:
        repo, manifest_path = self.write_fixture_manifest(mutation="duplicate_item_id")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("family surface-decoder-comparison" in error and "duplicate evidence item id surface-decoder-full" in error for error in errors),
            errors,
        )

    def test_rejects_cross_family_duplicate_evidence_item_ids(self) -> None:
        repo, manifest_path = self.write_fixture_manifest(mutation="cross_family_duplicate_item_id")
        errors = check_site_manifest.validate_manifest(repo, manifest_path)
        self.assertTrue(
            any("manifest" in error and "duplicate evidence item id surface-decoder-full" in error for error in errors),
            errors,
        )

    def test_rejects_empty_source_and_provenance_sources(self) -> None:
        for mutation, entry_id, rule in [
            ("empty_source_docs", "family surface-decoder-comparison", "source_docs must not be empty"),
            ("empty_provenance_sources", "surface-decoder-full", "provenance_sources must not be empty"),
        ]:
            repo, manifest_path = self.write_fixture_manifest(mutation=mutation)
            errors = check_site_manifest.validate_manifest(repo, manifest_path)
            self.assertTrue(any(entry_id in error and rule in error for error in errors), errors)

    def test_site_root_validation_rejects_missing_copied_checked_artifact(self) -> None:
        repo, manifest_path = self.write_fixture_manifest()
        site_root = repo / "_site"
        (site_root / "data").mkdir(parents=True)
        site_manifest = site_root / "data/benchmark-site.json"
        site_manifest.write_text(manifest_path.read_text(encoding="utf-8"), encoding="utf-8")

        errors = check_site_manifest.validate_manifest(repo, site_manifest, site_root=site_root)

        self.assertTrue(
            any(
                "surface-decoder-full" in error
                and "benchmarks/surface_decoder_compare/results/full/results.csv" in error
                and "not copied" in error
                for error in errors
            ),
            errors,
        )

    def test_copy_helper_copies_manifest_and_checked_artifacts_only(self) -> None:
        repo, manifest_path = self.write_fixture_manifest()
        site_root = repo / "_site"

        errors = copy_site_benchmark_data.copy_benchmark_site_data(repo, manifest_path, site_root)

        self.assertEqual(errors, [])
        self.assertTrue((site_root / "data/benchmark-site.json").is_file())
        self.assertTrue((site_root / "benchmarks/surface_decoder_compare/results/full/results.csv").is_file())
        self.assertTrue((site_root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").is_file())
        self.assertFalse((site_root / "benchmarks/out/local-only.csv").exists())

    def test_copy_helper_rejects_unchecked_tracked_artifact(self) -> None:
        repo, manifest_path = self.write_fixture_manifest(mutation="unchecked_tracked_artifact")
        site_root = repo / "_site"

        errors = copy_site_benchmark_data.copy_benchmark_site_data(repo, manifest_path, site_root)

        self.assertTrue(any("checked=True" in error for error in errors), errors)
        self.assertFalse((site_root / "data/benchmark-site.json").exists())
        self.assertFalse((site_root / "benchmarks/surface_decoder_compare/results/full/results.csv").exists())
        self.assertFalse((site_root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").exists())
        self.assertFalse((site_root / "benchmarks/surface_decoder_compare/results/full/unchecked.csv").exists())


if __name__ == "__main__":
    unittest.main()
