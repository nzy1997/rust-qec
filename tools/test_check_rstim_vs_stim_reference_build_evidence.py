from __future__ import annotations

import base64
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from statistics import median
from typing import Any, Callable


REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools" / "check_rstim_vs_stim_reference_build_evidence.py"
ARTIFACT_FILES = ("raw.jsonl", "summary.json", "report.md", "environment.json")
PROTOCOL = "reference-build-v1"
TIMER_SCOPE = "reference_build_only"
REFERENCE_DIGEST = "d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d"
MANIFEST_DIGEST = "9fc35393f362f709e90bfd64ab08eda5140844974a7e685fd1e5614f67e0c921"
FIXTURE_DIGEST = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
MEASUREMENT_BITS = 12121
PACKED_BYTES = 1516
STIM_VARIANT = "stim-reference-b8"
RSTIM_VARIANT = "rstim-packed-reference-b8"
FIXTURE_REL = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
MANIFEST_REL = "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
PYTHON_ROLE = "tool://python"
STIM_WORKER_ROLE = "tool://stim-reference-worker"
RSTIM_WORKER_ROLE = "tool://rstim-reference-worker"
STIM_WORKER_REL = "benchmarks/rstim_vs_stim_simulator/workers/stim_reference_build.py"
RSTIM_WORKER_VERSION = "rstim 0.1.1"


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command_stdout(command: list[str]) -> str:
    result = subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise AssertionError(f"{command!r} failed: {result.stderr}")
    return result.stdout.strip()


def rewrite_json(path: Path, mutate: Callable[[dict[str, Any]], None]) -> None:
    payload = json.loads(path.read_text(encoding="utf-8"))
    mutate(payload)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def rewrite_raw(path: Path, records: list[dict[str, Any]]) -> None:
    path.write_text("".join(json.dumps(record, sort_keys=True) + "\n" for record in records), encoding="utf-8")


def rewrite_artifact_hashes(bundle: Path) -> None:
    payload = {filename: sha256_file(bundle / filename) for filename in ARTIFACT_FILES}
    (bundle / "artifact-sha256.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def load_raw(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# Packed Reference-Build Evidence",
        "",
        "| variant | count | min_elapsed_ns | median_elapsed_ns | max_elapsed_ns | backend | parse_count | final_reference_build_count | byte_sha256 |",
        "| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- |",
    ]
    for variant in summary["variants"]:
        lines.append(
            f"| {variant['variant']} | {variant['count']} | {variant['min_elapsed_ns']} | "
            f"{variant['median_elapsed_ns']} | {variant['max_elapsed_ns']} | {variant['backend']} | "
            f"{variant['parse_count']} | {variant['final_reference_build_count']} | {variant['byte_sha256']} |"
        )
    return "\n".join(lines) + "\n"


def derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    variants = []
    for variant, backend in (
        (STIM_VARIANT, "stim_reference"),
        (RSTIM_VARIANT, "packed_inverse"),
    ):
        measured = [
            record
            for record in records
            if record["variant"] == variant and record["phase"] == "measured"
        ]
        elapsed = [record["elapsed_ns"] for record in measured]
        all_variant_records = [record for record in records if record["variant"] == variant]
        variants.append(
            {
                "variant": variant,
                "count": len(measured),
                "min_elapsed_ns": min(elapsed),
                "median_elapsed_ns": int(median(elapsed)),
                "max_elapsed_ns": max(elapsed),
                "measurement_bits": MEASUREMENT_BITS,
                "packed_bytes": PACKED_BYTES,
                "byte_sha256": REFERENCE_DIGEST,
                "backend": backend,
                "parse_count": 1,
                "final_reference_build_count": all_variant_records[-1]["reference_build_count"],
            }
        )
    return {
        "protocol": PROTOCOL,
        "timer_scope": TIMER_SCOPE,
        "measured_records": 14,
        "variants": variants,
    }


def write_runtime_binary(path: Path, content: bytes = b"test rstim reference build worker\n") -> Path:
    path.write_bytes(content)
    path.chmod(0o755)
    return path


