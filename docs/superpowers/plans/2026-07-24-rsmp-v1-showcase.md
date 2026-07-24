# RSMP v1 Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and publish a dedicated Sites webpage that introduces RSMP v1, demonstrates the real CLI workflow, presents measured compression evidence, and estimates large-scale storage/runtime from pinned data.

**Architecture:** Create a separate vinext Sites project at `sites/rsmp-v1-showcase/`. Keep all measured numbers in one shared evidence module, render one accessible route, and validate the numeric contract with Node tests plus the production build.

**Tech Stack:** Sites vinext starter, Next 16, React 19, TypeScript, CSS, Node's built-in test runner, and one project-local Open Graph image.

## Global Constraints

- Use the approved design in `docs/superpowers/specs/2026-07-24-rsmp-v1-showcase-design.md`.
- The site lives at `sites/rsmp-v1-showcase/`.
- The page has one route and no persistence, authentication, uploads, runtime environment variables, analytics, or external data dependency.
- RSMP unpack and verify require the original circuit, not a DEM.
- Sweep-bit circuits are unsupported in v1.
- Archives are sequential and do not provide random shot access.
- Present large-scale storage and runtime as `Projected`, not measured.
- Do not claim a universal compression ratio.
- Do not claim the ordinary working-tree readiness command passes while the historical binary-path defect remains.
- Publish the validated version using Sites.

---

## File Structure

- Create `sites/rsmp-v1-showcase/` with the Sites vinext starter.
- Create `sites/rsmp-v1-showcase/lib/rsmpEvidence.mjs` for the single evidence object, ratio math, projection math, and formatters used by both page and tests.
- Modify `sites/rsmp-v1-showcase/app/page.tsx` to render the showcase, terminal tabs, compression comparison, estimator, and evidence/caveat sections.
- Modify `sites/rsmp-v1-showcase/app/layout.tsx` for site metadata, Open Graph metadata, and favicon metadata.
- Modify `sites/rsmp-v1-showcase/app/globals.css` for the visual system and responsive layout.
- Replace `sites/rsmp-v1-showcase/tests/rendered-html.test.mjs` with server-rendered contract checks for the finished page.
- Create `sites/rsmp-v1-showcase/tests/rsmp-evidence.test.mjs` for projection and formatting checks.
- Create `sites/rsmp-v1-showcase/public/og.png` after the page direction is stable.

### Task 1: Initialize the Sites Project

**Files:**
- Create: `sites/rsmp-v1-showcase/**`

**Interfaces:**
- Consumes: empty `sites/rsmp-v1-showcase/` directory.
- Produces: vinext project with `package.json`, `app/page.tsx`, `app/layout.tsx`, `app/globals.css`, `tests/rendered-html.test.mjs`, and `.openai/hosting.json`.

- [ ] **Step 1: Run the Sites initializer once**

```bash
/Users/nzy/.codex/plugins/cache/openai-bundled/sites/0.1.30/scripts/init-site.sh sites/rsmp-v1-showcase
```

Expected: `npm ci` completes and the starter files exist.

- [ ] **Step 2: Inspect required starter files**

```bash
sed -n '1,220p' sites/rsmp-v1-showcase/app/page.tsx
sed -n '1,220p' sites/rsmp-v1-showcase/app/layout.tsx
sed -n '1,220p' sites/rsmp-v1-showcase/app/globals.css
sed -n '1,120p' sites/rsmp-v1-showcase/.openai/hosting.json
```

Expected: starter skeleton is present and `.openai/hosting.json` has no `project_id` yet.

### Task 2: Evidence Model and Projection Tests

**Files:**
- Create: `sites/rsmp-v1-showcase/lib/rsmpEvidence.mjs`
- Create: `sites/rsmp-v1-showcase/tests/rsmp-evidence.test.mjs`

**Interfaces:**
- Produces: `MEASURED_CASE`, `HIGH_ENTROPY_CONTROL`, `READINESS_DISCLOSURE`, `projectShots(shots)`, `formatBytes(bytes)`, `formatDuration(seconds)`, and `formatPercent(numerator, denominator, digits)`.

- [ ] **Step 1: Write the failing evidence tests**

