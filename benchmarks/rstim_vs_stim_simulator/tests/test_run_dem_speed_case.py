from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

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


if __name__ == "__main__":
    unittest.main()
