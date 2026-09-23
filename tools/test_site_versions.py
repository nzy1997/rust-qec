"""Version publication must never advertise missing or mislabeled snapshots."""
import json
from pathlib import Path
import tempfile
import subprocess
import shutil
import unittest
from unittest import mock

from tools.site_versions import assemble, build_versions, read_catalog, write_indexes


class SiteVersionsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.catalog = {"default": "master", "versions": [
            {"id": "master", "label": "Development · master", "ref": "master", "path": ""},
            {"id": "v1", "label": "Release v1", "ref": "a" * 40, "path": "versions/v1",
             "channel": "stable", "release_line": "1", "native_release": "1.0.0"},
        ]}
        self.sites = {}
        for version in self.catalog["versions"]:
            site = self.root / version["id"]
            site.mkdir()
            home = f'<body data-docs-version="{version["id"]}" data-root="."><h1 id="top">Docs</h1></body>'
            nested = f'<body data-docs-version="{version["id"]}" data-root=".."><h1 id="top">Docs</h1></body>'
            (site / "index.html").write_text(home)
            (site / "docs").mkdir()
            (site / "docs/index.html").write_text(nested)
            (site / "search-index.json").write_text(version["id"])
            (site / "interactive").mkdir()
            (site / "interactive/engine.wasm").write_bytes(version["id"].encode())
            (site / "js").mkdir()
            (site / "js/versions.js").write_text(f"navigation for {version['id']}")
            (site / "versions.json").write_text(f"isolated {version['id']}")
            self.sites[version["id"]] = site

    def test_two_snapshots_keep_assets_search_and_routes_separate(self):
        output = self.root / "published"
        assemble(self.catalog, self.sites, output)
        for prefix, expected in [(output, "master"), (output / "versions/v1", "v1")]:
            self.assertEqual((prefix / "search-index.json").read_text(), expected)
            self.assertEqual((prefix / "interactive/engine.wasm").read_bytes(), expected.encode())
            index = json.loads((prefix / "versions.json").read_text())
            self.assertEqual(index["current"], expected)
            self.assertEqual(set(index["versions"][0]["pages"]), {"", "docs/"})
            self.assertEqual(index["versions"][0]["pages"]["docs/"], ["top"])
        index = json.loads((output / "versions/v1/versions.json").read_text())
        self.assertEqual(index["versions"][0]["root"], "../../")
        self.assertEqual((output / "versions/v1/js/versions.js").read_text(), "navigation for master")
        self.assertEqual((self.sites["master"] / "versions.json").read_text(), "isolated master")
        self.assertEqual((self.sites["v1"] / "versions.json").read_text(), "isolated v1")

    def test_frozen_home_receives_current_version_navigation(self):
        stable_home = self.sites["v1"] / "index.html"
        stable_home.write_text('<body data-docs-version="v1" data-root="."><div class="site-frame"></div></body>')
        output = self.root / "published"
        assemble(self.catalog, self.sites, output)
        html = (output / "versions/v1/index.html").read_text()
        self.assertIn('id="docs-version"', html)
        self.assertIn('Release v1', html)
        self.assertIn('./get-started/#versions', html)

    def test_every_frozen_page_receives_authoritative_stable_identity(self):
        output = self.root / "published"
        assemble(self.catalog, self.sites, output)
        for page in [output / "versions/v1/index.html", output / "versions/v1/docs/index.html"]:
            html = page.read_text()
            self.assertIn('id="docs-edition-notice"', html)
            self.assertIn("Frozen stable edition:", html)
            self.assertIn("RustQEC 1, coordinated release v1.0.0", html)

    def test_missing_snapshot_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Every advertised"):
            write_indexes(self.catalog, {"master": self.sites["master"]})

    def test_relabeling_master_as_release_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "does not match"):
            assemble(self.catalog, {"master": self.sites["master"], "v1": self.sites["master"]}, self.root / "out")

    def test_version_directory_cannot_overwrite_a_documentation_route(self):
        self.catalog["versions"][1]["path"] = "docs"
        with self.assertRaisesRegex(ValueError, "collides"):
            assemble(self.catalog, self.sites, self.root / "out")

    def test_output_cannot_be_nested_inside_a_snapshot(self):
        with self.assertRaisesRegex(ValueError, "outside"):
            assemble(self.catalog, self.sites, self.sites['master'] / 'published')

    def test_invalid_registration_is_rejected(self):
        config = self.root / "versions.json"
        for path in ["../escape", "/absolute", "versions/../escape"]:
            self.catalog["versions"][1]["path"] = path
            config.write_text(json.dumps(self.catalog))
            with self.assertRaises(ValueError):
                read_catalog(config)
        self.catalog["versions"][1]["path"] = "versions/v1"
        self.catalog["versions"][1]["ref"] = "master"
        config.write_text(json.dumps(self.catalog))
        with self.assertRaisesRegex(ValueError, "full commit SHA"):
            read_catalog(config)

    def test_production_registers_development_and_frozen_0_3_docs(self):
        catalog = read_catalog(Path(__file__).resolve().parent.parent / "site/versions.json")
        self.assertEqual([v["id"] for v in catalog["versions"]], ["master", "0.3"])
        stable = catalog["versions"][1]
        self.assertEqual(stable["path"], "versions/0.3")
        self.assertEqual(stable["ref"], "c4ca7e9c08f44ec2e09a280d0bcda9906cd0daa5")
        self.assertEqual(stable["channel"], "stable")
        self.assertEqual(stable["native_release"], "0.3.3")
        self.assertEqual(catalog["versions"][0]["native_release"], "0.3.3")

    def test_additional_edition_builds_from_its_pinned_commit(self):
        repo = self.root / "repository"
        (repo / "site").mkdir(parents=True)
        (repo / "tools").mkdir()
        # Use the actual benchmark provenance guard, not a stub for Git tracking.
        shutil.copyfile(Path(__file__).with_name('check_site_manifest.py'), repo / 'tools/check_site_manifest.py')
        (repo / "site/config.toml").write_text('base_url = "https://example.org/rust-qec"\n')
        (repo / "tools/site_versions.py").write_text('# fixture supports version metadata\n')
        (repo / "tools/check_site_build.py").write_text('from pathlib import Path\nassert Path("_site/index.html").is_file()\n')
        external_artifact = repo / "site/static/data/atom-loss/sampling.json"
        external_artifact.parent.mkdir(parents=True)
        external_artifact.write_text("external artifact")
        (repo / "Makefile").write_text('build-site:\n\t./build-fixture.py\n')
        builder = repo / "build-fixture.py"
        builder.write_text('''#!/usr/bin/env python3
import os
from pathlib import Path
from tools.check_site_manifest import path_is_tracked
assert path_is_tracked(Path.cwd(), 'source.txt'), 'Benchmark evidence must remain Git-tracked'
Path('untracked.txt').write_text('not evidence')
assert not path_is_tracked(Path.cwd(), 'untracked.txt')
root = Path('_site')
root.mkdir()
version = os.environ['DOCS_VERSION']
base = os.environ['DOCS_SITE_BASE_URL']
assert Path('site/static/data/atom-loss/sampling.json').read_text() == 'external artifact'
(root / 'index.html').write_text(f'<body data-docs-version="{version}" data-root="." id="top">{base}</body>')
(root / 'source.txt').write_text(Path('source.txt').read_text())
''')
        builder.chmod(0o755)
        (repo / "source.txt").write_text("release source")
        for command in [
            ["git", "init", "-q"], ["git", "add", "."],
            ["git", "-c", "user.name=Test", "-c", "user.email=test@example.org", "commit", "-qm", "fixture"],
        ]:
            subprocess.run(command, cwd=repo, check=True)
        sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
        (repo / "source.txt").write_text("uncommitted development source")
        self.catalog["versions"][1]["ref"] = sha
        config = repo / "site/versions.json"
        config.write_text(json.dumps(self.catalog))
        with mock.patch("tools.site_versions.REPO", repo):
            build_versions(config, self.sites["master"], self.root / "published")
        snapshot = self.root / "published/versions/v1"
        self.assertEqual((snapshot / "source.txt").read_text(), "release source")
        self.assertIn('https://example.org/rust-qec/versions/v1', (snapshot / 'index.html').read_text())


if __name__ == "__main__":
    unittest.main()