```javascript
import assert from "node:assert/strict";
import test from "node:test";
import {
  HIGH_ENTROPY_CONTROL,
  MEASURED_CASE,
  formatBytes,
  formatDuration,
  formatPercent,
  projectShots,
} from "../lib/rsmpEvidence.mjs";

test("keeps the pinned measured RSMP compression values exact", () => {
  assert.equal(MEASURED_CASE.rawB8Bytes, 1552384);
  assert.equal(MEASURED_CASE.directZstdBytes, 325565);
  assert.equal(MEASURED_CASE.rsmpBytes, 186028);
  assert.equal(formatPercent(MEASURED_CASE.rsmpBytes, MEASURED_CASE.rawB8Bytes, 2), "11.98%");
  assert.equal(formatPercent(MEASURED_CASE.rsmpBytes, MEASURED_CASE.directZstdBytes, 2), "57.14%");
  assert.equal(HIGH_ENTROPY_CONTROL.rsmpBytes, 1049064);
  assert.equal(formatPercent(HIGH_ENTROPY_CONTROL.rsmpBytes, HIGH_ENTROPY_CONTROL.rawB8Bytes, 4), "100.0465%");
});

test("projects large shot counts from the 1024-shot evidence only", () => {
  const oneMillion = projectShots(1_000_000);
  assert.equal(formatBytes(oneMillion.rawBytes), "1.516 GB");
  assert.equal(formatBytes(oneMillion.rsmpBytes), "181.668 MB");
  assert.equal(formatDuration(oneMillion.packSeconds), "1m 38s");

  const hundredMillion = projectShots(100_000_000);
  assert.equal(formatBytes(hundredMillion.rsmpBytes), "18.167 GB");
  assert.equal(formatDuration(hundredMillion.packSeconds), "2h 43m");

  const billion = projectShots(1_000_000_000);
  assert.equal(formatBytes(billion.rawBytes), "1.516 TB");
  assert.equal(formatBytes(billion.rsmpBytes), "181.668 GB");
  assert.equal(formatDuration(billion.unpackSeconds), "11h 19m");
});

test("rejects invalid estimator input without NaN or infinity", () => {
  for (const bad of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.throws(() => projectShots(bad), /positive finite shot count/);
  }
});
```

- [ ] **Step 2: Run the test and watch it fail**

```bash
cd sites/rsmp-v1-showcase && node --test tests/rsmp-evidence.test.mjs
```

Expected: FAIL because `../lib/rsmpEvidence.mjs` does not exist.

- [ ] **Step 3: Implement the evidence module**

Implement exact evidence constants, projection math from the approved formulas, decimal byte formatting, duration formatting, and finite positive shot-count validation.

- [ ] **Step 4: Run the evidence tests again**

```bash
cd sites/rsmp-v1-showcase && node --test tests/rsmp-evidence.test.mjs
```

Expected: PASS.

### Task 3: Showcase Page and Render Contract

**Files:**
- Modify: `sites/rsmp-v1-showcase/app/page.tsx`
- Modify: `sites/rsmp-v1-showcase/app/layout.tsx`
- Modify: `sites/rsmp-v1-showcase/app/globals.css`
- Delete: `sites/rsmp-v1-showcase/app/_sites-preview/SkeletonPreview.tsx`
- Delete: `sites/rsmp-v1-showcase/app/_sites-preview/preview.css`
- Modify: `sites/rsmp-v1-showcase/package.json`
- Modify: `sites/rsmp-v1-showcase/package-lock.json`
- Modify: `sites/rsmp-v1-showcase/tests/rendered-html.test.mjs`

**Interfaces:**
- Consumes: evidence exports from `lib/rsmpEvidence.mjs`.
- Produces: one finished route with copyable Pack, Verify, and Unpack command tabs; zero-based compression bars; projected estimator; evidence links; and exact caveats.

- [ ] **Step 1: Write the failing rendered-page test**

