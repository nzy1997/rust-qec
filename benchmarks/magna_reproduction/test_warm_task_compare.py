"""Offline controls for replay diagnostic admission and retained failure evidence."""
import copy
import json
import subprocess
import unittest

import affine_task_compare as affine
import controller_task_compare as controller


def scalar(value=1):
    return dict(coefficients=[str(value), "0", "0", "0"], power=0)


def expected():
    return dict(raw_acceptance=scalar(), raw_error=scalar(0), raw_target_success=scalar(),
                history_mass=scalar(), target_normalization=scalar())


def valid(count):
    return dict(configuration=dict(max_terms=16, warm_replays=count,
                    probability_preparation="fixed", fiber_search_rank=0),
                compilation=dict(strategy="SharedCommutingLayer"),
                compile_ms=0.0, query_ms=1,
                warm_replays=(dict(count=count, query_ms=[0.25] * count,
                                   cache_unchanged=True, scope=controller.WARM_SCOPE) if count else None),
                **expected())


def compare(actual, count, kind):
    result = subprocess.CompletedProcess([], 0, json.dumps(actual), "replay-diagnostic-stderr")
    if kind == "controller":
        return controller.compare_result(result, expected(), 7, 16, False, "shared", "fixed", 0, count)
    return affine.compare_affine_result(result, expected(), 16, "shared", "fixed", 0, count)


class WarmComparisonTests(unittest.TestCase):
    def test_zero_one_two_and_maximum_counts_accept_complete_exact_diagnostics(self):
        for count in [0, 1, 2, 16]:
            for kind in ["affine", "controller"]:
                with self.subTest(count=count, kind=kind):
                    actual = valid(count)
                    row = compare(actual, count, kind)
                    self.assertEqual(row["status"], "matched")
                    self.assertEqual(row["compared"], expected())
                    self.assertEqual(row["rust"], actual)
                    self.assertEqual("constant_word" in row, kind == "controller")

    def test_corrupt_diagnostics_fail_with_raw_payload_stderr_and_native_reference(self):
        mutations = []
        for count in [0, 2]:
            for value in [None, False, True, "2", -1, 17, 1 if count == 2 else 2]:
                actual = valid(count)
                actual["configuration"]["warm_replays"] = value
                mutations.append((count, actual))
            for field in ["compile_ms", "query_ms"]:
                for value in [None, False, True, "1", -0.25, float("nan"), float("inf"), 10 ** 400]:
                    actual = valid(count)
                    actual[field] = value
                    mutations.append((count, actual))
            actual = valid(count)
            del actual["warm_replays"]
            mutations.append((count, actual))
        for value in [False, 0, [], {}, valid(2)["warm_replays"]]:
            actual = valid(0)
            actual["warm_replays"] = value
            mutations.append((0, actual))
        for value in [None, False, [], "2"]:
            actual = valid(2)
            actual["warm_replays"] = value
            mutations.append((2, actual))
        for field, values in [
            ("count", [None, False, True, "2", 1, 3]),
            ("query_ms", [None, "00", {"a": 0, "b": 0}, [], [0], [0, 0, 0],
                          [True, 0], [0, False], [0, -1], [0, float("nan")],
                          [float("inf"), 0], [10 ** 400, 0]]),
            ("cache_unchanged", [None, False, 1, "true"]),
            ("scope", [None, "independent Monte Carlo", 1]),
        ]:
            for value in values:
                actual = valid(2)
                actual["warm_replays"][field] = value
                mutations.append((2, actual))
            actual = valid(2)
            del actual["warm_replays"][field]
            mutations.append((2, actual))
        for count, actual in mutations:
            for kind in ["affine", "controller"]:
                with self.subTest(count=count, kind=kind, actual=actual):
                    row = compare(actual, count, kind)
                    self.assertEqual(row["status"], "failed")
                    self.assertEqual(row["returncode"], 0)
                    self.assertEqual(row["stdout"], json.dumps(actual))
                    self.assertEqual(row["stderr"], "replay-diagnostic-stderr")
                    self.assertEqual(row["reference"], expected())
                    self.assertEqual(row["error_type"], "ValueError")
                    self.assertTrue(row["error"])
                    if kind == "affine":
                        self.assertEqual(row["message"], row["error"])

    def test_replay_validation_keeps_existing_exact_and_preparation_checks(self):
        for mode in ["cold", "trace", "fixed"]:
            actual = valid(2)
            actual["configuration"].update(probability_preparation=mode,
                fiber_search_rank=0 if mode == "fixed" else None)
            result = subprocess.CompletedProcess([], 0, json.dumps(actual), "")
            row = controller.compare_result(result, expected(), 0, 16, False,
                "shared", mode, 0, 2)
            self.assertEqual(row["status"], "matched")
        actual = valid(2)
        actual["raw_acceptance"] = scalar(2)
        row = compare(actual, 2, "controller")
        self.assertEqual(row["status"], "mismatch")
        self.assertEqual(row["rust"], actual)
        for config in [dict(probability_preparation="wrong"), dict(fiber_search_rank=False), dict(max_terms=True)]:
            actual = copy.deepcopy(valid(2))
            actual["configuration"].update(config)
            row = compare(actual, 2, "affine")
            self.assertEqual(row["status"], "failed")
            self.assertEqual(row["reference"], expected())
            self.assertEqual(row["stdout"], json.dumps(actual))


if __name__ == "__main__":
    unittest.main()
