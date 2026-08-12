from __future__ import annotations

import hashlib
import importlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_all_portable_evidence.py"
CATALOG = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
FAIR_CLI_ARTIFACTS = ("raw.jsonl", "summary.json", "baseline-summary.json", "comparison.json", "report.md", "environment.json")
FIXTURE_REPO_PATH = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def rewrite_fair_cli_hashes(bundle: Path) -> None:
    write_json(
        bundle / "artifact-sha256.json",
        {filename: sha256_file(bundle / filename) for filename in FAIR_CLI_ARTIFACTS},
    )


def rewrite_catalog_fair_artifact_hashes(catalog_path: Path, fair_bundle: Path) -> None:
    text = catalog_path.read_text(encoding="utf-8")
    catalog = importlib.import_module("benchmarks.rstim_vs_stim_simulator.portable_provenance").load_catalog(catalog_path)
    fair_entry = next(bundle for bundle in catalog["bundles"] if bundle["id"] == "fair-cli-release")
    for artifact in fair_entry["artifacts"]:
        old_digest = artifact["sha256"]
        new_digest = sha256_file(fair_bundle / artifact["path"])
        text = text.replace(f'sha256 = "{old_digest}"', f'sha256 = "{new_digest}"', 1)
    catalog_path.write_text(text, encoding="utf-8")


class BlockStimImports:
    def find_spec(self, fullname: str, path: object | None = None, target: object | None = None) -> None:
        if fullname == "stim" or fullname.startswith("stim."):
            raise ModuleNotFoundError("blocked stim import during portability smoke test")
        return None


class AllPortableEvidenceCheckerTest(unittest.TestCase):
    def run_aggregate(self, *extra_args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), *extra_args],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_direct_script_help_imports_without_stim(self) -> None:
        result = self.run_aggregate("--help")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--catalog", result.stdout)

    def test_registry_covers_required_bundle_ids(self) -> None:
        checker = importlib.import_module("tools.check_all_portable_evidence")

        self.assertEqual(
            set(checker.CHECKERS),
            {
                "fair-cli-release",
                "compiled-steady-release",
                "reference-build-release",
                "frame-instruction-wide-release",
            },
        )

    def test_cli_accepts_committed_catalog_and_all_bundles(self) -> None:
        result = self.run_aggregate("--catalog", str(CATALOG))

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout,
            "\n".join(
                [
                    "PASS portable evidence catalog bundles=4 schema=2",
                    "PASS fair CLI sampling evidence variants=2 measured=14",
                    "PASS split precompile/sample/b8 evidence variants=5 measured=35 lifecycle=verified/9",
                    "PASS packed reference-build evidence variants=3 direct_speedup=20.978277",
                    "PASS instruction-wide frame-noise evidence outcome=improved builds=803 legacy_setups=80362 candidate_over_baseline=0.775851 attempts=82290688",
                    "PASS portable checked evidence bundles=4",
                    "",
                ]
            ),
        )
        self.assertEqual(result.stderr, "")

    def test_reference_build_catalog_provenance_matches_environment(self) -> None:
        provenance = importlib.import_module("benchmarks.rstim_vs_stim_simulator.portable_provenance")
        catalog = provenance.load_catalog(CATALOG)
        bundle = next(entry for entry in catalog["bundles"] if entry["id"] == "reference-build-release")
        environment = json.loads(
            (REPO_ROOT / bundle["bundle_path"] / "environment.json").read_text(encoding="utf-8")
        )

        catalog_identities = {identity["role"]: identity for identity in bundle["runtime_identities"]}
        environment_identities = {
            identity["role"]: identity for identity in environment["runtime_identities"]
        }
        self.assertEqual(catalog_identities, environment_identities)

        catalog_commands = {tuple(command["argv"]) for command in bundle["checked_commands"]}
        environment_commands = {
            tuple(argv) for argv in environment["worker_argv"].values()
        }
        self.assertEqual(catalog_commands, environment_commands)

    def test_cli_rejects_reference_build_catalog_environment_command_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            temp_repo = Path(tmp) / "repo"
            shutil.copytree(REPO_ROOT / "benchmarks", temp_repo / "benchmarks")
            catalog = temp_repo / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
            text = catalog.read_text(encoding="utf-8")
            text = text.replace(
                'argv = ["tool://rstim-reference-worker", "--protocol", "reference-build-v1"]',
                'argv = ["tool://rstim-reference-worker", "--protocol", "reference-build-v1", "--strategy", "canonical"]',
                1,
            )
            catalog.write_text(text, encoding="utf-8")

            result = self.run_aggregate("--catalog", str(catalog))

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("PASS portable evidence catalog bundles=4 schema=2", result.stdout)
        self.assertIn("FAIL portable checked evidence bundle=reference-build-release", result.stderr)
        self.assertIn("catalog checked_commands do not match environment.json worker_argv", result.stderr)

    def test_fair_cli_rehashed_absolute_fixture_path_fails_with_bundle_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            temp_repo = Path(tmp) / "repo"
            shutil.copytree(REPO_ROOT / "benchmarks", temp_repo / "benchmarks")
            catalog = temp_repo / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"
            fair_bundle = temp_repo / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release"
            absolute_fixture = str((REPO_ROOT / FIXTURE_REPO_PATH).resolve())

            records = [json.loads(line) for line in (fair_bundle / "raw.jsonl").read_text(encoding="utf-8").splitlines()]
            for record in records:
                argv = record["argv"]
                argv[argv.index("--in") + 1] = absolute_fixture
            (fair_bundle / "raw.jsonl").write_text(
                "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
                encoding="utf-8",
            )

            environment = json.loads((fair_bundle / "environment.json").read_text(encoding="utf-8"))
            for round_argv in environment["round_argv"]:
                argv = round_argv["argv"]
                argv[argv.index("--in") + 1] = absolute_fixture
            write_json(fair_bundle / "environment.json", environment)
            rewrite_fair_cli_hashes(fair_bundle)
            rewrite_catalog_fair_artifact_hashes(catalog, fair_bundle)

            result = self.run_aggregate("--catalog", str(catalog))

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("PASS portable evidence catalog bundles=4 schema=2", result.stdout)
        self.assertIn("FAIL portable checked evidence bundle=fair-cli-release", result.stderr)
        self.assertIn("stim-cli-b8 argv contains a host-absolute path", result.stderr)

    def test_aggregate_and_frame_checker_import_without_stim(self) -> None:
        for module_name in (
            "tools.check_all_portable_evidence",
            "tools.check_rstim_vs_stim_instruction_wide_noise_evidence",
            "benchmarks.rstim_vs_stim_simulator.run_frame_instruction_wide_benchmark",
            "benchmarks.rstim_vs_stim_simulator.inspect_fixture_load",
            "benchmarks.rstim_vs_stim_simulator.validate_cases",
        ):
            sys.modules.pop(module_name, None)

        blocker = BlockStimImports()
        sys.meta_path.insert(0, blocker)
        try:
            importlib.import_module("tools.check_rstim_vs_stim_instruction_wide_noise_evidence")
            importlib.import_module("tools.check_all_portable_evidence")
        finally:
            sys.meta_path.remove(blocker)


if __name__ == "__main__":
    unittest.main()
