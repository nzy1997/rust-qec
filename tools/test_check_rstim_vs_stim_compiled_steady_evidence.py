from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from typing import Any, Callable

from benchmarks.rstim_vs_stim_simulator import fair_cli_contract, run_compiled_steady


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_compiled_steady_evidence.py"
ARTIFACT_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json")


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rewrite_json(path: Path, mutate: Callable[[dict[str, Any]], None]) -> None:
    payload = json.loads(path.read_text(encoding="utf-8"))
    mutate(payload)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def rewrite_raw(path: Path, records: list[dict[str, Any]]) -> None:
    path.write_text("".join(json.dumps(record, sort_keys=True) + "\n" for record in records), encoding="utf-8")


def load_raw(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def rewrite_artifact_hashes(bundle: Path) -> None:
    payload = {filename: sha256_file(bundle / filename) for filename in ARTIFACT_FILES}
    (bundle / "artifact-sha256.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def write_valid_bundle(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    case = fair_cli_contract.EXPECTED_CASE
    fixture = (REPO_ROOT / case["canonical_input_path"]).resolve()
    fair_manifest = (REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml").resolve()
    source_manifest = (REPO_ROOT / case["source_manifest_path"]).resolve()
    python_executable = path / "python3"
    stim_extension = path / "_stim.so"
    rstim_worker = path / "rstim_compiled_steady_worker"
    for artifact, contents in (
        (python_executable, b"python test executable\n"),
        (stim_extension, b"stim extension\n"),
        (rstim_worker, b"rstim worker\n"),
    ):
        artifact.write_bytes(contents)

    records: list[dict[str, Any]] = []
    for variant, elapsed_base in (("stim", 1000), ("rstim", 2000)):
        telemetry = {
            "variant": variant,
            "compile_count": 1,
            "reference_build_count": 1,
            "sample_call_count": 0,
            "fixture_sha256": sha256_file(fixture),
            "measurement_count": case["measurement_count"],
            "bytes_per_shot": case["bytes_per_shot"],
        }
        records.append({"record_type": "ready", "variant": variant, "telemetry": telemetry})
        for request_id in range(9):
            records.append(
                {
                    "record_type": "sample",
                    "variant": variant,
                    "request_id": request_id,
                    "sample_call_count": request_id + 1,
                    "warmup": request_id < 2,
                    "elapsed_ns": elapsed_base + request_id,
                    "output_bytes": case["expected_output_bytes"],
                }
            )
        records.append(
            {
                "record_type": "final",
                "variant": variant,
                "telemetry": {**telemetry, "sample_call_count": 9},
            }
        )

    rewrite_raw(path / "raw.jsonl", records)
    summary = run_compiled_steady._summary(records)
    (path / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (path / "report.md").write_text(run_compiled_steady._render_report(summary), encoding="utf-8")
    environment = {
        "git_commit": "test-commit",
        "os": "test-os",
        "cpu_model": "test-cpu",
        "profile": "release",
        "timer_scope": case["timer_scope"],
        "seed_policy": "seed_once_then_advance_across_9_calls",
        "stim_version": case["stim_version"],
        "stim_python_probe": {"status": "ok", "version": case["stim_version"], "path": str(stim_extension)},
        "rstim_version": "rstim test",
        "rustc_version": "rustc test",
        "fair_manifest_path": str(fair_manifest),
        "fair_manifest_sha256": sha256_file(fair_manifest),
        "source_manifest_path": str(source_manifest),
        "source_manifest_sha256": sha256_file(source_manifest),
        "fixture_path": str(fixture),
        "fixture_sha256": sha256_file(fixture),
        "worker_argv": {"stim": ["python3", "stim_worker"], "rstim": [str(rstim_worker)]},
        "canonical_worker_argv": {"stim": ["python3", "stim_worker"], "rstim": [str(rstim_worker)]},
        "python_executable": str(python_executable),
        "python_executable_sha256": sha256_file(python_executable),
        "loaded_stim_extension_path": str(stim_extension),
        "loaded_stim_extension_sha256": sha256_file(stim_extension),
        "rstim_worker_binary_path": str(rstim_worker),
        "rstim_worker_binary_sha256": sha256_file(rstim_worker),
        "protocol_version": 1,
        "seed": 0,
        "warmup_rounds": 2,
        "measure_rounds": 7,
        "known_answer_preflight": [
            {"variant": "stim", "result_hex": "01", "ready": {"sample_call_count": 0}, "final": {"sample_call_count": 1}},
            {"variant": "rstim", "result_hex": "01", "ready": {"sample_call_count": 0}, "final": {"sample_call_count": 1}},
        ],
        "workers": [
            {"variant": "stim", "command": ["python3", "stim_worker"]},
            {"variant": "rstim", "command": [str(rstim_worker)]},
        ],
        "lifecycle": {"compile_count": 1, "reference_build_count": 1, "sample_call_count": 9},
    }
    (path / "environment.json").write_text(
        json.dumps(environment, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    rewrite_artifact_hashes(path)


class CheckCompiledSteadyEvidenceTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.bundle = Path(self.temp_dir.name) / "bundle"
        write_valid_bundle(self.bundle)

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def run_checker(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(CHECKER), "--dir", str(self.bundle)],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_accepts_valid_bundle(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout,
            "PASS compiled steady-state sampling evidence variants=2 measured=14 lifecycle=1/1/9\n",
        )

    def test_rejects_missing_raw_request_even_when_environment_claims_lifecycle(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        rewrite_raw(
            self.bundle / "raw.jsonl",
            [record for record in records if not (record["variant"] == "stim" and record.get("request_id") == 8)],
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-compiled-steady-b8 must contain exactly 9 sample records", result.stderr)

    def test_rejects_duplicate_request_id(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "rstim" and record.get("request_id") == 8)["request_id"] = 7
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("rstim-compiled-steady-b8 request IDs must be 0 through 8", result.stderr)

    def test_rejects_changed_cumulative_call_count(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "stim" and record.get("request_id") == 4)["sample_call_count"] = 9
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-compiled-steady-b8 sample_call_count for request 4 must be 5, got 9", result.stderr)

    def test_rejects_final_compile_count_semantically_before_hashes(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "rstim" and record["record_type"] == "final")["telemetry"]["compile_count"] = 9
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("rstim-compiled-steady-b8 final compile_count must be 1, got 9", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_summary_not_derived_from_raw(self) -> None:
        rewrite_json(self.bundle / "summary.json", lambda summary: summary.update({"measured_records": 99}))
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("summary.json does not match summary derived from raw.jsonl", result.stderr)

    def test_rejects_missing_hash_manifest(self) -> None:
        (self.bundle / "artifact-sha256.json").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required bundle file: artifact-sha256.json", result.stderr)


if __name__ == "__main__":
    unittest.main()
