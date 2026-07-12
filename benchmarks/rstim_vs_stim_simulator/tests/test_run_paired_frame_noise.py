from __future__ import annotations

import argparse
import dataclasses
import hashlib
import io
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator import run_paired_frame_noise


ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
FIXTURE_REPO_PATH = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
BASELINE_REV = "f10d1ed024d3519318ed244c9095724074519595"
BASELINE_COMMIT = "0" * 40
CANDIDATE_COMMIT = "1" * 40
EXPECTED_BYTES = 1_552_384
EXPECTED_SHA256 = hashlib.sha256(bytes(range(256)) * 6064).hexdigest()


def write_fake_rstim(path: Path, *, mode: str = "success") -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        textwrap.dedent(
            f"""\
            #!{sys.executable}
            import os
            import sys
            import time

            MODE = {mode!r}
            EXPECTED_BYTES = {EXPECTED_BYTES}

            if not sys.argv[1:]:
                print("rstim 0.1.1-test")
                sys.exit(0)

            log_path = os.environ.get("PAIRED_FAKE_INVOCATIONS")
            if log_path:
                with open(log_path, "a", encoding="utf-8") as log:
                    log.write("CLI\\t" + sys.argv[0] + "\\t" + " ".join(sys.argv[1:]) + "\\n")

            argv = sys.argv[1:]
            if argv[0] != "sample":
                print("expected sample", file=sys.stderr)
                sys.exit(2)
            for flag in ("--skip_reference_sample", "--shots", "--seed", "--out_format", "--in"):
                if flag not in argv:
                    print(f"missing {{flag}}", file=sys.stderr)
                    sys.exit(2)
            if argv[argv.index("--out_format") + 1] != "b8":
                print("expected b8", file=sys.stderr)
                sys.exit(2)
            if argv[argv.index("--shots") + 1] != "1024":
                print("expected 1024 shots", file=sys.stderr)
                sys.exit(2)

            payload = bytes(range(256)) * (EXPECTED_BYTES // 256)
            if MODE == "short-output":
                sys.stdout.buffer.write(payload[:-1])
                sys.exit(0)
            if MODE == "delayed":
                sys.stdout.buffer.write(payload[:-1])
                sys.stdout.buffer.flush()
                time.sleep(0.15)
                sys.stdout.buffer.write(payload[-1:])
                sys.stdout.buffer.flush()
                sys.stdout.close()
                time.sleep(0.15)
                sys.stderr.buffer.write(b"drained stderr\\n" * 1024)
                sys.stderr.buffer.flush()
                sys.exit(0)

            sys.stdout.buffer.write(payload)
            sys.exit(0)
            """
        ),
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path


def fake_builds(root: Path) -> dict[str, run_paired_frame_noise.RevisionBuild]:
    return {
        "baseline": run_paired_frame_noise.RevisionBuild(
            label="baseline",
            requested_rev=BASELINE_REV,
            resolved_commit=BASELINE_COMMIT,
            source_dir=root / "baseline-src",
            target_dir=root / "baseline-target",
            binary_path=write_fake_rstim(root / "baseline-target/release/rstim"),
        ),
        "candidate": run_paired_frame_noise.RevisionBuild(
            label="candidate",
            requested_rev="HEAD",
            resolved_commit=CANDIDATE_COMMIT,
            source_dir=root / "candidate-src",
            target_dir=root / "candidate-target",
            binary_path=write_fake_rstim(root / "candidate-target/release/rstim"),
        ),
    }


def run_with_fake_builds(
    out_dir: Path, builds: dict[str, run_paired_frame_noise.RevisionBuild]
) -> dict[str, object]:
    def materialize(
        revision: str, *, repo_root: Path, temp_root: Path, label: str
    ) -> run_paired_frame_noise.RevisionBuild:
        return builds[label]

    def build(revision: run_paired_frame_noise.RevisionBuild) -> Path:
        return revision.binary_path

    with (
        mock.patch(
            "benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.materialize_revision",
            side_effect=materialize,
        ),
        mock.patch(
            "benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.build_revision",
            side_effect=build,
        ),
    ):
        return run_paired_frame_noise.run_paired_frame_noise(
            argparse.Namespace(
                baseline_rev=BASELINE_REV,
                candidate_rev="HEAD",
                fixture=FIXTURE,
                shots=1024,
                warmup_rounds=2,
                measure_rounds=7,
                out_dir=out_dir,
            ),
            repo_root=ROOT,
        )


