from __future__ import annotations

import csv
import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]


def _write_manifest(path: Path) -> None:
    path.write_text(
        textwrap.dedent(
            """
            manifest_version = 1
            suite = "qec_code_random_window"

            [[cases]]
            case_id = "bb72_no_target_smoke"
            code_id = "bb72"
            distance_side = "any"
            iterations = 500
            restarts = 1
            seed = 7
            target_upper_bound = 6
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb72"
            baseline_required = true

            [[cases]]
            case_id = "bb144_no_target_smoke"
            code_id = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
            distance_side = "any"
            iterations = 500
            restarts = 1
            seed = 7
            target_upper_bound = 12
            baseline_key = "codeDistancePYPI:bivariate_bicycle:bb144"
            baseline_required = true
            """
        ).lstrip(),
        encoding="utf-8",
    )


def _row(
    case_id: str,
    seed: int,
    status: str,
    *,
    upper_bound: int,
    elapsed_s: float,
    build_profile: str = "release",
    target_weight: int | None = None,
    target_upper_bound: int,
) -> dict[str, object]:
    row: dict[str, object] = {
        "case_id": case_id,
        "status": status,
        "seed": seed,
        "iterations": 500,
        "restarts": 1,
        "target_weight": target_weight,
        "target_upper_bound": target_upper_bound,
        "elapsed_s": elapsed_s,
        "build_profile": build_profile,
        "command": [
            "target/release/qec-code",
            "code",
            "css-distance",
            "random-window-upper-bound",
            "--seed",
            str(seed),
            "--json",
        ],
    }
    if status == "ok":
        assert upper_bound is not None
        row["upper_bound"] = upper_bound
    else:
        row["upper_bound"] = upper_bound
        row["stderr_context"] = "fixture failure"
    return row


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )


def _read_csv_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


