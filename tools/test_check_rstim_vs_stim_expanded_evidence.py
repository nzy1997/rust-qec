#!/usr/bin/env python3
from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_expanded_evidence.py"
RESULTS_ROOT = REPO_ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "results"
DEFAULT_CORRECTNESS_DIR = RESULTS_ROOT / "distributions"
DEFAULT_FULL_CORRECTNESS = RESULTS_ROOT / "full" / "correctness-summary.json"
DEFAULT_SPEED_DIRS = [
    RESULTS_ROOT / "release",
    RESULTS_ROOT / "release-repetition-sample",
    RESULTS_ROOT / "release-surface-detect",
]
DEFAULT_DEM_SPEED_DIR = RESULTS_ROOT / "release-dem-sample"
OLD_DEBUG_SUMMARY = RESULTS_ROOT / "full" / "speed-summary.json"
SURFACE_CASE = "surface-detect-d13-r13"
DEM_CASE = "stim-style-surface-dem-sample-d11-r100-b1024"


def rewrite_json(path: Path, mutate: Callable[[dict[str, Any]], None]) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    mutate(data)
    path.write_text(
        json.dumps(data, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


class ExpandedEvidenceCheckerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmpdir.cleanup)
        self.root = Path(self.tmpdir.name)
        self.speed_dirs: list[Path] = []
        for source in DEFAULT_SPEED_DIRS:
            destination = self.root / source.name
            shutil.copytree(source, destination)
            self.speed_dirs.append(destination)
        self.dem_speed_dir = self.root / DEFAULT_DEM_SPEED_DIR.name
        shutil.copytree(DEFAULT_DEM_SPEED_DIR, self.dem_speed_dir)

    def run_checker(
        self,
        *,
        correctness_dir: Path = DEFAULT_CORRECTNESS_DIR,
        full_correctness: Path = DEFAULT_FULL_CORRECTNESS,
        speed_dirs: list[Path] | None = None,
        dem_speed_dir: Path | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "python3",
                str(CHECKER),
                "--correctness-dir",
                str(correctness_dir),
                "--full-correctness",
                str(full_correctness),
                "--speed-dirs",
                ",".join(str(path) for path in (speed_dirs or self.speed_dirs)),
                "--dem-speed-dir",
                str(dem_speed_dir or self.dem_speed_dir),
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_accepts_committed_expanded_evidence(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual("PASS expanded rstim-vs-Stim evidence\n", result.stdout)

    def test_rejects_missing_surface_detect_case(self) -> None:
        summary_path = self.speed_dirs[2] / "summary.json"
        rewrite_json(
            summary_path,
            lambda data: data.__setitem__(
                "cases",
                [
                    case
                    for case in data["cases"]
                    if case.get("case_label") != SURFACE_CASE
                ],
            ),
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            f"missing required evidence case {SURFACE_CASE}", result.stderr
        )

    def test_rejects_dem_directory_without_summary(self) -> None:
        (self.dem_speed_dir / "summary.json").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(f"missing required evidence case {DEM_CASE}", result.stderr)

    def test_rejects_dem_summary_without_required_case(self) -> None:
        rewrite_json(
            self.dem_speed_dir / "summary.json",
            lambda data: data["cases"][0].__setitem__(
                "case_label", "other-dem-case"
            ),
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(f"missing required evidence case {DEM_CASE}", result.stderr)

    def test_rejects_missing_required_speed_variant(self) -> None:
        summary_path = self.speed_dirs[1] / "summary.json"

        def remove_stim_cli(data: dict[str, object]) -> None:
            case = data["cases"][0]
            case["present_variants"].remove("stim-cli")
            case["variants"] = [
                variant
                for variant in case["variants"]
                if variant.get("tool_variant") != "stim-cli"
            ]

        rewrite_json(summary_path, remove_stim_cli)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing required variant stim-cli", result.stderr)

    def test_rejects_missing_speed_environment_metadata(self) -> None:
        rewrite_json(
            self.speed_dirs[2] / "environment.json",
            lambda data: data.pop("cargo_version"),
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("environment.json missing cargo_version", result.stderr)

    def test_rejects_old_debug_summary_as_release_evidence(self) -> None:
        shutil.copyfile(OLD_DEBUG_SUMMARY, self.speed_dirs[0] / "summary.json")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "release evidence reuses old #406 debug summary", result.stderr
        )

    def test_rejects_missing_distribution_correctness_case(self) -> None:
        correctness_dir = self.root / "distributions"
        shutil.copytree(DEFAULT_CORRECTNESS_DIR, correctness_dir)
        summary_path = correctness_dir / "summary.json"
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        missing_case = summary["cases"][0]["case_id"]
        summary["cases"] = summary["cases"][1:]
        summary_path.write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        result = self.run_checker(correctness_dir=correctness_dir)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            f"missing distribution evidence for case {missing_case}", result.stderr
        )


if __name__ == "__main__":
    unittest.main()
