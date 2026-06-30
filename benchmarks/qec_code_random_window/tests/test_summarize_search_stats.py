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


class SearchStatsSummaryTest(unittest.TestCase):
    def write_manifest(self, path: Path) -> None:
        path.write_text(
            """
manifest_version = 1
suite = "qec_code_random_window"

[[cases]]
case_id = "stats_case"
code_id = "surface_rotated:d=5"
distance_side = "any"
iterations = 20
restarts = 2
seed = 7
target_weight = 5
target_upper_bound = 5
baseline_key = "unmapped:stats"
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

    def row(self, *, stats: dict[str, object]) -> dict[str, object]:
        return {
            "case_id": "stats_case",
            "status": "ok",
            "seed": 7,
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

    def test_summarizes_search_stats(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            rows = [
                self.row(
                    stats={
                        "permutations_sampled": 2,
                        "kernel_basis_generations": 4,
                        "component_candidates_generated": 8,
                        "zero_candidates_rejected": 1,
                        "stabilizer_span_candidates_rejected": 2,
                        "witness_validation_candidates_rejected": 3,
                        "valid_witnesses_found": 2,
                        "best_witness_updates": 1,
                        "target_reached": True,
                    }
                ),
                self.row(
                    stats={
                        "permutations_sampled": 3,
                        "kernel_basis_generations": 6,
                        "component_candidates_generated": 10,
                        "zero_candidates_rejected": 0,
                        "stabilizer_span_candidates_rejected": 1,
                        "witness_validation_candidates_rejected": 4,
                        "valid_witnesses_found": 3,
                        "best_witness_updates": 2,
                        "target_reached": False,
                    }
                ),
            ]
            runs.write_text(
                "".join(json.dumps(row) + "\n" for row in rows),
                encoding="utf-8",
            )

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertEqual(result.returncode, 0, result.stderr)
            row = read_csv_rows(out_dir / "summary.csv")[0]
            self.assertEqual(row["search_stats_rows"], "2")
            self.assertEqual(row["search_stats_total_permutations_sampled"], "5")
            self.assertEqual(
                row["search_stats_total_component_candidates_generated"], "18"
            )
            self.assertEqual(row["search_stats_total_best_witness_updates"], "3")
            self.assertEqual(row["search_stats_target_reached_count"], "1")
            markdown = (out_dir / "summary.md").read_text(encoding="utf-8")
            self.assertIn("stats_rows=2", markdown)
            self.assertIn("permutations=5", markdown)
            self.assertIn("target_reached=1", markdown)

    def test_rejects_valid_witnesses_found_above_component_candidates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            bad_stats = {
                "permutations_sampled": 1,
                "kernel_basis_generations": 1,
                "component_candidates_generated": 1,
                "zero_candidates_rejected": 0,
                "stabilizer_span_candidates_rejected": 0,
                "witness_validation_candidates_rejected": 0,
                "valid_witnesses_found": 2,
                "best_witness_updates": 1,
                "target_reached": False,
            }
            runs.write_text(json.dumps(self.row(stats=bad_stats)) + "\n", encoding="utf-8")

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("search_stats.valid_witnesses_found", result.stderr)
            self.assertIn("search_stats.component_candidates_generated", result.stderr)

    def test_rejects_best_witness_updates_above_component_candidates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            bad_stats = {
                "permutations_sampled": 1,
                "kernel_basis_generations": 1,
                "component_candidates_generated": 1,
                "zero_candidates_rejected": 0,
                "stabilizer_span_candidates_rejected": 0,
                "witness_validation_candidates_rejected": 0,
                "valid_witnesses_found": 1,
                "best_witness_updates": 2,
                "target_reached": False,
            }
            runs.write_text(json.dumps(self.row(stats=bad_stats)) + "\n", encoding="utf-8")

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("search_stats.best_witness_updates", result.stderr)
            self.assertIn("search_stats.component_candidates_generated", result.stderr)

    def test_rejects_best_witness_updates_above_valid_witnesses(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            bad_stats = {
                "permutations_sampled": 1,
                "kernel_basis_generations": 1,
                "component_candidates_generated": 3,
                "zero_candidates_rejected": 0,
                "stabilizer_span_candidates_rejected": 0,
                "witness_validation_candidates_rejected": 0,
                "valid_witnesses_found": 1,
                "best_witness_updates": 2,
                "target_reached": False,
            }
            runs.write_text(json.dumps(self.row(stats=bad_stats)) + "\n", encoding="utf-8")

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("search_stats.best_witness_updates", result.stderr)
            self.assertIn("search_stats.valid_witnesses_found", result.stderr)

    def test_rejects_non_boolean_target_reached(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            bad_stats = {
                "permutations_sampled": 1,
                "kernel_basis_generations": 1,
                "component_candidates_generated": 1,
                "zero_candidates_rejected": 0,
                "stabilizer_span_candidates_rejected": 0,
                "witness_validation_candidates_rejected": 0,
                "valid_witnesses_found": 1,
                "best_witness_updates": 1,
                "target_reached": "yes",
            }
            runs.write_text(json.dumps(self.row(stats=bad_stats)) + "\n", encoding="utf-8")

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("search_stats.target_reached", result.stderr)

    def test_rejects_negative_counters(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            manifest = tmp_path / "cases.toml"
            runs = tmp_path / "runs.jsonl"
            out_dir = tmp_path / "summary"
            self.write_manifest(manifest)
            bad_stats = {
                "permutations_sampled": -1,
                "kernel_basis_generations": 1,
                "component_candidates_generated": 1,
                "zero_candidates_rejected": 0,
                "stabilizer_span_candidates_rejected": 0,
                "witness_validation_candidates_rejected": 0,
                "valid_witnesses_found": 1,
                "best_witness_updates": 1,
                "target_reached": False,
            }
            runs.write_text(json.dumps(self.row(stats=bad_stats)) + "\n", encoding="utf-8")

            result = self.run_summarizer(manifest, runs, out_dir)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("search_stats.permutations_sampled", result.stderr)


if __name__ == "__main__":
    unittest.main()
