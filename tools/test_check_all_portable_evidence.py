from __future__ import annotations

import importlib
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


class BlockStimImports:
    def find_spec(self, fullname: str, path: object | None = None, target: object | None = None) -> None:
        if fullname == "stim" or fullname.startswith("stim."):
            raise ModuleNotFoundError("blocked stim import during portability smoke test")
        return None


class AllPortableEvidenceCheckerTest(unittest.TestCase):
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
        finally:
            sys.meta_path.remove(blocker)


if __name__ == "__main__":
    unittest.main()