class MultiSeedSummaryTest(unittest.TestCase):
    def run_summarizer(
        self,
        manifest: Path,
        runs: Path,
        out_dir: Path,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                "-m",
                "benchmarks.qec_code_random_window.summarize",
                "--cases",
                str(manifest),
                "--runs",
                str(runs),
                "--out-dir",
                str(out_dir),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_multiseed_no_target_summary_reports_seed_stability_fields(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            _write_manifest(manifest)
            _write_jsonl(
                runs,
                [
                    _row("bb72_no_target_smoke", 7, "ok", upper_bound=6, elapsed_s=1.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 11, "ok", upper_bound=7, elapsed_s=3.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 17, "cli_error", upper_bound=None, elapsed_s=0.5, target_upper_bound=6),
                    _row("bb144_no_target_smoke", 7, "ok", upper_bound=12, elapsed_s=4.0, target_upper_bound=12),
                    _row("bb144_no_target_smoke", 11, "ok", upper_bound=13, elapsed_s=8.0, target_upper_bound=12),
                    _row("bb144_no_target_smoke", 17, "ok", upper_bound=12, elapsed_s=6.0, target_upper_bound=12),
                ],
            )

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertEqual(result.returncode, 0, result.stderr)
            rows = {row["case_id"]: row for row in _read_csv_rows(out_dir / "summary.csv")}
            self.assertEqual(rows["bb72_no_target_smoke"]["attempted_seed_rows"], "3")
            self.assertEqual(rows["bb72_no_target_smoke"]["successful_seed_rows"], "2")
            self.assertEqual(rows["bb72_no_target_smoke"]["run_seed_values"], "7;11;17")
            self.assertEqual(rows["bb72_no_target_smoke"]["run_target_weight_values"], "")
            self.assertEqual(rows["bb72_no_target_smoke"]["run_build_profile_values"], "release")
            self.assertEqual(rows["bb72_no_target_smoke"]["best_upper_bound"], "6")
            self.assertEqual(rows["bb72_no_target_smoke"]["target_hit_count"], "1")
            self.assertEqual(rows["bb72_no_target_smoke"]["target_hit_rate"], "0.500000")
            self.assertEqual(rows["bb72_no_target_smoke"]["median_elapsed_s"], "2.0")
            self.assertEqual(rows["bb72_no_target_smoke"]["min_elapsed_s"], "1.0")
            self.assertEqual(rows["bb72_no_target_smoke"]["max_elapsed_s"], "3.0")
            self.assertEqual(rows["bb144_no_target_smoke"]["attempted_seed_rows"], "3")
            self.assertEqual(rows["bb144_no_target_smoke"]["successful_seed_rows"], "3")
            self.assertEqual(rows["bb144_no_target_smoke"]["target_hit_count"], "2")
            self.assertEqual(rows["bb144_no_target_smoke"]["target_hit_rate"], "0.666667")
            self.assertEqual(rows["bb144_no_target_smoke"]["median_elapsed_s"], "6.0")
            markdown = (out_dir / "summary.md").read_text(encoding="utf-8")
            self.assertIn("observed_seeds", markdown)
            self.assertIn("target_hits", markdown)
            self.assertIn("target_weight", markdown)
            self.assertIn("build_profile", markdown)
            self.assertIn("| bb72_no_target_smoke |", markdown)
            self.assertIn("7;11;17", markdown)
            self.assertIn("1/2 (0.500000)", markdown)
            self.assertIn("none", markdown)
            self.assertNotIn("--target-weight", markdown)

    def test_rejects_missing_seed_or_mixed_build_profile(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            _write_manifest(manifest)

            missing_seed_runs = tmp_path / "missing-seed.jsonl"
            _write_jsonl(
                missing_seed_runs,
                [
                    _row("bb72_no_target_smoke", 7, "ok", upper_bound=6, elapsed_s=1.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 11, "ok", upper_bound=7, elapsed_s=3.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 17, "ok", upper_bound=6, elapsed_s=2.0, target_upper_bound=6),
                    _row("bb144_no_target_smoke", 7, "ok", upper_bound=12, elapsed_s=4.0, target_upper_bound=12),
                    _row("bb144_no_target_smoke", 11, "ok", upper_bound=13, elapsed_s=8.0, target_upper_bound=12),
                ],
            )

            missing_result = self.run_summarizer(
                manifest,
                missing_seed_runs,
                tmp_path / "missing-summary",
            )

            self.assertNotEqual(missing_result.returncode, 0)
            self.assertIn("bb144_no_target_smoke", missing_result.stderr)
            self.assertIn("seed", missing_result.stderr)
            self.assertIn("7;11;17", missing_result.stderr)

            mixed_profile_runs = tmp_path / "mixed-profile.jsonl"
            _write_jsonl(
                mixed_profile_runs,
                [
                    _row("bb72_no_target_smoke", 7, "ok", upper_bound=6, elapsed_s=1.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 11, "ok", upper_bound=7, elapsed_s=3.0, target_upper_bound=6),
                    _row("bb72_no_target_smoke", 17, "ok", upper_bound=6, elapsed_s=2.0, target_upper_bound=6),
                    _row("bb144_no_target_smoke", 7, "ok", upper_bound=12, elapsed_s=4.0, target_upper_bound=12),
                    _row("bb144_no_target_smoke", 11, "ok", upper_bound=13, elapsed_s=8.0, target_upper_bound=12, build_profile="debug"),
                    _row("bb144_no_target_smoke", 17, "ok", upper_bound=12, elapsed_s=6.0, target_upper_bound=12),
                ],
            )

            mixed_result = self.run_summarizer(
                manifest,
                mixed_profile_runs,
                tmp_path / "mixed-summary",
            )

            self.assertNotEqual(mixed_result.returncode, 0)
            self.assertIn("bb144_no_target_smoke", mixed_result.stderr)
            self.assertIn("build_profile", mixed_result.stderr)
            self.assertIn("debug;release", mixed_result.stderr)


if __name__ == "__main__":
    unittest.main()
