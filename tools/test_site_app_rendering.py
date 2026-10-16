#!/usr/bin/env python3
from __future__ import annotations

import shutil
import subprocess
import textwrap
import unittest
from pathlib import Path


class SiteAppRenderingTest(unittest.TestCase):
    @unittest.skipUnless(shutil.which("node"), "node is required to execute site/app.js")
    def test_checked_result_cards_render_manifest_provenance(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        script = textwrap.dedent(
            r"""
            const fs = require("fs");
            const vm = require("vm");

            const appJs = fs.readFileSync("site/app.js", "utf8");
            const manifest = JSON.parse(fs.readFileSync("site/benchmark-site.json", "utf8"));

            function makeElement(name) {
              return {
                name,
                children: [],
                className: "",
                innerHTML: "",
                textContent: "",
                type: "",
                classList: {
                  add() {},
                  remove() {},
                },
                appendChild(child) {
                  this.children.push(child);
                },
                addEventListener(eventName, handler) {
                  if (eventName === "click") {
                    this.click = handler;
                  }
                },
                click() {},
              };
            }

            function schemaFixture() {
              return {
                type: "object",
                description: "QP101 test schema",
                $defs: {},
              };
            }

            async function renderCheckedCards(manifestFixture) {
              const elements = new Map();
              const fetchPromises = [];
              const document = {
                getElementById(id) {
                  if (!elements.has(id)) {
                    elements.set(id, makeElement(id));
                  }
                  return elements.get(id);
                },
                createElement(tagName) {
                  return makeElement(tagName);
                },
              };
              const fetch = (url) => {
                const fixture = url === "data/benchmark-site.json" ? manifestFixture : schemaFixture();
                const promise = Promise.resolve({
                  ok: true,
                  json: () => Promise.resolve(fixture),
                });
                fetchPromises.push(promise);
                return promise;
              };
              vm.runInNewContext(appJs, { document, fetch }, { filename: "site/app.js" });
              await Promise.all(fetchPromises);
              await new Promise((resolve) => setImmediate(resolve));
              return elements.get("checked-benchmark-result-cards").innerHTML;
            }

            function evidenceItem(manifestFixture, itemId) {
              for (const family of manifestFixture.families) {
                for (const item of family.evidence_items || []) {
                  if (item.id === itemId) {
                    return item;
                  }
                }
              }
              throw new Error(`${itemId} not found`);
            }

            function assertIncludes(html, expected) {
              if (!html.includes(expected)) {
                throw new Error(`rendered HTML did not include ${JSON.stringify(expected)}`);
              }
            }

            (async () => {
              const html = await renderCheckedCards(manifest);
              const surfaceItem = evidenceItem(manifest, "surface-decoder-full");
              const artifactHashes = surfaceItem.provenance.artifact_hashes.value;
              const artifactPath = Object.keys(artifactHashes)[0];
              const sha256 = artifactHashes[artifactPath].sha256;
              const bbItem = evidenceItem(manifest, "bb-circuit-full");
              const bbArtifactHashes = bbItem.provenance.artifact_hashes.value;
              const bbArtifactPath = Object.keys(bbArtifactHashes).find((path) =>
                path.includes("reference_gap_report.md")
              );
              if (!bbArtifactPath) {
                throw new Error("BB reference gap report hash not found");
              }
              const bbSha256 = bbArtifactHashes[bbArtifactPath].sha256;
              for (const expected of [
                "<h4>Provenance</h4>",
                "artifact_hashes",
                "recorded",
                "not_recorded",
                "historical checked artifact predates canonical provenance capture",
                "checked artifact hashes recorded",
                artifactPath,
                sha256,
                "make surface-decoder-compare-full",
                bbArtifactPath,
                bbSha256,
                "make bb-circuit-bposd-compare-full",
              ]) {
                assertIncludes(html, expected);
              }

              const mutatedManifest = JSON.parse(JSON.stringify(manifest));
              evidenceItem(mutatedManifest, "surface-decoder-full").provenance.artifact_hashes = {
                status: "not_recorded",
                reason: "hashes were not captured",
              };
              const mutatedHtml = await renderCheckedCards(mutatedManifest);
              assertIncludes(mutatedHtml, "hashes were not captured");
            })().catch((error) => {
              console.error(error && error.stack ? error.stack : error);
              process.exit(1);
            });
            """
        )

        result = subprocess.run(
            ["node", "-e", script],
            cwd=repo_root,
            text=True,
            capture_output=True,
            check=False,
        )

        self.assertEqual(
            result.returncode,
            0,
            f"node stdout:\n{result.stdout}\nnode stderr:\n{result.stderr}",
        )


if __name__ == "__main__":
    unittest.main()
