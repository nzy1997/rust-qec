import unittest

from benchmarks.minimal_cases import build_cases


class MinimalCasesTest(unittest.TestCase):
    def test_case_names_and_sizes_are_stable(self):
        cases = build_cases()
        self.assertEqual(
            [case.name for case in cases],
            ["boundary-2", "square-4", "blossom-3"],
        )
        self.assertEqual([case.num_detectors for case in cases], [2, 4, 3])
        self.assertEqual([case.num_edges for case in cases], [3, 9, 6])

    def test_syndromes_are_exhaustive(self):
        cases = {case.name: case for case in build_cases()}
        self.assertEqual(len(cases["boundary-2"].syndromes), 4)
        self.assertEqual(len(cases["square-4"].syndromes), 16)
        self.assertEqual(len(cases["blossom-3"].syndromes), 8)
        self.assertEqual(cases["square-4"].syndromes[0], [0, 0, 0, 0])
        self.assertEqual(cases["square-4"].syndromes[-1], [1, 1, 1, 1])


if __name__ == "__main__":
    unittest.main()
