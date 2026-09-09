import { expect, test } from "@playwright/test";

const CIRCUIT_HEREDOC = `cat > circuit.stim <<'STIM'
R 0
X_ERROR(1) 0
M 0
DETECTOR rec[-1]
OBSERVABLE_INCLUDE(0) rec[-1]
STIM`;

async function codeToolbarFor(page, text) {
  const code = page.locator("pre").filter({ hasText: text });
  return code.locator("xpath=preceding-sibling::div[contains(@class, 'code-toolbar')][1]");
}

test("home leads to the Cargo installation path", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Install with Cargo" }).click();
  await expect(page).toHaveURL(/\/#install$/);
  await expect(page.getByRole("heading", { name: "Choose the path that fits your machine" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Cargo install" })).toBeVisible();
});

test("home offers Cargo and Shot Lab as primary destinations", async ({ page }) => {
  await page.goto("/");

  const actions = page.locator(".home-hero .actions a");
  await expect(actions).toHaveCount(2);
  expect(await actions.allTextContents()).toEqual([
    "Install with Cargo",
    "Try Shot Lab",
  ]);
  expect(await actions.evaluateAll((links) => links.map((link) => link.getAttribute("href")))).toEqual([
    "#install",
    "interactive/",
  ]);

  const destinations = await actions.evaluateAll((links) => links.map((link) => link.href));
  expect(new Set(destinations).size).toBe(2);
  await expect(page.locator('a[href="#install"]')).toHaveCount(1);
});

test("installation starts with one copyable command and keeps manual steps optional", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: async (text) => { window.__copiedText = text; } },
    });
  });
  await page.goto("/get-started/");
  const installation = page.locator('section[aria-labelledby="install"]');
  const native = installation.locator("#native-install");
  const manual = native.locator("details");
  await expect(manual).not.toHaveAttribute("open", "");
  await installation.getByRole("button", { name: "Copy Shell · install v0.3.0" }).click();
  await expect.poll(() => page.evaluate(() => window.__copiedText)).toBe(
    "cargo install --locked rustqec-cli --version 0.3.0",
  );
  await native.locator("summary").first().click();
  await expect(installation.getByRole("link", { name: "Inspect the installer" })).toHaveAttribute("href", "../install.sh");
  await manual.locator("summary").click();
  await expect(manual).toContainText("sha256sum -c");
});

test("copying preserves the complete circuit heredoc", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: async (text) => { window.__copiedText = text; } },
    });
  });
  await page.goto("/get-started/");
  const toolbar = await codeToolbarFor(page, "cat > circuit.stim");
  await toolbar.getByRole("button", { name: "Copy Shell · create input" }).click();
  await expect(toolbar.locator(".copy-status")).toHaveText("Copied");
  await expect.poll(() => page.evaluate(() => window.__copiedText)).toBe(CIRCUIT_HEREDOC);
});

test("copy failure gives a manual-copy response", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: async () => { throw new Error("blocked"); } },
    });
  });
  await page.goto("/get-started/");
  const toolbar = await codeToolbarFor(page, "cat > circuit.stim");
  await toolbar.getByRole("button", { name: "Copy Shell · create input" }).click();
  await expect(toolbar.locator(".copy-status")).toHaveText("Copy unavailable. Select the code and copy manually.");
});

test("navigation opens from the keyboard and Escape returns focus", async ({ page }) => {
  await page.goto("/");
  const guides = page.locator(".nav-group").filter({ hasText: "Guides" });
  const summary = guides.locator("summary");
  await summary.focus();
  await page.keyboard.press("Enter");
  await expect(guides).toHaveAttribute("open", "");
  await page.keyboard.press("Escape");
  await expect(guides).not.toHaveAttribute("open", "");
  await expect.poll(() => page.evaluate(() => document.activeElement?.tagName)).toBe("SUMMARY");
});

