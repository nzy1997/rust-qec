from __future__ import annotations

import argparse
import hashlib
import json
import os
import statistics
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator import run_fair_cli


ROOT = Path(__file__).resolve().parents[3]
FAIR_MANIFEST = ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "fair_cli_cases.toml"
SOURCE_MANIFEST = ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "cases.full.toml"
FIXTURE = (
    ROOT
    / "benchmarks"
    / "rstim_vs_stim_simulator"
    / "fixtures"
    / "stim_surface_code_rotated_memory_z_d11_r100.stim"
)
CASE_ID = "stim_surface_d11_r100"
SHOTS = 1024
MEASUREMENT_COUNT = 12121
OUTPUT_FORMAT = "b8"
TIMER_SCOPE = "cli_end_to_end"
EXPECTED_OUTPUT_BYTES = 1_552_384
EXPECTED_OUTPUT_SHA256 = hashlib.sha256(bytes(range(256)) * 6064).hexdigest()
RAW_RECORD_KEYS = {
    "case_id",
    "variant",
    "phase",
    "round_index",
    "seed",
    "argv",
    "shots",
    "measurement_count",
    "output_format",
    "timer_scope",
    "elapsed_ns",
    "actual_output_bytes",
    "stdout_sha256",
    "exit_code",
}