def write_valid_bundle(path: Path, *, rstim_worker: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    packed = b"\x00" * PACKED_BYTES
    packed_base64 = base64.b64encode(packed).decode("ascii")
    fixture = REPO_ROOT / FIXTURE_REL
    manifest = REPO_ROOT / MANIFEST_REL
    runner_python = Path(sys.executable).resolve()
    stim_worker_module = REPO_ROOT / STIM_WORKER_REL

    records: list[dict[str, Any]] = []
    for variant, backend, elapsed_base in (
        (STIM_VARIANT, "stim_reference", 1000),
        (RSTIM_VARIANT, "packed_inverse", 2000),
    ):
        for round_index in range(9):
            records.append(
                {
                    "protocol": PROTOCOL,
                    "variant": variant,
                    "phase": "warmup" if round_index < 2 else "measured",
                    "round": round_index,
                    "elapsed_ns": elapsed_base + round_index,
                    "packed_base64": packed_base64,
                    "packed_bytes": PACKED_BYTES,
                    "measurement_bits": MEASUREMENT_BITS,
                    "byte_sha256": REFERENCE_DIGEST,
                    "backend": backend,
                    "timer_scope": TIMER_SCOPE,
                    "parse_count": 1,
                    "reference_build_count": round_index + 1,
                }
            )

    rewrite_raw(path / "raw.jsonl", records)
    summary = derive_summary(records)
    (path / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (path / "report.md").write_text(render_report(summary), encoding="utf-8")
    environment = {
        "profile": "release",
        "protocol": PROTOCOL,
        "timer_scope": TIMER_SCOPE,
        "seed_policy": "deterministic_no_seed_reference_builds",
        "fixture_path": FIXTURE_REL,
        "fixture_sha256": FIXTURE_DIGEST,
        "manifest_path": MANIFEST_REL,
        "manifest_sha256": MANIFEST_DIGEST,
        "stim_version": "1.15.0",
        "worker_argv": {
            STIM_VARIANT: [PYTHON_ROLE, "-m", "benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build", "--protocol", PROTOCOL],
            RSTIM_VARIANT: [RSTIM_WORKER_ROLE, "--protocol", PROTOCOL],
        },
        "canonical_worker_argv": {
            STIM_VARIANT: [PYTHON_ROLE, "-m", "benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build", "--protocol", PROTOCOL],
            RSTIM_VARIANT: [RSTIM_WORKER_ROLE, "--protocol", PROTOCOL],
        },
        "runner_argv": [
            PYTHON_ROLE,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark",
            "--fixture",
            FIXTURE_REL,
            "--manifest",
            MANIFEST_REL,
            "--stim-python",
            PYTHON_ROLE,
            "--rstim-worker",
            RSTIM_WORKER_ROLE,
            "--warmup-rounds",
            "2",
            "--measure-rounds",
            "7",
            "--out-dir",
            str(path),
        ],
        "runtime_identities": [
            {
                "role": PYTHON_ROLE,
                "version": sys.version.split()[0],
                "basename": runner_python.name,
                "sha256": sha256_file(runner_python),
            },
            {
                "role": STIM_WORKER_ROLE,
                "version": "1.15.0",
                "basename": "stim_reference_build.py",
                "sha256": sha256_file(stim_worker_module),
            },
            {
                "role": RSTIM_WORKER_ROLE,
                "version": RSTIM_WORKER_VERSION,
                "basename": "rstim_reference_build_worker",
                "sha256": sha256_file(rstim_worker),
            },
        ],
        "warmup_rounds": 2,
        "measure_rounds": 7,
        "git_commit": command_stdout(["git", "rev-parse", "HEAD"]),
        "git_dirty": False,
        "os": "test-os",
        "cpu_model": "test-cpu",
        "rustc_version": "rustc test",
        "cargo_version": "cargo test",
        "python_version": sys.version.split()[0],
    }
    self_check_manifest = sha256_file(manifest)
    if self_check_manifest != MANIFEST_DIGEST:
        raise AssertionError(f"manifest digest drifted: {self_check_manifest}")
    self_check_fixture = sha256_file(fixture)
    if self_check_fixture != FIXTURE_DIGEST:
        raise AssertionError(f"fixture digest drifted: {self_check_fixture}")
    (path / "environment.json").write_text(
        json.dumps(environment, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    rewrite_artifact_hashes(path)


class CheckReferenceBuildEvidenceTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.bundle = Path(self.temp_dir.name) / "bundle"
        self.rstim_worker = write_runtime_binary(Path(self.temp_dir.name) / "rstim_reference_build_worker")
        write_valid_bundle(self.bundle, rstim_worker=self.rstim_worker)

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def run_checker(self, *extra_args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(CHECKER), "--dir", str(self.bundle), *extra_args],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_accepts_valid_bundle(self) -> None:
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS packed reference-build evidence\n")

    def test_accepts_matching_runtime_binary_when_supplied(self) -> None:
        result = self.run_checker("--verify-runtime-binary", str(self.rstim_worker))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS packed reference-build evidence\n")

    def test_accepts_recorded_git_commit_without_local_object(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["git_commit"] = "f" * 40

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "PASS packed reference-build evidence\n")

    def test_rejects_supplied_runtime_binary_with_different_sha(self) -> None:
        other = write_runtime_binary(Path(self.temp_dir.name) / "different-worker", b"different worker\n")
        result = self.run_checker("--verify-runtime-binary", str(other))
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("runtime binary SHA-256 does not match recorded identity", result.stderr)

    def test_rejects_changed_decoded_byte_before_hash_mismatch(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        packed = bytearray(base64.b64decode(records[0]["packed_base64"]))
        packed[0] = 1
        records[0]["packed_base64"] = base64.b64encode(packed).decode("ascii")
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("decoded packed bytes SHA-256", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_mismatched_digest(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        records[0]["byte_sha256"] = "0" * 64
        rewrite_raw(self.bundle / "raw.jsonl", records)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("byte_sha256", result.stderr)
        self.assertIn(REFERENCE_DIGEST, result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_legacy_rstim_backend(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(record for record in records if record["variant"] == RSTIM_VARIANT)["backend"] = "tableau"
        rewrite_raw(self.bundle / "raw.jsonl", records)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("rstim-packed-reference-b8 backend must be packed_inverse", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_timer_scope_including_parsing(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        records[0]["timer_scope"] = "reference_build_including_parse"
        rewrite_raw(self.bundle / "raw.jsonl", records)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("timer_scope must be reference_build_only", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_parse_count_not_one(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        records[0]["parse_count"] = 2
        rewrite_raw(self.bundle / "raw.jsonl", records)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("parse_count must be integer 1", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_missing_final_reference_build_count_nine(self) -> None:
        records = load_raw(self.bundle / "raw.jsonl")
        next(
            record
            for record in records
            if record["variant"] == RSTIM_VARIANT and record["round"] == 8
        )["reference_build_count"] = 8
        rewrite_raw(self.bundle / "raw.jsonl", records)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("final reference_build_count must be 9", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_summary_variant_stats_not_derived_from_raw(self) -> None:
        def mutate(summary: dict[str, Any]) -> None:
            summary["variants"][0]["median_elapsed_ns"] += 1
            summary["variants"][0]["final_reference_build_count"] = 8

        rewrite_json(self.bundle / "summary.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("summary.json does not match summary derived from raw.jsonl", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_report_not_derived_from_summary(self) -> None:
        report_path = self.bundle / "report.md"
        report = report_path.read_text(encoding="utf-8")
        report_path.write_text(report.replace(f"| {STIM_VARIANT} | 7 |", f"| {STIM_VARIANT} | 6 |", 1), encoding="utf-8")
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("report.md does not match summary.json", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_wrong_runner_argv_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["runner_argv"] = [*environment["runner_argv"], "--unchecked"]

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment runner_argv", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_runner_argv_wrong_out_dir_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["runner_argv"][-1] = str(self.bundle.parent / "unrelated-output")

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment runner_argv --out-dir", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_short_runner_argv_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["runner_argv"] = environment["runner_argv"][:7]

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment runner_argv must match the full canonical runner command", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_noncanonical_fixture_digest_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["fixture_sha256"] = "0" * 64

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment fixture_sha256 must be canonical reference-build fixture SHA-256", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_noncanonical_fixture_path_before_hash_mismatch(self) -> None:
        substitute = self.bundle.parent / "same-fixture-bytes.stim"
        substitute.write_bytes((REPO_ROOT / FIXTURE_REL).read_bytes())

        def mutate(environment: dict[str, Any]) -> None:
            environment["fixture_path"] = str(substitute)

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment fixture_path must be a repo-relative POSIX path", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_absolute_repository_paths_before_artifact_hash_validation(self) -> None:
        cases = (
            ("fixture_path", str(REPO_ROOT / FIXTURE_REL), "environment fixture_path"),
            ("manifest_path", str(REPO_ROOT / MANIFEST_REL), "environment manifest_path"),
            ("runner_fixture", str(REPO_ROOT / FIXTURE_REL), "environment runner_argv fixture"),
            ("runner_manifest", str(REPO_ROOT / MANIFEST_REL), "environment runner_argv manifest"),
        )
        for field, value, label in cases:
            with self.subTest(field=field):
                write_valid_bundle(self.bundle, rstim_worker=self.rstim_worker)

                def mutate(environment: dict[str, Any]) -> None:
                    if field == "runner_fixture":
                        environment["runner_argv"][4] = value
                    elif field == "runner_manifest":
                        environment["runner_argv"][6] = value
                    else:
                        environment[field] = value

                rewrite_json(self.bundle / "environment.json", mutate)
                rewrite_artifact_hashes(self.bundle)
                result = self.run_checker()
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(f"{label} must be a repo-relative POSIX path", result.stderr)
                self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_non_normalized_repository_paths_before_artifact_hash_validation(self) -> None:
        cases = (
            ("fixture_path", "benchmarks/rstim_vs_stim_simulator/fixtures/../fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim", "environment fixture_path"),
            ("manifest_path", "benchmarks/rstim_vs_stim_simulator/cases/../cases.full.toml", "environment manifest_path"),
            ("runner_fixture", "benchmarks/rstim_vs_stim_simulator/fixtures/../fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim", "environment runner_argv fixture"),
            ("runner_manifest", "benchmarks/rstim_vs_stim_simulator/cases/../cases.full.toml", "environment runner_argv manifest"),
        )
        for field, value, label in cases:
            with self.subTest(field=field):
                write_valid_bundle(self.bundle, rstim_worker=self.rstim_worker)

                def mutate(environment: dict[str, Any]) -> None:
                    if field == "runner_fixture":
                        environment["runner_argv"][4] = value
                    elif field == "runner_manifest":
                        environment["runner_argv"][6] = value
                    else:
                        environment[field] = value

                rewrite_json(self.bundle / "environment.json", mutate)
                rewrite_artifact_hashes(self.bundle)
                result = self.run_checker()
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(f"{label} must be a repo-relative POSIX path", result.stderr)
                self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_noncanonical_worker_argv_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["canonical_worker_argv"][RSTIM_VARIANT][0] = "target/debug/rstim_reference_build_worker"

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment canonical_worker_argv", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_bad_runtime_identity_sha_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["runtime_identities"][0]["sha256"] = "not-a-sha"

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment runtime_identities tool://python sha256", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_missing_runtime_identity_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["runtime_identities"] = [
                identity
                for identity in environment["runtime_identities"]
                if identity["role"] != RSTIM_WORKER_ROLE
            ]

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment runtime_identities must contain exactly", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_invalid_git_commit_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            environment["git_commit"] = "not-a-commit"

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment git_commit", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_rehashed_environment_missing_provenance_before_hash_mismatch(self) -> None:
        def mutate(environment: dict[str, Any]) -> None:
            del environment["stim_version"]

        rewrite_json(self.bundle / "environment.json", mutate)
        rewrite_artifact_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment stim_version", result.stderr)
        self.assertNotIn("artifact-sha256.json", result.stderr)

    def test_rejects_missing_hash_manifest(self) -> None:
        (self.bundle / "artifact-sha256.json").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required bundle file: artifact-sha256.json", result.stderr)


if __name__ == "__main__":
    unittest.main()
