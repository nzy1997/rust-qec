import tempfile
import unittest
from pathlib import Path

import numpy as np

from benchmarks.surface_decoder_compare.drivers.dem_matrix import (
    lower_dem_to_matrix_problem,
)
from benchmarks.surface_decoder_compare.drivers.ldpc_driver import LdpcDriver
from benchmarks.surface_decoder_compare.schema import CaseBundle, CaseSpec, TIER_CONFIGS


class DemMatrixTest(unittest.TestCase):
    def test_lower_dem_to_matrix_problem_tracks_separator_and_exact_probs(self) -> None:
        problem = lower_dem_to_matrix_problem(
            "error(1) D0 L0\nerror(0.25) D0 ^ D1 L0\nerror(0.2) D1\n"
        )

        self.assertEqual(problem.num_dets, 2)
        self.assertEqual(problem.num_obs, 1)
        self.assertEqual(problem.detector_columns, [[0], [0, 1], [1]])
        self.assertEqual(problem.observable_columns, [[0], [0], []])
        self.assertEqual(problem.probabilities, [1.0, 0.25, 0.2])

    def test_lower_dem_to_matrix_problem_ignores_non_error_lines_and_toggles_duplicates(
        self,
    ) -> None:
        problem = lower_dem_to_matrix_problem(
            "repeat 2 {\n"
            "    error(0.3) D0 D0 L0 L0\n"
            "}\n"
            "detector(1, 2) D0\n"
        )

        self.assertEqual(problem.detector_columns, [[], []])
        self.assertEqual(problem.observable_columns, [[], []])
        self.assertEqual(problem.probabilities, [0.3, 0.3])

    def test_ldpc_driver_decodes_a_synthetic_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            dem_path = root / "model.dem"
            dets_path = root / "detections.b8"
            obs_path = root / "observables.b8"
            metadata_path = root / "metadata.json"
            circuit_path = root / "circuit.stim"

            dem_path.write_text("error(0.125) D0 L0\nerror(0.25) D1\n")
            circuit_path.write_text("M 0\n")
            np.asarray([[0b0000_0001]], dtype=np.uint8).tofile(dets_path)
            np.asarray([[0b0000_0001]], dtype=np.uint8).tofile(obs_path)
            metadata_path.write_text("{}")

            bundle = CaseBundle(
                spec=CaseSpec(distance=3, rounds=3, p=0.001),
                tier=TIER_CONFIGS["smoke"],
                seed=12345,
                num_dets=2,
                num_obs=1,
                num_shots=1,
                circuit_path=circuit_path,
                dem_path=dem_path,
                dets_b8_path=dets_path,
                obs_b8_path=obs_path,
                metadata_path=metadata_path,
            )
            row = LdpcDriver().run_case(bundle, batch_size=1)

            self.assertEqual(row.decoder, "ldpc")
            self.assertEqual(row.backend, "native")
            self.assertEqual(row.shots_used, 1)


if __name__ == "__main__":
    unittest.main()