def write_fake_cli(path: Path, *, mode: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        textwrap.dedent(
            f"""\
            #!{os.sys.executable}
            import os
            import sys
            import time

            MODE = {mode!r}

            def fail(message):
                print(message, file=sys.stderr)
                sys.exit(2)

            def expected_output_bytes():
                return bytes(range(256)) * 6064

            def record_invocation():
                log_path = os.environ.get("FAKE_CLI_INVOCATIONS")
                if log_path:
                    with open(log_path, "a", encoding="utf-8") as log:
                        log.write("CLI\\t" + sys.argv[0] + "\\t" + " ".join(sys.argv[1:]) + "\\n")

            if sys.argv[1:] == ["--version"]:
                print("stim 1.14.0" if MODE == "bad-stim-version" else "stim 1.15.0")
                sys.exit(0)
            if not sys.argv[1:]:
                print("rstim 0.0.0-test")
                sys.exit(0)

            record_invocation()
            argv = sys.argv[1:]
            if not argv or argv[0] != "sample":
                fail("expected sample command")
            for flag in ("--shots", "--seed", "--out_format", "--in"):
                if flag not in argv:
                    fail(f"missing {{flag}}")
            if argv[argv.index("--out_format") + 1] != "b8":
                fail("expected --out_format b8")

            shots = argv[argv.index("--shots") + 1]
            input_path = argv[argv.index("--in") + 1]
            input_text = open(input_path, encoding="utf-8").read()
            if input_text == "X 0\\nM 0\\n":
                if shots != "1":
                    fail("preflight must use --shots 1")
                if MODE == "malformed-preflight":
                    sys.stdout.buffer.write(b"\\x01\\x00")
                elif MODE == "zero-preflight":
                    sys.stdout.buffer.write(b"\\x00")
                else:
                    sys.stdout.buffer.write(b"\\x01")
                sys.exit(0)
            if shots != "1024":
                fail("benchmark must use --shots 1024")

            payload = expected_output_bytes()
            if MODE == "fail-benchmark":
                sys.stderr.write("benchmark failed\\n")
                sys.exit(7)
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
                sys.stderr.buffer.write(b"delayed stderr\\n" * 1024)
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


def read_jsonl(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def read_invocations(path: Path) -> list[list[str]]:
    invocations: list[list[str]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("CLI\t"):
            continue
        _, executable, argv = line.split("\t", 2)
        invocations.append([executable, *argv.split(" ")])
    return invocations


def make_args(out_dir: Path, *, warmup_rounds: int = 2, measure_rounds: int = 7) -> argparse.Namespace:
    return argparse.Namespace(
        manifest=FAIR_MANIFEST,
        case=CASE_ID,
        profile="release",
        warmup_rounds=warmup_rounds,
        measure_rounds=measure_rounds,
        out_dir=out_dir,
    )


def expected_argv(binary: Path, seed: int) -> list[str]:
    return [
        str(binary),
        "sample",
        "--shots",
        str(SHOTS),
        "--seed",
        str(seed),
        "--out_format",
        OUTPUT_FORMAT,
        "--in",
        str(FIXTURE),
    ]


def write_time_cli_probe(path: Path) -> tuple[Path, bytes]:
    stderr_payload = b"stderr-drain\n" * 1024
    path.write_text(
        textwrap.dedent(
            f"""\
            #!{os.sys.executable}
            import sys
            import time

            sys.stdout.buffer.write(b"ok")
            sys.stdout.buffer.flush()
            sys.stdout.close()
            time.sleep(0.15)
            sys.stderr.buffer.write({stderr_payload!r})
            sys.stderr.buffer.flush()
            time.sleep(0.15)
            """
        ),
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path, stderr_payload


class RunFairCliTest(unittest.TestCase):
    def assert_artifacts(self, out_dir: Path, stim: Path, rstim: Path) -> list[dict[str, object]]:
        records = read_jsonl(out_dir / "raw.jsonl")
        self.assertEqual(len(records), 18)
        for record in records:
            self.assertTrue(RAW_RECORD_KEYS <= record.keys(), record)
            self.assertEqual(record["case_id"], CASE_ID)
            self.assertEqual(record["shots"], SHOTS)
            self.assertEqual(record["measurement_count"], MEASUREMENT_COUNT)
            self.assertEqual(record["output_format"], OUTPUT_FORMAT)
            self.assertEqual(record["timer_scope"], TIMER_SCOPE)
            self.assertEqual(record["exit_code"], 0)
            self.assertEqual(record["actual_output_bytes"], EXPECTED_OUTPUT_BYTES)
            self.assertEqual(record["stdout_sha256"], EXPECTED_OUTPUT_SHA256)

        self.assertEqual({record["variant"] for record in records}, {"stim-cli-b8", "rstim-cli-b8"})
        self.assertEqual({record["phase"] for record in records}, {"warmup", "measured"})
        for variant, binary in (("stim-cli-b8", stim), ("rstim-cli-b8", rstim)):
            variant_records = [record for record in records if record["variant"] == variant]
            self.assertEqual([record["seed"] for record in variant_records], list(range(9)))
            self.assertEqual(
                [(record["phase"], record["round_index"]) for record in variant_records],
                [("warmup", 0), ("warmup", 1)] + [("measured", index) for index in range(7)],
            )
            for record in variant_records:
                self.assertEqual(record["argv"], expected_argv(binary, record["seed"]))

        summary = json.loads((out_dir / "summary.json").read_text(encoding="utf-8"))
        measured = [record for record in records if record["phase"] == "measured"]
        self.assertEqual(len(measured), 14)
        for variant in ("stim-cli-b8", "rstim-cli-b8"):
            samples = [record["elapsed_ns"] for record in measured if record["variant"] == variant]
            summary_variant = next(item for item in summary["variants"] if item["variant"] == variant)
            self.assertEqual(summary_variant["sample_count"], 7)
            self.assertEqual(summary_variant["elapsed_ns"]["median"], statistics.median(samples))

        report_text = (out_dir / "report.md").read_text(encoding="utf-8")
        self.assertIn(CASE_ID, report_text)
        for summary_variant in summary["variants"]:
            self.assertIn(summary_variant["variant"], report_text)
            self.assertIn(str(summary_variant["sample_count"]), report_text)
            self.assertIn(str(summary_variant["elapsed_ns"]["median"]), report_text)
        self.assertNotRegex(report_text, r"(?i)warmup.*(?:sample[_ ]?count|samples?).*\\b2\\b")

        environment = json.loads((out_dir / "environment.json").read_text(encoding="utf-8"))
        self.assertTrue(environment["git_commit"])
        self.assertTrue(environment["os"])
        self.assertTrue(environment["cpu_model"])
        self.assertEqual(environment["stim_version"], "1.15.0")
        self.assertEqual(environment["rstim_version"], "rstim 0.0.0-test")
        self.assertTrue(environment["rustc_version"])
        self.assertEqual(environment["manifest"], str(FAIR_MANIFEST))
        self.assertEqual(environment["manifest_sha256"], hashlib.sha256(FAIR_MANIFEST.read_bytes()).hexdigest())
        self.assertEqual(environment["source_manifest"], "benchmarks/rstim_vs_stim_simulator/cases.full.toml")
        self.assertEqual(
            environment["source_manifest_sha256"], hashlib.sha256(SOURCE_MANIFEST.read_bytes()).hexdigest()
        )
        self.assertEqual(
            environment["fixture"],
            "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
        )
        self.assertEqual(environment["fixture_sha256"], hashlib.sha256(FIXTURE.read_bytes()).hexdigest())
        self.assertEqual(environment["stim_binary"], str(stim))
        self.assertEqual(environment["stim_binary_sha256"], hashlib.sha256(stim.read_bytes()).hexdigest())
        self.assertEqual(environment["rstim_binary"], str(rstim))
        self.assertEqual(environment["rstim_binary_sha256"], hashlib.sha256(rstim.read_bytes()).hexdigest())
        self.assertEqual(environment["warmup_rounds"], 2)
        self.assertEqual(environment["measure_rounds"], 7)
        self.assertEqual(environment["profile"], "release")
        self.assertEqual(environment["timer_scope"], TIMER_SCOPE)
        self.assertEqual(environment["seed_policy"], "round_index_0_through_8")
        self.assertEqual(environment["known_answer_preflight"], "passed")

        expected_round_argv = [
            {
                "variant": record["variant"],
                "phase": record["phase"],
                "round_index": record["round_index"],
                "seed": record["seed"],
                "argv": record["argv"],
            }
            for record in records
        ]
        self.assertEqual(environment["round_argv"], expected_round_argv)
        return records

    def test_main_writes_symmetric_artifacts_for_all_rounds(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            stim = write_fake_cli(fake_bin / "stim", mode="success")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
            out_dir = root / "out"
            invocations = root / "invocations.txt"
            with (
                mock.patch.dict(
                    os.environ,
                    {
                        "PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}",
                        "FAKE_CLI_INVOCATIONS": str(invocations),
                    },
                ),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim) as build,
            ):
                result = run_fair_cli.main(
                    [
                        "--manifest", str(FAIR_MANIFEST), "--case", CASE_ID, "--profile", "release",
                        "--warmup-rounds", "2", "--measure-rounds", "7", "--out-dir", str(out_dir),
                    ]
                )
            self.assertEqual(result, 0)
            self.assertTrue((out_dir / "raw.jsonl").is_file())
            self.assertTrue((out_dir / "summary.json").is_file())
            self.assertTrue((out_dir / "report.md").is_file())
            self.assertTrue((out_dir / "environment.json").is_file())
            build.assert_called_once()
            self.assert_artifacts(out_dir, stim, rstim)
            logged_invocations = read_invocations(invocations)
            self.assertEqual(len(logged_invocations), 20)
            self.assertEqual({Path(call[0]).resolve() for call in logged_invocations[:2]}, {stim.resolve(), rstim.resolve()})
            self.assertTrue(all("--shots" in call and call[call.index("--shots") + 1] == "1" for call in logged_invocations[:2]))
            benchmark_invocations = [call for call in logged_invocations if str(FIXTURE) in call]
            self.assertEqual(len(benchmark_invocations), 18)
            for binary in (stim, rstim):
                binary_invocations = [
                    call for call in benchmark_invocations if Path(call[0]).resolve() == binary.resolve()
                ]
                self.assertEqual(binary_invocations, [expected_argv(binary, seed) for seed in range(9)])

    def test_build_finishes_before_timed_processes_and_is_not_sample_elapsed(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            stim = write_fake_cli(fake_bin / "stim", mode="success")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
            out_dir = root / "out"
            invocations = root / "invocations.txt"

            def fake_build(profile: str, *, repo_root: Path) -> Path:
                with invocations.open("a", encoding="utf-8") as log:
                    log.write("BUILD\tstart\n")
                self.assertEqual(profile, "release")
                self.assertEqual(repo_root, ROOT)
                import time

                time.sleep(0.8)
                with invocations.open("a", encoding="utf-8") as log:
                    log.write("BUILD\tend\n")
                return rstim

            with (
                mock.patch.dict(
                    os.environ,
                    {
                        "PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}",
                        "FAKE_CLI_INVOCATIONS": str(invocations),
                    },
                ),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", side_effect=fake_build),
            ):
                run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            log_lines = invocations.read_text(encoding="utf-8").splitlines()
            self.assertEqual(log_lines[:2], ["BUILD\tstart", "BUILD\tend"])
            self.assertEqual({Path(call[0]).resolve() for call in read_invocations(invocations)[:2]}, {stim.resolve(), rstim.resolve()})
            records = read_jsonl(out_dir / "raw.jsonl")
            self.assertTrue(all(record["elapsed_ns"] < 600_000_000 for record in records), records)

    def test_timing_includes_stdout_completion_and_process_exit_for_both_variants(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="delayed")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="delayed")
            out_dir = root / "out"
            with (
                mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            records = read_jsonl(out_dir / "raw.jsonl")
            for variant in ("stim-cli-b8", "rstim-cli-b8"):
                measured = next(record for record in records if record["variant"] == variant and record["phase"] == "measured")
                self.assertGreaterEqual(measured["elapsed_ns"], 300_000_000)

    def test_slow_rstim_run_is_not_rejected_by_speed_ratio_gate(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="success")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="delayed")
            out_dir = root / "out"
            with (
                mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            records = read_jsonl(out_dir / "raw.jsonl")
            self.assertEqual(len(records), 2)
            self.assertEqual({record["variant"] for record in records}, {"stim-cli-b8", "rstim-cli-b8"})

    def test_time_cli_captures_complete_stderr_and_process_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            probe, stderr_payload = write_time_cli_probe(root / "stderr_probe.py")

            result = run_fair_cli.time_cli([str(probe)], cwd=root)

            self.assertEqual(result.exit_code, 0)
            self.assertEqual(result.stdout, b"ok")
            self.assertEqual(result.stderr, stderr_payload)
            self.assertGreaterEqual(result.elapsed_ns, 300_000_000)

    def test_time_cli_uses_timer_around_communicate(self) -> None:
        process = mock.Mock()
        process.communicate.return_value = (b"stdout", b"stderr")
        process.returncode = 0

        with (
            mock.patch(
                "benchmarks.rstim_vs_stim_simulator.run_fair_cli.subprocess.Popen",
                return_value=process,
            ) as popen,
            mock.patch(
                "benchmarks.rstim_vs_stim_simulator.run_fair_cli.time.perf_counter_ns",
                side_effect=[100, 300_000_150],
            ),
        ):
            result = run_fair_cli.time_cli(["fake", "arg"], cwd=ROOT)

        popen.assert_called_once_with(
            ["fake", "arg"],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        process.communicate.assert_called_once_with()
        self.assertEqual(result.exit_code, 0)
        self.assertEqual(result.stdout, b"stdout")
        self.assertEqual(result.stderr, b"stderr")
        self.assertEqual(result.elapsed_ns, 300_000_050)

    def test_preflight_rejects_malformed_known_answer_before_writing_raw_records(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="success")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="malformed-preflight")
            out_dir = root / "out"
            invocations = root / "invocations.txt"
            with (
                mock.patch.dict(
                    os.environ,
                    {
                        "PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}",
                        "FAKE_CLI_INVOCATIONS": str(invocations),
                    },
                ),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                with self.assertRaisesRegex(RuntimeError, "known-answer preflight"):
                    run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            self.assertFalse((out_dir / "raw.jsonl").exists())
            logged_invocations = read_invocations(invocations)
            self.assertEqual({Path(call[0]).resolve() for call in logged_invocations}, {stim.resolve(), rstim.resolve()})
            self.assertTrue(all("--shots" in call and call[call.index("--shots") + 1] == "1" for call in logged_invocations))

    def test_preflight_rejects_malformed_stim_known_answer_before_writing_raw_records(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            stim = write_fake_cli(fake_bin / "stim", mode="malformed-preflight")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
            out_dir = root / "out"
            with (
                mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                with self.assertRaisesRegex(RuntimeError, "known-answer preflight"):
                    run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            self.assertFalse((out_dir / "raw.jsonl").exists())

    def test_preflight_rejects_zero_known_answer_before_writing_raw_records(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="success")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="zero-preflight")
            out_dir = root / "out"
            invocations = root / "invocations.txt"
            with (
                mock.patch.dict(
                    os.environ,
                    {
                        "PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}",
                        "FAKE_CLI_INVOCATIONS": str(invocations),
                    },
                ),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                with self.assertRaisesRegex(RuntimeError, "known-answer preflight"):
                    run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            self.assertFalse((out_dir / "raw.jsonl").exists())
            logged_invocations = read_invocations(invocations)
            self.assertEqual({Path(call[0]).resolve() for call in logged_invocations}, {stim.resolve(), rstim.resolve()})
            self.assertTrue(all("--shots" in call and call[call.index("--shots") + 1] == "1" for call in logged_invocations))

    def test_preflight_rejects_zero_stim_known_answer_before_writing_raw_records(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            stim = write_fake_cli(fake_bin / "stim", mode="zero-preflight")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
            out_dir = root / "out"
            with (
                mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                with self.assertRaisesRegex(RuntimeError, "known-answer preflight"):
                    run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            self.assertFalse((out_dir / "raw.jsonl").exists())

    def test_rejects_wrong_stim_version_before_writing_raw_records(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="bad-stim-version")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
            out_dir = root / "out"
            with (
                mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                with self.assertRaisesRegex(RuntimeError, "Stim.*1\\.15\\.0|stim.*1\\.15\\.0"):
                    run_fair_cli.run_fair_cli(make_args(out_dir), repo_root=ROOT)

            self.assertFalse((out_dir / "raw.jsonl").exists())

    def test_rejects_nonzero_benchmark_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="success")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="fail-benchmark")
            out_dir = root / "out"
            with (
                mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                with self.assertRaisesRegex(RuntimeError, "exit|failed|code 7"):
                    run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            self.assertFalse((out_dir / "summary.json").exists())

    def test_rejects_nonzero_stim_benchmark_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="fail-benchmark")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
            out_dir = root / "out"
            with (
                mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                with self.assertRaisesRegex(RuntimeError, "exit|failed|code 7"):
                    run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            self.assertFalse((out_dir / "summary.json").exists())

    def test_rejects_short_benchmark_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="success")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="short-output")
            out_dir = root / "out"
            with (
                mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                with self.assertRaisesRegex(RuntimeError, "1552384|output bytes"):
                    run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            self.assertFalse((out_dir / "summary.json").exists())

    def test_rejects_short_stim_benchmark_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="short-output")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
            out_dir = root / "out"
            with (
                mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
            ):
                with self.assertRaisesRegex(RuntimeError, "1552384|output bytes"):
                    run_fair_cli.run_fair_cli(make_args(out_dir, warmup_rounds=0, measure_rounds=1), repo_root=ROOT)

            self.assertFalse((out_dir / "summary.json").exists())

    def test_manifest_validation_precedes_all_benchmark_processes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="success")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
            out_dir = root / "out"
            invocations = root / "invocations.txt"
            with (
                mock.patch.dict(
                    os.environ,
                    {
                        "PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}",
                        "FAKE_CLI_INVOCATIONS": str(invocations),
                    },
                ),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_fair_cli.fair_cli_contract.validate_case",
                    return_value=["manifest validation failed"],
                ),
            ):
                with self.assertRaisesRegex(RuntimeError, "manifest validation failed"):
                    run_fair_cli.run_fair_cli(make_args(out_dir), repo_root=ROOT)

            self.assertFalse((out_dir / "raw.jsonl").exists())
            if invocations.exists():
                self.assertFalse(any(str(FIXTURE) in call for call in invocations.read_text(encoding="utf-8").splitlines()))


if __name__ == "__main__":
    unittest.main()
