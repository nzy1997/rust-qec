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

test("home leads to the installed first-circuit path", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Run your first circuit" }).click();
  await expect(page).toHaveURL(/\/get-started\/$/);
  await expect(page.getByRole("heading", { name: "Your first detector event" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "1. Install a native package" })).toBeVisible();
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
  for (const path of ["/", "/get-started/", "/decoding/", "/qp101/protocol/", "/interactive/"]) {
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
