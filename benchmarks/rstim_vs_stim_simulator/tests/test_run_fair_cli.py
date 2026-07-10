from __future__ import annotations

import argparse
import hashlib
import json
import os
import statistics
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator import run_fair_cli


ROOT = Path(__file__).resolve().parents[3]
FAIR_MANIFEST = ROOT / "benchmarks" / "rstim_vs_stim_simulator" / "fair_cli_cases.toml"
EXPECTED_OUTPUT_BYTES = 1_552_384


def write_fake_cli(path: Path, *, mode: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        textwrap.dedent(
            f"""\
            #!{os.sys.executable}
            import sys
            import time

            MODE = {mode!r}

            def expected_output_bytes():
                return bytes(range(256)) * 6064

            if sys.argv[1:] == ["--version"]:
                print("stim 1.15.0")
                sys.exit(0)

            input_path = sys.argv[sys.argv.index("--in") + 1]
            if open(input_path, encoding="utf-8").read() == "X 0\\nM 0\\n":
                sys.stdout.buffer.write(b"\\x00" if MODE == "bad-preflight" else b"\\x01")
                sys.exit(0)

            payload = expected_output_bytes()
            if MODE == "delayed":
                sys.stdout.buffer.write(payload[:-1])
                sys.stdout.buffer.flush()
                time.sleep(0.15)
                sys.stdout.buffer.write(payload[-1:])
                sys.stdout.buffer.flush()
                sys.stdout.close()
                time.sleep(0.15)
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


class RunFairCliTest(unittest.TestCase):
    def test_writes_symmetric_artifacts_for_all_rounds(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            stim = write_fake_cli(fake_bin / "stim", mode="success")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
            out_dir = root / "out"
            args = argparse.Namespace(
                manifest=FAIR_MANIFEST,
                case="stim_surface_d11_r100",
                profile="release",
                warmup_rounds=2,
                measure_rounds=7,
                out_dir=out_dir,
            )

            with (
                mock.patch.dict(
                    os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim",
                    return_value=rstim,
                ),
            ):
                run_fair_cli.run_fair_cli(args, repo_root=ROOT, command_line=["run-fair-cli"])

            records = read_jsonl(out_dir / "raw.jsonl")
            self.assertEqual(len(records), 18)
            required_keys = {
                "argv",
                "variant",
                "phase",
                "round_index",
                "seed",
                "elapsed_ns",
                "exit_code",
                "actual_output_bytes",
            }
            for record in records:
                self.assertTrue(required_keys <= record.keys(), record)
                self.assertEqual(record["exit_code"], 0)
                self.assertEqual(record["actual_output_bytes"], EXPECTED_OUTPUT_BYTES)

            self.assertEqual({record["variant"] for record in records}, {"stim-cli-b8", "rstim-cli-b8"})
            self.assertEqual({record["phase"] for record in records}, {"warmup", "measured"})
            for variant in ("stim-cli-b8", "rstim-cli-b8"):
                variant_records = [record for record in records if record["variant"] == variant]
                self.assertEqual(
                    sorted(record["round_index"] for record in variant_records if record["phase"] == "warmup"),
                    [0, 1],
                )
                self.assertEqual(
                    sorted(record["round_index"] for record in variant_records if record["phase"] == "measured"),
                    list(range(7)),
                )
                self.assertEqual(sorted(record["seed"] for record in variant_records), list(range(9)))

            self.assertTrue(
                all(
                    Path(record["argv"][0]).resolve() == stim.resolve()
                    for record in records
                    if record["variant"] == "stim-cli-b8"
                )
            )
            self.assertTrue(
                all(
                    Path(record["argv"][0]).resolve() == rstim.resolve()
                    for record in records
                    if record["variant"] == "rstim-cli-b8"
                )
            )

            summary = json.loads((out_dir / "summary.json").read_text(encoding="utf-8"))
            measured = [record for record in records if record["phase"] == "measured"]
            self.assertEqual(len(measured), 14)
            for variant in ("stim-cli-b8", "rstim-cli-b8"):
                samples = [record["elapsed_ns"] for record in measured if record["variant"] == variant]
                summary_variant = next(item for item in summary["variants"] if item["variant"] == variant)
                self.assertEqual(summary_variant["sample_count"], 7)
                self.assertEqual(summary_variant["elapsed_ns"]["median"], statistics.median(samples))

            environment = json.loads((out_dir / "environment.json").read_text(encoding="utf-8"))
            self.assertEqual(environment["manifest"], str(FAIR_MANIFEST))
            self.assertEqual(
                environment["source_manifest"],
                "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
            )
            self.assertEqual(
                environment["fixture"],
                "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
            )
            self.assertEqual(environment["rstim_binary"], str(rstim))
            self.assertEqual(environment["rstim_binary_sha256"], hashlib.sha256(rstim.read_bytes()).hexdigest())
            self.assertEqual(
                environment["argv"],
                {
                    "stim-cli-b8": next(
                        record["argv"] for record in records if record["variant"] == "stim-cli-b8"
                    ),
                    "rstim-cli-b8": next(
                        record["argv"] for record in records if record["variant"] == "rstim-cli-b8"
                    ),
                },
            )
            self.assertEqual(environment["warmup_rounds"], 2)
            self.assertEqual(environment["measure_rounds"], 7)
            self.assertEqual(environment["profile"], "release")
            self.assertEqual(environment["timer_scope"], "cli_end_to_end")
            self.assertEqual(environment["seed_policy"], "round_index_0_through_8")
            self.assertEqual(environment["stim_version"], "1.15.0")
            self.assertEqual(environment["known_answer_preflight"], "passed")

    def test_timing_includes_stdout_completion_and_process_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="delayed")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="delayed")
            out_dir = root / "out"
            args = argparse.Namespace(
                manifest=FAIR_MANIFEST,
                case="stim_surface_d11_r100",
                profile="release",
                warmup_rounds=0,
                measure_rounds=1,
                out_dir=out_dir,
            )

            with (
                mock.patch.dict(
                    os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim",
                    return_value=rstim,
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_fair_cli.time.perf_counter_ns",
                    side_effect=[1, 2, 3, 4, 10, 300_000_010, 20, 300_000_020],
                ),
            ):
                run_fair_cli.run_fair_cli(args, repo_root=ROOT, command_line=["run-fair-cli"])

            records = read_jsonl(out_dir / "raw.jsonl")
            self.assertTrue(any(record["elapsed_ns"] >= 300_000_000 for record in records))

    def test_rejects_a_known_answer_preflight_mismatch_before_writing_raw_records(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            write_fake_cli(fake_bin / "stim", mode="bad-preflight")
            rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="bad-preflight")
            out_dir = root / "out"
            args = argparse.Namespace(
                manifest=FAIR_MANIFEST,
                case="stim_surface_d11_r100",
                profile="release",
                warmup_rounds=0,
                measure_rounds=1,
                out_dir=out_dir,
            )

            with (
                mock.patch.dict(
                    os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}
                ),
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim",
                    return_value=rstim,
                ),
            ):
                with self.assertRaisesRegex(RuntimeError, "known-answer preflight"):
                    run_fair_cli.run_fair_cli(args, repo_root=ROOT, command_line=["run-fair-cli"])

            self.assertFalse((out_dir / "raw.jsonl").exists())


if __name__ == "__main__":
    unittest.main()
