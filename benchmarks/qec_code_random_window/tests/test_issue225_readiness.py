from __future__ import annotations

import csv
import json
import tempfile
import unittest
from pathlib import Path

from benchmarks.qec_code_random_window import issue225_readiness


def _stats(**overrides: object) -> dict[str, object]:
    stats = {
        "permutations_sampled": 10,
        "kernel_basis_generations": 500,
        "component_candidates_generated": 25,
        "zero_candidates_rejected": 0,
        "weight_pruned_candidates": 3,
        "stabilizer_span_candidates_rejected": 4,
        "witness_validation_candidates_rejected": 5,
        "valid_witnesses_found": 2,
        "best_witness_updates": 1,
        "target_reached": False,
        "permutation_time_ns": 100,
        "kernel_basis_time_ns": 200,
        "span_filter_time_ns": 300,
        "witness_validation_time_ns": 400,
        "best_update_time_ns": 50,
        "total_search_time_ns": 1200,
    }
    stats.update(overrides)
    return stats

BB144_CODE_ID = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"
LADDER_CASES = [
    ("surface_rotated_d5", "surface_rotated_d5", 5),
    ("toric_d5", "toric_d5", 5),
    ("bb72", "bb72", 6),
    ("bb144", BB144_CODE_ID, 12),
]
MULTISEED_CASES = [
    ("bb72_no_target_smoke", "bb72", 6),
    ("bb144_no_target_smoke", BB144_CODE_ID, 12),
]
MULTISEEDS = [7, 11, 17]
SUMMARY_FIELDS = [
    "case_id",
    "code_id",
    "distance_side",
    "baseline_key",
    "baseline_required",
    "manifest_seed",
    "manifest_iterations",
    "manifest_restarts",
    "manifest_target_weight",
    "target_upper_bound",
    "attempted_seed_rows",
    "successful_seed_rows",
    "best_upper_bound",
    "median_elapsed_s",
    "min_elapsed_s",
    "max_elapsed_s",
    "target_hit_count",
    "target_hit_rate",
    "run_seed_values",
    "run_iterations_values",
    "run_restarts_values",
    "run_target_weight_values",
    "run_build_profile_values",
    "run_status_values",
    "search_stats_rows",
    "search_stats_total_permutations_sampled",
    "search_stats_total_kernel_basis_generations",
    "search_stats_total_component_candidates_generated",
    "search_stats_total_zero_candidates_rejected",
    "search_stats_total_weight_pruned_candidates",
    "search_stats_total_stabilizer_span_candidates_rejected",
    "search_stats_total_witness_validation_candidates_rejected",
    "search_stats_total_valid_witnesses_found",
    "search_stats_total_best_witness_updates",
    "search_stats_target_reached_count",
    "search_timing_rows",
    "search_timing_total_permutation_time_ns",
    "search_timing_total_kernel_basis_time_ns",
    "search_timing_total_span_filter_time_ns",
    "search_timing_total_witness_validation_time_ns",
    "search_timing_total_best_update_time_ns",
    "search_timing_total_total_search_time_ns",
    "summary_status",
]


