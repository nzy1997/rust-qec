from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
import sys
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


def ensure_canonical_rstim_worker_binary() -> tuple[Path, bool]:
    worker = (REPO_ROOT / "target/release/rstim_compiled_steady_worker").resolve()
    if worker.is_file():
        return worker, False
    worker.parent.mkdir(parents=True, exist_ok=True)
    worker.write_bytes(b"test rstim compiled steady worker\n")
    worker.chmod(0o755)
    return worker, True


def write_valid_bundle(path: Path, *, rstim_worker: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    case = fair_cli_contract.EXPECTED_CASE
    fixture = (REPO_ROOT / case["canonical_input_path"]).resolve()
    fair_manifest = (REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml").resolve()
    source_manifest = (REPO_ROOT / case["source_manifest_path"]).resolve()
    stim_worker_module = (REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/workers/stim_compiled_steady.py").resolve()
    python_executable = Path(shutil.which("python3") or sys.executable).resolve()
    stim_extension = path.parent / "_stim.so"
    for artifact, contents in (
        (stim_extension, b"stim extension\n"),
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
                    "shots": case["shots"],
                    "output_format": case["output_format"],
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
    worker_argv = {
        "stim": [*run_compiled_steady.default_stim_worker_command(), "--input", str(fixture), "--seed", "0"],
        "rstim": [*run_compiled_steady.default_rstim_worker_command("release"), "--input", str(fixture), "--seed", "0"],
    }
    environment = {
        "git_commit": "test-commit",
        "os": "test-os",
        "cpu_model": "test-cpu",
        "profile": "release",
        "timer_scope": case["timer_scope"],
        "seed_policy": "seed_once_then_advance_across_9_calls",
        "stim_version": case["stim_version"],
        "stim_python_probe": {
            "status": "ok",
            "version": case["stim_version"],
            "path": str(stim_extension),
            "sha256": sha256_file(stim_extension),
        },
        "rstim_version": "rstim test",
        "rustc_version": "rustc test",
        "fair_manifest_path": str(fair_manifest),
        "fair_manifest_sha256": sha256_file(fair_manifest),
        "source_manifest_path": str(source_manifest),
        "source_manifest_sha256": sha256_file(source_manifest),
        "fixture_path": str(fixture),
        "fixture_sha256": sha256_file(fixture),
        "worker_argv": worker_argv,
        "canonical_worker_argv": worker_argv,
        "stim_worker_module_path": str(stim_worker_module),
        "stim_worker_module_sha256": sha256_file(stim_worker_module),
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
            {
                "variant": "stim",
                "argv": [*run_compiled_steady.default_stim_worker_command(), "--input", str(path / "known_answer.stim"), "--seed", "0"],
                "result_hex": "01",
                "ready": {
                    "variant": "stim",
                    "compile_count": 1,
                    "reference_build_count": 1,
                    "sample_call_count": 0,
                    "measurement_count": 1,
                    "bytes_per_shot": 1,
                    "fixture_sha256": "0" * 64,
                },
                "final": {
                    "variant": "stim",
                    "compile_count": 1,
                    "reference_build_count": 1,
                    "sample_call_count": 1,
                    "measurement_count": 1,
                    "bytes_per_shot": 1,
                    "fixture_sha256": "0" * 64,
                },
            },
            {
                "variant": "rstim",
                "argv": [*run_compiled_steady.default_rstim_worker_command("release"), "--input", str(path / "known_answer.stim"), "--seed", "0"],
                "result_hex": "01",
                "ready": {
                    "variant": "rstim",
                    "compile_count": 1,
                    "reference_build_count": 1,
                    "sample_call_count": 0,
                    "measurement_count": 1,
                    "bytes_per_shot": 1,
                    "fixture_sha256": "0" * 64,
                },
                "final": {
                    "variant": "rstim",
                    "compile_count": 1,
                    "reference_build_count": 1,
                    "sample_call_count": 1,
                    "measurement_count": 1,
                    "bytes_per_shot": 1,
                    "fixture_sha256": "0" * 64,
                },
            },
        ],
        "workers": [
            {"variant": "stim", "command": worker_argv["stim"]},
            {"variant": "rstim", "command": worker_argv["rstim"]},
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
        self.rstim_worker, created_worker = ensure_canonical_rstim_worker_binary()
        if created_worker:
            self.addCleanup(self.rstim_worker.unlink, missing_ok=True)
        write_valid_bundle(self.bundle, rstim_worker=self.rstim_worker)

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

    def test_rejects_changed_sample_shots(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "stim" and record.get("request_id") == 3)["shots"] = 512
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-compiled-steady-b8 shots for request 3 must be 1024, got 512", result.stderr)

    def test_rejects_changed_sample_output_format(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "rstim" and record.get("request_id") == 3)["output_format"] = "01"
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("rstim-compiled-steady-b8 output_format for request 3 must be b8, got '01'", result.stderr)

    def test_rejects_boolean_lifecycle_counter(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "stim" and record["record_type"] == "ready")["telemetry"]["compile_count"] = True
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-compiled-steady-b8 ready compile_count must be integer 1, got True", result.stderr)

    def test_rejects_out_of_order_lifecycle_records(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        final_index = next(
            index
            for index, record in enumerate(records)
            if record["variant"] == "stim" and record["record_type"] == "final"
        )
        final = records.pop(final_index)
        records.insert(1, final)
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "stim-compiled-steady-b8 records must appear as ready, nine samples, then final",
            result.stderr,
        )

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

    def test_rejects_rehashed_report_not_derived_from_raw(self) -> None:
        (self.bundle / "report.md").write_text("not the canonical report\n", encoding="utf-8")
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("report.md does not match report derived from raw.jsonl", result.stderr)

    def test_rejects_noncanonical_worker_argv_even_when_hashes_match(self) -> None:
        def make_noncanonical(environment: dict[str, Any]) -> None:
            fixture = environment["fixture_path"]
            command = [
                str(Path(environment["python_executable"]).resolve()),
                "-m",
                "benchmarks.rstim_vs_stim_simulator.workers.not_the_steady_worker",
                "--input",
                fixture,
                "--seed",
                "0",
            ]
            environment["worker_argv"]["stim"] = command
            environment["canonical_worker_argv"]["stim"] = command
            for worker in environment["workers"]:
                if worker["variant"] == "stim":
                    worker["command"] = command

        rewrite_json(self.bundle / "environment.json", make_noncanonical)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment canonical_worker_argv must match release worker commands", result.stderr)

    def test_rejects_noncanonical_stim_worker_module_hash(self) -> None:
        substitute = self.bundle.parent / "stim_compiled_steady.py"
        substitute.write_text("# not the canonical worker module\n", encoding="utf-8")

        def replace_module(environment: dict[str, Any]) -> None:
            environment["stim_worker_module_path"] = str(substitute)
            environment["stim_worker_module_sha256"] = sha256_file(substitute)

        rewrite_json(self.bundle / "environment.json", replace_module)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment stim_worker_module_path must name the canonical Stim worker module", result.stderr)

    def test_rejects_fixture_sha_that_is_not_canonical_even_when_self_consistent(self) -> None:
        def replace_fixture_hash(environment: dict[str, Any]) -> None:
            environment["fixture_sha256"] = "1" * 64

        records = load_raw(self.bundle / "raw.jsonl")
        for record in records:
            if "telemetry" in record:
                record["telemetry"]["fixture_sha256"] = "1" * 64
        rewrite_raw(self.bundle / "raw.jsonl", records)
        rewrite_json(self.bundle / "environment.json", replace_fixture_hash)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("fixture_sha256 must match canonical fixture SHA-256", result.stderr)

    def test_rejects_noncanonical_fair_manifest_even_when_hashes_match(self) -> None:
        substitute = self.bundle.parent / "fair_cli_cases.toml"
        substitute.write_text("[[cases]]\ncase_id = \"wrong\"\n", encoding="utf-8")

        def replace_manifest(environment: dict[str, Any]) -> None:
            environment["fair_manifest_path"] = str(substitute)
            environment["fair_manifest_sha256"] = sha256_file(substitute)

        rewrite_json(self.bundle / "environment.json", replace_manifest)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment fair_manifest_path must name the canonical fair manifest", result.stderr)

    def test_rejects_malformed_preflight_telemetry(self) -> None:
        def replace_ready(environment: dict[str, Any]) -> None:
            environment["known_answer_preflight"][0]["ready"] = "not an object"

        rewrite_json(self.bundle / "environment.json", replace_ready)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment stim preflight ready must be a JSON object", result.stderr)

    def test_rejects_preflight_argv_with_duplicate_seed_flag(self) -> None:
        def duplicate_seed(environment: dict[str, Any]) -> None:
            environment["known_answer_preflight"][0]["argv"].extend(["--seed", "1"])

        rewrite_json(self.bundle / "environment.json", duplicate_seed)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment stim preflight argv must match canonical shape", result.stderr)

    def test_rejects_boolean_environment_lifecycle_counter(self) -> None:
        def replace_lifecycle(environment: dict[str, Any]) -> None:
            environment["lifecycle"] = {
                "compile_count": True,
                "reference_build_count": 1,
                "sample_call_count": 9,
            }

        rewrite_json(self.bundle / "environment.json", replace_lifecycle)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment lifecycle compile_count must be integer 1, got True", result.stderr)

    def test_rejects_extra_bundle_file(self) -> None:
        (self.bundle / "extra.txt").write_text("unexpected\n", encoding="utf-8")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("unexpected bundle file: extra.txt", result.stderr)

    def test_rejects_missing_hash_manifest(self) -> None:
        (self.bundle / "artifact-sha256.json").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required bundle file: artifact-sha256.json", result.stderr)


if __name__ == "__main__":
    unittest.main()
