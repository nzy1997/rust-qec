#!/usr/bin/env python3
from __future__ import annotations

import unittest

import tools.check_site_build as check_site_build


class SiteBuildCheckerTest(unittest.TestCase):
    def test_self_test_exercises_required_mutations(self) -> None:
        self.assertEqual(check_site_build.run_self_test(), [])

    def test_valid_fixture_prints_required_pass_summary_areas(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)
        output = check_site_build.format_summary(results)

        self.assertNotIn("FAIL", output)
        for marker in [
            "PASS QP101 assets",
            "PASS workspace overview",
            "PASS benchmark methodology",
            "PASS checked benchmark artifacts",
            "PASS local-only/future classifications",
            "SUMMARY: PASS",
        ]:
            self.assertIn(marker, output)

    def test_rejects_missing_qp101_schema(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        (fixture.site_root / "qp101.schema.json").unlink()

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(result.status == "FAIL" and "QP101" in result.area and "qp101.schema.json" in result.detail for result in results),
            check_site_build.format_summary(results),
        )

    def test_rejects_missing_checked_benchmark_plot(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        (fixture.site_root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").unlink()

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any("checked benchmark artifacts" in result.area and "surface_decoder_compare.png" in result.detail for result in results),
            check_site_build.format_summary(results),
        )

    def test_rejects_missing_claims_policy_caveat(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "index.html"
        index.write_text(index.read_text(encoding="utf-8").replace("Claims Policy", "Claims"), encoding="utf-8")

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(result.status == "FAIL" and "benchmark methodology" in result.area and "Claims Policy" in result.detail for result in results),
            check_site_build.format_summary(results),
        )

    def test_rejects_unmanifested_checked_artifact_link(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8")
            + '<a href="benchmarks/surface_decoder_compare/results/full/not-in-manifest.csv">bad</a>\n',
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any("not listed as a checked manifest artifact" in result.detail for result in results),
            check_site_build.format_summary(results),
        )


if __name__ == "__main__":
    unittest.main()
