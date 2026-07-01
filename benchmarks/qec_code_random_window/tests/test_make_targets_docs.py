from __future__ import annotations

import csv
import re
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
MAKEFILE = ROOT / "Makefile"
CASES_SMOKE = ROOT / "benchmarks" / "qec_code_random_window" / "cases.smoke.toml"
CASES_NO_TARGET_SMOKE = (
    ROOT / "benchmarks" / "qec_code_random_window" / "cases.no-target-smoke.toml"
)
CASES_NO_TARGET_LADDER_SMOKE = (
    ROOT / "benchmarks" / "qec_code_random_window" / "cases.no-target-ladder-smoke.toml"
)
SHOWCASE = ROOT / "docs" / "showcases" / "qec-code-random-window-benchmark.md"
SHOWCASE_INDEX = ROOT / "docs" / "showcases" / "README.md"


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def read_csv_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def smoke_local_summary(path: Path, out_dir: Path) -> Path:
    manifest = tomllib.loads(path.read_text(encoding="utf-8"))
    cases = manifest["cases"]
    summary_path = out_dir / "local_summary.csv"
    lines = [
        "case_id,code_id,distance_side,baseline_key,baseline_required,best_upper_bound,median_elapsed_s"
    ]
    for case in cases:
        lines.append(
            ",".join(
                [
                    case["case_id"],
                    str(case["code_id"]),
                    str(case["distance_side"]),
                    case["baseline_key"],
                    "true" if case["baseline_required"] else "false",
                    "10",
                    "1.0",
                ]
            )
        )
    summary_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return summary_path


def make_target_body(makefile: str, target: str) -> str:
    match = re.search(rf"^{re.escape(target)}:\n(?P<body>(?:\t.*\n)+)", makefile, re.MULTILINE)
    if match is None:
        raise AssertionError(f"missing Make target {target}")
    return match.group("body")


