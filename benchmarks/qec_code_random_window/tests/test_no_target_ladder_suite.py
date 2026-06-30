from __future__ import annotations

import copy
import csv
import json
import re
import tomllib
import unittest
from pathlib import Path

from benchmarks.qec_code_random_window.validate_cases import (
    NO_TARGET_LADDER_REQUIRED_CASE_IDS,
    validate_no_target_ladder_manifest,
)


ROOT = Path(__file__).resolve().parents[3]
PACKAGE_DIR = ROOT / "benchmarks" / "qec_code_random_window"
MAKEFILE = ROOT / "Makefile"
NO_TARGET_LADDER_SMOKE_MANIFEST = PACKAGE_DIR / "cases.no-target-ladder-smoke.toml"
NO_TARGET_LADDER_OUTPUT_DIR = (
    ROOT / "benchmarks" / "out" / "qec_code_random_window" / "no-target-ladder-smoke"
)
NO_TARGET_LADDER_LOCAL_RUNS = NO_TARGET_LADDER_OUTPUT_DIR / "local-runs.jsonl"
NO_TARGET_LADDER_SUMMARY_CSV = (
    NO_TARGET_LADDER_OUTPUT_DIR / "summary" / "summary.csv"
)
NO_TARGET_LADDER_CASE_IDS = [
    "surface_rotated_d5",
    "toric_d5",
    "bb72",
    "bb144",
]


def _load_manifest(path: Path) -> dict[str, object]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def _read_jsonl_rows(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def _read_csv_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def _require_artifact_rows(
    jsonl_path: Path,
    summary_csv_path: Path,
) -> tuple[list[dict[str, object]], list[dict[str, str]]]:
    if not jsonl_path.exists():
        raise unittest.SkipTest(f"missing artifact: {jsonl_path}")
    if not summary_csv_path.exists():
        raise unittest.SkipTest(f"missing artifact: {summary_csv_path}")
    return _read_jsonl_rows(jsonl_path), _read_csv_rows(summary_csv_path)


def _makefile_target_body(makefile: str, target: str) -> str:
    match = re.search(rf"^{re.escape(target)}:\n(?P<body>(?:\t.*\n)+)", makefile, re.MULTILINE)
    if match is None:
        raise AssertionError(f"missing Make target {target}")
    return match.group("body")


class NoTargetLadderSuiteTest(unittest.TestCase):
    def test_rejects_target_weight_or_missing_required_case(self) -> None:
        manifest = _load_manifest(NO_TARGET_LADDER_SMOKE_MANIFEST)
        base_errors = validate_no_target_ladder_manifest(manifest)
        self.assertEqual(base_errors, [])

        case_ids = sorted(
            case["case_id"]
            for case in manifest["cases"]
            if isinstance(case, dict) and "case_id" in case
        )
        self.assertEqual(sorted(NO_TARGET_LADDER_REQUIRED_CASE_IDS), case_ids)

        with_target_weight = copy.deepcopy(manifest)
        with_target_weight["cases"][0]["target_weight"] = 5
        target_weight_errors = validate_no_target_ladder_manifest(with_target_weight)
        self.assertIn(
            'case "surface_rotated_d5" must omit field "target_weight" for no-target ladder runs',
            target_weight_errors,
        )

        missing_bb144 = copy.deepcopy(manifest)
        missing_bb144["cases"] = [
            case for case in manifest["cases"] if case["case_id"] != "bb144"
        ]
        missing_case_errors = validate_no_target_ladder_manifest(missing_bb144)
        self.assertIn(
            'no-target ladder manifest missing required case "bb144"',
            missing_case_errors,
        )

    def test_makefile_targets_no_target_ladder_smoke_pipeline(self) -> None:
        makefile = MAKEFILE.read_text(encoding="utf-8")
        body = _makefile_target_body(makefile, "qec-code-random-window-bench-no-target-ladder-smoke")

        self.assertIn("qec-code-random-window-bench-no-target-ladder-smoke", makefile)
        self.assertIn("qec-code-random-window-bench-no-target-ladder-smoke - Run release/no-target-ladder random-window smoke", makefile)
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

    def test_no_target_ladder_smoke_outputs_and_rows_are_release_no_target(self) -> None:
        jsonl_rows, _ = _require_artifact_rows(
            NO_TARGET_LADDER_LOCAL_RUNS,
            NO_TARGET_LADDER_SUMMARY_CSV,
        )
        case_ids = {row["case_id"] for row in jsonl_rows}
        for case_id in NO_TARGET_LADDER_CASE_IDS:
            self.assertIn(case_id, case_ids)

        for row in jsonl_rows:
            self.assertEqual(row["build_profile"], "release")
            self.assertIsNone(row["target_weight"])
            self.assertNotIn("--target-weight", row["command"])
            raw_cli_json = row["raw_cli_json"]
            self.assertIsInstance(raw_cli_json, dict)
            raw_options = raw_cli_json["options"]
            self.assertIsNone(raw_options["target_weight"])
            if row["status"] == "ok":
                self.assertIsInstance(row["upper_bound"], int)
                self.assertGreater(row["upper_bound"], 0)

    def test_no_target_ladder_summary_rows_are_reported(self) -> None:
        _, summary_rows = _require_artifact_rows(
            NO_TARGET_LADDER_LOCAL_RUNS,
            NO_TARGET_LADDER_SUMMARY_CSV,
        )
        summary_case_ids = {row["case_id"] for row in summary_rows}
        for case_id in NO_TARGET_LADDER_CASE_IDS:
            self.assertIn(case_id, summary_case_ids)


if __name__ == "__main__":
    unittest.main()
