#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_gap_artifact.py"
DEFAULT_SUMMARY = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
SELECTED_CASE_LABEL = "stim-style-surface-sample-d11-r100-b1024"


def selected_case(
    *,
    present_variants: list[str] | None = None,
    stim_rate: float = 5690.64878525516,
    rstim_rate: float = 21.774891038227285,
    stim_status: str = "completed",
    rstim_status: str = "completed",
    stim_samples: int = 1,
    rstim_samples: int = 1,
) -> dict[str, object]:
    return {
        "case_label": SELECTED_CASE_LABEL,
        "workload": "sample",
        "tier": "report_only",
        "present_variants": present_variants
        if present_variants is not None
        else ["rstim-compiled", "rstim-interpreted", "stim-cli"],
        "variants": [
            {
                "tool_variant": "stim-cli",
                "sample_count": stim_samples,
                "median_shots_per_second": stim_rate,
                "status": stim_status,
            },
            {
                "tool_variant": "rstim-compiled",
                "sample_count": rstim_samples,
                "median_shots_per_second": rstim_rate,
                "status": rstim_status,
            },
        ],
    }


class RstimVsStimGapArtifactCheckerTest(unittest.TestCase):
    def run_checker(self, path: Path | None = None) -> subprocess.CompletedProcess[str]:
        args = ["python3", str(CHECKER)]
        if path is not None:
            args.append(str(path))
        return subprocess.run(
            args,
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def write_summary(self, case: dict[str, object]) -> tuple[tempfile.TemporaryDirectory[str], Path]:
        tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(tmpdir.cleanup)
        path = Path(tmpdir.name) / "speed-summary.json"
        path.write_text(json.dumps({"cases": [case]}), encoding="utf-8")
        return tmpdir, path

    def test_default_checked_artifact_passes(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "PASS checked #406 gap is preserved: stim-cli is 261.34x faster than rstim-compiled",
            result.stdout,
        )

    def test_rejects_equal_speed_fixture(self) -> None:
        _, path = self.write_summary(selected_case(stim_rate=100.0, rstim_rate=100.0))
        result = self.run_checker(path)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("ratio outside 200-300", result.stderr)

    def test_rejects_changed_large_gap_fixture(self) -> None:
        _, path = self.write_summary(selected_case(stim_rate=6000.0, rstim_rate=24.0))
        result = self.run_checker(path)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("selected-case rate changed", result.stderr)

    def test_rejects_missing_rstim_compiled_fixture(self) -> None:
        case = selected_case(present_variants=["stim-cli"])
        case["variants"] = [case["variants"][0]]  # type: ignore[index]
        _, path = self.write_summary(case)
        result = self.run_checker(path)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing rstim-compiled", result.stderr)

    def test_rejects_default_path_copy_with_manifest_hash_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo = Path(tmpdir) / "repo"
            summary_path = repo / "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json"
            manifest_path = repo / "site/benchmark-site.json"
            summary_path.parent.mkdir(parents=True)
            manifest_path.parent.mkdir(parents=True)
            summary_path.write_text(
                json.dumps({"cases": [selected_case(stim_rate=6000.0, rstim_rate=23.0)]}),
                encoding="utf-8",
            )
            manifest_path.write_text(
                (REPO_ROOT / "site/benchmark-site.json").read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            result = subprocess.run(
                ["python3", str(CHECKER)],
                cwd=repo,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("checked artifact hash differs from site manifest", result.stderr)


if __name__ == "__main__":
    unittest.main()
