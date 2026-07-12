from __future__ import annotations

import importlib
import subprocess
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_all_portable_evidence.py"
CATALOG = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml"


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
                    "PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9",
                    "PASS packed reference-build evidence",
                    "PASS instruction-wide frame-noise evidence builds=803 attempts=82290688 legacy_setups=80362",
                    "PASS portable checked evidence bundles=4",
                    "",
                ]
            ),
        )
        self.assertEqual(result.stderr, "")

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
