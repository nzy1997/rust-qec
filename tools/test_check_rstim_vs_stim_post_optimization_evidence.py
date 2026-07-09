#!/usr/bin/env python3
from __future__ import annotations

import copy
import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_post_optimization_evidence.py"
DEFAULT_OLD_SUMMARY = (
    REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
)
DEFAULT_OLD_SUMMARY_REL = "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
DEFAULT_NEW_DIR_REL = "benchmarks/rstim_vs_stim_simulator/results/release"
DEFAULT_DOCS_REL = "docs/showcases/rstim-vs-stim-simulator.md"
DEFAULT_MANIFEST_REL = "site/benchmark-site.json"
SELECTED_CASE_LABEL = "stim-style-surface-sample-d11-r100-b1024"


def sha256_text(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


class RstimVsStimPostOptimizationEvidenceCheckerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmpdir.cleanup)
        self.root = Path(self.tmpdir.name)

        old_summary = self.root / DEFAULT_OLD_SUMMARY_REL
        old_summary.parent.mkdir(parents=True)
        shutil.copy(DEFAULT_OLD_SUMMARY, old_summary)

    def run_checker(
        self,
        *,
        old: Path | None = None,
        new_dir: Path | None = None,
        docs: Path | None = None,
        manifest: Path | None = None,
    ) -> subprocess.CompletedProcess[str]:
        args = ["python3", str(CHECKER)]
        if old is not None:
            args.extend(["--old", str(old)])
        if new_dir is not None:
            args.extend(["--new-dir", str(new_dir)])
        if docs is not None:
            args.extend(["--docs", str(docs)])
        if manifest is not None:
            args.extend(["--manifest", str(manifest)])
        return subprocess.run(
            args,
            cwd=self.root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def load_old_summary(self) -> dict[str, object]:
        return json.loads(DEFAULT_OLD_SUMMARY.read_text(encoding="utf-8"))

    def write_valid_release_fixture(self) -> Path:
        release = self.root / DEFAULT_NEW_DIR_REL
        release.mkdir(parents=True)
        summary = copy.deepcopy(self.load_old_summary())
        summary["fixture"] = "post-optimization-release"
        (release / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
        (release / "report.md").write_text(
            (
                "# Release report\n"
                "This report-only Stim comparison captures the selected release evidence.\n"
                f"Case: {SELECTED_CASE_LABEL}\n"
            ),
            encoding="utf-8",
        )
        environment = {
            "profile": "release",
            "evidence_kind": "rstim-vs-stim post-optimization release evidence",
            "rstim_binary_path": "target/release/rstim",
            "rustc_version": "rustc 1.89.0",
            "cargo_version": "cargo 1.89.0",
            "stim_cli_status": "missing",
            "stim_cli.stderr": "stim not installed in fixture",
        }
        (release / "environment.json").write_text(json.dumps(environment), encoding="utf-8")
        return release

    def write_docs_fixture(self, *, broad_claim: str | None = None, include_release_link: bool = True) -> Path:
        docs = self.root / DEFAULT_DOCS_REL
        docs.parent.mkdir(parents=True, exist_ok=True)
        lines = [
            "# rstim versus Stim simulator",
            "",
            f"- Checked #406 artifact: `{DEFAULT_OLD_SUMMARY_REL}`",
        ]
        if include_release_link:
            lines.append(f"- Post-optimization release artifact: `{DEFAULT_NEW_DIR_REL}/summary.json`")
        if broad_claim is not None:
            lines.append(broad_claim)
        docs.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return docs

    def write_manifest_fixture(
        self,
        release: Path,
        *,
        include_release_item: bool = True,
        release_claims_limit: str | None = None,
    ) -> Path:
        manifest = self.root / DEFAULT_MANIFEST_REL
        manifest.parent.mkdir(parents=True, exist_ok=True)
        release_artifacts = [
            f"{DEFAULT_NEW_DIR_REL}/summary.json",
            f"{DEFAULT_NEW_DIR_REL}/report.md",
            f"{DEFAULT_NEW_DIR_REL}/environment.json",
        ]
        artifact_hashes = {
            artifact: {"sha256": sha256_text(self.root / artifact)}
            for artifact in release_artifacts
        }
        evidence_items: list[dict[str, object]] = [
            {
                "id": "rstim-vs-stim-full",
                "artifacts": [{"path": DEFAULT_OLD_SUMMARY_REL, "kind": "speed-summary", "checked": True}],
                "provenance": {
                    "artifact_hashes": {
                        "status": "recorded",
                        "value": {
                            DEFAULT_OLD_SUMMARY_REL: {
                                "sha256": sha256_text(self.root / DEFAULT_OLD_SUMMARY_REL)
                            }
                        },
                    }
                },
            }
        ]
        if include_release_item:
            evidence_items.append(
                {
                    "id": "rstim-vs-stim-release",
                    "artifacts": [
                        {"path": artifact, "kind": Path(artifact).name, "checked": True}
                        for artifact in release_artifacts
                    ],
                    "provenance": {"artifact_hashes": {"status": "recorded", "value": artifact_hashes}},
                    "claims_limit": release_claims_limit
                    or "One selected workload and one recorded environment only.",
                }
            )
        manifest.write_text(
            json.dumps(
                {
                    "families": [
                        {
                            "id": "rstim-vs-stim-simulator",
                            "evidence_items": evidence_items,
                        }
                    ]
                }
            ),
            encoding="utf-8",
        )
        return manifest

    def test_accepts_separate_release_fixture(self) -> None:
        release = self.write_valid_release_fixture()
        docs = self.write_docs_fixture()
        manifest = self.write_manifest_fixture(release)
        result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "PASS post-optimization evidence is separate from the checked #406 artifact",
            result.stdout,
        )

    def test_rejects_reused_old_summary_as_new_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            release = Path(tmp) / "release"
            release.mkdir()
            old_summary = self.root / DEFAULT_OLD_SUMMARY_REL
            shutil.copy(DEFAULT_OLD_SUMMARY, release / "summary.json")
            (release / "environment.json").write_text('{"profile":"release"}\n', encoding="utf-8")
            (release / "report.md").write_text("# pretend report\n", encoding="utf-8")
            result = self.run_checker(old=old_summary, new_dir=release)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("new summary reuses the checked #406 summary", result.stderr)

    def test_rejects_old_summary_hash_mismatch_against_manifest(self) -> None:
        release = self.write_valid_release_fixture()
        docs = self.write_docs_fixture()
        manifest = self.write_manifest_fixture(release)
        old_summary_path = self.root / DEFAULT_OLD_SUMMARY_REL
        old_summary = json.loads(old_summary_path.read_text(encoding="utf-8"))
        old_summary["mutated_after_manifest_hash"] = True
        old_summary_path.write_text(json.dumps(old_summary), encoding="utf-8")
        result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("checked artifact hash differs from site manifest", result.stderr)

    def test_rejects_missing_environment_metadata(self) -> None:
        release = self.write_valid_release_fixture()
        docs = self.write_docs_fixture()
        manifest = self.write_manifest_fixture(release)
        environment = json.loads((release / "environment.json").read_text(encoding="utf-8"))
        del environment["rstim_binary_path"]
        (release / "environment.json").write_text(json.dumps(environment), encoding="utf-8")
        result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment.json missing rstim_binary_path", result.stderr)

    def test_rejects_empty_successful_stim_version_metadata(self) -> None:
        release = self.write_valid_release_fixture()
        docs = self.write_docs_fixture()
        environment = json.loads((release / "environment.json").read_text(encoding="utf-8"))
        environment["stim_cli_status"] = "ok"
        environment["stim_cli_version"] = ""
        environment["stim_cli.stderr"] = "No mode was given."
        environment["stim_cli"] = {"status": "ok", "version": "", "stderr": "No mode was given."}
        (release / "environment.json").write_text(json.dumps(environment), encoding="utf-8")
        manifest = self.write_manifest_fixture(release)
        result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment.json stim_cli_version is empty for ok Stim CLI", result.stderr)

    def test_rejects_missing_report_file(self) -> None:
        release = self.write_valid_release_fixture()
        docs = self.write_docs_fixture()
        manifest = self.write_manifest_fixture(release)
        (release / "report.md").unlink()
        result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required release file: report.md", result.stderr)

    def test_rejects_missing_site_item(self) -> None:
        release = self.write_valid_release_fixture()
        docs = self.write_docs_fixture()
        manifest = self.write_manifest_fixture(release, include_release_item=False)
        result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("site manifest missing evidence item rstim-vs-stim-release", result.stderr)

    def test_rejects_missing_docs_link(self) -> None:
        release = self.write_valid_release_fixture()
        docs = self.write_docs_fixture(include_release_link=False)
        manifest = self.write_manifest_fixture(release)
        result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(DEFAULT_NEW_DIR_REL + "/summary.json", result.stderr)

    def test_rejects_broad_parity_wording(self) -> None:
        release = self.write_valid_release_fixture()
        docs = self.write_docs_fixture(broad_claim="This page claims broad rstim/stim performance parity.")
        manifest = self.write_manifest_fixture(release)
        result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("docs contain forbidden broad parity wording", result.stderr)

    def test_rejects_broad_parity_wording_in_manifest(self) -> None:
        release = self.write_valid_release_fixture()
        docs = self.write_docs_fixture()
        manifest = self.write_manifest_fixture(
            release,
            release_claims_limit="This site manifest claims all-workload parity.",
        )
        result = self.run_checker(new_dir=release, docs=docs, manifest=manifest)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("site manifest contains forbidden broad parity wording", result.stderr)


if __name__ == "__main__":
    unittest.main()