class RunPairedFrameNoiseTest(unittest.TestCase):
    def test_non_pinned_baseline_revision_rejected_before_materialization(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            materialize = mock.Mock()
            args = argparse.Namespace(
                baseline_rev="HEAD~1",
                candidate_rev="HEAD",
                fixture=FIXTURE,
                shots=1024,
                warmup_rounds=2,
                measure_rounds=7,
                out_dir=Path(temp_dir) / "out",
            )
            with mock.patch(
                "benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.materialize_revision",
                materialize,
            ):
                with self.assertRaisesRegex(ValueError, f"pinned.*{BASELINE_REV}"):
                    run_paired_frame_noise.run_paired_frame_noise(args, repo_root=ROOT)

            materialize.assert_not_called()

    def test_same_revision_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "baseline and candidate revisions must differ"):
            run_paired_frame_noise.ensure_distinct_revisions("a" * 40, "a" * 40)

    def test_canonical_command_requires_skip_reference_sample(self) -> None:
        argv = [
            "tool://rstim-baseline",
            "sample",
            "--shots",
            "1024",
            "--seed",
            "0",
            "--out_format",
            "b8",
            "--in",
            FIXTURE_REPO_PATH,
        ]
        with self.assertRaisesRegex(ValueError, "--skip_reference_sample"):
            run_paired_frame_noise.validate_canonical_command(
                argv,
                variant="baseline-rstim-frame-noise-b8",
                fixture=Path(FIXTURE_REPO_PATH),
                shots=1024,
                seed=0,
            )

    def test_time_cli_includes_complete_stderr_drain_and_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            cli = write_fake_rstim(root / "rstim", mode="delayed")
            result = run_paired_frame_noise.time_cli(
                [
                    str(cli), "sample", "--skip_reference_sample", "--shots", "1024",
                    "--seed", "0", "--out_format", "b8", "--in", str(FIXTURE),
                ],
                cwd=ROOT,
            )
            self.assertEqual(result.exit_code, 0)
            self.assertEqual(len(result.stdout), EXPECTED_BYTES)
            self.assertIn(b"drained stderr", result.stderr)
            self.assertGreaterEqual(result.elapsed_ns, 300_000_000)

    def test_runner_writes_paired_artifacts_and_alternates_order(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            out_dir = root / "out"
            invocations = root / "invocations.txt"
            builds = fake_builds(root)
            with mock.patch.dict(os.environ, {"PAIRED_FAKE_INVOCATIONS": str(invocations)}):
                summary = run_with_fake_builds(out_dir, builds)

            self.assertEqual(summary["measured_record_count"], 14)
            self.assertEqual({path.name for path in out_dir.iterdir()}, {
                "raw.jsonl", "summary.json", "report.md", "environment.json", "artifact-sha256.json",
            })
            records = [json.loads(line) for line in (out_dir / "raw.jsonl").read_text(encoding="utf-8").splitlines()]
            self.assertEqual(len(records), 18)
            self.assertEqual({record["variant"] for record in records}, {
                "baseline-rstim-frame-noise-b8", "candidate-rstim-frame-noise-b8",
            })
            measured = [record for record in records if record["phase"] == "measured"]
            self.assertEqual(len(measured), 14)
            for phase in ("warmup", "measured"):
                phase_records = [record for record in records if record["phase"] == phase]
                for round_index in sorted({record["round_index"] for record in phase_records}):
                    pair = [record for record in phase_records if record["round_index"] == round_index]
                    expected = ["baseline-rstim-frame-noise-b8", "candidate-rstim-frame-noise-b8"]
                    if round_index % 2 == 1:
                        expected.reverse()
                    self.assertEqual([record["variant"] for record in pair], expected)
                    self.assertEqual(len({record["seed"] for record in pair}), 1)
            for record in records:
                self.assertEqual(record["actual_output_bytes"], EXPECTED_BYTES)
                self.assertEqual(record["stdout_sha256"], EXPECTED_SHA256)
                self.assertIn("--skip_reference_sample", record["argv"])
                self.assertEqual(record["argv"][record["argv"].index("--out_format") + 1], "b8")
                self.assertEqual(record["argv"][record["argv"].index("--in") + 1], FIXTURE_REPO_PATH)

            environment = json.loads((out_dir / "environment.json").read_text(encoding="utf-8"))
            self.assertEqual(environment["baseline_revision"]["resolved_commit"], BASELINE_COMMIT)
            self.assertEqual(environment["candidate_revision"]["resolved_commit"], CANDIDATE_COMMIT)
            self.assertEqual(environment["fixture_path"], FIXTURE_REPO_PATH)
            self.assertEqual(environment["expected_output_bytes"], EXPECTED_BYTES)
            self.assertNotIn(str(root), json.dumps(records))
            self.assertNotIn(str(root), json.dumps(summary))

            hashes = json.loads((out_dir / "artifact-sha256.json").read_text(encoding="utf-8"))
            self.assertEqual(set(hashes), {"raw.jsonl", "summary.json", "report.md", "environment.json"})
            for filename, digest in hashes.items():
                self.assertEqual(digest, hashlib.sha256((out_dir / filename).read_bytes()).hexdigest())

    def test_short_candidate_output_fails_before_summary_generation(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            out_dir = root / "out"
            builds = fake_builds(root)
            builds["candidate"] = dataclasses.replace(
                builds["candidate"],
                binary_path=write_fake_rstim(root / "candidate-target/release/rstim", mode="short-output"),
            )
            with self.assertRaisesRegex(RuntimeError, "1552384|output bytes"):
                run_with_fake_builds(out_dir, builds)
            self.assertFalse((out_dir / "summary.json").exists())

    def test_materialize_revision_uses_git_archive_without_checkout(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            calls: list[list[str]] = []

            def fake_run(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                calls.append(argv)
                if argv[:2] == ["git", "rev-parse"]:
                    return subprocess.CompletedProcess(argv, 0, stdout=(CANDIDATE_COMMIT + "\n").encode(), stderr=b"")
                if argv[:2] == ["git", "archive"]:
                    archive = root / "archive.tar"
                    source_file = root / "Cargo.toml"
                    source_file.write_text("[workspace]\n", encoding="utf-8")
                    with tarfile.open(archive, "w") as tar:
                        tar.add(source_file, arcname="Cargo.toml")
                    return subprocess.CompletedProcess(argv, 0, stdout=archive.read_bytes(), stderr=b"")
                raise AssertionError(argv)

            with mock.patch("benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.subprocess.run", side_effect=fake_run):
                build = run_paired_frame_noise.materialize_revision("HEAD", repo_root=ROOT, temp_root=root, label="candidate")

            self.assertEqual(build.resolved_commit, CANDIDATE_COMMIT)
            self.assertTrue((build.source_dir / "Cargo.toml").is_file())
            self.assertTrue(any(call[:2] == ["git", "archive"] for call in calls))
            self.assertFalse(any(item == "checkout" for call in calls for item in call))

    def test_main_prints_required_success_line(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            out_dir = root / "out"
            builds = fake_builds(root)
            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.materialize_revision",
                    side_effect=lambda revision, *, repo_root, temp_root, label: builds[label],
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_paired_frame_noise.build_revision",
                    side_effect=lambda revision: revision.binary_path,
                ),
                mock.patch("sys.stdout", new_callable=io.StringIO) as stdout,
            ):
                code = run_paired_frame_noise.main([
                    "--baseline-rev", BASELINE_REV, "--candidate-rev", "HEAD", "--fixture", str(FIXTURE),
                    "--shots", "1024", "--warmup-rounds", "2", "--measure-rounds", "7", "--out-dir", str(out_dir),
                ])
            self.assertEqual(code, 0)
            self.assertEqual(stdout.getvalue().strip(), "PASS paired frame-noise benchmark variants=2 measured=14 bytes=1552384")