test("long protocol page supplies rendered content and usable table-of-contents anchors", async ({ page }) => {
  await page.goto("/qp101/protocol/");
  await expect(page.getByRole("heading", { level: 1, name: /QP101-ZY: Quantum Circuit JSON Format/ })).toBeVisible();
  await expect(page.locator("main")).not.toContainText("{{ load_data");
  await expect(page.locator(".page-toc")).toBeVisible();
  const anchor = page.locator('.page-toc a[href="#schema-identity"]');
  await expect(anchor).toHaveText("Schema identity");
  await anchor.click();
  await expect(page).toHaveURL(/#schema-identity$/);
  await expect(page.locator("#schema-identity")).toBeInViewport();
});

test("decoder evidence keeps provenance layered and never stringifies objects", async ({ page }) => {
  await page.goto("/decoding/");
  const evidence = page.locator(".decoder-evidence");
  await expect(evidence.locator(".evidence-provenance").first()).toBeAttached();
  await expect(evidence).not.toContainText("[object Object]");

  const provenance = evidence.locator(".evidence-provenance").first();
  await expect(provenance).not.toHaveAttribute("open", "");
  await provenance.locator("summary").click();
  await expect(provenance).toHaveAttribute("open", "");
  await expect(provenance.locator(".provenance-card-list .provenance-row").first()).toBeVisible();
  await expect(evidence.locator(".evidence-reproduction").first()).not.toHaveAttribute("open", "");
});

test("dynamic evidence commands copy executable source-checkout commands", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: async (text) => { window.__copiedText = text; } },
    });
  });
  await page.goto("/decoding/");
  const reproduction = page.locator(".evidence-reproduction").first();
  await reproduction.locator("summary").click();
  const toolbar = await codeToolbarFor(reproduction, "make surface-decoder-compare-full");
  await expect(toolbar).toBeVisible();
  await toolbar.getByRole("button", { name: "Copy Shell · source checkout" }).click();
  await expect.poll(() => page.evaluate(() => window.__copiedText)).toBe("make surface-decoder-compare-full\nmake bench-surface-full");
  await expect.poll(() => page.evaluate(() => window.__copiedText.includes("$"))).toBe(false);
});

test("captures the primary docs and Shot Lab surfaces for review", async ({ page }, testInfo) => {
  await page.goto("/");
  const home = testInfo.outputPath("docs-home.png");
  await page.screenshot({ path: home, fullPage: true });
  await testInfo.attach("docs-home", { path: home, contentType: "image/png" });
  await page.goto("/get-started/");
  const quickstart = testInfo.outputPath("docs-quickstart.png");
  await page.screenshot({ path: quickstart, fullPage: true });
  await testInfo.attach("docs-quickstart", { path: quickstart, contentType: "image/png" });
  await page.goto("/interactive/");
  await expect(page.getByRole("button", { name: "Sample", exact: true })).toBeVisible();
  const shotLab = testInfo.outputPath("docs-shot-lab.png");
  await page.screenshot({ path: shotLab, fullPage: true });
  await testInfo.attach("docs-shot-lab", { path: shotLab, contentType: "image/png" });
});

test("key documentation pages fit a 390px viewport without page overflow", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  for (const path of ["/", "/docs/", "/support/", "/sampling-data/", "/get-started/", "/decoding/", "/qp101/protocol/", "/interactive/"]) {
    await page.goto(path);
    await expect.poll(() => page.evaluate(() => ({ width: document.documentElement.scrollWidth, viewport: window.innerWidth }))).toEqual({ width: 390, viewport: 390 });
    for (const label of ["Guides", "Reference"]) {
      const menu = page.locator(".nav-group").filter({ hasText: label });
      await menu.locator("summary").click();
      const bounds = await menu.locator(".nav-menu").boundingBox();
      expect(bounds.x).toBeGreaterThanOrEqual(0);
      expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
      await menu.locator("summary").click();
    }
  }
});

test("search finds commands and concepts, preserves queries, and handles no matches", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("searchbox", { name: "Search documentation" }).fill("rmatching");
  await page.locator(".nav-search button").click();
  await expect(page).toHaveURL(/\/docs\/\?q=rmatching/);
  await expect(page.locator("#search-results")).toContainText("Quantum error correction decoders");
  const query = page.locator("#docs-query");
  await query.fill("b8");
  await expect(page.locator("#search-results")).toContainText("Sampling and training data");
  await query.fill("atom loss");
  await expect(page.locator("#search-results")).toContainText("Sampling and training data");
  await query.fill("zz-no-such-command-707");
  await expect(page.locator("#search-status")).toContainText("No matching pages");
  await expect(page.locator("#search-results li")).toHaveCount(0);
  await query.fill("   ");
  await expect(page.locator("#search-status")).toContainText("Browse the index below");
});

test("search failure keeps the reference index usable", async ({ page }) => {
  await page.route("**/data/docs-search.json", (route) => route.fulfill({ status: 503, body: "unavailable" }));
  await page.goto("/docs/?q=rmatching");
  await expect(page.locator("#search-status")).toContainText("Search is unavailable");
  await expect(page.locator('main a[href="../decoding/#first-decode"]').first()).toBeVisible();
});

