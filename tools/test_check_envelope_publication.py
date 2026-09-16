#!/usr/bin/env python3
"""Offline regression tests for the envelope publication verifier."""

from __future__ import annotations

import copy
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

    def test_published_support_identity_is_bound_to_the_scope_plan(self) -> None:
        matrix = json.loads(
            (publication.REPO_ROOT / "docs/envelope-support.json").read_text(encoding="utf-8")
        )
        plan = publication.mle_scope.load_plan(
            publication.REPO_ROOT / "docs/envelope-mle-scope.json"
        )
        gate = {
            "decoders": {
                "envelope-matching": {"decision": "supported"},
                "envelope-mle": {"decision": "supported"},
            }
        }
        publication.check_published_support_bindings("v0.3.2", matrix, gate, plan)

        bad_asset = copy.deepcopy(matrix)
        bad_asset["decoders"]["envelope-mle"]["published_support"][
            "evidence_asset"
        ] = "stale.tar.gz"
        with self.assertRaises(publication.PublicationError):
            publication.check_published_support_bindings("v0.3.2", bad_asset, gate, plan)

        bad_plan = copy.deepcopy(plan)
        bad_plan["maturity"]["promotion_release"] = "v0.3.1"
        with self.assertRaises(publication.PublicationError):
            publication.check_published_support_bindings("v0.3.2", matrix, gate, bad_plan)


if __name__ == "__main__":
    unittest.main()
