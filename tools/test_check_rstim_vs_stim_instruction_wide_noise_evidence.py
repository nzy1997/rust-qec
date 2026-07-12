#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from typing import Any, Callable

REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKER = REPO_ROOT / "tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py"
FIXTURE = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
MANIFEST = REPO_ROOT / "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
PUBLISHED_RSTIM_RUNTIME_IDENTITY = {
    "role": "tool://rstim",
    "version": "rstim 0.1.1",
    "basename": "rstim",
    "sha256": "336ab36864ba884314507d39378628aa653f16f9c51693512da510cbf3982568",
}
PAIRED_ARTIFACTS = (
    "paired-raw.jsonl",
    "paired-summary.json",
    "paired-report.md",
)
REQUIRED_ARTIFACTS = (
    "raw.jsonl",
    "summary.json",
    "report.md",
    *PAIRED_ARTIFACTS,
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


def write_checker_repo_fixture(
    repo_root: Path,
    runtime_identity: dict[str, str],
    mutate_catalog_text: Callable[[str], str] | None = None,
) -> Path:
    tools_dir = repo_root / "tools"
    tools_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(CHECKER, tools_dir / CHECKER.name)

    benchmarks_dir = repo_root / "benchmarks"
    benchmarks_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(REPO_ROOT / "benchmarks/__init__.py", benchmarks_dir / "__init__.py")
    shutil.copytree(
        REPO_ROOT / "benchmarks/rstim_vs_stim_simulator",
        benchmarks_dir / "rstim_vs_stim_simulator",
    )

    catalog_path = benchmarks_dir / "rstim_vs_stim_simulator/evidence_bundles.toml"
    catalog_text = catalog_path.read_text(encoding="utf-8")
    catalog_text = catalog_text.replace(PUBLISHED_RSTIM_RUNTIME_IDENTITY["sha256"], runtime_identity["sha256"])
    if mutate_catalog_text is not None:
        catalog_text = mutate_catalog_text(catalog_text)
    catalog_path.write_text(catalog_text, encoding="utf-8")
    return tools_dir / CHECKER.name


def write_valid_bundle(bundle: Path) -> None:
    from benchmarks.rstim_vs_stim_simulator import run_frame_instruction_wide_benchmark as runner
    from benchmarks.rstim_vs_stim_simulator import run_paired_frame_noise as paired_runner

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
    paired_records = []
    for phase, rounds in (("warmup", 2), ("measured", 7)):
        for round_index in range(rounds):
            seed = round_index if phase == "warmup" else 2 + round_index
            for ordering_slot, (variant, revision_label, resolved_revision) in enumerate(
                (
                    (
                        paired_runner.BASELINE_VARIANT,
                        "baseline",
                        paired_runner.PINNED_BASELINE_REV,
                    ),
                    (
                        paired_runner.CANDIDATE_VARIANT,
                        "candidate",
                        "2" * 40,
                    ),
                )
            ):
                paired_records.append(
                    {
                        "case_id": "stim_surface_d11_r100",
                        "variant": variant,
                        "phase": phase,
                        "round_index": round_index,
                        "ordering_slot": len(paired_records),
                        "seed": seed,
                        "argv": paired_runner._canonical_argv(
                            paired_runner.TOOL_ROLES[variant],
                            fixture=FIXTURE,
                            shots=1024,
                            seed=seed,
                            repo_root=REPO_ROOT,
                        ),
                        "shots": 1024,
                        "measurement_count": 12_121,
                        "output_format": "b8",
                        "expected_output_bytes": 1_552_384,
                        "resolved_revision": resolved_revision,
                        "revision_label": revision_label,
                        "elapsed_ns": 1_000,
                        "timer_scope": "process_spawn_stdout_stderr_drain_exit",
                        "exit_code": 0,
                        "actual_output_bytes": 1_552_384,
                        "stdout_sha256": "d" * 64,
                        "stderr_bytes": 0,
                    }
                )
    (bundle / "paired-raw.jsonl").write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in paired_records),
        encoding="utf-8",
    )
    paired_summary = paired_runner._summary(
        paired_records,
        baseline=paired_runner.RevisionBuild("baseline", paired_runner.PINNED_BASELINE_REV, paired_runner.PINNED_BASELINE_REV, bundle, bundle, bundle / "rstim"),
        candidate=paired_runner.RevisionBuild("candidate", "HEAD", "2" * 40, bundle, bundle, bundle / "rstim"),
    )
    (bundle / "paired-summary.json").write_text(
        json.dumps(paired_summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (bundle / "paired-report.md").write_text(paired_runner._report(paired_summary), encoding="utf-8")
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
        "output_format": "01",
        "expected_output_bytes": (12_000 + 1 + 1) * 1024,
        "stim_output_bytes": (12_000 + 1 + 1) * 1024,
        "rstim_output_bytes": (12_000 + 1 + 1) * 1024,
        "stim_stdout_sha256": "b" * 64,
        "rstim_stdout_sha256": "c" * 64,
        "sample_count": 1024,
        "selected_columns": [0, 12000],
        "selected_pairs": [[0, 12000]],
        "max_delta": 0.0,
        "max_tolerance": 0.01,
        "failure_reasons": [],
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
        "rstim_version": PUBLISHED_RSTIM_RUNTIME_IDENTITY["version"],
        "rustc_version": "rustc test",
        "os": "test-os",
        "cpu_model": "test-cpu",
        "fixture": "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
        "fixture_sha256": sha256_file(FIXTURE),
        "manifest": "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
        "manifest_sha256": sha256_file(MANIFEST),
        "runtime_identities": [
            PUBLISHED_RSTIM_RUNTIME_IDENTITY,
        ],
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

    def run_checker(
        self,
        *extra_args: str,
        checker_path: Path = CHECKER,
        cwd: Path = REPO_ROOT,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(checker_path), "--dir", str(self.bundle), *extra_args],
            cwd=cwd,
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
            "PASS instruction-wide frame-noise evidence outcome=neutral builds=803 "
            "legacy_setups=80362 candidate_over_baseline=1.0 attempts=82290688\n",
        )

    def test_default_validation_does_not_require_live_runtime_binary(self) -> None:
        (self.bundle / "rstim").unlink()

        result = self.run_checker()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout,
            "PASS instruction-wide frame-noise evidence outcome=neutral builds=803 "
            "legacy_setups=80362 candidate_over_baseline=1.0 attempts=82290688\n",
        )

    def test_verify_runtime_binary_accepts_matching_supplied_binary(self) -> None:
        identity = dict(PUBLISHED_RSTIM_RUNTIME_IDENTITY)
        identity["sha256"] = sha256_file(self.bundle / "rstim")
        rewrite_json(
            self.bundle / "environment.json",
            lambda payload: payload.update({"runtime_identities": [identity]}),
        )
        rewrite_hashes(self.bundle)
        repo_root = Path(self.tmpdir.name) / "checker-repo"
        checker_path = write_checker_repo_fixture(repo_root, identity)

        result = self.run_checker(
            "--verify-runtime-binary",
            str(self.bundle / "rstim"),
            checker_path=checker_path,
            cwd=repo_root,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("PASS instruction-wide frame-noise evidence", result.stdout)

    def test_verify_runtime_binary_rejects_different_supplied_binary(self) -> None:
        other_binary = self.bundle / "other-rstim"
        other_binary.write_bytes(b"different runtime binary\n")

        result = self.run_checker("--verify-runtime-binary", str(other_binary))

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("runtime binary SHA-256 does not match recorded identity", result.stderr)

    def test_rejects_catalog_schema_other_than_v2(self) -> None:
        repo_root = Path(self.tmpdir.name) / "checker-repo"
        checker_path = write_checker_repo_fixture(
            repo_root,
            PUBLISHED_RSTIM_RUNTIME_IDENTITY,
            lambda text: text.replace("schema = 2", "schema = 1", 1),
        )

        result = self.run_checker(checker_path=checker_path, cwd=repo_root)

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("evidence catalog schema must be 2", result.stderr)

    def test_rejects_duplicate_frame_catalog_bundle(self) -> None:
        repo_root = Path(self.tmpdir.name) / "checker-repo"
        checker_path = write_checker_repo_fixture(
            repo_root,
            PUBLISHED_RSTIM_RUNTIME_IDENTITY,
            lambda text: text + '\n[[bundles]]\nid = "frame-instruction-wide-release"\n',
        )

        result = self.run_checker(checker_path=checker_path, cwd=repo_root)

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn('evidence catalog must contain exactly one bundle "frame-instruction-wide-release"', result.stderr)

    def test_rejects_malformed_or_extra_catalog_runtime_identity(self) -> None:
        for mutation in (
            lambda text: text + (
                '\n[[bundles.runtime_identities]]\n'
                'role = "tool://extra"\n'
                'version = "extra 1.0"\n'
                'basename = "extra"\n'
                'sha256 = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"\n'
                'unexpected = "field"\n'
            ),
            lambda text: text + (
                '\n[[bundles.runtime_identities]]\n'
                'role = "tool://extra"\n'
                'version = "extra 1.0"\n'
                'basename = "extra"\n'
                'sha256 = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"\n'
            ),
        ):
            with self.subTest(mutation=mutation):
                repo_root = Path(self.tmpdir.name) / f"checker-repo-{id(mutation)}"
                checker_path = write_checker_repo_fixture(
                    repo_root,
                    PUBLISHED_RSTIM_RUNTIME_IDENTITY,
                    mutation,
                )

                result = self.run_checker(checker_path=checker_path, cwd=repo_root)

                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(
                    'evidence catalog bundle "frame-instruction-wide-release" must contain exactly one runtime identity',
                    result.stderr,
                )

    def test_rejects_rstim_version_disagreeing_with_runtime_identity(self) -> None:
        rewrite_json(
            self.bundle / "environment.json",
            lambda payload: payload.update({"rstim_version": "rstim 0.1.1-other"}),
        )
        rewrite_hashes(self.bundle)

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment rstim_version must match runtime identity version", result.stderr)

    def test_rejects_malformed_or_extra_environment_runtime_identity(self) -> None:
        for identities, message in (
            (
                [
                    PUBLISHED_RSTIM_RUNTIME_IDENTITY,
                    {
                        "role": "tool://extra",
                        "version": "extra 1.0",
                        "basename": "extra",
                        "sha256": "d" * 64,
                    },
                ],
                "environment runtime_identities must contain exactly one tool://rstim identity",
            ),
            (
                ["not an identity"],
                "environment runtime_identities[0] must be an object",
            ),
            (
                [{"role": "tool://rstim", "version": "rstim 0.1.1", "basename": "rstim"}],
                "environment runtime_identities[0] missing required field(s): sha256",
            ),
            (
                [dict(PUBLISHED_RSTIM_RUNTIME_IDENTITY, unexpected="field")],
                "environment runtime_identities[0] unsupported field(s): unexpected",
            ),
            (
                [dict(PUBLISHED_RSTIM_RUNTIME_IDENTITY, role="tool://extra")],
                "environment runtime_identities[0] role must be tool://rstim",
            ),
        ):
            with self.subTest(message=message):
                write_valid_bundle(self.bundle)
                rewrite_json(
                    self.bundle / "environment.json",
                    lambda payload, identities=identities: payload.update({"runtime_identities": identities}),
                )
                rewrite_hashes(self.bundle)

                result = self.run_checker()

                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(message, result.stderr)

    def test_rejects_legacy_runtime_binary_path_fields_without_hashing_them(self) -> None:
        rewrite_json(
            self.bundle / "environment.json",
            lambda payload: (
                payload.pop("runtime_identities"),
                payload.update(
                    {
                        "rstim_binary": str(self.bundle / "missing-rstim"),
                        "rstim_binary_sha256": "0" * 64,
                    }
                ),
            ),
        )
        rewrite_hashes(self.bundle)

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment runtime_identities must contain exactly one tool://rstim identity", result.stderr)
        self.assertNotIn("does not exist", result.stderr)

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
            ("status", "failed", "correctness-summary status must be pass"),
            ("mode", "sample", "correctness-summary mode must be detect"),
        ):
            with self.subTest(field=field):
                write_valid_bundle(self.bundle)
                rewrite_json(self.bundle / "correctness-summary.json", lambda payload: payload.update({field: value}))
                result = self.run_checker()
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(message, result.stderr)
                self.assertNotIn("artifact", result.stderr.lower())

    def test_rejects_mismatched_correctness_output_bytes(self) -> None:
        for field in ("stim_output_bytes", "rstim_output_bytes"):
            with self.subTest(field=field):
                write_valid_bundle(self.bundle)
                rewrite_json(self.bundle / "correctness-summary.json", lambda payload: payload.update({field: 1}))
                rewrite_hashes(self.bundle)
                result = self.run_checker()
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(f"correctness-summary {field} must be 12290048", result.stderr)

    def test_rejects_inconsistent_raw_measurement_metadata_before_hash_error(self) -> None:
        records = [json.loads(line) for line in (self.bundle / "raw.jsonl").read_text().splitlines()]
        records[1]["stdout_sha256"] = "d" * 64
        (self.bundle / "raw.jsonl").write_text(
            "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
            encoding="utf-8",
        )
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("raw measurement field stdout_sha256 must be identical", result.stderr)
        self.assertNotIn("artifact", result.stderr.lower())

    def test_rejects_paired_classification_mismatch(self) -> None:
        rewrite_json(self.bundle / "paired-summary.json", lambda payload: payload.update({"outcome": "improved"}))
        rewrite_hashes(self.bundle)

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("paired-summary outcome must be neutral", result.stderr)

    def test_rejects_paired_candidate_regression_limit_before_hash_error(self) -> None:
        rewrite_json(
            self.bundle / "paired-summary.json",
            lambda payload: (
                payload["variants"][1].update({"median_elapsed_ns": 1100, "mean_elapsed_ns": 1100}),
                payload.update({"candidate_over_baseline": 1.1, "outcome": "regressed"}),
            ),
        )

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("candidate frame-noise path exceeds 1.05 non-regression limit", result.stderr)
        self.assertNotIn("artifact", result.stderr.lower())

    def test_accepts_candidate_first_paired_ordering_with_baseline_revision_pinned(self) -> None:
        records = [json.loads(line) for line in (self.bundle / "paired-raw.jsonl").read_text().splitlines()]
        candidate_first_records = []
        for index in range(0, len(records), 2):
            candidate_first_records.extend((records[index + 1], records[index]))
        for ordering_slot, record in enumerate(candidate_first_records):
            record["ordering_slot"] = ordering_slot
        (self.bundle / "paired-raw.jsonl").write_text(
            "".join(json.dumps(record, sort_keys=True) + "\n" for record in candidate_first_records),
            encoding="utf-8",
        )
        rewrite_json(
            self.bundle / "environment.json",
            lambda payload: payload["artifact_sha256"].update(
                {"paired-raw.jsonl": sha256_file(self.bundle / "paired-raw.jsonl")}
            ),
        )
        rewrite_hashes(self.bundle)

        result = self.run_checker()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("candidate_over_baseline=1.0", result.stdout)

    def test_rejects_mismatched_fixture_manifest_or_artifact_hash(self) -> None:
        for field, message in (
            ("fixture_sha256", "environment fixture_sha256 does not match fixture"),
            ("manifest_sha256", "environment manifest_sha256 does not match manifest"),
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

    def test_rejects_dirty_published_provenance(self) -> None:
        rewrite_json(self.bundle / "environment.json", lambda payload: payload.update({"git_dirty": True}))
        rewrite_hashes(self.bundle)
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("environment git_dirty must be false for published release evidence", result.stderr)

    def test_rejects_missing_hash_manifest(self) -> None:
        (self.bundle / "artifact-sha256.json").unlink()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("missing required bundle file: artifact-sha256.json", result.stderr)


if __name__ == "__main__":
    unittest.main()
