import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";

const LOCAL_CIRCUIT = `
R 0
REPEAT 2 {
  X_ERROR(0) 0
  M 0
  DETECTOR rec[-1]
  R 0
}
`;

const MEASUREMENT_ERROR_CIRCUIT = `
MPAD(1) 0
DETECTOR rec[-1]
`;

test("fixed gadget gallery edits downstream state and resets history on sample", async ({ page }) => {
  await page.goto("/interactive/");
  await expect(page.getByRole("button", { name: "Sample", exact: true })).toBeVisible();
  await expect(page.locator("#shot-file")).toHaveCount(0);
  await expect(page.locator("#shot-close")).toHaveCount(0);

  const layout = await page.locator(".shot-workspace").evaluate((workspace) => {
    const stage = workspace.querySelector("#shot-stage").getBoundingClientRect();
    const view = workspace.querySelector(".shot-view-panel").getBoundingClientRect();
    const detail = workspace.querySelector("#shot-detail").getBoundingClientRect();
    const bounds = workspace.getBoundingClientRect();
    return {
      stageWidth: stage.width,
      workspaceWidth: bounds.width,
      viewBottom: view.bottom,
      stageTop: stage.top,
      stageBottom: stage.bottom,
      detailTop: detail.top,
    };
  });
  expect(layout.stageWidth / layout.workspaceWidth).toBeGreaterThan(0.98);
  expect(layout.viewBottom).toBeLessThanOrEqual(layout.stageTop + 1);
  expect(layout.detailTop).toBeGreaterThanOrEqual(layout.stageBottom - 1);
  expect(await page.locator(".shot-toolbar button").evaluateAll(
    (buttons) => buttons.every((button) => getComputedStyle(button).whiteSpace === "nowrap"),
  )).toBe(true);

  const ids = await page.locator("[data-noise-event-id]").evaluateAll((nodes) =>
    nodes.map((node) => node.dataset.noiseEventId),
  );
  expect(ids.length).toBe(9);
  expect(new Set(ids).size).toBe(ids.length);
  await expect(page.locator("#shot-summary")).toContainText("0/2 detectors");

  const measurementResults = await page.locator("#shot-stage svg").evaluate((svg) => {
    const measurementXs = new Set(
      [...svg.querySelectorAll("text")]
        .filter((node) => ["M", "MR", "MRL"].includes(node.textContent))
        .map((node) => node.getAttribute("x")),
    );
    return [...svg.querySelectorAll(".annotation")]
      .filter(
        (node) =>
          node.dataset.annotationTags?.includes("query-result") &&
          measurementXs.has(node.getAttribute("x")),
      )
      .map((node) => ({ text: node.textContent, y: node.getAttribute("y") }));
  });
  expect(measurementResults).toHaveLength(4);
  expect(new Set(measurementResults.map(({ y }) => y)).size).toBe(4);
  expect(measurementResults.at(-1).text).toContain("L=0");
  expect(measurementResults.at(-1).text).toContain("M=0");

  const canvas = page.locator("#shot-canvas");
  const scale = () => canvas.evaluate((node) => new DOMMatrix(getComputedStyle(node).transform).a);
  const initialScale = await scale();
  await page.getByRole("button", { name: "Zoom in" }).click();
  expect(await scale()).toBeGreaterThan(initialScale);
  await page.getByRole("button", { name: "Zoom out" }).click();
  expect(await scale()).toBeCloseTo(initialScale, 5);

  const stage = page.locator("#shot-stage");
  await stage.hover();
  const transformBeforeWheel = await canvas.getAttribute("style");
  const scrollBeforeWheel = await page.evaluate(() => window.scrollY);
  await page.mouse.wheel(0, 320);
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(scrollBeforeWheel);
  expect(await canvas.getAttribute("style")).toBe(transformBeforeWheel);

  await stage.hover();
  const stageBox = await stage.boundingBox();
  const transformBeforeDrag = await canvas.getAttribute("style");
  await page.mouse.move(stageBox.x + stageBox.width / 2, stageBox.y + stageBox.height - 18);
  await page.mouse.down();
  await page.mouse.move(stageBox.x + stageBox.width / 2 + 32, stageBox.y + stageBox.height - 38);
  await page.mouse.up();
  expect(await canvas.getAttribute("style")).not.toBe(transformBeforeDrag);

  await page.locator("[data-noise-event-id]").first().click();
  await page.locator("#shot-popover").getByRole("button", { name: "X", exact: true }).click();
  await expect(page.locator("#shot-summary")).toContainText("1 active errors");
  await expect(page.locator("#shot-base-badge")).toHaveText("Base: no-error");
  await expect(page.locator("#shot-summary")).toContainText("Edited: 1 override");
  await expect(page.locator("[data-annotation-tags*='manual-override']")).toHaveCount(1);
  await expect(page.getByRole("button", { name: "Undo error edit" })).toBeEnabled();
  await page.getByRole("button", { name: "Undo error edit" }).click();
  await expect(page.locator("#shot-summary")).toContainText("Current: no active errors");
  await expect(page.locator("#shot-summary")).not.toContainText("Edited:");

  const transform = await page.locator("#shot-canvas").getAttribute("style");
  await page.getByRole("button", { name: "Sample", exact: true }).click();
  await expect(page.locator("#shot-base-badge")).toHaveText("Base: sampled");
  await expect(page.locator("[data-annotation-tags*='manual-override']")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Undo error edit" })).toBeDisabled();
  expect(await page.locator("#shot-canvas").getAttribute("style")).toBe(transform);
});

test("diagram result controls have keyboard names and focus mode expands the workspace", async ({ page }) => {
  await page.goto("/interactive/");
  const measurement = page.locator("[data-measurement-ids]").first();
  const detector = page.locator("[data-detector-id]").first();

  await expect(measurement).toHaveAttribute("role", "button");
  await expect(measurement).toHaveAttribute("aria-label", /^Measurement .+Press Enter to inspect its result\.$/);
  await expect(detector).toHaveAttribute("role", "button");
  await expect(detector).toHaveAttribute("aria-label", /^Detector .+Press Enter to inspect its result\.$/);
  await measurement.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#shot-detail .eyebrow")).toHaveText("measurement");

  const focus = page.locator("#shot-focus");
  await focus.click();
  await expect(focus).toHaveText("Exit focus");
  await expect(focus).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("body")).toHaveClass(/shot-focus/);
  await focus.click();
  await expect(focus).toHaveText("Focus circuit");
  await expect(page.locator("body")).not.toHaveClass(/shot-focus/);
});

