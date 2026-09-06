#!/usr/bin/env python3
from __future__ import annotations

import shutil
import subprocess
import textwrap
import unittest
from pathlib import Path


class SiteAppRenderingTest(unittest.TestCase):
    @unittest.skipUnless(shutil.which("node"), "node is required to execute site/static/js/benchmarks.js")
    def test_checked_result_cards_render_manifest_provenance(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        script = textwrap.dedent(
            r"""
            const fs = require("fs");
            const vm = require("vm");

            const appJs = fs.readFileSync("site/static/js/benchmarks.js", "utf8");
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
              const checkedCards = makeElement("checked-benchmark-result-cards");
              checkedCards.dataset = { evidenceItems: "surface-decoder-full bb-circuit-full" };
              const document = {
                body: { dataset: { root: "." } },
                getElementById(id) {
                  if (!elements.has(id)) {
                    elements.set(id, makeElement(id));
                  }
                  return elements.get(id);
                },
                querySelectorAll(selector) {
                  return selector === "[data-evidence-items]" ? [checkedCards] : [];
                },
                createElement(tagName) {
                  return makeElement(tagName);
                },
              };
              const fetch = (url) => {
                const fixture = url.endsWith("data/benchmark-site.json") ? manifestFixture : schemaFixture();
                const promise = Promise.resolve({
                  ok: true,
                  json: () => Promise.resolve(fixture),
                });
                fetchPromises.push(promise);
                return promise;
              };
              vm.runInNewContext(appJs, { document, fetch }, { filename: "site/static/js/benchmarks.js" });
              await Promise.all(fetchPromises);
              await new Promise((resolve) => setImmediate(resolve));
              return checkedCards.innerHTML;
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

            function assertExcludes(html, unexpected) {
              if (html.includes(unexpected)) {
                throw new Error(`rendered HTML unexpectedly included ${JSON.stringify(unexpected)}`);
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
              const bbExternalCommit = bbItem.provenance.external_repository_commits.value[0];
              for (const expected of [
                "<h4>Provenance</h4>",
                "<summary>Reproduce this result</summary>",
                "<summary>Full provenance and sources</summary>",
                "<strong>Evidence status:</strong>",
                "Checked artifacts are available for this full benchmark run.",
                "The broader benchmark family has partial checked coverage.",
                'data-family-status="partial"',
                'data-item-status="existing"',
                'data-evidence-tier="full"',
                "Open the full-size figure to inspect its axes, labels, and legend.",
                "Checked figure for Checked BB72/BB144 full artifacts.",
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
                bbItem.claims_limit,
                bbItem.caveats[0],
                "benchmarks/bb_circuit_bposd_compare/README.md",
                bbExternalCommit.repository,
                bbExternalCommit.commit,
                bbExternalCommit.role,
                `https://github.com/${bbExternalCommit.repository}`,
                `https://github.com/${bbExternalCommit.repository}/commit/${bbExternalCommit.commit}`,
              ]) {
                assertIncludes(html, expected);
              }
              for (const unexpected of [
                "[object Object]",
                "family: partial",
                "status: existing",
                "tier: full",
                "Reproduce and inspect",
              ]) {
                assertExcludes(html, unexpected);
              }

              const mutatedManifest = JSON.parse(JSON.stringify(manifest));
              evidenceItem(mutatedManifest, "surface-decoder-full").provenance.artifact_hashes = {
                status: "not_recorded",
                reason: "hashes were not captured",
              };
              const mutatedHtml = await renderCheckedCards(mutatedManifest);
              assertIncludes(mutatedHtml, "hashes were not captured");

              const objectManifest = JSON.parse(JSON.stringify(manifest));
              evidenceItem(objectManifest, "bb-circuit-full").provenance.external_repository_commits.value = {
                repository: "example/singular-reference",
                commit: "abcdef0123456789",
                role: "single object fixture",
              };
              const objectHtml = await renderCheckedCards(objectManifest);
              assertIncludes(
                objectHtml,
                "https://github.com/example/singular-reference/commit/abcdef0123456789"
              );
              assertIncludes(objectHtml, "single object fixture");
              assertExcludes(objectHtml, "[object Object]");

              const statusManifest = JSON.parse(JSON.stringify(manifest));
              const partialItem = evidenceItem(statusManifest, "bb-circuit-full");
              partialItem.status = "partial";
              partialItem.tier = "regression-gate";
              const localFamily = statusManifest.families.find((family) =>
                (family.evidence_items || []).some((item) => item.id === "surface-decoder-full")
              );
              localFamily.status = "local-only";
              const localItem = evidenceItem(statusManifest, "surface-decoder-full");
              localItem.status = "local-only";
              localItem.tier = "smoke";
              const statusHtml = await renderCheckedCards(statusManifest);
              assertIncludes(
                statusHtml,
                "Checked artifacts cover part of this regression gate; the claims limit below defines its boundary."
              );
              assertIncludes(
                statusHtml,
                "This quick smoke run is a local workflow; it does not provide checked site artifacts."
              );
              assertIncludes(
                statusHtml,
                "The broader benchmark family is documented for local runs only."
              );

              const nestedManifest = JSON.parse(JSON.stringify(manifest));
              const nestedItem = evidenceItem(nestedManifest, "bb-circuit-full");
              nestedItem.provenance.external_repository_commits.value = [
                {
                  repository: "example/reference",
                  commit: "0123456789abcdef",
                  role: "comparison <contract>",
                  details: {
                    branches: ["main", { label: "reviewed & pinned" }],
                  },
                },
                {
                  role: "identity intentionally missing",
                },
              ];
              nestedItem.provenance.nested_fixture = {
                status: "recorded",
                value: [{ platform: "<linux>", flags: ["--locked", { mode: "safe & exact" }] }],
              };
              const nestedHtml = await renderCheckedCards(nestedManifest);
              for (const expected of [
                "https://github.com/example/reference/commit/0123456789abcdef",
                "comparison &lt;contract&gt;",
                "reviewed &amp; pinned",
                "identity intentionally missing",
                "not recorded",
                "&lt;linux&gt;",
                "safe &amp; exact",
              ]) {
                assertIncludes(nestedHtml, expected);
              }
              for (const unexpected of ["[object Object]", "<linux>", "safe & exact"] ) {
                assertExcludes(nestedHtml, unexpected);
              }
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

    @unittest.skipUnless(shutil.which("node"), "node is required to execute site/static/js/qp101-browser.js")
    def test_schema_browser_renders_loaded_status_and_detail(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        script = textwrap.dedent(
            r"""
            const fs = require("fs");
            const vm = require("vm");

            const browserJs = fs.readFileSync("site/static/js/qp101-browser.js", "utf8");

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

            async function renderSchemaBrowser() {
              const elements = new Map();
              const fetchPromises = [];
              const document = {
                body: { dataset: { root: "." } },
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
                if (!url.endsWith("qp101.schema.json")) {
                  throw new Error(`unexpected fetch url ${url}`);
                }
                const promise = Promise.resolve({
                  ok: true,
                  json: () => Promise.resolve(schemaFixture()),
                });
                fetchPromises.push(promise);
                return promise;
              };
              vm.runInNewContext(
                browserJs,
                { document, fetch },
                { filename: "site/static/js/qp101-browser.js" }
              );
              await Promise.all(fetchPromises);
              await new Promise((resolve) => setImmediate(resolve));
              await new Promise((resolve) => setImmediate(resolve));
              return elements;
            }

            function assertEqual(actual, expected, message) {
              if (actual !== expected) {
                throw new Error(`${message}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
              }
            }

            function assertIncludes(html, expected) {
              if (!html.includes(expected)) {
                throw new Error(`rendered HTML did not include ${JSON.stringify(expected)}`);
              }
            }

            (async () => {
              const elements = await renderSchemaBrowser();
              assertEqual(elements.get("schema-status").textContent, "Loaded", "schema-status textContent");
              assertIncludes(elements.get("schema-detail").innerHTML, "QP101 test schema");
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