test("protocol subsections have stable permalinks and active location feedback", async ({ page }) => {
  await page.goto("/qp101/protocol/");
  const group = page.locator(".toc-section").filter({ has: page.locator('a[href="#operation-model"]') });
  await group.locator("summary").click();
  await group.getByRole("link", { name: "noise", exact: true }).click();
  await expect(page).toHaveURL(/#noise$/);
  await expect(group.locator('a[href="#noise"]')).toHaveAttribute("aria-current", "location");
  await expect(page.locator('#noise .heading-anchor')).toHaveAttribute("href", /#noise$/);
});

test("output is labeled separately and never copied with the command", async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, "clipboard", { value: { writeText: async (text) => { window.__copiedText = text; } } }));
  await page.goto("/get-started/#detector-output");
  const section = page.locator('section[aria-labelledby="detector-output"]');
  await expect(section.locator(".output-toolbar")).toHaveText("Expected file contents");
  await expect(section.locator(".output-toolbar button")).toHaveCount(0);
  await section.getByRole("button", { name: "Copy Shell · installed CLI" }).click();
  await expect.poll(() => page.evaluate(() => window.__copiedText)).toContain("rustqec circuit detect");
  expect(await page.evaluate(() => window.__copiedText)).not.toContain("shot D0 L0");
});

test("development guides point to a master checkout and stable checkout is explicit", async ({ page }) => {
  await page.goto("/sampling-data/");
  await page.getByRole("link", { name: "configured repository checkout" }).click();
  await expect(page.locator('pre[data-language="Shell · development source"]')).toContainText("git clone --branch master");
  await page.locator("#stable-source summary").click();
  await expect(page.locator('pre[data-language="Shell · stable source"]')).toContainText("git clone --branch v0.3.0");
});

test("support table labels stay intact while the table, not the page, scrolls", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/support/");
  await expect(page.locator(".support-level").first()).toBeAttached();
  expect(await page.locator(".support-level").first().evaluate((cell) => getComputedStyle(cell).whiteSpace)).toBe("nowrap");
  const wrapper = page.locator(".table-wrap").filter({ has: page.locator(".support-level") }).first();
  expect(await wrapper.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390);
});

test("figures open in place, zoom, and return focus without losing the reading position", async ({ page }) => {
  await page.goto("/decoding/");
  const trigger = page.locator("[data-figure-viewer]").first();
  await trigger.scrollIntoViewIfNeeded();
  await trigger.click();
  const dialog = page.getByRole("dialog", { name: "Benchmark figure" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Zoom in", exact: true }).click();
  expect(await dialog.locator("img").evaluate((el) => el.clientWidth > el.parentElement.clientWidth)).toBe(true);
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  await expect(trigger).toBeFocused();
  await expect(page).toHaveURL(/\/decoding\/$/);
});

test("desktop Shot Lab shows selected event details alongside the circuit", async ({ page }) => {
  await page.setViewportSize({ width: 1212, height: 768 });
  await page.goto("/interactive/");
  const noise = page.locator("#shot-canvas .noise-site").first();
  await expect(noise).toBeVisible();
  const initialStage = await page.locator(".shot-stage-wrap").boundingBox();
  expect(initialStage.y + initialStage.height).toBeLessThanOrEqual(768);
  await noise.click();
  await expect(page.locator("#shot-detail")).toContainText("X_ERROR");
  for (const focused of [false, true]) {
    if (focused) await page.getByRole("button", { name: "Focus circuit", exact: true }).click();
    const stage = await page.locator(".shot-stage-wrap").boundingBox();
    const panel = await page.locator("#shot-detail").boundingBox();
    expect(panel.x).toBeGreaterThanOrEqual(stage.x + stage.width - 1);
    expect(Math.abs(panel.y - stage.y)).toBeLessThan(2);
    expect(panel.y + 120).toBeLessThan(768);
  }
});

for (const width of [768, 1050]) {
  test(`chapter destinations clear the sticky navigation at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await page.goto("/get-started/#source-build");
    const target = page.locator("#source-build");
    await expect(target).toBeInViewport({ ratio: 1 });
    await expect.poll(async () => {
      const heading = await target.boundingBox();
      const nav = await page.locator(".nav-shell").boundingBox();
      return heading.y - nav.y - nav.height;
    }).toBeGreaterThanOrEqual(0);
    const link = page.locator('.page-toc a[href="#first-circuit"]');
    await link.click();
    await expect(page).toHaveURL(/#first-circuit$/);
    await expect(page.locator("#first-circuit")).toBeInViewport({ ratio: 1 });
    await expect.poll(async () => {
      const heading = await page.locator("#first-circuit").boundingBox();
      const nav = await page.locator(".nav-shell").boundingBox();
      return heading.y - nav.y - nav.height;
    }).toBeGreaterThanOrEqual(0);
  });
}

test("highlighted decoder source is the exact downloadable runnable example", async ({ page, request }) => {
  const source = await request.get("/examples/first-decode/src/main.rs");
  expect(source.ok()).toBe(true);
  await page.goto("/decoding/");
  await expect(page.locator('pre[data-language="Rust"] code')).toHaveText(await source.text(), { useInnerText: false });
});
