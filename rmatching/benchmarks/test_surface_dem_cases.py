import unittest

from benchmarks.surface_dem_cases import DEFAULT_SEED, DEFAULT_SHOTS, build_cases


class SurfaceDemCasesTest(unittest.TestCase):
    def test_case_names_and_detector_counts_are_stable(self):
        cases = build_cases()
        self.assertEqual(
            [case.name for case in cases],
            ["surface-d5-p0.001", "surface-d17-p0.001"],
        )
        self.assertEqual([case.distance for case in cases], [5, 17])
        self.assertEqual([case.p for case in cases], [0.001, 0.001])
        self.assertEqual([case.num_detectors for case in cases], [120, 4896])

    def test_sampled_batches_are_deterministic(self):
        first = build_cases(shots=8, seed=DEFAULT_SEED)
        second = build_cases(shots=8, seed=DEFAULT_SEED)
        self.assertEqual(first[0].syndromes, second[0].syndromes)
        self.assertEqual(first[0].observables, second[0].observables)
        self.assertEqual(first[1].syndromes, second[1].syndromes)
        self.assertEqual(first[1].observables, second[1].observables)

    def test_sampled_batches_match_requested_shot_count(self):
        cases = build_cases(shots=16, seed=DEFAULT_SEED)
        self.assertEqual(len(cases[0].syndromes), 16)
        self.assertEqual(len(cases[0].observables), 16)
        self.assertEqual(len(cases[1].syndromes), 16)
        self.assertEqual(len(cases[1].observables), 16)
        self.assertEqual(cases[0].num_syndromes, 16)
        self.assertEqual(cases[1].num_syndromes, 16)

    def test_default_configuration_is_nonempty(self):
        cases = build_cases()
        self.assertEqual(DEFAULT_SHOTS, cases[0].num_syndromes)
        for case in cases:
            self.assertTrue(case.dem)
            self.assertGreater(case.num_detectors, 0)
            self.assertEqual(len(case.syndromes), DEFAULT_SHOTS)


if __name__ == "__main__":
    unittest.main()
