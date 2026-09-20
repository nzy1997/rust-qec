import { expect, test } from "@playwright/test";

test.describe.configure({ mode: "serial" });

test("Shot Lab exposes recovery actions when WebAssembly cannot load", async ({ page }) => {
  const failedWasm = page.waitForResponse(
    (response) => response.url().endsWith("/interactive/pkg/rstim_shot_web_bg.wasm") && response.status() === 503,
  );
  await page.route("**/interactive/pkg/rstim_shot_web_bg.wasm", (route) =>
    route.fulfill({ status: 503, contentType: "text/plain", body: "temporarily unavailable" }),
  );
  await page.goto("/interactive/");
  await failedWasm;

  const startup = page.locator("[data-shot-startup]");
  await expect(startup).toHaveAttribute("data-state", "error", { timeout: 15_000 });
  await expect(startup).toHaveAttribute("role", "alert");
  await expect(startup).toContainText(/could not start|could not be loaded|unavailable/i);
  await expect(startup.getByRole("link", { name: "Refresh this page" })).toBeVisible();
  await expect(startup.getByRole("link", { name: "Use the circuit diagram guide" })).toHaveAttribute("href", "../qp101/");
  await expect(startup.getByRole("link", { name: "Report a persistent problem" })).toHaveAttribute("href", "../support/#get-help");
});

test("Shot Lab shows one useful failure state when JavaScript is disabled", async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto("/interactive/");

  await expect(page.locator("[data-shot-startup]")).toBeHidden();
  const fallback = page.locator(".shot-fallback");
  await expect(fallback).toContainText("Shot Lab is unavailable");
  await expect(fallback.getByRole("link", { name: "Use the circuit diagram guide" })).toBeVisible();
  await expect(fallback.getByRole("link", { name: "Get help" })).toBeVisible();
  await context.close();
});


test("Schema Browser reports a failed fetch and recovers on retry", async ({ page }) => {
  const failedSchema = page.waitForResponse(
    (response) => response.url().endsWith("/qp101.schema.json") && response.status() === 503,
  );
  await page.route("**/qp101.schema.json", (route) =>
    route.fulfill({ status: 503, contentType: "application/json", body: "{}" }),
  );
  await page.goto("/qp101/");
  await failedSchema;
  await page.locator("#schema-browser-title").click();

  const browser = page.locator("#schema-browser");
  await expect(browser.locator("#schema-status")).toHaveText("Unavailable");
  await expect(browser.getByRole("heading", { name: "Schema browser is unavailable" })).toBeVisible();
  await expect(browser.getByRole("link", { name: "Open raw schema" })).toHaveAttribute("href", "../qp101.schema.json");
  await expect(browser.getByRole("link", { name: "Read the format specification" })).toHaveAttribute("href", "../qp101/protocol/");

  await page.unroute("**/qp101.schema.json");
  await browser.getByRole("button", { name: "Try again" }).click();
  await expect(browser.locator("#schema-status")).toHaveText("Loaded");
  await expect(browser.getByRole("button", { name: "QP101 document" })).toBeVisible();

  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390);
});
