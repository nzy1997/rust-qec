"""Portable complete-loop and fail-closed generator regressions; no native build."""
import json
import builtins
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock

import original_ratio_rounding_vectors as g


FIXTURE = Path(__file__).resolve().parents[2] / "rmagna/tests/data/original-ratio-rounding.json"


class OriginalRatioRoundingTests(unittest.TestCase):
    def test_complete_original_inputs_and_portable_fixture(self):
        fixture = json.loads(FIXTURE.read_text())
        inputs = g.ratio_inputs()
        self.assertEqual(inputs, [{k: row[k] for k in ("n", "d", "a", "b")}
                                  for row in fixture["rational_ratios"]["cases"]])
        large = g.large_inputs()
        self.assertEqual(large, [{k: v for k, v in row.items() if k != "variants"}
                                for row in fixture["large_rounding"]["cases"]])
        self.assertEqual(len(inputs), 350)
        self.assertEqual(len(large), 237)
        midpoints = [row for row in large if row["kind"] == "midpoint"]
        self.assertEqual(len(midpoints), 27)
        self.assertEqual({row["offset"] for row in midpoints}, {-1, 0, 1})
        self.assertEqual({int(row["q"]) % 2 for row in midpoints}, {0, 1})
        # Lookup is from the retained native-checked fixture, rather than a second
        # implementation of the generator's rounding arithmetic.
        values = {}
        for row in fixture["rational_ratios"]["cases"]:
            values[(int(row["n"]), row["a"], int(row["d"]), row["b"])] = row["expected"]
        for row in fixture["large_rounding"]["cases"]:
            for v in row["variants"]:
                values[(v["sign"] * int(row["n"]), v["power"], int(row["d"]), 0)] = v["expected"]
        calls = []

        class Oracle:
            def file_scalar_ratios(self, pairs):
                import struct
                calls.append(len(pairs))
                result = []
                for ((n, a), (d, b)) in pairs:
                    self_outer.assertEqual(n[1:], (0, 0, 0))
                    self_outer.assertEqual(d[1:], (0, 0, 0))
                    wanted = values[(n[0], a, d[0], b)]
                    mantissa, = struct.unpack(">d", bytes.fromhex(wanted["mantissa_bits"]))
                    result.append((mantissa, wanted["exponent"]))
                return tuple(result)

        self_outer = self
        actual = g.portable_values(Oracle(), fixture["test_source_sha256"])
        self.assertEqual((json.dumps(actual, indent=2) + "\n").encode(), FIXTURE.read_bytes())
        self.assertEqual(calls, [350] + [1] * 711)
        self.assertEqual(set(actual), {"schema", "upstream_commit", "reference_sha256",
                                      "test_source_sha256", "rational_ratios", "large_rounding"})

    def test_wrong_native_bits_exponents_shapes_and_counts_are_rejected(self):
        expected = [{"mantissa_bits": g.bits(0.5), "exponent": 1}]
        for result in [(), ((0.5, 2),), ((0.5000000000000001, 1),),
                       ((float("nan"), 1),), ((1.0, 0),), ((0.5, True),), ((0.5,),)]:
            with self.subTest(result=result), self.assertRaises(ValueError):
                g.check_native(mock.Mock(file_scalar_ratios=mock.Mock(return_value=result)),
                               [(((1, 0, 0, 0), 0), ((1, 0, 0, 0), 0))], expected)

    def test_helper_identity_mismatch_fails_before_any_native_import(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            helper = root / "digest" / ("_native_testing" + g.sysconfig.get_config_var("EXT_SUFFIX"))
            helper.parent.mkdir()
            helper.write_bytes(b"not a library")
            helper.chmod(0o444)
            with mock.patch.object(g, "source_digest", return_value=("digest", [], {})):
                with self.assertRaisesRegex(ValueError, "binary identity"):
                    g.verify_helper(root, helper, {"helper": {"path": str(helper), "sha256": "0" * 64}})
                with self.assertRaisesRegex(ValueError, "Python ABI"):
                    g.verify_helper(root, helper.with_name("wrong.so"), {})
            helper.chmod(0o644)

    def test_capture_rechecks_source_and_binary_identities_after_import_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            native = root / "core/magna/native.so"
            native.parent.mkdir(parents=True)
            native.write_bytes(b"production placeholder, never loaded")
            helper = root / "helper.so"
            helper.write_bytes(b"helper placeholder, never loaded")
            record = root / "record.json"
            record.write_text("{}")
            original_import = builtins.__import__

            def import_without_native(name, *args, **kwargs):
                if name == "magna":
                    return SimpleNamespace(_native=SimpleNamespace(__file__=str(native)))
                return original_import(name, *args, **kwargs)

            with mock.patch.object(g.reference, "verify_source", return_value={"source": "same"}) as verify:
                with mock.patch.object(g, "verify_helper", return_value={}), \
                     mock.patch.object(builtins, "__import__", side_effect=import_without_native), \
                     mock.patch.object(g.importlib.util, "spec_from_file_location", side_effect=RuntimeError("load refused")):
                    with self.assertRaisesRegex(RuntimeError, "load refused"):
                        g.capture(root, helper, record)
                self.assertEqual(verify.call_count, 2)
            self.assertEqual(native.read_bytes(), b"production placeholder, never loaded")
            self.assertEqual(helper.read_bytes(), b"helper placeholder, never loaded")

    def test_destinations_reject_existing_aliases_and_all_input_trees(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            upstream, old = root / "upstream", root / "old"
            upstream.mkdir()
            old.mkdir()
            helper = old / "helper.so"
            record = root / "record.json"
            record.write_text(json.dumps({"source_root": str(old)}))
            valid = root / "valid.json"
            self.assertEqual(g.admit_destinations([valid, root / "receipt.json"], upstream, helper, record),
                             [valid, root / "receipt.json"])
            alias = root / "alias"
            alias.symlink_to(upstream, target_is_directory=True)
            broken = root / "broken"
            broken.symlink_to(root / "absent")
            valid.write_text("keep")
            for outputs in ([valid, root / "other"], [broken, root / "other"],
                            [root / "same", root / "same"], [alias / "new", root / "other"],
                            [old / "new", root / "other"],
                            [FIXTURE.parent / "forbidden-new", root / "other"]):
                with self.subTest(outputs=outputs), self.assertRaises(ValueError):
                    g.admit_destinations(outputs, upstream, helper, record)
            self.assertEqual(valid.read_text(), "keep")
            self.assertFalse((upstream / "new").exists())
            self.assertFalse((old / "new").exists())


if __name__ == "__main__":
    unittest.main()
