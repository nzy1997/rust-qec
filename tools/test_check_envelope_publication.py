#!/usr/bin/env python3
"""Offline regression tests for the envelope publication verifier."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from tools import check_envelope_publication as publication


class EnvelopePublicationSelfTest(unittest.TestCase):
    def test_self_test_rejects_defective_bundles(self) -> None:
        self.assertEqual(publication.self_test(), 0)

    def test_missing_release_dir_is_an_error(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(publication.PublicationError):
                publication.verify_release_dir(Path(temporary) / "missing", ())

    def test_legacy_release_without_bundle_lacks_verified_promotion(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            legacy = Path(temporary)
            (legacy / "release-manifest.json").write_text("{}", encoding="utf-8")
            with self.assertRaises(publication.PublicationError) as caught:
                publication.verify_release_dir(legacy, ("envelope-matching",))
            self.assertIn("lacks a verified envelope promotion", str(caught.exception))


if __name__ == "__main__":
    unittest.main()