test("Escape returns focus to the noise site that opened its outcome menu", async ({ page }) => {
  await page.goto("/interactive/");
  const noise = page.locator("[data-noise-event-id]").first();
  await noise.click();
  const popover = page.locator("#shot-popover");
  await expect(popover.getByRole("button", { name: "I", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(popover).toBeHidden();
  await expect(noise).toBeFocused();
});

test("choosing an outcome focuses the replacement noise site", async ({ page }) => {
  await page.goto("/interactive/");
  const noise = page.locator("[data-noise-event-id]").first();
  const id = await noise.getAttribute("data-noise-event-id");
  await noise.click();
  const choice = page.locator("#shot-popover").getByRole("button", { name: "X", exact: true });
  await choice.focus();
  await page.keyboard.press("Enter");
  const replacement = page.locator(`[data-noise-event-id="${id}"]`);
  await expect(page.locator("#shot-popover")).toBeHidden();
  await expect(replacement).toBeFocused();
});

test("MPAD measurement error keeps its noise-site name and Enter uses the noise selection path", async ({ page }) => {
  await page.goto("/interactive/local/");
  await page.locator("#shot-file").setInputFiles({
    name: "measurement-error.stim",
    mimeType: "text/plain",
    buffer: Buffer.from(MEASUREMENT_ERROR_CIRCUIT),
  });
  const site = page.locator("[data-noise-event-id][data-measurement-ids]");
  await expect(site).toHaveCount(1);
  await expect(site).toHaveAttribute("aria-label", /^Noise site .+Press Enter to inspect this outcome\.$/);
  await site.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#shot-popover")).toBeHidden();
  await expect(page.locator("#shot-detail .eyebrow")).toHaveText("Noise event");
});

test("embedded viewer focus mode does not depend on docs page metadata", async () => {
  const [embeddedHtml, embeddedCss] = await Promise.all([
    readFile(new URL("../../../rstim/assets/shot-viewer/index.html", import.meta.url), "utf8"),
    readFile(new URL("../../../rstim/assets/shot-viewer/shot-viewer.css", import.meta.url), "utf8"),
  ]);
  expect(embeddedHtml).not.toContain('data-page="shot"');
  expect(embeddedCss).toContain("body.shot-focus .shot-app");
  expect(embeddedCss).not.toContain('body[data-page="shot"].shot-focus');
});

test("local mode starts blank, loads only in-browser, rejects oversized input, and resets on reload", async ({ page }) => {
  const requests = [];
  page.on("request", (request) => requests.push({ method: request.method(), url: request.url() }));
  await page.goto("/interactive/local/");
  await expect(page.locator("#shot-empty")).toBeVisible();
  await expect(page.locator("#shot-workspace")).toBeHidden();

  await page.locator("#shot-file").setInputFiles({
    name: "local.stim",
    mimeType: "text/plain",
    buffer: Buffer.from(LOCAL_CIRCUIT),
  });
  await expect(page.locator("#shot-workspace")).toBeVisible();
  await expect(page.locator("[data-noise-event-id]")).toHaveCount(2);
  expect(requests.every(({ method, url }) => method === "GET" && url.startsWith("http://127.0.0.1:8765/"))).toBe(true);

  await page.getByRole("button", { name: "Close circuit" }).click();
  await expect(page.locator("#shot-empty")).toBeVisible();
  await page.locator("#shot-file").setInputFiles({
    name: "too-large.stim",
    mimeType: "text/plain",
    buffer: Buffer.from("REPEAT 1000000 {\n X_ERROR(0.1) 0\n}\n"),
  });
  await expect(page.locator("#shot-error")).toContainText("exceeds limit");

  await page.reload();
  await expect(page.locator("#shot-empty")).toBeVisible();
  await expect(page.locator("#shot-workspace")).toBeHidden();
});

test("SVG and PDF downloads contain provenance and remain vector", async ({ page }) => {
  await page.goto("/interactive/");
  await expect(page.getByRole("button", { name: "Export SVG" })).toBeVisible();

  const [svgDownload] = await Promise.all([
    page.waitForEvent("download"),
    page.getByRole("button", { name: "Export SVG" }).click(),
  ]);
  expect(svgDownload.suggestedFilename()).toMatch(
    /^fixed-circuit-noiseless-\d+-\d{8}\.svg$/,
  );
  const svg = await readFile(await svgDownload.path(), "utf8");
  expect(svg).toContain("rstim-shot-provenance");
  expect(svg).toContain('"format_version":"rstim-shot-provenance-v1"');
  expect(svg).toContain("<svg");

  const [pdfDownload] = await Promise.all([
    page.waitForEvent("download"),
    page.getByRole("button", { name: "Export PDF" }).click(),
  ]);
  expect(pdfDownload.suggestedFilename()).toMatch(
    /^fixed-circuit-noiseless-\d+-\d{8}\.pdf$/,
  );
  const pdf = await readFile(await pdfDownload.path());
  const pdfText = pdf.toString("latin1");
  expect(pdf.subarray(0, 5).toString()).toBe("%PDF-");
  expect(pdfText).toContain("/Type /Page");
  expect(pdfText).not.toContain("/Subtype /Image");
});

test("WASM uses the fixed-width DEPOLARIZE2 golden branch", async ({ page }) => {
  await page.goto("/interactive/");
  await expect(page.getByRole("button", { name: "Sample", exact: true })).toBeVisible();
  const outcome = await page.evaluate(async () => {
    const module = await import("/interactive/pkg/rstim_shot_web.js");
    await module.default({
      module_or_path: new URL("/interactive/pkg/rstim_shot_web_bg.wasm", window.location.href),
    });
    const session = new module.ShotSession("DEPOLARIZE2(1) 0 1\n", 1, 0);
    try {
      const snapshot = JSON.parse(session.sample(0x89abcdef, 0x01234567));
      return snapshot.shot.result.noise_events[0].effective_outcome;
    } finally {
      session.free();
    }
  });
  expect(outcome).toEqual({ kind: "pauli_pair", first: "y", second: "x" });
});

test("read-only channel sites remain inspectable, stable, and filterable", async ({ page }) => {
  await page.goto("/interactive/local/");
  await page.locator("#shot-file").setInputFiles({
    name: "channels.stim",
    mimeType: "text/plain",
    buffer: Buffer.from(
      "PAULI_CHANNEL_1(0.1,0.2,0.3) 0\n" +
      "HERALDED_PAULI_CHANNEL_1(0.1,0.2,0.3,0.4) 1\n" +
      "CORRELATED_ERROR(0.25) X2 Y3\n",
    ),
  });

  const sites = page.locator("[data-noise-event-id]");
  await expect(sites).toHaveCount(3);
  const before = await sites.evaluateAll((nodes) => nodes.map((node) => node.dataset.noiseEventId));

  await sites.first().click();
  await expect(page.locator("#shot-detail")).toContainText("pX=0.1, pY=0.2, pZ=0.3");
  await expect(page.locator("#shot-detail")).toContainText("Total probability");
  await expect(page.locator("#shot-detail")).toContainText("0.6");
  await expect(page.locator("#shot-detail")).toContainText("read-only");
  await expect(page.locator("#shot-popover")).toBeHidden();
  await expect.poll(() => page.locator("#shot-detail").evaluate((detail) => {
    const rect = detail.getBoundingClientRect();
    return Math.max(-rect.top, rect.bottom - window.innerHeight, 0);
  })).toBeLessThanOrEqual(1);

  await page.locator("#shot-filter-errors").uncheck();
  await expect(sites.first()).toHaveCSS("opacity", "0.12");
  await page.locator("#shot-filter-errors").check();

  await page.getByRole("button", { name: "Sample", exact: true }).click();
  const after = await sites.evaluateAll((nodes) => nodes.map((node) => node.dataset.noiseEventId));
  expect(after).toEqual(before);
});

test("measurement flips are read-only noise sites with stable interaction ids", async ({ page }) => {
  await page.goto("/interactive/local/");
  await page.locator("#shot-file").setInputFiles({
    name: "measurement-flips.stim",
    mimeType: "text/plain",
    buffer: Buffer.from(
      "MPAD(1) 0 1\n" +
      "MXX(1) 2 3 4 5\n" +
      "MYY(1) 6 7\n" +
      "MZZ(1) 8 9\n" +
      "MPP(1) X10*X11 Z12\n" +
      "MXX 13 14\n",
    ),
  });

  const sites = page.locator("[data-noise-event-id]");
  await expect(sites).toHaveCount(8);
  const before = await sites.evaluateAll((nodes) => nodes.map((node) => node.dataset.noiseEventId));
  expect(new Set(before).size).toBe(before.length);

  await sites.first().click();
  await expect(page.locator("#shot-detail")).toContainText("MPAD");
  await expect(page.locator("#shot-detail")).toContainText("Flip probability");
  await expect(page.locator("#shot-detail")).toContainText("read-only");
  await expect(page.locator("#shot-popover")).toBeHidden();

  await page.locator("#shot-filter-errors").uncheck();
  await expect(sites.first()).toHaveCSS("opacity", "0.12");
  await page.locator("#shot-filter-errors").check();

  await page.getByRole("button", { name: "Sample", exact: true }).click();
  const after = await sites.evaluateAll((nodes) => nodes.map((node) => node.dataset.noiseEventId));
  expect(after).toEqual(before);
  await sites.first().click();
  await expect(page.locator("#shot-detail")).toContainText("FLIPPED");
});
