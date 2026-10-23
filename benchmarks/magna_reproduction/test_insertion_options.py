"""Ensure comparison receipts cannot silently select another insertion route."""
import argparse
import contextlib
import io
import unittest

import insertion_options as options


def arguments(*values):
    parser = argparse.ArgumentParser()
    options.add_arguments(parser)
    args = parser.parse_args(values)
    options.validate(args, parser)
    return args


class InsertionOptionsTests(unittest.TestCase):
    def test_modes_defaults_explicit_limits_and_child_arguments(self):
        direct = arguments()
        self.assertEqual(options.command(direct), ["--insertion-preparation", "direct"])
        self.assertEqual(options.resolved(direct), dict(insertion_preparation="direct", max_insertion_table_bytes=None, max_insertion_catalog_entries=None))
        prepared = arguments("--insertion-preparation", "prepared")
        self.assertEqual(options.resolved(prepared)["max_insertion_table_bytes"], 67108864)
        self.assertEqual(options.resolved(prepared)["max_insertion_catalog_entries"], 1_000_000)
        selected = arguments("--insertion-preparation", "prepared", "--max-insertion-table-bytes", "19", "--max-insertion-catalog-entries", "3")
        self.assertEqual(options.command(selected), ["--insertion-preparation", "prepared", "--max-insertion-table-bytes", "19", "--max-insertion-catalog-entries", "3"])

    def test_malformed_modes_limits_and_silent_direct_limits_reject(self):
        for values in [("--insertion-preparation", "unknown"), ("--max-insertion-table-bytes", "3"), ("--insertion-preparation", "prepared", "--max-insertion-table-bytes", "-1"), ("--insertion-preparation", "prepared", "--max-insertion-catalog-entries", "nan")]:
            with self.subTest(values=values), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                arguments(*values)

    def test_valid_retained_catalog_receipts(self):
        direct = arguments()
        options.verify(dict(configuration=options.resolved(direct), prepared_insertions=None), direct)
        prepared = arguments("--insertion-preparation", "prepared")
        for entries, size in [(0, 0), (2, 19)]:
            options.verify(dict(configuration=options.resolved(prepared), prepared_insertions=dict(catalog_entries=entries, estimated_peak_bytes=size)), prepared)
        configuration = dict(options.resolved(prepared), causal_strategy="ColdPrefix")
        with self.assertRaises(RuntimeError):
            options.verify(dict(configuration=configuration, prepared_insertions=None), prepared)

    def test_mismatched_missing_and_boolean_configuration_cannot_pass(self):
        prepared = arguments("--insertion-preparation", "prepared", "--max-insertion-table-bytes", "1")
        for key, value in [("insertion_preparation", "direct"), ("max_insertion_catalog_entries", 2), ("max_insertion_table_bytes", True)]:
            actual = dict(configuration=dict(options.resolved(prepared), **{key:value}), prepared_insertions=dict(catalog_entries=0, estimated_peak_bytes=0))
            with self.assertRaises(RuntimeError):
                options.verify(actual, prepared)
        with self.assertRaises(RuntimeError):
            options.verify(dict(configuration={}), prepared)

    def test_invalid_catalog_diagnostics_and_unrequested_catalog_reject(self):
        prepared = arguments("--insertion-preparation", "prepared", "--max-insertion-table-bytes", "10", "--max-insertion-catalog-entries", "2")
        for catalog in [None, {}, dict(catalog_entries=True,estimated_peak_bytes=1), dict(catalog_entries=3,estimated_peak_bytes=1), dict(catalog_entries=1,estimated_peak_bytes=11), dict(catalog_entries=-1,estimated_peak_bytes=1), dict(catalog_entries=0,estimated_peak_bytes=1), dict(catalog_entries=1,estimated_peak_bytes=0)]:
            with self.subTest(catalog=catalog), self.assertRaises(RuntimeError):
                options.verify(dict(configuration=options.resolved(prepared),prepared_insertions=catalog), prepared)
        direct = arguments()
        with self.assertRaises(RuntimeError):
            options.verify(dict(configuration=options.resolved(direct),prepared_insertions=dict(catalog_entries=0,estimated_peak_bytes=0)),direct)


if __name__ == "__main__":
    unittest.main()