```javascript
import assert from "node:assert/strict";
import test from "node:test";

async function render() {
  const workerUrl = new URL("../dist/server/index.js", import.meta.url);
  workerUrl.searchParams.set("test", `${process.pid}-${Date.now()}`);
  const { default: worker } = await import(workerUrl.href);
  return worker.fetch(new Request("http://localhost/", { headers: { accept: "text/html" } }), {
    ASSETS: { fetch: async () => new Response("Not found", { status: 404 }) },
  }, {
    waitUntil() {},
    passThroughOnException() {},
  });
}

test("server-renders the RSMP v1 showcase contract", async () => {
  const response = await render();
  assert.equal(response.status, 200);
  const html = await response.text();
  assert.match(html, /RSMP v1/);
  assert.match(html, /11\.98%/);
  assert.match(html, /57\.14%/);
  assert.match(html, /pack_samples/);
  assert.match(html, /unpack_samples/);
  assert.match(html, /verify_only/);
  assert.match(html, /Projected/);
  assert.match(html, /requires the original circuit, not a DEM/i);
  assert.match(html, /Sweep-bit circuits are unsupported/i);
  assert.match(html, /clean exported checkout/i);
  assert.doesNotMatch(html, /codex-preview|Building your site|react-loading-skeleton/);
});
```

- [ ] **Step 2: Run the rendered-page test through the build and watch it fail**

```bash
cd sites/rsmp-v1-showcase && npm test
```

Expected: FAIL because the starter skeleton still renders.

- [ ] **Step 3: Implement the finished page**

Build the page with the approved narrative sections, use semantic HTML, accessible buttons/tabs, local scrolling for command blocks, labels for `Measured` and `Projected`, and no decorative SVG illustration. Remove `react-loading-skeleton` from dependencies after deleting the starter preview component.

- [ ] **Step 4: Run the rendered-page and evidence tests**

```bash
cd sites/rsmp-v1-showcase && npm test
```

Expected: PASS.

### Task 4: Social Card, Build, and Local Verification

**Files:**
- Create: `sites/rsmp-v1-showcase/public/og.png`
- Modify: `sites/rsmp-v1-showcase/app/layout.tsx`

**Interfaces:**
- Consumes: stable headline, evidence metrics, and site palette.
- Produces: a bespoke Open Graph image referenced by site metadata, or no `og:image` if the one generated image is unusable.

- [ ] **Step 1: Generate exactly one social-preview image**

Use the built-in image generation tool with a prompt for a landscape RSMP v1 social card using the site's evidence, palette, and exact title text.

- [ ] **Step 2: Inspect and persist the image**

If the image text is legible and not invented, copy it to `sites/rsmp-v1-showcase/public/og.png` and wire Open Graph/X metadata. If it is unusable after one retry, omit `og:image`.

- [ ] **Step 3: Run the final production build**

```bash
cd sites/rsmp-v1-showcase && npm run build
```

Expected: PASS.

- [ ] **Step 4: Run the full site test command**

```bash
cd sites/rsmp-v1-showcase && npm test
```

Expected: PASS.

### Task 5: Publish with Sites

**Files:**
- Modify: `sites/rsmp-v1-showcase/.openai/hosting.json`

**Interfaces:**
- Consumes: the exact validated source tree and successful build output.
- Produces: a deployed private Sites URL.

- [ ] **Step 1: Create the Sites project once**

Call the Sites connector once because `.openai/hosting.json` has no existing `project_id`, then persist the returned `project_id`.

- [ ] **Step 2: Commit and push the exact validated source**

Commit only the plan and site source changes, push the branch with the returned source credential, and use the pushed branch-head SHA as `commit_sha`.

- [ ] **Step 3: Package and save a version**

Use the Sites package helper for `sites/rsmp-v1-showcase/`, then save exactly one version with the connector.

- [ ] **Step 4: Deploy privately and inspect status**

Deploy the saved version privately, poll until success or failure, and open the successful deployed URL in Codex.

## Self-Review

- Spec coverage: covered first-viewport conclusion, transform explanation, CLI workflow, measured compression, high-entropy control, estimator, evidence/caveats, accessibility, build validation, and publishing.
- Placeholder scan: no `TBD`, `TODO`, or undefined task handoffs remain.
- Type consistency: the evidence module exports used by tests and page are named consistently in Task 2 and Task 3.
