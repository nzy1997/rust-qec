#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from typing import Any, Callable

from benchmarks.rstim_vs_stim_simulator import fair_cli_contract, run_fair_cli


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_fair_cli_evidence.py"
REQUIRED_ARTIFACTS = (
    "raw.jsonl",
    "summary.json",
    "baseline-summary.json",
    "comparison.json",
    "report.md",
    "environment.json",
)
BASELINE_SUMMARY_SHA256 = "131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07"
REFERENCE_SUMMARY = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json"
FIXTURE_REPO_PATH = fair_cli_contract.EXPECTED_CASE["canonical_input_path"]
KNOWN_ANSWER_INPUT_TOKEN = "artifact://known-answer-preflight.stim"
TOOL_ROLES = {
    "stim-cli-b8": "tool://stim",
    "rstim-cli-b8": "tool://rstim",
}


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rewrite_json(path: Path, mutate: Callable[[dict[str, Any]], None]) -> None:
    payload = json.loads(path.read_text(encoding="utf-8"))
    mutate(payload)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def rewrite_artifact_hashes(bundle: Path) -> None:
    payload = {filename: sha256_file(bundle / filename) for filename in REQUIRED_ARTIFACTS}
    (bundle / "artifact-sha256.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def expected_recorded_argv(variant: str, seed: int) -> list[str]:
    case = fair_cli_contract.EXPECTED_CASE
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        str(case["shots"]),
        "--seed",
        str(seed),
        "--out_format",
        case["output_format"],
        "--in",
        FIXTURE_REPO_PATH,
    ]


def expected_preflight_argv(variant: str) -> list[str]:
    case = fair_cli_contract.EXPECTED_CASE
    return [
        TOOL_ROLES[variant],
        "sample",
        "--shots",
        "1",
        "--seed",
        "0",
        "--out_format",
        case["output_format"],
        "--in",
        KNOWN_ANSWER_INPUT_TOKEN,
    ]