class Issue225ReadinessTest(unittest.TestCase):
    def _good_fixture(self, root: Path) -> dict[str, Path]:
        evidence = root / "issue225_evidence.json"
        evidence.write_text(
            json.dumps(
                {
                    "issue_225": {
                        "issue": 225,
                        "url": "https://github.com/nzy1997/rust-qec/issues/225",
                        "summary": "random-window upper-bound goal and acceleration closure readiness",
                    },
                    "chain": [
                        {
                            "milestone": "M1: benchmark evidence and no-target semantics",
                            "issue": 337,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/337",
                            "title": "Add a release no-target issue-225 ladder profiling smoke",
                            "pr": 340,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/340",
                            "merged_at": "2026-06-30T09:46:34Z",
                            "evidence": "Adds release/no-target ladder smoke for surface_rotated_d5, toric_d5, bb72, and bb144.",
                        },
                        {
                            "milestone": "M1: benchmark evidence and no-target semantics",
                            "issue": 338,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/338",
                            "title": "Report random-window search counters in CLI JSON",
                            "pr": 341,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/341",
                            "merged_at": "2026-06-30T10:02:25Z",
                            "evidence": "Adds search_stats counters and benchmark summary aggregation.",
                        },
                        {
                            "milestone": "M1: benchmark evidence and no-target semantics",
                            "issue": 339,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/339",
                            "title": "Add multi-seed no-target stability reporting",
                            "pr": 342,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/342",
                            "merged_at": "2026-07-01T01:31:41Z",
                            "evidence": "Adds BB72/BB144 no-target multi-seed summaries for seeds 7, 11, and 17.",
                        },
                        {
                            "milestone": "M2: diagnostics and pruning",
                            "issue": 343,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/343",
                            "title": "Add per-stage timing diagnostics to random-window search",
                            "pr": 347,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/347",
                            "merged_at": "2026-07-01T04:00:41Z",
                            "evidence": "Adds kernel, span, witness, and total timing buckets.",
                        },
                        {
                            "milestone": "M2: diagnostics and pruning",
                            "issue": 344,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/344",
                            "title": "Replace inner-loop witness validation with CSS component checks",
                            "pr": 349,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/349",
                            "merged_at": "2026-07-01T06:09:01Z",
                            "evidence": "Adds algebraic CSS component filtering before full witness construction.",
                        },
                        {
                            "milestone": "M2: diagnostics and pruning",
                            "issue": 345,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/345",
                            "title": "Prune candidates that cannot beat current best",
                            "pr": 348,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/348",
                            "merged_at": "2026-07-01T04:51:43Z",
                            "evidence": "Adds current-best pruning and weight_pruned_candidates evidence.",
                        },
                        {
                            "milestone": "M2: diagnostics and pruning",
                            "issue": 346,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/346",
                            "title": "Introduce a reusable GF(2) workspace",
                            "pr": 350,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/350",
                            "merged_at": "2026-07-01T07:14:04Z",
                            "evidence": "Reuses GF(2) workspace state for random-window kernel-basis generation.",
                        },
                        {
                            "milestone": "M3: bit-packed acceleration",
                            "issue": 351,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/351",
                            "title": "Add bit-packed GF(2) row primitives",
                            "pr": 355,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/355",
                            "merged_at": "2026-07-01T10:45:48Z",
                            "evidence": "Adds dense GF(2) row packing, XOR, parity, popcount, and zero checks.",
                        },
                        {
                            "milestone": "M3: bit-packed acceleration",
                            "issue": 352,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/352",
                            "title": "Use bit-packed kernel-basis generation",
                            "pr": 357,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/357",
                            "merged_at": "2026-07-01T12:19:13Z",
                            "evidence": "Routes random-window kernel-basis workspace through bit-packed GF(2) rows.",
                        },
                        {
                            "milestone": "M3: bit-packed acceleration",
                            "issue": 353,
                            "issue_url": "https://github.com/nzy1997/rust-qec/issues/353",
                            "title": "Use bit-packed CSS span filtering",
                            "pr": 356,
                            "pr_url": "https://github.com/nzy1997/rust-qec/pull/356",
                            "merged_at": "2026-07-01T12:52:16Z",
                            "evidence": "Routes CSS component filtering through bit-packed kernel and stabilizer-span checks.",
                        },
                    ],
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )

        ladder_runs = root / "ladder-runs.jsonl"
        ladder_rows = [
            self._run_row(case_id, code_id, seed=0, upper_bound=best_upper_bound)
            for case_id, code_id, best_upper_bound in LADDER_CASES
        ]
        self._write_jsonl(ladder_runs, ladder_rows)

        ladder_summary = root / "ladder-summary.csv"
        self._write_summary_csv(
            ladder_summary,
            [
                self._summary_row(case_id, code_id, best_upper_bound, "0")
                for case_id, code_id, best_upper_bound in LADDER_CASES
            ],
        )

        multiseed_runs = root / "multiseed-runs.jsonl"
        multiseed_rows = [
            self._run_row(case_id, code_id, seed=seed, upper_bound=best_upper_bound)
            for case_id, code_id, best_upper_bound in MULTISEED_CASES
            for seed in MULTISEEDS
        ]
        self._write_jsonl(multiseed_runs, multiseed_rows)

        multiseed_summary = root / "multiseed-summary.csv"
        self._write_summary_csv(
            multiseed_summary,
            [
                self._summary_row(case_id, code_id, best_upper_bound, "7;11;17")
                for case_id, code_id, best_upper_bound in MULTISEED_CASES
            ],
        )

        return {
            "evidence": evidence,
            "ladder_runs": ladder_runs,
            "ladder_summary": ladder_summary,
            "multiseed_runs": multiseed_runs,
            "multiseed_summary": multiseed_summary,
        }

    def _run_row(self, case_id: str, code_id: str, *, seed: int, upper_bound: int) -> dict[str, object]:
        return {
            "case_id": case_id,
            "code_id": code_id,
            "distance_side": "X",
            "seed": seed,
            "iterations": 100,
            "restarts": 1,
            "target_weight": None,
            "build_profile": "release",
            "target_upper_bound": upper_bound,
            "baseline_key": f"baseline:{case_id}",
            "baseline_required": False,
            "command": [
                "target/release/qec-code",
                "code",
                "css-distance",
                "random-window-upper-bound",
                "--code-id",
                code_id,
                "--iterations",
                "100",
                "--restarts",
                "1",
                "--seed",
                str(seed),
                "--json",
            ],
            "elapsed_s": 1.25,
            "upper_bound": upper_bound,
            "raw_cli_json": {
                "status": "completed",
                "method": "random-window-upper-bound",
                "upper_bound": upper_bound,
                "search_stats": _stats(),
            },
            "status": "ok",
        }

    def _summary_row(
        self,
        case_id: str,
        code_id: str,
        best_upper_bound: int,
        run_seed_values: str,
    ) -> dict[str, str]:
        stats = _stats()
        seed_count = len(run_seed_values.split(";"))
        return {
            "case_id": case_id,
            "code_id": code_id,
            "distance_side": "X",
            "baseline_key": f"baseline:{case_id}",
            "baseline_required": "false",
            "manifest_seed": "",
            "manifest_iterations": "100",
            "manifest_restarts": "1",
            "manifest_target_weight": "",
            "target_upper_bound": str(best_upper_bound),
            "attempted_seed_rows": str(seed_count),
            "successful_seed_rows": str(seed_count),
            "best_upper_bound": str(best_upper_bound),
            "median_elapsed_s": "1.25",
            "min_elapsed_s": "1.25",
            "max_elapsed_s": "1.25",
            "target_hit_count": "0",
            "target_hit_rate": "0.0",
            "run_seed_values": run_seed_values,
            "run_iterations_values": "100",
            "run_restarts_values": "1",
            "run_target_weight_values": "",
            "run_build_profile_values": "release",
            "run_status_values": "ok",
            "search_stats_rows": str(seed_count),
            "search_stats_total_permutations_sampled": str(stats["permutations_sampled"] * seed_count),
            "search_stats_total_kernel_basis_generations": str(stats["kernel_basis_generations"] * seed_count),
            "search_stats_total_component_candidates_generated": str(stats["component_candidates_generated"] * seed_count),
            "search_stats_total_zero_candidates_rejected": str(stats["zero_candidates_rejected"] * seed_count),
            "search_stats_total_weight_pruned_candidates": str(stats["weight_pruned_candidates"] * seed_count),
            "search_stats_total_stabilizer_span_candidates_rejected": str(stats["stabilizer_span_candidates_rejected"] * seed_count),
            "search_stats_total_witness_validation_candidates_rejected": str(stats["witness_validation_candidates_rejected"] * seed_count),
            "search_stats_total_valid_witnesses_found": str(stats["valid_witnesses_found"] * seed_count),
            "search_stats_total_best_witness_updates": str(stats["best_witness_updates"] * seed_count),
            "search_stats_target_reached_count": "0",
            "search_timing_rows": str(seed_count),
            "search_timing_total_permutation_time_ns": str(stats["permutation_time_ns"] * seed_count),
            "search_timing_total_kernel_basis_time_ns": str(stats["kernel_basis_time_ns"] * seed_count),
            "search_timing_total_span_filter_time_ns": str(stats["span_filter_time_ns"] * seed_count),
            "search_timing_total_witness_validation_time_ns": str(stats["witness_validation_time_ns"] * seed_count),
            "search_timing_total_best_update_time_ns": str(stats["best_update_time_ns"] * seed_count),
            "search_timing_total_total_search_time_ns": str(stats["total_search_time_ns"] * seed_count),
            "summary_status": "ok",
        }

    def _write_jsonl(self, path: Path, rows: list[dict[str, object]]) -> None:
        path.write_text(
            "\n".join(json.dumps(row) for row in rows) + "\n",
            encoding="utf-8",
        )

    def _write_summary_csv(self, path: Path, rows: list[dict[str, str]]) -> None:
        with path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=SUMMARY_FIELDS)
            writer.writeheader()
            writer.writerows(rows)

    def test_accepts_good_fixture_and_formats_report(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            paths = self._good_fixture(Path(tmp))

            report = issue225_readiness.evaluate_readiness(
                evidence_path=paths["evidence"],
                ladder_runs_path=paths["ladder_runs"],
                ladder_summary_path=paths["ladder_summary"],
                multiseed_runs_path=paths["multiseed_runs"],
                multiseed_summary_path=paths["multiseed_summary"],
            )
            markdown = report.to_markdown()

        self.assertEqual(report.decision, "PASS")
        self.assertIn("issue_225_readiness: PASS", markdown)
        for issue in ("225", "337", "338", "339", "343", "344", "345", "346", "351", "352", "353"):
            self.assertIn(issue, markdown)
            self.assertIn(f"https://github.com/nzy1997/rust-qec/issues/{issue}", markdown)
        for pr in ("340", "341", "342", "347", "348", "349", "350", "355", "356", "357"):
            self.assertIn(f"https://github.com/nzy1997/rust-qec/pull/{pr}", markdown)
        for token in ("surface_rotated_d5", "toric_d5", "bb72", "bb144", "5", "6", "12"):
            self.assertIn(token, markdown)
        self.assertIn(BB144_CODE_ID.replace("|", r"\|"), markdown)
        for token in (
            "target_weight = null",
            "target_reached = false",
            "build_profile = release",
            "weight_pruned_candidates",
            "kernel_basis_generations",
            "component_candidates_generated",
            "kernel_basis_time_ns",
            "span_filter_time_ns",
            "witness_validation_time_ns",
            "total_search_time_ns",
            "7;11;17",
        ):
            self.assertIn(token, markdown)

    def test_committed_issue225_evidence_uses_canonical_github_slug(self) -> None:
        evidence = Path(__file__).resolve().parents[3] / "benchmarks/qec_code_random_window/issue225_evidence.json"
        old_repo = "/".join(("nzy1997", "rstim"))
        old_issue_base = f"https://github.com/{old_repo}/issues/"
        old_pull_base = f"https://github.com/{old_repo}/pull/"
        content = evidence.read_text(encoding="utf-8")

        self.assertIn("https://github.com/nzy1997/rust-qec/issues/", content)
        self.assertIn("https://github.com/nzy1997/rust-qec/pull/", content)
        self.assertNotIn(old_issue_base, content)
        self.assertNotIn(old_pull_base, content)

    def test_rejects_missing_bb144_or_targeted_run(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            paths = self._good_fixture(Path(tmp))
            ladder_rows = [json.loads(line) for line in paths["ladder_runs"].read_text(encoding="utf-8").splitlines() if line.strip()]
            with paths["ladder_summary"].open(newline="", encoding="utf-8") as handle:
                ladder_summary_rows = list(csv.DictReader(handle))

            missing_rows = [row for row in ladder_rows if row["case_id"] != "bb144"]
            missing_summary = [row for row in ladder_summary_rows if row["case_id"] != "bb144"]
            self._write_jsonl(paths["ladder_runs"], missing_rows)
            self._write_summary_csv(paths["ladder_summary"], missing_summary)
            with self.assertRaises(issue225_readiness.Issue225ReadinessError) as cm:
                issue225_readiness.evaluate_readiness(
                    evidence_path=paths["evidence"],
                    ladder_runs_path=paths["ladder_runs"],
                    ladder_summary_path=paths["ladder_summary"],
                    multiseed_runs_path=paths["multiseed_runs"],
                    multiseed_summary_path=paths["multiseed_summary"],
                )
            self.assertIn("bb144", str(cm.exception))
            self.assertIn("missing", str(cm.exception))

            paths = self._good_fixture(Path(tmp))
            ladder_rows = [json.loads(line) for line in paths["ladder_runs"].read_text(encoding="utf-8").splitlines() if line.strip()]
            for row in ladder_rows:
                if row["case_id"] == "bb72":
                    row["target_weight"] = 6
                    row["command"].extend(["--target-weight", "6"])
                    row["raw_cli_json"]["search_stats"]["target_reached"] = True
            self._write_jsonl(paths["ladder_runs"], ladder_rows)
            with self.assertRaises(issue225_readiness.Issue225ReadinessError) as targeted_cm:
                issue225_readiness.evaluate_readiness(
                    evidence_path=paths["evidence"],
                    ladder_runs_path=paths["ladder_runs"],
                    ladder_summary_path=paths["ladder_summary"],
                    multiseed_runs_path=paths["multiseed_runs"],
                    multiseed_summary_path=paths["multiseed_summary"],
                )
            self.assertIn("bb72", str(targeted_cm.exception))
            self.assertTrue(
                "target_weight" in str(targeted_cm.exception)
                or "target_reached" in str(targeted_cm.exception)
            )

    def test_rejects_missing_timing_or_loose_upper_bound(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            paths = self._good_fixture(Path(tmp))
            ladder_rows = [json.loads(line) for line in paths["ladder_runs"].read_text(encoding="utf-8").splitlines() if line.strip()]
            for row in ladder_rows:
                if row["case_id"] == "bb72":
                    del row["raw_cli_json"]["search_stats"]["kernel_basis_time_ns"]
            self._write_jsonl(paths["ladder_runs"], ladder_rows)
            with self.assertRaises(issue225_readiness.Issue225ReadinessError) as timing_cm:
                issue225_readiness.evaluate_readiness(
                    evidence_path=paths["evidence"],
                    ladder_runs_path=paths["ladder_runs"],
                    ladder_summary_path=paths["ladder_summary"],
                    multiseed_runs_path=paths["multiseed_runs"],
                    multiseed_summary_path=paths["multiseed_summary"],
                )
            self.assertIn("bb72", str(timing_cm.exception))
            self.assertIn("kernel_basis_time_ns", str(timing_cm.exception))

            paths = self._good_fixture(Path(tmp))
            ladder_rows = [json.loads(line) for line in paths["ladder_runs"].read_text(encoding="utf-8").splitlines() if line.strip()]
            with paths["ladder_summary"].open(newline="", encoding="utf-8") as handle:
                ladder_summary_rows = list(csv.DictReader(handle))
            for row in ladder_rows:
                if row["case_id"] == "bb144":
                    row["upper_bound"] = 13
                    row["raw_cli_json"]["upper_bound"] = 13
            for row in ladder_summary_rows:
                if row["case_id"] == "bb144":
                    row["best_upper_bound"] = "13"
            self._write_jsonl(paths["ladder_runs"], ladder_rows)
            self._write_summary_csv(paths["ladder_summary"], ladder_summary_rows)
            with self.assertRaises(issue225_readiness.Issue225ReadinessError) as bound_cm:
                issue225_readiness.evaluate_readiness(
                    evidence_path=paths["evidence"],
                    ladder_runs_path=paths["ladder_runs"],
                    ladder_summary_path=paths["ladder_summary"],
                    multiseed_runs_path=paths["multiseed_runs"],
                    multiseed_summary_path=paths["multiseed_summary"],
                )
            self.assertIn("bb144", str(bound_cm.exception))
            self.assertIn("best_upper_bound", str(bound_cm.exception))

    def test_rejects_missing_no_target_fields_or_summary_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            paths = self._good_fixture(Path(tmp))
            ladder_rows = [json.loads(line) for line in paths["ladder_runs"].read_text(encoding="utf-8").splitlines() if line.strip()]
            for row in ladder_rows:
                if row["case_id"] == "bb72":
                    del row["target_weight"]
                    row["command"] = [
                        "target/release/qec-code",
                        "code",
                        "css-distance",
                        "random-window-upper-bound",
                        "--target-weight=6",
                        "--json",
                    ]
            self._write_jsonl(paths["ladder_runs"], ladder_rows)
            with self.assertRaises(issue225_readiness.Issue225ReadinessError) as row_cm:
                issue225_readiness.evaluate_readiness(
                    evidence_path=paths["evidence"],
                    ladder_runs_path=paths["ladder_runs"],
                    ladder_summary_path=paths["ladder_summary"],
                    multiseed_runs_path=paths["multiseed_runs"],
                    multiseed_summary_path=paths["multiseed_summary"],
                )
            self.assertIn("bb72", str(row_cm.exception))
            self.assertIn("target_weight", str(row_cm.exception))
            self.assertIn("--target-weight", str(row_cm.exception))

            paths = self._good_fixture(Path(tmp))
            ladder_rows = [json.loads(line) for line in paths["ladder_runs"].read_text(encoding="utf-8").splitlines() if line.strip()]
            for row in ladder_rows:
                if row["case_id"] == "bb72":
                    row["command"] = "target/release/qec-code --json"
            self._write_jsonl(paths["ladder_runs"], ladder_rows)
            with self.assertRaises(issue225_readiness.Issue225ReadinessError) as malformed_command_cm:
                issue225_readiness.evaluate_readiness(
                    evidence_path=paths["evidence"],
                    ladder_runs_path=paths["ladder_runs"],
                    ladder_summary_path=paths["ladder_summary"],
                    multiseed_runs_path=paths["multiseed_runs"],
                    multiseed_summary_path=paths["multiseed_summary"],
                )
            self.assertIn("bb72", str(malformed_command_cm.exception))
            self.assertIn("command", str(malformed_command_cm.exception))

            paths = self._good_fixture(Path(tmp))
            with paths["ladder_summary"].open(newline="", encoding="utf-8") as handle:
                ladder_summary_rows = list(csv.DictReader(handle))
            for row in ladder_summary_rows:
                if row["case_id"] == "bb144":
                    row["run_target_weight_values"] = "12"
                    row["run_build_profile_values"] = "debug;release"
                    row["search_stats_target_reached_count"] = "1"
                    row["summary_status"] = "no_success"
            self._write_summary_csv(paths["ladder_summary"], ladder_summary_rows)
            with self.assertRaises(issue225_readiness.Issue225ReadinessError) as summary_cm:
                issue225_readiness.evaluate_readiness(
                    evidence_path=paths["evidence"],
                    ladder_runs_path=paths["ladder_runs"],
                    ladder_summary_path=paths["ladder_summary"],
                    multiseed_runs_path=paths["multiseed_runs"],
                    multiseed_summary_path=paths["multiseed_summary"],
                )
            self.assertIn("bb144", str(summary_cm.exception))
            self.assertIn("run_target_weight_values", str(summary_cm.exception))
            self.assertIn("run_build_profile_values", str(summary_cm.exception))
            self.assertIn("search_stats_target_reached_count", str(summary_cm.exception))
            self.assertIn("summary_status", str(summary_cm.exception))


if __name__ == "__main__":
    unittest.main()
