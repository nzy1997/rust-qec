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

test("fixed mode expands repeats, edits downstream state, and resets history on sample", async ({ page }) => {
  await page.goto("/interactive/");
  await expect(page.getByRole("button", { name: "Sample", exact: true })).toBeVisible();
  await expect(page.locator("#shot-file")).toHaveCount(0);
  await expect(page.locator("#shot-close")).toHaveCount(0);

  const ids = await page.locator("[data-noise-event-id]").evaluateAll((nodes) =>
    nodes.map((node) => node.dataset.noiseEventId),
  );
  expect(ids.length).toBe(8);
  expect(new Set(ids).size).toBe(ids.length);

  await page.locator("[data-noise-event-id]").first().click();
  await page.locator("#shot-popover").getByRole("button", { name: "X", exact: true }).click();
  await expect(page.locator("#shot-summary")).toContainText("1 active errors");
  await expect(page.locator("[data-annotation-tags*='manual-override']")).toHaveCount(1);
  await expect(page.getByRole("button", { name: "Undo error edit" })).toBeEnabled();

  const transform = await page.locator("#shot-canvas").getAttribute("style");
  await page.getByRole("button", { name: "Sample", exact: true }).click();
  await expect(page.locator("#shot-base-badge")).toHaveText("Sampled");
  await expect(page.locator("[data-annotation-tags*='manual-override']")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Undo error edit" })).toBeDisabled();
  expect(await page.locator("#shot-canvas").getAttribute("style")).toBe(transform);
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

  await page.locator("#shot-filter-errors").uncheck();
  await expect(sites.first()).toHaveCSS("opacity", "0.12");
  await page.locator("#shot-filter-errors").check();

  await page.getByRole("button", { name: "Sample", exact: true }).click();
  const after = await sites.evaluateAll((nodes) => nodes.map((node) => node.dataset.noiseEventId));
  expect(after).toEqual(before);
});
