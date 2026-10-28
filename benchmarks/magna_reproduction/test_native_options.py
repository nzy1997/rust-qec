"""Offline admission/receipt controls; exact circuit witnesses run separately."""
import argparse
import contextlib
import copy
import io
import json
import subprocess
import sys
import unittest
from unittest.mock import patch

import controller_task_compare as driver
import native_options as native
from test_controller_task_compare import expected, valid


def arguments(*flags):
    parser = argparse.ArgumentParser()
    native.add_arguments(parser)
    return parser.parse_args(flags)


def receipt(args):
    actual = valid()
    for family, policy in native.resolved(args).items():
        actual["configuration"][family + "_preparation"] = policy["mode"]
        actual["configuration"][family + "_preparation_options"] = policy
        actual["prepared_" + family + "_cache"] = dict(
            cached_queries=2, fixed_resources=2, catalogue_resources=0,
            native_routes=dict(direct=1, product=1, walsh=0, injected=0, injection_budget_skips=0),
            native_transports=dict(prepared=0, budget_skipped=0))
    return actual


class NativeOptionsTests(unittest.TestCase):
    def test_orphan_and_out_of_range_options_reject_before_reference_or_decoder_imports(self):
        invalid = [["--" + flag, "1"] for flag, _, _, _ in native.LIMITS]
        invalid += [["--native-transport", "off"],
                    ["--probability-preparation", "native", "--injection-max-work", "0"],
                    ["--moment-preparation", "native", "--native-transport-max-terms", "0"],
                    ["--moment-preparation", "native", "--native-max-terms", "-1"],
                    ["--moment-preparation", "native", "--native-max-core-work", str(2**128)],
                    ["--moment-preparation", "native", "--native-max-terms", str(2**64)]]
        original_import = __import__
        def checked(name, *args, **kwargs):
            if name in ("reference", "witness_environment", "magna"):
                self.fail("invalid flags reached reference/decoder admission")
            return original_import(name, *args, **kwargs)
        for flags in invalid:
            with self.subTest(flags=flags), patch.object(sys, "argv", ["compare", "--upstream", ".", "--rust-example", "missing"] + flags), patch("builtins.__import__", side_effect=checked), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as failure:
                    driver.main()
                self.assertEqual(failure.exception.code, 2)

    def test_mixed_policy_forwarding_and_wide_limits_are_exact(self):
        args = arguments("--probability-preparation", "native", "--moment-preparation", "native_injection",
                         "--native-transport", "prepared", "--native-max-signature-work", str(2**100),
                         "--injection-max-work", "0", "--native-transport-max-terms", "0")
        parser = argparse.ArgumentParser()
        native.validate(args, parser)
        self.assertEqual(vars(arguments(*native.command(args))), vars(args))
        policies = native.resolved(args)
        self.assertIsNone(policies["probability"]["injection"])
        self.assertEqual(policies["moment"]["injection"]["max_work"], "0")
        self.assertEqual(policies["moment"]["selection"]["max_signature_work"], str(2**100))
        self.assertEqual(policies["moment"]["selection"]["construction"]["max_json_bytes"], 16 << 20)
        self.assertEqual(policies["moment"]["transport"]["max_terms"], 0)
        native.verify(receipt(args), args)

    def test_every_configuration_leaf_and_count_rejects_boolean_or_wrong_value(self):
        args = arguments("--probability-preparation", "native_injection", "--native-transport", "prepared",
                         "--native-max-quotient-rank", "1")
        base = receipt(args)
        native.verify(base, args)
        paths = []
        def leaves(value, prefix):
            if isinstance(value, dict):
                for key, item in value.items():
                    leaves(item, prefix + [key])
            else:
                paths.append(prefix)
        leaves(base["configuration"]["probability_preparation_options"], ["configuration", "probability_preparation_options"])
        leaves(base["prepared_probability_cache"], ["prepared_probability_cache"])
        for path in paths:
            for invalid in (True, "wrong"):
                actual = copy.deepcopy(base)
                node = actual
                for key in path[:-1]:
                    node = node[key]
                node[path[-1]] = invalid
                with self.subTest(path=path, invalid=invalid), self.assertRaises(ValueError):
                    native.verify(actual, args)
        altered = copy.deepcopy(base)
        altered["prepared_probability_cache"]["native_routes"]["walsh"] = 1
        with self.assertRaises(ValueError):
            native.verify(altered, args)

    def test_native_validation_failure_retains_raw_successful_child_output(self):
        args = arguments("--moment-preparation", "native")
        actual = receipt(args)
        actual["configuration"]["moment_preparation_options"]["selection"]["max_quotient_rank"] = True
        payload = json.dumps(actual)
        row = driver.compare_result(subprocess.CompletedProcess([], 0, payload, "native-stderr"),
                                    expected(), 0, 16, False, native_args=args)
        self.assertEqual(row["status"], "failed")
        self.assertEqual(row["stdout"], payload)
        self.assertEqual(row["stderr"], "native-stderr")
        self.assertEqual(row["reference"], expected())


if __name__ == "__main__":
    unittest.main()