class QecRandomWindowBenchmarkDocsTest(unittest.TestCase):
    def test_makefile_exposes_smoke_pipeline_without_external_baselines(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-smoke")

        self.assertIn("QEC_CODE_RANDOM_WINDOW_SMOKE_CASES := benchmarks/qec_code_random_window/cases.smoke.toml", makefile)
        self.assertIn("QEC_CODE_RANDOM_WINDOW_SMOKE_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/smoke", makefile)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_SMOKE_CASES)", body)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_SMOKE_DIR)", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.validate_cases", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.run_local", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.summarize", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.compare_paper", body)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_BASELINE_HEADER)", body)
        self.assertNotIn("--strict-baselines", body)

    def test_makefile_exposes_smoke_pipeline_uses_header_only_baselines(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-smoke")

        self.assertIn(
            "$(QEC_CODE_RANDOM_WINDOW_BASELINE_HEADER)",
            body,
        )
        self.assertNotIn("import_paper_baselines", body)

    def test_smoke_target_generates_no_paper_baseline_rows_with_header_only_baseline_csv(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            local_summary = smoke_local_summary(CASES_SMOKE, out_dir)
            paper_baselines = out_dir / "paper_baselines.csv"
            paper_baselines.write_text(
                "case_id,paper_case,baseline_method,baseline_upper_bound,baseline_elapsed_s,source_file,source_sheet,source_row\n",
                encoding="utf-8",
            )

            command = [
                sys.executable,
                "-m",
                "benchmarks.qec_code_random_window.compare_paper",
                "--cases",
                str(CASES_SMOKE),
                "--local-summary",
                str(local_summary),
                "--paper-baselines",
                str(paper_baselines),
                "--out-dir",
                str(out_dir / "results"),
            ]
            result = subprocess.run(
                command,
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            rows = read_csv_rows(out_dir / "results" / "comparison.csv")
            self.assertEqual(len(rows), 4)
            for row in rows:
                self.assertEqual(row["comparison_status"], "no_paper_baseline")
                self.assertEqual(row["paper_method"], "NA")
                self.assertEqual(row["paper_upper_bound"], "NA")
                self.assertEqual(row["paper_elapsed_s"], "NA")
                self.assertEqual(row["baseline_provenance"], "NA")
                self.assertEqual(row["baseline_source_file"], "NA")
                self.assertEqual(row["baseline_source_sheet"], "NA")
                self.assertEqual(row["baseline_source_row"], "NA")
                self.assertEqual(row["upper_bound_delta"], "NA")
                self.assertEqual(row["elapsed_time_ratio"], "NA")

    def test_makefile_exposes_full_pipeline_with_imported_strict_baselines(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-full")

        self.assertIn("QEC_CODE_RANDOM_WINDOW_FULL_CASES := benchmarks/qec_code_random_window/cases.full.toml", makefile)
        self.assertIn("QEC_CODE_RANDOM_WINDOW_FULL_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/full", makefile)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_FULL_CASES)", body)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_FULL_DIR)", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.import_paper_baselines", body)
        self.assertIn("CODEDISTANCE_PAPER_RESULTS_DIR", body)
        self.assertIn("--strict-baselines", body)

    def test_makefile_exposes_release_no_target_smoke_pipeline(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-no-target-smoke")

        self.assertIn("qec-code-random-window-bench-no-target-smoke", makefile)
        self.assertIn(
            "QEC_CODE_RANDOM_WINDOW_NO_TARGET_SMOKE_CASES := benchmarks/qec_code_random_window/cases.no-target-smoke.toml",
            makefile,
        )
        self.assertIn(
            "QEC_CODE_RANDOM_WINDOW_NO_TARGET_SMOKE_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/no-target-smoke",
            makefile,
        )
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_NO_TARGET_SMOKE_CASES)", body)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_NO_TARGET_SMOKE_DIR)", body)
        self.assertIn("cargo build --release -p qec-code", body)
        self.assertIn("--qec-code-bin target/release/qec-code", body)
        self.assertIn("--build-profile release", body)
        self.assertNotIn("--target-weight", body)

    def test_makefile_exposes_release_no_target_multiseed_smoke_pipeline(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-no-target-multiseed-smoke")

        self.assertIn("qec-code-random-window-bench-no-target-multiseed-smoke", makefile)
        self.assertIn(
            "QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/no-target-multiseed-smoke",
            makefile,
        )
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_NO_TARGET_SMOKE_CASES)", body)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_NO_TARGET_MULTISEED_SMOKE_DIR)", body)
        self.assertIn("cargo build --release -p qec-code", body)
        self.assertIn("--qec-code-bin target/release/qec-code", body)
        self.assertIn("--build-profile release", body)
        self.assertIn("--seeds 7 11 17", body)
        self.assertIn("run_status=0", body)
        self.assertIn("|| run_status=$$?", body)
        self.assertIn("--expected-seeds 7 11 17", body)
        self.assertIn("exit $$run_status", body)
        self.assertNotIn("--target-weight", body)

    def test_makefile_exposes_no_target_ladder_smoke_pipeline(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-no-target-ladder-smoke")

        self.assertIn("qec-code-random-window-bench-no-target-ladder-smoke", makefile)
        self.assertIn(
            "QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_CASES := benchmarks/qec_code_random_window/cases.no-target-ladder-smoke.toml",
            makefile,
        )
        self.assertIn(
            "QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/no-target-ladder-smoke",
            makefile,
        )
        self.assertIn("--no-target-ladder-smoke", body)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_CASES)", body)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_NO_TARGET_LADDER_SMOKE_DIR)", body)
        self.assertIn("cargo build --release -p qec-code", body)
        self.assertIn("--build-profile release", body)
        self.assertNotIn("--target-weight", body)

    def test_makefile_exposes_issue225_readiness_smoke_pipeline(self) -> None:
        makefile = read_text(MAKEFILE)
        body = make_target_body(makefile, "qec-code-random-window-bench-issue225-readiness-smoke")

        self.assertIn("qec-code-random-window-bench-issue225-readiness-smoke", makefile)
        self.assertIn(
            "QEC_CODE_RANDOM_WINDOW_ISSUE225_READINESS_SMOKE_DIR := $(QEC_CODE_RANDOM_WINDOW_OUT)/issue225-readiness-smoke",
            makefile,
        )
        self.assertIn(
            "QEC_CODE_RANDOM_WINDOW_ISSUE225_EVIDENCE := benchmarks/qec_code_random_window/issue225_evidence.json",
            makefile,
        )
        self.assertIn("$(MAKE) qec-code-random-window-bench-no-target-ladder-smoke", body)
        self.assertIn("$(MAKE) qec-code-random-window-bench-no-target-multiseed-smoke", body)
        self.assertIn("python3 -m benchmarks.qec_code_random_window.issue225_readiness", body)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_ISSUE225_EVIDENCE)", body)
        self.assertIn("$(QEC_CODE_RANDOM_WINDOW_ISSUE225_READINESS_SMOKE_DIR)", body)

    def test_no_target_ladder_smoke_manifest_case_ids_are_documented(self) -> None:
        manifest = tomllib.loads(CASES_NO_TARGET_LADDER_SMOKE.read_text(encoding="utf-8"))
        cases = manifest["cases"]
        self.assertEqual(sorted([case["case_id"] for case in cases]), [
            "bb144",
            "bb72",
            "surface_rotated_d5",
            "toric_d5",
        ])
        self.assertTrue(all("target_weight" not in case for case in cases))

    def test_no_target_smoke_manifest_is_distinct_from_known_target_manifests(self) -> None:
        manifest = tomllib.loads(CASES_NO_TARGET_SMOKE.read_text(encoding="utf-8"))
        cases = manifest["cases"]

        self.assertEqual([case["case_id"] for case in cases], [
            "bb72_no_target_smoke",
            "bb144_no_target_smoke",
        ])
        self.assertTrue(all("target_weight" not in case for case in cases))
        self.assertTrue(any(case["target_upper_bound"] == 12 for case in cases))

    def test_showcase_documents_smoke_command_outputs_and_limits(self) -> None:
        showcase = read_text(SHOWCASE)
        index = read_text(SHOWCASE_INDEX)

        self.assertIn("make qec-code-random-window-bench-smoke", showcase)
        self.assertIn("make qec-code-random-window-bench-no-target-smoke", showcase)
        self.assertIn("make qec-code-random-window-bench-no-target-multiseed-smoke", showcase)
        self.assertIn("make qec-code-random-window-bench-no-target-ladder-smoke", showcase)
        self.assertIn("random-window-upper-bound", showcase)
        self.assertIn("only the local `random-window-upper-bound`", showcase)
        self.assertIn("release/no-target", showcase)
        self.assertIn("release/no-target multiseed", showcase)
        self.assertIn("release/no-target-ladder", showcase)
        self.assertIn("no-target-ladder", showcase)
        self.assertIn("known-target", showcase)
        self.assertIn("CODEDISTANCE_PAPER_RESULTS_DIR", showcase)
        self.assertIn("benchmarks/out/qec_code_random_window/", showcase)
        self.assertIn("benchmarks/out/qec_code_random_window/no-target-multiseed-smoke/", showcase)
        self.assertIn("`NA`", showcase)
        self.assertIn("no-target-smoke", showcase)
        self.assertIn("no-target-multiseed-smoke", showcase)
        self.assertIn("no-target-ladder-smoke", showcase)
        self.assertIn("observed seeds", showcase)
        self.assertIn("attempted and successful seed counts", showcase)
        self.assertIn("target-hit rates", showcase)
        self.assertIn("elapsed min/median/max", showcase)
        self.assertIn("unset no-target weights", showcase)
        self.assertIn("qec-code random-window benchmark", index.lower())

    def test_readme_documents_issue225_readiness_target(self) -> None:
        readme = read_text(ROOT / "benchmarks" / "qec_code_random_window" / "README.md")
        self.assertIn("make qec-code-random-window-bench-issue225-readiness-smoke", readme)
        self.assertIn("issue225-readiness-smoke/report.md", readme)
        self.assertIn("issue_225_readiness: PASS", readme)
        self.assertIn("local evidence only", readme)
        self.assertIn("do not commit generated benchmark output", readme)