def write_valid_bundle(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    case = fair_cli_contract.EXPECTED_CASE
    fixture = REPO_ROOT / case["canonical_input_path"]
    fair_manifest = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml"
    source_manifest = REPO_ROOT / case["source_manifest_path"]
    stim_binary = path / "stim"
    rstim_binary = path / "rstim"
    stim_binary.write_bytes(b"temporary stim binary\n")
    rstim_binary.write_bytes(b"temporary rstim binary\n")

    records: list[dict[str, Any]] = []
    for variant, elapsed_base, stdout_hash in (
        ("stim-cli-b8", 1000, "a" * 64),
        ("rstim-cli-b8", 2000, "b" * 64),
    ):
        for phase, count in (("warmup", 2), ("measured", 7)):
            for round_index in range(count):
                seed = len([record for record in records if record["variant"] == variant])
                records.append(
                    {
                        "case_id": case["case_id"],
                        "variant": variant,
                        "phase": phase,
                        "round_index": round_index,
                        "seed": seed,
                        "argv": expected_recorded_argv(variant, seed),
                        "shots": case["shots"],
                        "measurement_count": case["measurement_count"],
                        "output_format": case["output_format"],
                        "timer_scope": case["timer_scope"],
                        "elapsed_ns": elapsed_base + seed,
                        "actual_output_bytes": case["expected_output_bytes"],
                        "stdout_sha256": stdout_hash,
                        "exit_code": 0,
                    }
                )

    (path / "raw.jsonl").write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records), encoding="utf-8"
    )
    summary = run_fair_cli._summary(records, case=case)
    (path / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    baseline_source = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/baseline-summary.json"
    if not baseline_source.exists():
        baseline_source = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/summary.json"
    (path / "baseline-summary.json").write_bytes(baseline_source.read_bytes())
    baseline_summary = json.loads((path / "baseline-summary.json").read_text(encoding="utf-8"))
    reference_evidence = run_fair_cli._reference_evidence(repo_root=REPO_ROOT)
    comparison = run_fair_cli._comparison(baseline_summary, summary, reference_evidence)
    (path / "comparison.json").write_text(
        json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (path / "report.md").write_text(run_fair_cli._render_report(summary, comparison), encoding="utf-8")
    environment = {
        "git_commit": "test-commit",
        "os": "test-os",
        "cpu_model": "test-cpu",
        "profile": "release",
        "timer_scope": case["timer_scope"],
        "seed_policy": case["seed_policy"],
        "stim_version": case["stim_version"],
        "rstim_version": "rstim test",
        "rustc_version": "rustc test",
        "manifest": "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml",
        "manifest_sha256": sha256_file(fair_manifest),
        "fair_manifest_path": "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml",
        "fair_manifest_sha256": sha256_file(fair_manifest),
        "source_manifest": case["source_manifest_path"],
        "source_manifest_sha256": sha256_file(source_manifest),
        "source_manifest_path": case["source_manifest_path"],
        "fixture": case["canonical_input_path"],
        "fixture_sha256": sha256_file(fixture),
        "fixture_path": case["canonical_input_path"],
        "runtime_identities": [
            {
                "role": "tool://stim",
                "version": "1.15.0",
                "basename": "stim",
                "sha256": "e7f31b9ac1780080161b3992e70644ade97dbe97369a9464997645c437a29323",
            },
            {
                "role": "tool://rstim",
                "version": "rstim 0.1.1",
                "basename": "rstim",
                "sha256": "2db6fa113495235829ca1dc7e4f8080befe3e6336f8effb61800b9e84510182a",
            },
        ],
        "round_argv": [
            {key: record[key] for key in ("variant", "phase", "round_index", "seed", "argv")}
            for record in records
        ],
        "warmup_rounds": 2,
        "measure_rounds": 7,
        "known_answer_preflight": "passed",
        "known_answer_preflight_details": [
            {
                "variant": variant,
                "argv": expected_preflight_argv(variant),
                "exit_code": 0,
                "stdout_hex": "01",
                "stdout_sha256": hashlib.sha256(bytes.fromhex("01")).hexdigest(),
                "elapsed_ns": 1,
            }
            for variant in ("stim-cli-b8", "rstim-cli-b8")
        ],
        "reference_evidence": reference_evidence,
    }
    (path / "environment.json").write_text(
        json.dumps(environment, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    rewrite_artifact_hashes(path)


class FairCliEvidenceCheckerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmpdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmpdir.cleanup)
        self.bundle = Path(self.tmpdir.name) / "bundle"
        write_valid_bundle(self.bundle)

    def run_checker(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(CHECKER), "--dir", str(self.bundle)],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_accepts_valid_bundle(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual("PASS fair CLI sampling evidence variants=2 measured=14\n", result.stdout)

    def test_committed_bundle_records_comparison_details(self) -> None:
        checker = __import__("tools.check_rstim_vs_stim_fair_cli_evidence", fromlist=["validate_bundle"])
        result = checker.validate_bundle(REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/fair-cli-release")
        self.assertEqual(result["baseline_rstim_over_stim"], 3.576)
        self.assertGreater(result["candidate_rstim_over_stim"], 1.0)
        self.assertEqual(result["reference_strategy"], "direct_inverse_repeat_folded")

    def test_rejects_candidate_summary_reused_from_baseline(self) -> None:
        (self.bundle / "summary.json").write_bytes((self.bundle / "baseline-summary.json").read_bytes())
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("candidate summary must differ from pinned baseline summary", result.stderr)

    def test_rejects_mismatched_reference_evidence_hash(self) -> None:
        def break_reference_hash(environment: dict[str, Any]) -> None:
            environment["reference_evidence"]["summary_sha256"] = "0" * 64

        rewrite_json(self.bundle / "environment.json", break_reference_hash)
        comparison = json.loads((self.bundle / "comparison.json").read_text(encoding="utf-8"))
        comparison["reference_summary_sha256"] = "0" * 64
        (self.bundle / "comparison.json").write_text(
            json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("reference_evidence summary_sha256 does not match reference summary", result.stderr)

    def test_rejects_unsupported_parity_wording_when_ratio_exceeds_one(self) -> None:
        report = (self.bundle / "report.md").read_text(encoding="utf-8") + "\nThis candidate reaches parity with Stim.\n"
        (self.bundle / "report.md").write_text(report, encoding="utf-8")
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("unsupported parity claim while candidate ratio exceeds 1.0", result.stderr)

    def test_rejects_comparison_not_derived_from_candidate(self) -> None:
        comparison = json.loads((self.bundle / "comparison.json").read_text(encoding="utf-8"))
        comparison["baseline_rstim_over_stim"] = 9.999
        (self.bundle / "comparison.json").write_text(
            json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("comparison.json does not match comparison derived from baseline and candidate summaries", result.stderr)

    def test_rejects_raw_semantic_error_before_artifact_hashes(self) -> None:
        records = [json.loads(line) for line in (self.bundle / "raw.jsonl").read_text().splitlines()]
        records[0]["output_format"] = "01"
        (self.bundle / "raw.jsonl").write_text(
            "".join(json.dumps(record, sort_keys=True) + "\n" for record in records), encoding="utf-8"
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-cli-b8 output_format must be b8", result.stderr)

    def test_rejects_host_absolute_raw_fixture_argument_before_artifact_hashes(self) -> None:
        records = [json.loads(line) for line in (self.bundle / "raw.jsonl").read_text().splitlines()]
        records[0]["argv"][records[0]["argv"].index("--in") + 1] = "/tmp/copied-fixture.stim"
        (self.bundle / "raw.jsonl").write_text(
            "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
            encoding="utf-8",
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-cli-b8 argv contains a host-absolute path", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_legacy_environment_binary_paths(self) -> None:
        def add_live_paths(environment: dict[str, Any]) -> None:
            environment["stim_binary"] = "/opt/homebrew/bin/stim"
            environment["stim_binary_sha256"] = "a" * 64
            environment["rstim_binary"] = "/tmp/rstim"
            environment["rstim_binary_sha256"] = "b" * 64

        rewrite_json(self.bundle / "environment.json", add_live_paths)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment must not contain live runtime path fields", result.stderr)

    def test_rejects_summary_not_derived_from_raw(self) -> None:
        rewrite_json(self.bundle / "summary.json", lambda summary: summary.update(measured_record_count=999))
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("summary.json does not match summary derived from raw.jsonl", result.stderr)

    def test_rejects_report_not_derived_from_raw(self) -> None:
        (self.bundle / "report.md").write_text("not the canonical report\n", encoding="utf-8")
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("report.md does not match report derived from raw.jsonl", result.stderr)

    def test_rejects_missing_artifact_hashes(self) -> None:
        (self.bundle / "artifact-sha256.json").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required bundle file: artifact-sha256.json", result.stderr)

    def test_rejects_self_consistent_substituted_provenance_paths(self) -> None:
        substitute = self.bundle / "unrelated-input"
        substitute.write_bytes(b"unrelated provenance input\n")

        def substitute_provenance(environment: dict[str, Any]) -> None:
            digest = sha256_file(substitute)
            for path_field, hash_field in (
                ("fair_manifest_path", "fair_manifest_sha256"),
                ("source_manifest_path", "source_manifest_sha256"),
                ("fixture_path", "fixture_sha256"),
            ):
                environment[path_field] = str(substitute)
                environment[hash_field] = digest
            environment["manifest"] = str(substitute)
            environment["manifest_sha256"] = digest
            environment["source_manifest"] = str(substitute)
            environment["source_manifest_sha256"] = digest
            environment["fixture"] = str(substitute)
            environment["fixture_sha256"] = digest

        rewrite_json(self.bundle / "environment.json", substitute_provenance)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "environment manifest must be benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml",
            result.stderr,
        )

    def test_rejects_absolute_fair_manifest_provenance(self) -> None:
        fair_manifest = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml"

        def record_absolute_manifest(environment: dict[str, Any]) -> None:
            environment["fair_manifest_path"] = str(fair_manifest)

        rewrite_json(self.bundle / "environment.json", record_absolute_manifest)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "environment fair_manifest_path must be benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml",
            result.stderr,
        )

    def test_rejects_absolute_source_manifest_provenance_aliases(self) -> None:
        source_manifest = REPO_ROOT / fair_cli_contract.EXPECTED_CASE["source_manifest_path"]

        def record_absolute_source_manifest(environment: dict[str, Any]) -> None:
            environment["source_manifest"] = str(source_manifest)
            environment["source_manifest_path"] = str(source_manifest)

        rewrite_json(self.bundle / "environment.json", record_absolute_source_manifest)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            f"environment source_manifest_path must be {fair_cli_contract.EXPECTED_CASE['source_manifest_path']}",
            result.stderr,
        )

    def test_rejects_absolute_fixture_provenance_aliases(self) -> None:
        fixture = REPO_ROOT / fair_cli_contract.EXPECTED_CASE["canonical_input_path"]

        def record_absolute_fixture(environment: dict[str, Any]) -> None:
            environment["fixture"] = str(fixture)
            environment["fixture_path"] = str(fixture)

        rewrite_json(self.bundle / "environment.json", record_absolute_fixture)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            f"environment fixture_path must be {fair_cli_contract.EXPECTED_CASE['canonical_input_path']}",
            result.stderr,
        )

    def test_rejects_missing_manifest_alias_provenance(self) -> None:
        def remove_manifest_aliases(environment: dict[str, Any]) -> None:
            del environment["manifest"]
            del environment["manifest_sha256"]

        rewrite_json(self.bundle / "environment.json", remove_manifest_aliases)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "environment manifest must be benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml",
            result.stderr,
        )

    def test_rejects_output_bytes_not_derived_from_measurements_and_shots(self) -> None:
        records = [json.loads(line) for line in (self.bundle / "raw.jsonl").read_text().splitlines()]
        records[2]["actual_output_bytes"] -= 1
        (self.bundle / "raw.jsonl").write_text(
            "".join(json.dumps(record, sort_keys=True) + "\n" for record in records), encoding="utf-8"
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "stim-cli-b8 actual_output_bytes must be 1552384 (1516 bytes per shot * 1024 shots)",
            result.stderr,
        )

    def test_rejects_preflight_detail_without_canonical_argv(self) -> None:
        def remove_argv(environment: dict[str, Any]) -> None:
            del environment["known_answer_preflight_details"][0]["argv"]

        rewrite_json(self.bundle / "environment.json", remove_argv)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-cli-b8 known-answer preflight argv must match canonical shape", result.stderr)

    def test_rejects_preflight_detail_with_wrong_stdout_hash(self) -> None:
        def replace_stdout_hash(environment: dict[str, Any]) -> None:
            environment["known_answer_preflight_details"][0]["stdout_sha256"] = "d" * 64

        rewrite_json(self.bundle / "environment.json", replace_stdout_hash)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-cli-b8 known-answer preflight stdout_sha256 must hash stdout_hex", result.stderr)

    def test_rejects_preflight_detail_with_nonzero_exit_code(self) -> None:
        def replace_exit_code(environment: dict[str, Any]) -> None:
            environment["known_answer_preflight_details"][0]["exit_code"] = 1

        rewrite_json(self.bundle / "environment.json", replace_exit_code)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-cli-b8 known-answer preflight exit_code must be 0", result.stderr)

    def test_rejects_preflight_detail_with_wrong_stdout_hex(self) -> None:
        def replace_stdout_hex(environment: dict[str, Any]) -> None:
            environment["known_answer_preflight_details"][0]["stdout_hex"] = "00"
            environment["known_answer_preflight_details"][0]["stdout_sha256"] = hashlib.sha256(
                bytes.fromhex("00")
            ).hexdigest()

        rewrite_json(self.bundle / "environment.json", replace_stdout_hex)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-cli-b8 known-answer preflight stdout_hex must be 01", result.stderr)

    def test_rejects_preflight_detail_without_elapsed_ns(self) -> None:
        def remove_elapsed_ns(environment: dict[str, Any]) -> None:
            del environment["known_answer_preflight_details"][0]["elapsed_ns"]

        rewrite_json(self.bundle / "environment.json", remove_elapsed_ns)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-cli-b8 known-answer preflight elapsed_ns must be a nonnegative integer", result.stderr)

    def test_raw_aggregate_changes_require_regeneration_and_accept_regenerated_artifacts(self) -> None:
        records = [json.loads(line) for line in (self.bundle / "raw.jsonl").read_text().splitlines()]
        records[2]["elapsed_ns"] += 10_000
        (self.bundle / "raw.jsonl").write_text(
            "".join(json.dumps(record, sort_keys=True) + "\n" for record in records), encoding="utf-8"
        )

        stale_result = self.run_checker()
        self.assertNotEqual(stale_result.returncode, 0, stale_result.stdout)
        self.assertIn("summary.json does not match summary derived from raw.jsonl", stale_result.stderr)
        self.assertNotIn("artifact-sha256.json", stale_result.stderr)

        summary = run_fair_cli._summary(records, case=fair_cli_contract.EXPECTED_CASE)
        (self.bundle / "summary.json").write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        baseline_summary = json.loads((self.bundle / "baseline-summary.json").read_text(encoding="utf-8"))
        reference_evidence = run_fair_cli._reference_evidence(repo_root=REPO_ROOT)
        comparison = run_fair_cli._comparison(baseline_summary, summary, reference_evidence)
        (self.bundle / "comparison.json").write_text(
            json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        (self.bundle / "report.md").write_text(run_fair_cli._render_report(summary, comparison), encoding="utf-8")
        rewrite_artifact_hashes(self.bundle)

        regenerated_result = self.run_checker()
        self.assertEqual(regenerated_result.returncode, 0, regenerated_result.stderr)
        self.assertEqual("PASS fair CLI sampling evidence variants=2 measured=14\n", regenerated_result.stdout)


if __name__ == "__main__":
    unittest.main()
