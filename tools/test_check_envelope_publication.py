#!/usr/bin/env python3
"""Offline regression tests for the envelope publication verifier."""

from __future__ import annotations

import contextlib
import io
import json
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

    def test_verification_marker_records_verified_bundle_identity(self) -> None:
        result = {
            "tag": "v1.2.3",
            "version": "1.2.3",
            "source_sha": "a" * 40,
            "evidence_bundle": {"asset": "evidence.tar.gz", "sha256": "b" * 64},
            "supported_decoders": ["envelope-matching"],
        }
        with tempfile.TemporaryDirectory() as temporary:
            marker = Path(temporary) / "verification.json"
            publication.write_verification_marker(result, marker)
            record = json.loads(marker.read_text(encoding="utf-8"))
        self.assertEqual(record["schema_version"], publication.VERIFICATION_SCHEMA)
        self.assertEqual(record["verification"], "pass")
        self.assertEqual(record["evidence_bundle"], result["evidence_bundle"])

    def test_marker_output_requires_release_verification_and_expected_decoder(self) -> None:
        with contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                publication.main(["--self-test", "--marker-out", "marker.json"])
            with self.assertRaises(SystemExit):
                publication.main(["--release-dir", "release", "--marker-out", "marker.json"])


if __name__ == "__main__":
    unittest.main()
