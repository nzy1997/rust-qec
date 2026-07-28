#!/usr/bin/env python3
from __future__ import annotations

import json
import unittest
from unittest import mock

import tools.check_site_build as check_site_build

RSTIM_DEM_SUMMARY_PATH = "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/summary.json"


class SiteBuildCheckerTest(unittest.TestCase):
    def test_valid_site_does_not_require_benchmark_campaign_route(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        self.assertFalse((fixture.site_root / "benchmark-campaigns/index.html").exists())

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertFalse(
            any(result.status == "FAIL" for result in results),
            check_site_build.format_summary(results),
        )

    def test_self_test_exercises_required_mutations(self) -> None:
        self.assertEqual(check_site_build.run_self_test(), [])

    def test_self_test_requires_dedicated_provenance_failure(self) -> None:
        with mock.patch.object(
            check_site_build,
            "check_checked_provenance",
            return_value=check_site_build.pass_("checked benchmark provenance", "patched out"),
        ):
            failures = check_site_build.run_self_test()

        self.assertTrue(
            any("missing_surface_provenance" in failure for failure in failures),
            failures,
        )

    def test_valid_fixture_prints_required_pass_summary_areas(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)
        output = check_site_build.format_summary(results)

        self.assertNotIn("FAIL", output)
        for marker in [
            "PASS QP101 assets",
            "PASS site pages",
            "PASS benchmark methodology",
            "PASS checked benchmark artifacts",
            "PASS checked benchmark provenance",
            "PASS local-only/future classifications",
            "SUMMARY: PASS",
        ]:
            self.assertIn(marker, output)
        self.assertIn("surface-decoder-full", output)
        self.assertIn("bb-circuit-full", output)
        self.assertIn("rstim-vs-stim-full", output)
        self.assertIn("partial checked evidence", output)
        self.assertIn("not_recorded", output)
        self.assertIn("checked artifact hashes", output)

    def test_valid_fixture_includes_rsmp_showcase_pages_contract(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)

        index = (fixture.site_root / "index.html").read_text(encoding="utf-8")
        showcase = (fixture.site_root / "rsmp-v1-showcase/index.html").read_text(encoding="utf-8")
        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)
        summary = check_site_build.format_summary(results)

        self.assertNotIn("FAIL", summary)
        self.assertIn('href="rsmp-v1-showcase/"', index)
        for marker in [
            "RSMP v1",
            "11.98%",
            "57.14%",
            "pack_samples",
            "unpack_samples",
            "Projected",
            "requires the original circuit, not a DEM",
            "Sweep-bit circuits are unsupported",
            "non-hermetic checker behavior",
        ]:
            self.assertIn(marker, showcase)

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

    def test_rejects_checked_item_missing_built_manifest_provenance(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        manifest_path = fixture.site_root / "data/benchmark-site.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        del manifest["families"][0]["evidence_items"][0]["provenance"]
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "checked benchmark provenance"
                and "surface-decoder-full" in result.detail
                and "provenance" in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )

    def test_rejects_missing_rstim_vs_stim_checked_artifact(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        artifact_path = "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json"
        (fixture.site_root / artifact_path).unlink()

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "checked benchmark artifacts"
                and "correctness-summary.json" in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )

    def test_rejects_missing_copied_rstim_dem_summary(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        (fixture.site_root / RSTIM_DEM_SUMMARY_PATH).unlink()

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "checked benchmark artifacts"
                and RSTIM_DEM_SUMMARY_PATH in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )

    def test_source_doc_readme_links_are_not_checked_artifact_references(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index_path = fixture.site_root / "index.html"
        index_path.write_text(
            index_path.read_text(encoding="utf-8")
            + '<a href="https://github.com/nzy1997/rstim/blob/master/benchmarks/surface_decoder_compare/README.md">surface docs</a>\n'
            + '<a href="https://github.com/nzy1997/rstim/blob/master/benchmarks/rstim_vs_stim_simulator/README.md">rstim docs</a>\n',
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertFalse(
            any(
                result.status == "FAIL"
                and result.area == "checked benchmark artifacts"
                and "README.md" in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )

    def test_rejects_copied_checked_artifact_hash_mutation(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        artifact_path = "benchmarks/surface_decoder_compare/results/full/results.csv"
        (fixture.site_root / artifact_path).write_text("mutated copied artifact\n", encoding="utf-8")

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "checked benchmark provenance"
                and artifact_path in result.detail
                and "sha256" in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )

    def test_invalid_app_js_blocks_provenance_pass(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        (fixture.site_root / "js/benchmarks.js").write_bytes(b"\xff\xfe\xfa")

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)
        summary = check_site_build.format_summary(results)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "checked benchmark provenance"
                and "provenance" in result.detail
                and "js/benchmarks.js" in result.detail
                for result in results
            ),
            summary,
        )
        self.assertNotIn("PASS checked benchmark provenance", summary)

    def test_rejects_missing_distributed_evidence_boundary(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "decoding/index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                "checked full artifacts support", "checked full artifacts describe"
            ),
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and "benchmark methodology" in result.area
                and "checked full artifacts support" in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )

    def test_rejects_html_reference_that_escapes_site_root(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        outside = fixture.repo_root / "outside.txt"
        outside.write_text("outside\n", encoding="utf-8")
        index = fixture.site_root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace('href="simulator/"', 'href="../outside.txt"', 1),
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)
        summary = check_site_build.format_summary(results)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "site pages"
                and "../outside.txt" in result.detail
                and "escape" in result.detail
                for result in results
            ),
            summary,
        )

    def test_rejects_js_reference_that_escapes_site_root(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        outside = fixture.repo_root / "outside.txt"
        outside.write_text("outside\n", encoding="utf-8")
        app = fixture.site_root / "js/benchmarks.js"
        app.write_text(app.read_text(encoding="utf-8") + '\nconst escaped = "../../outside.txt";\n', encoding="utf-8")

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)
        summary = check_site_build.format_summary(results)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "site pages"
                and "../../outside.txt" in result.detail
                and "escape" in result.detail
                for result in results
            ),
            summary,
        )

    def test_missing_index_or_app_returns_fail_summary_instead_of_raising(self) -> None:
        for relative in ("index.html", "simulator/index.html", "js/benchmarks.js"):
            with self.subTest(relative=relative):
                fixture = check_site_build.make_fixture_site()
                self.addCleanup(fixture.cleanup)
                (fixture.site_root / relative).unlink()

                results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)
                summary = check_site_build.format_summary(results)

                self.assertIn("SUMMARY: FAIL", summary)
                self.assertTrue(
                    any(result.status == "FAIL" and relative in result.detail for result in results),
                    summary,
                )

    def test_invalid_utf8_returns_fail_summary_instead_of_raising(self) -> None:
        for relative in ("index.html", "js/benchmarks.js", "data/benchmark-site.json"):
            with self.subTest(relative=relative):
                fixture = check_site_build.make_fixture_site()
                self.addCleanup(fixture.cleanup)
                (fixture.site_root / relative).write_bytes(b"\xff\xfe\xfa")

                results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)
                summary = check_site_build.format_summary(results)

                self.assertIn("SUMMARY: FAIL", summary)
                self.assertTrue(
                    any(result.status == "FAIL" and relative in result.detail for result in results),
                    summary,
                )

    def test_rejects_missing_evidence_phrase_even_if_manifest_keeps_it(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "simulator/index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                "not a general performance claim", "not a universal speed ranking"
            ),
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and "benchmark methodology" in result.area
                and "not a general performance claim" in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )

    def test_rejects_unmanifested_checked_artifact_link(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "decoding/index.html"
        index.write_text(
            index.read_text(encoding="utf-8")
            + '<a href="../benchmarks/surface_decoder_compare/results/full/not-in-manifest.csv">bad</a>\n',
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any("not listed as a checked manifest artifact" in result.detail for result in results),
            check_site_build.format_summary(results),
        )

    def test_rejects_unmanifested_rstim_dem_artifact_link(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        missing_artifact = (
            "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/not-in-manifest.json"
        )
        index = fixture.site_root / "simulator/index.html"
        index.write_text(
            index.read_text(encoding="utf-8") + f'<a href="../{missing_artifact}">bad</a>\n',
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "checked benchmark artifacts"
                and missing_artifact in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )

    def test_rejects_root_absolute_html_href(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace('href="simulator/"', 'href="/simulator/"', 1),
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "site pages"
                and "root-absolute" in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )

    def test_rejects_broken_cross_page_anchor(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                'href="simulator/#circuit-simulation"', 'href="simulator/#missing-anchor"', 1
            ),
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "site pages"
                and "missing-anchor" in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )


if __name__ == "__main__":
    unittest.main()
