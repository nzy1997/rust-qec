from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
import sys
from types import SimpleNamespace
from typing import Any, Callable

from benchmarks.rstim_vs_stim_simulator import fair_cli_contract, run_compiled_steady


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_compiled_steady_evidence.py"
COMMITTED_BUNDLE = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/results/compiled-steady-release"
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


def rehash_bundle(bundle: Path) -> None:
    rewrite_artifact_hashes(bundle)


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
    atom_loss_fixture = (REPO_ROOT / run_compiled_steady.ATOM_LOSS_FIXTURE_PATH).resolve()
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
    for variant_index, variant in enumerate(run_compiled_steady.VARIANTS, start=1):
        elapsed_base = variant_index * 1000
        variant_fixture = atom_loss_fixture if variant == run_compiled_steady.ATOM_LOSS_VARIANT else fixture
        compile_count, reference_build_count = run_compiled_steady._expected_lifecycle_counts(variant, 0)
        telemetry = {
            "variant": variant,
            "compile_count": compile_count,
            "reference_build_count": reference_build_count,
            "sample_call_count": 0,
            "fixture_sha256": sha256_file(variant_fixture),
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
                    "sample_b8_elapsed_ns": elapsed_base + request_id,
                    "end_to_end_elapsed_ns": elapsed_base + request_id + 100,
                    "output_bytes": case["expected_output_bytes"],
                }
            )
        final_compile_count, final_reference_build_count = run_compiled_steady._expected_lifecycle_counts(
            variant, 9
        )
        records.append(
            {
                "record_type": "final",
                "variant": variant,
                "telemetry": {
                    **telemetry,
                    "compile_count": final_compile_count,
                    "reference_build_count": final_reference_build_count,
                    "sample_call_count": 9,
                },
            }
        )

    fixture_rel = fixture.relative_to(REPO_ROOT).as_posix()
    atom_loss_fixture_rel = atom_loss_fixture.relative_to(REPO_ROOT).as_posix()
    fair_manifest_rel = fair_manifest.relative_to(REPO_ROOT).as_posix()
    source_manifest_rel = source_manifest.relative_to(REPO_ROOT).as_posix()
    stim_worker_module_rel = stim_worker_module.relative_to(REPO_ROOT).as_posix()
    rewrite_raw(path / "raw.jsonl", records)
    summary = run_compiled_steady._summary(records)
    (path / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (path / "report.md").write_text(run_compiled_steady._render_report(summary), encoding="utf-8")
    worker_argv = {
        variant: run_compiled_steady._portable_worker_argv(
            variant,
            atom_loss_fixture_rel if variant == run_compiled_steady.ATOM_LOSS_VARIANT else fixture_rel,
            seed=0,
        )
        for variant in run_compiled_steady.VARIANTS
    }
    environment = {
        "git_commit": "test-commit",
        "os": "test-os",
        "cpu_model": "test-cpu",
        "profile": "release",
        "timer_scope": run_compiled_steady.PRIMARY_TIMER_SCOPE,
        "secondary_timer_scope": run_compiled_steady.SECONDARY_TIMER_SCOPE,
        "seed_policy": "precompiled_and_rstim_interpreted_seed_once;stim_direct_seed_per_call",
        "stim_version": case["stim_version"],
        "stim_python_probe": {
            "status": "ok",
            "version": case["stim_version"],
        },
        "rstim_version": "rstim test",
        "rustc_version": "rustc test",
        "fair_manifest_path": fair_manifest_rel,
        "fair_manifest_sha256": sha256_file(fair_manifest),
        "source_manifest_path": source_manifest_rel,
        "source_manifest_sha256": sha256_file(source_manifest),
        "fixture_path": fixture_rel,
        "fixture_sha256": sha256_file(fixture),
        "atom_loss_fixture_path": atom_loss_fixture_rel,
        "atom_loss_fixture_sha256": sha256_file(atom_loss_fixture),
        "worker_argv": worker_argv,
        "canonical_worker_argv": worker_argv,
        "stim_worker_module_path": stim_worker_module_rel,
        "stim_worker_module_sha256": sha256_file(stim_worker_module),
        "runtime_identities": [
            {
                "role": "tool://python",
                "version": "Python test",
                "basename": python_executable.name,
                "sha256": sha256_file(python_executable),
            },
            {
                "role": "tool://stim-extension",
                "version": case["stim_version"],
                "basename": stim_extension.name,
                "sha256": sha256_file(stim_extension),
            },
            {
                "role": "tool://stim-worker",
                "version": case["stim_version"],
                "basename": stim_worker_module.name,
                "sha256": sha256_file(stim_worker_module),
            },
            {
                "role": "tool://rstim-worker",
                "version": "rstim test",
                "basename": rstim_worker.name,
                "sha256": sha256_file(rstim_worker),
            },
        ],
        "protocol_version": run_compiled_steady.PROTOCOL_VERSION,
        "seed": 0,
        "warmup_rounds": 2,
        "measure_rounds": 7,
        "known_answer_preflight": [
            {
                "variant": variant,
                "argv": run_compiled_steady._portable_worker_argv(
                    variant, "fixture://sample-b8-known-answer", seed=0
                ),
                "result_hex": "01",
                "ready": {
                    "variant": variant,
                    "compile_count": run_compiled_steady._expected_lifecycle_counts(variant, 0)[0],
                    "reference_build_count": run_compiled_steady._expected_lifecycle_counts(variant, 0)[1],
                    "sample_call_count": 0,
                    "measurement_count": 1,
                    "bytes_per_shot": 1,
                    "fixture_sha256": "0" * 64,
                },
                "final": {
                    "variant": variant,
                    "compile_count": run_compiled_steady._expected_lifecycle_counts(variant, 1)[0],
                    "reference_build_count": run_compiled_steady._expected_lifecycle_counts(variant, 1)[1],
                    "sample_call_count": 1,
                    "measurement_count": 1,
                    "bytes_per_shot": 1,
                    "fixture_sha256": "0" * 64,
                },
            }
            for variant in run_compiled_steady.VARIANTS
        ],
        "workers": [
            {"variant": variant, "command": worker_argv[variant]}
            for variant in run_compiled_steady.VARIANTS
        ],
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
            "PASS unified sample+b8 evidence variants=5 measured=35 lifecycle=verified/9\n",
        )

    def test_rejects_absolute_fair_manifest_path_with_required_message(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["fair_manifest_path"] = (
                "/Users/nzy/pycode/agent-desk/config/.agent-desk/worktrees/"
                "nzy1997-rstim/issue-454-run-1-agent-issue-454-publish-compiled-steady-state-sampling-evidence-run-1/"
                "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml"
            )

        rewrite_json(self.bundle / "environment.json", mutate)
        rehash_bundle(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("fair_manifest_path must be repository-relative", result.stderr)

    def test_rejects_runtime_identity_required_live_path(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["runtime_identities"][0]["required_live_path"] = True

        rewrite_json(self.bundle / "environment.json", mutate)
        rehash_bundle(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("checked evidence must not require a live runtime path", result.stderr)

    def test_rejects_obsolete_top_level_live_runtime_provenance_fields(self) -> None:
        obsolete_fields = (
            "python_executable",
            "loaded_stim_extension_path",
            "rstim_worker_binary_path",
            "python_executable_sha256",
            "loaded_stim_extension_sha256",
            "rstim_worker_binary_sha256",
        )
        for field in obsolete_fields:
            with self.subTest(field=field):
                write_valid_bundle(self.bundle, rstim_worker=self.rstim_worker)

                def mutate(environment: dict[str, Any], *, field: str = field) -> None:
                    environment[field] = "0" * 64 if field.endswith("_sha256") else "/obsolete/live/path"

                rewrite_json(self.bundle / "environment.json", mutate)
                rehash_bundle(self.bundle)
                result = self.run_checker()
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(
                    f"environment contains obsolete live runtime provenance field: {field}",
                    result.stderr,
                )

    def test_rejects_obsolete_stim_python_probe_live_runtime_provenance_fields(self) -> None:
        obsolete_fields = ("path", "package_path", "sha256")
        for field in obsolete_fields:
            with self.subTest(field=field):
                write_valid_bundle(self.bundle, rstim_worker=self.rstim_worker)

                def mutate(environment: dict[str, Any], *, field: str = field) -> None:
                    environment["stim_python_probe"][field] = "0" * 64 if field == "sha256" else "/obsolete/live/path"

                rewrite_json(self.bundle / "environment.json", mutate)
                rehash_bundle(self.bundle)
                result = self.run_checker()
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(
                    f"environment stim_python_probe contains obsolete live runtime provenance field: {field}",
                    result.stderr,
                )

    def test_rejects_host_absolute_worker_argv(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["worker_argv"]["stim-precompiled"][0] = "/usr/bin/python3"
            environment["canonical_worker_argv"]["stim-precompiled"] = environment["worker_argv"]["stim-precompiled"]
            environment["workers"][0]["command"] = environment["worker_argv"]["stim-precompiled"]

        rewrite_json(self.bundle / "environment.json", mutate)
        rehash_bundle(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("worker argv contains host-absolute path", result.stderr)

    def test_rejects_missing_raw_request_even_when_environment_claims_lifecycle(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        rewrite_raw(
            self.bundle / "raw.jsonl",
            [record for record in records if not (record["variant"] == "stim-precompiled" and record.get("request_id") == 8)],
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-precompiled must contain exactly 9 sample records", result.stderr)

    def test_rejects_duplicate_request_id(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "rstim-precompiled" and record.get("request_id") == 8)["request_id"] = 7
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("rstim-precompiled request IDs must be 0 through 8", result.stderr)

    def test_rejects_changed_cumulative_call_count(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "stim-precompiled" and record.get("request_id") == 4)["sample_call_count"] = 9
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-precompiled sample_call_count for request 4 must be 5, got 9", result.stderr)

    def test_rejects_changed_sample_shots(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "stim-precompiled" and record.get("request_id") == 3)["shots"] = 512
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-precompiled shots for request 3 must be 1024, got 512", result.stderr)

    def test_rejects_changed_sample_output_format(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "rstim-precompiled" and record.get("request_id") == 3)["output_format"] = "01"
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("rstim-precompiled output_format for request 3 must be b8, got '01'", result.stderr)

    def test_rejects_missing_worker_sample_b8_timing(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "stim-precompiled" and record.get("request_id") == 3).pop(
            "sample_b8_elapsed_ns"
        )
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "stim-precompiled sample_b8_elapsed_ns for request 3 must be a nonnegative integer",
            result.stderr,
        )

    def test_rejects_end_to_end_timing_shorter_than_worker_timing(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        sample = next(
            record for record in records if record["variant"] == "rstim-precompiled" and record.get("request_id") == 3
        )
        sample["end_to_end_elapsed_ns"] = sample["sample_b8_elapsed_ns"] - 1
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "rstim-precompiled end_to_end_elapsed_ns for request 3 must be at least sample_b8_elapsed_ns",
            result.stderr,
        )

    def test_rejects_boolean_lifecycle_counter(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "stim-precompiled" and record["record_type"] == "ready")["telemetry"]["compile_count"] = True
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("stim-precompiled ready compile_count must be integer 1, got True", result.stderr)

    def test_rejects_out_of_order_lifecycle_records(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        final_index = next(
            index
            for index, record in enumerate(records)
            if record["variant"] == "stim-precompiled" and record["record_type"] == "final"
        )
        final = records.pop(final_index)
        records.insert(1, final)
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "stim-precompiled records must appear as ready, nine samples, then final",
            result.stderr,
        )

    def test_rejects_final_compile_count_semantically_before_hashes(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == "rstim-precompiled" and record["record_type"] == "final")["telemetry"]["compile_count"] = 9
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("rstim-precompiled final compile_count must be 1, got 9", result.stderr)
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
                "tool://python",
                "-m",
                "benchmarks.rstim_vs_stim_simulator.workers.not_the_steady_worker",
                "--input",
                fixture,
                "--seed",
                "0",
            ]
            environment["worker_argv"]["stim-precompiled"] = command
            environment["canonical_worker_argv"]["stim-precompiled"] = command
            for worker in environment["workers"]:
                if worker["variant"] == "stim-precompiled":
                    worker["command"] = command

        rewrite_json(self.bundle / "environment.json", make_noncanonical)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment canonical_worker_argv must match release worker commands", result.stderr)

    def test_rejects_noncanonical_stim_worker_module_hash(self) -> None:
        substitute = REPO_ROOT / "Cargo.toml"

        def replace_module(environment: dict[str, Any]) -> None:
            environment["stim_worker_module_path"] = substitute.relative_to(REPO_ROOT).as_posix()
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
        substitute = REPO_ROOT / "Cargo.toml"

        def replace_manifest(environment: dict[str, Any]) -> None:
            environment["fair_manifest_path"] = substitute.relative_to(REPO_ROOT).as_posix()
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
        self.assertIn("environment rstim-precompiled preflight ready must be a JSON object", result.stderr)

    def test_rejects_preflight_argv_with_duplicate_seed_flag(self) -> None:
        def duplicate_seed(environment: dict[str, Any]) -> None:
            environment["known_answer_preflight"][0]["argv"].extend(["--seed", "1"])

        rewrite_json(self.bundle / "environment.json", duplicate_seed)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment rstim-precompiled preflight argv must match canonical shape", result.stderr)

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

    def test_runner_collect_environment_emits_portable_provenance(self) -> None:
        case = fair_cli_contract.EXPECTED_CASE
        fixture = (REPO_ROOT / case["canonical_input_path"]).resolve()
        stim_extension = self.bundle.parent / "_runner_stim.so"
        stim_extension.write_bytes(b"runner stim extension\n")
        args = SimpleNamespace(
            manifest=REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml",
            profile="release",
            seed=7,
            warmup_rounds=2,
            measure_rounds=7,
        )

        environment = run_compiled_steady._collect_environment(
            args=args,
            case=case,
            input_path=fixture,
            atom_loss_input_path=(REPO_ROOT / run_compiled_steady.ATOM_LOSS_FIXTURE_PATH).resolve(),
            rstim_command=[str(self.rstim_worker)],
            worker_details=[
                {"variant": "stim", "command": ["python3", "--input", str(fixture), "--seed", "0"]},
                {"variant": "rstim", "command": [str(self.rstim_worker), "--input", str(fixture), "--seed", "0"]},
            ],
            preflight_results=[
                {"variant": variant, "argv": ["placeholder"]}
                for variant in run_compiled_steady.VARIANTS
            ],
            stim_probe={
                "status": "ok",
                "version": case["stim_version"],
                "extension_module": "stim._stim_sse2",
                "path": str(stim_extension),
                "sha256": sha256_file(stim_extension),
            },
        )

        fixture_rel = fixture.relative_to(REPO_ROOT).as_posix()
        self.assertEqual(environment["fixture_path"], fixture_rel)
        self.assertEqual(environment["seed"], 7)
        self.assertEqual(
            environment["worker_argv"]["stim-precompiled"],
            [
                "tool://python",
                "-m",
                "benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady",
                "--variant",
                "stim-precompiled",
                "--input",
                fixture_rel,
                "--seed",
                "7",
            ],
        )
        self.assertEqual(
            environment["known_answer_preflight"][1]["argv"],
            [
                "tool://python",
                "-m",
                "benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady",
                "--variant",
                "stim-precompiled",
                "--input",
                "fixture://sample-b8-known-answer",
                "--seed",
                "7",
            ],
        )
        self.assertEqual(environment["workers"][0]["command"], environment["worker_argv"]["rstim-precompiled"])
        self.assertNotIn("python_executable", environment)
        self.assertNotIn("loaded_stim_extension_path", environment)
        self.assertNotIn("rstim_worker_binary_path", environment)
        self.assertEqual(
            {identity["role"] for identity in environment["runtime_identities"]},
            {"tool://python", "tool://stim-extension", "tool://stim-worker", "tool://rstim-worker"},
        )


if __name__ == "__main__":
    unittest.main()
