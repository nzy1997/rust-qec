import json
import tempfile
import unittest
from pathlib import Path

from benchmarks.surface_decoder_compare.cases import (
    build_case_specs,
    materialize_case_bundle,
)
from benchmarks.surface_decoder_compare.schema import CaseSpec, TIER_CONFIGS


class CaseBuilderTest(unittest.TestCase):
    def test_case_specs_cover_the_default_smoke_grid(self) -> None:
        specs = build_case_specs()
        self.assertEqual(len(specs), 3)
        self.assertEqual(specs[0], CaseSpec(distance=3, rounds=3, p=0.002))
        self.assertEqual(specs[-1], CaseSpec(distance=3, rounds=3, p=0.010))
        self.assertTrue(all(spec.rounds == spec.distance for spec in specs))

    def test_case_specs_cover_the_focused_full_grid(self) -> None:
        specs = build_case_specs(tier_name="full")
        self.assertEqual(len(specs), 6)
        self.assertEqual(specs[0], CaseSpec(distance=3, rounds=3, p=0.002))
        self.assertEqual(specs[-1], CaseSpec(distance=5, rounds=5, p=0.010))
        self.assertTrue(all(spec.rounds == spec.distance for spec in specs))

    def test_materialize_case_bundle_writes_reproducible_artifacts(self) -> None:
        spec = CaseSpec(distance=3, rounds=3, p=0.001)
        tier = TIER_CONFIGS["smoke"]
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            first = materialize_case_bundle(root, spec, tier, seed=12345)
            second = materialize_case_bundle(root, spec, tier, seed=12345)

            self.assertTrue(first.circuit_path.exists())
            self.assertTrue(first.dem_path.exists())
            self.assertTrue(first.dets_b8_path.exists())
            self.assertTrue(first.obs_b8_path.exists())

            metadata = json.loads(first.metadata_path.read_text())
            self.assertEqual(metadata["distance"], 3)
            self.assertEqual(metadata["rounds"], 3)
            self.assertEqual(metadata["p"], 0.001)
            self.assertEqual(metadata["num_shots"], tier.max_shots)

            self.assertEqual(first.num_dets, second.num_dets)
            self.assertEqual(first.num_obs, second.num_obs)
            self.assertEqual(
                first.dets_b8_path.read_bytes(),
                second.dets_b8_path.read_bytes(),
            )
            self.assertEqual(
                first.obs_b8_path.read_bytes(),
                second.obs_b8_path.read_bytes(),
            )


if __name__ == "__main__":
    unittest.main()
