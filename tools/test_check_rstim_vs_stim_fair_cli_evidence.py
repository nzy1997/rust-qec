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
REQUIRED_ARTIFACTS = ("raw.jsonl", "summary.json", "report.md", "environment.json")


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
    for variant, executable, elapsed_base, stdout_hash in (
        ("stim-cli-b8", stim_binary, 1000, "a" * 64),
        ("rstim-cli-b8", rstim_binary, 2000, "b" * 64),
    ):
        for phase, count in (("warmup", 2), ("measured", 7)):
            for round_index in range(count):
                seed = len([record for record in records if record["variant"] == variant])
                argv = fair_cli_contract.expand_argv(
                    fair_cli_contract.EXPECTED_ARGV[variant],
                    case,
                    seed=seed,
                    rstim_binary=str(rstim_binary),
                )
                argv[0] = str(executable)
                records.append(
                    {
                        "case_id": case["case_id"],
                        "variant": variant,
                        "phase": phase,
                        "round_index": round_index,
                        "seed": seed,
                        "argv": argv,
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
    (path / "report.md").write_text(run_fair_cli._render_report(summary), encoding="utf-8")
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
        "manifest": str(fair_manifest),
        "manifest_sha256": sha256_file(fair_manifest),
        "fair_manifest_path": str(fair_manifest),
        "fair_manifest_sha256": sha256_file(fair_manifest),
        "source_manifest": case["source_manifest_path"],
        "source_manifest_sha256": sha256_file(source_manifest),
        "source_manifest_path": case["source_manifest_path"],
        "fixture": case["canonical_input_path"],
        "fixture_sha256": sha256_file(fixture),
        "fixture_path": case["canonical_input_path"],
        "stim_binary": str(stim_binary),
        "stim_binary_sha256": sha256_file(stim_binary),
        "rstim_binary": str(rstim_binary),
        "rstim_binary_sha256": sha256_file(rstim_binary),
        "round_argv": [
            {key: record[key] for key in ("variant", "phase", "round_index", "seed", "argv")}
            for record in records
        ],
        "warmup_rounds": 2,
        "measure_rounds": 7,
        "known_answer_preflight": "passed",
        "known_answer_preflight_details": [
            {"variant": variant, "exit_code": 0, "stdout_hex": "01", "stdout_sha256": "c" * 64}
            for variant in ("stim-cli-b8", "rstim-cli-b8")
        ],
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

    def test_rejects_raw_semantic_error_before_artifact_hashes(self) -> None:
        records = [json.loads(line) for line in (self.bundle / "raw.jsonl").read_text().splitlines()]
        records[0]["output_format"] = "01"
        (self.bundle / "raw.jsonl").write_text(
            "".join(json.dumps(record, sort_keys=True) + "\n" for record in records), encoding="utf-8"
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-cli-b8 output_format must be b8", result.stderr)

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


if __name__ == "__main__":
    unittest.main()
