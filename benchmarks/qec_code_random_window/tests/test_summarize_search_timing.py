from __future__ import annotations

import csv
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]


def read_csv_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


class SearchTimingSummaryTest(unittest.TestCase):
    def write_manifest(self, path: Path) -> None:
        path.write_text(
            """
manifest_version = 1
suite = "qec_code_random_window"

[[cases]]
case_id = "timing_case"
code_id = "surface_rotated:d=5"
distance_side = "any"
iterations = 20
restarts = 2
seed = 7
target_weight = 5
target_upper_bound = 5
baseline_key = "unmapped:timing"
baseline_required = false
""".lstrip(),
            encoding="utf-8",
        )

    def run_summarizer(
        self, manifest: Path, runs: Path, out_dir: Path
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

    def row(self, *, stats: dict[str, object], seed: int = 7) -> dict[str, object]:
        return {
            "case_id": "timing_case",
            "status": "ok",
            "seed": seed,
            "iterations": 20,
            "restarts": 2,
            "target_weight": 5,
            "upper_bound": 5,
            "elapsed_s": 1.25,
            "raw_cli_json": {
                "status": "completed",
                "method": "random-window-upper-bound",
                "search_stats": stats,
            },
        }

    def stats(self, **overrides: object) -> dict[str, object]:
        stats: dict[str, object] = {
            "permutations_sampled": 2,
            "kernel_basis_generations": 4,
            "component_candidates_generated": 8,
            "zero_candidates_rejected": 1,
            "weight_pruned_candidates": 0,
            "stabilizer_span_candidates_rejected": 2,
            "witness_validation_candidates_rejected": 3,
            "valid_witnesses_found": 2,
            "best_witness_updates": 1,
            "target_reached": True,
            "permutation_time_ns": 100,
            "kernel_basis_time_ns": 200,
            "span_filter_time_ns": 300,
            "witness_validation_time_ns": 400,
            "best_update_time_ns": 50,
            "total_search_time_ns": 1200,
        }
        stats.update(overrides)
        return stats

    def test_summarizes_search_timing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            rows = [
                self.row(stats=self.stats()),
                self.row(
                    seed=11,
                    stats=self.stats(
                        permutation_time_ns=150,
                        kernel_basis_time_ns=250,
                        span_filter_time_ns=350,
                        witness_validation_time_ns=450,
                        best_update_time_ns=75,
                        total_search_time_ns=1500,
                        target_reached=False,
                    ),
                ),
            ]
            runs.write_text(
                "".join(json.dumps(row) + "\n" for row in rows),
                encoding="utf-8",
            )

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertEqual(result.returncode, 0, result.stderr)
            row = read_csv_rows(out_dir / "summary.csv")[0]
            self.assertEqual(row["search_timing_rows"], "2")
            self.assertEqual(row["search_timing_total_permutation_time_ns"], "250")
            self.assertEqual(row["search_timing_total_kernel_basis_time_ns"], "450")
            self.assertEqual(row["search_timing_total_span_filter_time_ns"], "650")
            self.assertEqual(
                row["search_timing_total_witness_validation_time_ns"], "850"
            )
            self.assertEqual(row["search_timing_total_best_update_time_ns"], "125")
            self.assertEqual(row["search_timing_total_total_search_time_ns"], "2700")
            markdown = (out_dir / "summary.md").read_text(encoding="utf-8")
            self.assertIn("timing_rows=2", markdown)
            self.assertIn("total=0.003 ms", markdown)
            self.assertIn("kernel=0.001 ms", markdown)
            self.assertIn("witness=0.001 ms", markdown)

    def test_rejects_negative_or_inconsistent_timing(self) -> None:
        cases = [
            (
                self.stats(permutation_time_ns=-1),
                ["search_stats.permutation_time_ns"],
            ),
            (
                self.stats(total_search_time_ns=100),
                [
                    "search_stats.total_search_time_ns",
                    "search_stats.permutation_time_ns",
                ],
            ),
        ]
        for bad_stats, expected_stderr in cases:
            with self.subTest(expected_stderr=expected_stderr):
                with tempfile.TemporaryDirectory() as tmp:
                    tmp_path = Path(tmp)
                    manifest = tmp_path / "cases.toml"
                    runs = tmp_path / "runs.jsonl"
                    out_dir = tmp_path / "summary"
                    self.write_manifest(manifest)
                    runs.write_text(
                        json.dumps(self.row(stats=bad_stats)) + "\n",
                        encoding="utf-8",
                    )

                    result = self.run_summarizer(manifest, runs, out_dir)

                    self.assertNotEqual(result.returncode, 0)
                    for expected in expected_stderr:
                        self.assertIn(expected, result.stderr)


if __name__ == "__main__":
    unittest.main()
