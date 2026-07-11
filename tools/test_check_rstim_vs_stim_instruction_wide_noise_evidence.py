#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from typing import Any, Callable


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py"
FIXTURE = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
MANIFEST = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
REQUIRED_ARTIFACTS = (
    "raw.jsonl",
    "summary.json",
    "report.md",
    "environment.json",
    "fixture-load.json",
    "correctness-summary.json",
)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rewrite_json(path: Path, mutate: Callable[[dict[str, Any]], None]) -> None:
    payload = json.loads(path.read_text(encoding="utf-8"))
    mutate(payload)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def rewrite_hashes(bundle: Path) -> None:
    (bundle / "artifact-sha256.json").write_text(
        json.dumps(
            {filename: sha256_file(bundle / filename) for filename in REQUIRED_ARTIFACTS},
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def write_valid_bundle(bundle: Path) -> None:
    from benchmarks.rstim_vs_stim_simulator import run_frame_instruction_wide_benchmark as runner

    bundle.mkdir(parents=True, exist_ok=True)
    rstim_binary = bundle / "rstim"
    rstim_binary.write_bytes(b"fake rstim binary\n")
    raw_records = [
        {
            "case_id": "stim_surface_d11_r100",
            "phase": "measured",
            "round_index": 0,
            "seed": 7,
            "operation": "X_ERROR",
            "sampling_path": "sparse",
            "instructions": 203,
            "targets": 24_362,
            "iterator_builds": 203,
            "attempt_count": 24_946_688,
            "elapsed_ns": 123,
            "stdout_sha256": "a" * 64,
            "actual_output_bytes": 1_552_384,
            "expected_output_bytes": 1_552_384,
            "output_bits": 12_121,
            "bytes_per_shot": 1_516,
            "output_format": "b8",
            "timer_scope": "process_spawn_stdout_stderr_drain_exit",
        },
        {
            "case_id": "stim_surface_d11_r100",
            "phase": "measured",
            "round_index": 0,
            "seed": 7,
            "operation": "DEPOLARIZE1",
            "sampling_path": "sparse",
            "instructions": 200,
            "targets": 12_000,
            "iterator_builds": 200,
            "attempt_count": 12_288_000,
            "elapsed_ns": 123,
            "stdout_sha256": "a" * 64,
            "actual_output_bytes": 1_552_384,
            "expected_output_bytes": 1_552_384,
            "output_bits": 12_121,
            "bytes_per_shot": 1_516,
            "output_format": "b8",
            "timer_scope": "process_spawn_stdout_stderr_drain_exit",
        },
        {
            "case_id": "stim_surface_d11_r100",
            "phase": "measured",
            "round_index": 0,
            "seed": 7,
            "operation": "DEPOLARIZE2",
            "sampling_path": "sparse",
            "instructions": 400,
            "pairs": 44_000,
            "iterator_builds": 400,
            "attempt_count": 45_056_000,
            "elapsed_ns": 123,
            "stdout_sha256": "a" * 64,
            "actual_output_bytes": 1_552_384,
            "expected_output_bytes": 1_552_384,
            "output_bits": 12_121,
            "bytes_per_shot": 1_516,
            "output_format": "b8",
            "timer_scope": "process_spawn_stdout_stderr_drain_exit",
        },
    ]
    (bundle / "raw.jsonl").write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in raw_records),
        encoding="utf-8",
    )
    summary = runner.derive_summary(raw_records, measurement=runner.MeasurementSummary(
        elapsed_ns=123,
        stdout_sha256="a" * 64,
        actual_output_bytes=1_552_384,
        expected_output_bytes=1_552_384,
        output_bits=12_121,
        bytes_per_shot=1_516,
    ))
    (bundle / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (bundle / "report.md").write_text(runner.render_report(summary), encoding="utf-8")
    fixture_load = {
        "case_id": "stim_surface_d11_r100",
        "status": "pass",
        "actual_measurements": 12_121,
        "actual_detectors": 12_000,
        "actual_observables": 1,
        "operations": {
            "X_ERROR": {"operation_count": 203, "target_count": 24_362},
            "DEPOLARIZE1": {"operation_count": 200, "target_count": 12_000},
            "DEPOLARIZE2": {"operation_count": 400, "target_count": 88_000},
        },
    }
    (bundle / "fixture-load.json").write_text(
        json.dumps(fixture_load, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    correctness = {
        "status": "pass",
        "mode": "detect",
        "seed": 7,
        "shots": 1024,
        "detectors": 12_000,
        "observables": 1,
        "output_format": "b8",
        "expected_output_bytes": ((12_000 + 1 + 7) // 8) * 1024,
        "stim_stdout_sha256": "b" * 64,
        "rstim_stdout_sha256": "b" * 64,
    }
    (bundle / "correctness-summary.json").write_text(
        json.dumps(correctness, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    environment = {
        "git_commit": "1" * 40,
        "git_dirty": False,
        "profile": "release",
        "case_id": "stim_surface_d11_r100",
        "seed": 7,
        "shots": 1024,
        "warmup_rounds": 0,
        "measure_rounds": 1,
        "timer_scope": "process_spawn_stdout_stderr_drain_exit",
        "stim_version": "1.15.0",
        "rstim_version": "rstim 0.1.1-test",
        "rustc_version": "rustc test",
        "os": "test-os",
        "cpu_model": "test-cpu",
        "fixture": str(FIXTURE),
        "fixture_sha256": sha256_file(FIXTURE),
        "manifest": str(MANIFEST),
        "manifest_sha256": sha256_file(MANIFEST),
        "rstim_binary": str(rstim_binary),
        "rstim_binary_sha256": sha256_file(rstim_binary),
        "runner_argv": ["python3", "-m", "benchmarks.rstim_vs_stim_simulator.run_frame_instruction_wide_benchmark"],
        "child_argv": {
            "measurement": ["rstim", "sample"],
            "correctness_stim": ["stim", "detect"],
            "correctness_rstim": ["rstim", "detect"],
        },
        "artifact_sha256": {},
    }
    for filename in REQUIRED_ARTIFACTS:
        if filename != "environment.json":
            environment["artifact_sha256"][filename] = sha256_file(bundle / filename)
    (bundle / "environment.json").write_text(
        json.dumps(environment, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    environment["artifact_sha256"]["environment.json"] = sha256_file(bundle / "environment.json")
    (bundle / "environment.json").write_text(
        json.dumps(environment, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    rewrite_hashes(bundle)


class InstructionWideEvidenceCheckerTest(unittest.TestCase):
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
        self.assertEqual(
            result.stdout,
            "PASS instruction-wide frame-noise evidence builds=803 attempts=82290688 legacy_setups=80362\n",
        )

    def test_rejects_fixture_load_count_substituted_for_iterator_builds_before_hash_error(self) -> None:
        records = [json.loads(line) for line in (self.bundle / "raw.jsonl").read_text().splitlines()]
        records[0]["iterator_builds"] = 80_362
        (self.bundle / "raw.jsonl").write_text(
            "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
            encoding="utf-8",
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("X_ERROR iterator_builds must be 203", result.stderr)
        self.assertNotIn("artifact", result.stderr.lower())

    def test_rejects_failed_or_sample_mode_correctness(self) -> None:
        for field, value, message in (
            ("status", "fail", "correctness-summary status must be pass"),
            ("mode", "sample", "correctness-summary mode must be detect"),
        ):
            with self.subTest(field=field):
                write_valid_bundle(self.bundle)
                rewrite_json(self.bundle / "correctness-summary.json", lambda payload: payload.update({field: value}))
                rewrite_hashes(self.bundle)
                result = self.run_checker()
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(message, result.stderr)

    def test_rejects_mismatched_fixture_manifest_binary_or_artifact_hash(self) -> None:
        for field, message in (
            ("fixture_sha256", "environment fixture_sha256 does not match fixture"),
            ("manifest_sha256", "environment manifest_sha256 does not match manifest"),
            ("rstim_binary_sha256", "environment rstim_binary_sha256 does not match rstim_binary"),
        ):
            with self.subTest(field=field):
                write_valid_bundle(self.bundle)
                rewrite_json(self.bundle / "environment.json", lambda payload: payload.update({field: "0" * 64}))
                rewrite_hashes(self.bundle)
                result = self.run_checker()
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(message, result.stderr)

        write_valid_bundle(self.bundle)
        rewrite_json(self.bundle / "artifact-sha256.json", lambda payload: payload.update({"raw.jsonl": "0" * 64}))
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("artifact-sha256.json raw.jsonl does not match raw.jsonl", result.stderr)

    def test_rejects_missing_hash_manifest(self) -> None:
        (self.bundle / "artifact-sha256.json").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required bundle file: artifact-sha256.json", result.stderr)


if __name__ == "__main__":
    unittest.main()
