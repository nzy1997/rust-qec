from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest import mock

from benchmarks.rstim_vs_stim_simulator import run_dem_speed_case


class RunDemSpeedCaseValidationTest(unittest.TestCase):
    def test_load_and_validate_dem_case_rejects_bad_counts(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            dem_path = root / "case.dem"
            metadata_path = root / "case.dem.metadata.json"
            dem_path.write_text("error(0.1) D0 L0\n")
            dem_hash = run_dem_speed_case.sha256_file(dem_path)
            metadata_path.write_text(
                json.dumps(
                    {
                        "case_label": "stim-style-surface-dem-sample-d11-r100-b1024",
                        "dem_path": str(dem_path),
                        "dem_sha256": dem_hash,
                        "expected_detectors": 11999,
                        "expected_observables": 1,
                        "shots": 1024,
                        "source_circuit_path": "fixtures/source.stim",
                        "source_circuit_sha256": "0" * 64,
                        "generation_command": "stim analyze_errors --decompose_errors < source.stim > case.dem",
                    }
                )
                + "\n"
            )
            case = run_dem_speed_case.DemCase(
                label="stim-style-surface-dem-sample-d11-r100-b1024",
                dem_path=dem_path,
                metadata_path=metadata_path,
                shots=1024,
                expected_detectors=12000,
                expected_observables=1,
            )

            with self.assertRaisesRegex(ValueError, "DEM metadata mismatch"):
                run_dem_speed_case.load_and_validate_dem_case(case)

    def test_load_and_validate_dem_case_rejects_bad_observable_count(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            dem_path = root / "case.dem"
            metadata_path = root / "case.dem.metadata.json"
            dem_path.write_text("error(0.1) D0 L0\n")
            dem_hash = run_dem_speed_case.sha256_file(dem_path)
            metadata_path.write_text(
                json.dumps(
                    {
                        "case_label": "stim-style-surface-dem-sample-d11-r100-b1024",
                        "dem_path": str(dem_path),
                        "dem_sha256": dem_hash,
                        "expected_detectors": 1,
                        "expected_observables": 0,
                        "shots": 1024,
                        "source_circuit_path": "fixtures/source.stim",
                        "source_circuit_sha256": "0" * 64,
                        "generation_command": "stim analyze_errors --decompose_errors < source.stim > case.dem",
                    }
                )
                + "\n"
            )
            case = run_dem_speed_case.DemCase(
                label="stim-style-surface-dem-sample-d11-r100-b1024",
                dem_path=dem_path,
                metadata_path=metadata_path,
                shots=1024,
                expected_detectors=1,
                expected_observables=1,
            )

            with self.assertRaisesRegex(ValueError, "DEM metadata mismatch"):
                run_dem_speed_case.load_and_validate_dem_case(case)

    def test_load_and_validate_dem_case_accepts_matching_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            dem_path = root / "case.dem"
            metadata_path = root / "case.dem.metadata.json"
            dem_path.write_text("error(0.1) D0 L0\n")
            dem_hash = run_dem_speed_case.sha256_file(dem_path)
            metadata_path.write_text(
                json.dumps(
                    {
                        "case_label": "stim-style-surface-dem-sample-d11-r100-b1024",
                        "dem_path": str(dem_path),
                        "dem_sha256": dem_hash,
                        "expected_detectors": 1,
                        "expected_observables": 1,
                        "shots": 1024,
                        "source_circuit_path": "fixtures/source.stim",
                        "source_circuit_sha256": "0" * 64,
                        "generation_command": "stim analyze_errors --decompose_errors < source.stim > case.dem",
                    }
                )
                + "\n"
            )
            case = run_dem_speed_case.DemCase(
                label="stim-style-surface-dem-sample-d11-r100-b1024",
                dem_path=dem_path,
                metadata_path=metadata_path,
                shots=1024,
                expected_detectors=1,
                expected_observables=1,
            )

            dem_text, metadata = run_dem_speed_case.load_and_validate_dem_case(case)

            self.assertEqual(dem_text, "error(0.1) D0 L0\n")
            self.assertEqual(metadata["dem_sha256"], dem_hash)


class RunDemSpeedCaseWorkflowTest(unittest.TestCase):
    def test_run_dem_speed_case_invokes_sample_dem_and_writes_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            out_dir = repo_root / "out"
            binary = repo_root / "target/release/rstim"
            binary.parent.mkdir(parents=True)
            binary.write_text("")
            args = argparse.Namespace(
                profile="release",
                case=run_dem_speed_case.FULL_CASE_LABEL,
                warmup_rounds=0,
                measure_rounds=1,
                out_dir=out_dir,
            )
            calls: list[tuple[list[str], dict[str, object]]] = []

            def fake_run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                calls.append((command, kwargs))
                if command == ["rustc", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "rustc 1.93.1\n", "")
                if command == ["cargo", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "cargo 1.93.1\n", "")
                if command == [str(binary)]:
                    return subprocess.CompletedProcess(command, 0, "rstim 0.1.1\n", "")
                if command == ["stim", "--version"]:
                    return subprocess.CompletedProcess(command, 0, "stim 1.15.0\n", "")
                if command == ["stim", "sample_dem", "--shots", "1024"]:
                    return subprocess.CompletedProcess(command, 0, "", "")
                if command == [str(binary), "sample_dem", "--shots", "1024"]:
                    return subprocess.CompletedProcess(command, 0, "", "")
                raise AssertionError(f"unexpected command: {command}")

            with (
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_dem_speed_case.build_rstim",
                    return_value=binary,
                ),
                mock.patch("benchmarks.rstim_vs_stim_simulator.run_dem_speed_case.subprocess.run") as mocked,
                mock.patch(
                    "benchmarks.rstim_vs_stim_simulator.run_dem_speed_case.time.perf_counter_ns",
                    side_effect=[100, 120, 200, 240],
                ),
            ):
                mocked.side_effect = fake_run
                run_dem_speed_case.run_dem_speed_case(
                    args,
                    repo_root=repo_root,
                    command_line=["run-dem-speed-case", "--profile", "release"],
                )

            stim_call = next(call for call in calls if call[0] == ["stim", "sample_dem", "--shots", "1024"])
            rstim_call = next(
                call for call in calls if call[0] == [str(binary), "sample_dem", "--shots", "1024"]
            )
            for _, kwargs in [stim_call, rstim_call]:
                self.assertEqual(kwargs["input"], run_dem_speed_case.FULL_CASE.dem_path.read_text())
                self.assertTrue(kwargs["text"])
                self.assertIs(kwargs["stdout"], subprocess.DEVNULL)
                self.assertIs(kwargs["stderr"], subprocess.PIPE)

            raw_records = [
                json.loads(line) for line in (out_dir / "raw.jsonl").read_text().splitlines() if line.strip()
            ]
            self.assertEqual(len(raw_records), 2)
            self.assertEqual(
                sorted(record["tool_variant"] for record in raw_records),
                ["rstim-sample-dem", "stim-sample-dem"],
            )
            self.assertTrue(all(record["status"] == "completed" for record in raw_records))

            summary = json.loads((out_dir / "summary.json").read_text())
            self.assertEqual(len(summary["cases"]), 1)
            case_summary = summary["cases"][0]
            self.assertEqual(case_summary["case_label"], run_dem_speed_case.FULL_CASE_LABEL)
            self.assertEqual(case_summary["workload"], "sample_dem")
            self.assertEqual(case_summary["tier"], "report_only")
            self.assertEqual(
                case_summary["expected_variants"],
                ["stim-sample-dem", "rstim-sample-dem"],
            )
            self.assertEqual(
                case_summary["present_variants"],
                ["rstim-sample-dem", "stim-sample-dem"],
            )
            self.assertTrue(all(item["status"] == "completed" for item in case_summary["variants"]))

            report_text = (out_dir / "report.md").read_text()
            self.assertIn(run_dem_speed_case.FULL_CASE_LABEL, report_text)
            self.assertIn("sample_dem", report_text)
            self.assertIn("stim-sample-dem", report_text)
            self.assertIn("rstim-sample-dem", report_text)

            environment = json.loads((out_dir / "environment.json").read_text())
            self.assertEqual(environment["profile"], "release")
            self.assertEqual(environment["case_label"], run_dem_speed_case.FULL_CASE_LABEL)
            self.assertEqual(environment["case_labels"], [run_dem_speed_case.FULL_CASE_LABEL])
            self.assertEqual(environment["case_count"], 1)
            self.assertEqual(environment["command_line"], ["run-dem-speed-case", "--profile", "release"])
            self.assertEqual(environment["dem_path"], str(run_dem_speed_case.FULL_CASE.dem_path.resolve()))
            self.assertEqual(environment["dem_sha256"], run_dem_speed_case.sha256_file(run_dem_speed_case.FULL_CASE.dem_path))
            self.assertEqual(environment["expected_detectors"], 12000)
            self.assertEqual(environment["expected_observables"], 1)

    def test_main_returns_1_and_prints_error_for_runtime_failure(self) -> None:
        stderr = StringIO()
        with (
            mock.patch(
                "benchmarks.rstim_vs_stim_simulator.run_dem_speed_case.run_dem_speed_case",
                side_effect=RuntimeError("boom"),
            ),
            mock.patch("sys.stderr", stderr),
        ):
            result = run_dem_speed_case.main(
                [
                    "--profile",
                    "release",
                    "--case",
                    run_dem_speed_case.FULL_CASE_LABEL,
                    "--out-dir",
                    "out",
                ]
            )

        self.assertEqual(result, 1)
        self.assertIn("boom", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
