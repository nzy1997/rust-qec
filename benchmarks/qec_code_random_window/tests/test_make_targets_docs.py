from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
MAKEFILE = ROOT / "Makefile"
SHOWCASE = ROOT / "docs" / "showcases" / "qec-code-random-window-benchmark.md"
SHOWCASE_INDEX = ROOT / "docs" / "showcases" / "README.md"


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def make_target_body(makefile: str, target: str) -> str:
    match = re.search(rf"^{re.escape(target)}:\n(?P<body>(?:\t.*\n)+)", makefile, re.MULTILINE)
    if match is None:
        raise AssertionError(f"missing Make target {target}")
    return match.group("body")


class QecRandomWindowBenchmarkDocsTest(unittest.TestCase):
    def test_makefile_exposes_smoke_pipeline_without_external_baselines(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-smoke")

        self.assertIn("benchmarks/qec_code_random_window/cases.smoke.toml", body)
        self.assertIn("benchmarks/out/qec_code_random_window/smoke", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.validate_cases", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.run_local", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.summarize", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.compare_paper", body)
        self.assertIn("case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row", body)
        self.assertNotIn("--strict-baselines", body)
        self.assertNotIn("CODEDISTANCE_PAPER_RESULTS_DIR", body)

    def test_makefile_exposes_full_pipeline_with_imported_strict_baselines(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-full")

        self.assertIn("benchmarks/qec_code_random_window/cases.full.toml", body)
        self.assertIn("benchmarks/out/qec_code_random_window/full", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.import_paper_baselines", body)
        self.assertIn("CODEDISTANCE_PAPER_RESULTS_DIR", body)
        self.assertIn("--strict-baselines", body)

    def test_showcase_documents_smoke_command_outputs_and_limits(self) -> None:
        showcase = read_text(SHOWCASE)
        index = read_text(SHOWCASE_INDEX)

        self.assertIn("make qec-code-random-window-bench-smoke", showcase)
        self.assertIn("random-window-upper-bound", showcase)
        self.assertIn("only the local `random-window-upper-bound`", showcase)
        self.assertIn("CODEDISTANCE_PAPER_RESULTS_DIR", showcase)
        self.assertIn("benchmarks/out/qec_code_random_window/", showcase)
        self.assertIn("`NA`", showcase)
        self.assertIn("qec-code random-window benchmark", index.lower())
