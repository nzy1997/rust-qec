import { test, expect } from '@playwright/test';

test('atom-loss evidence exposes three real figures and downloadable measurements', async ({ page }) => {
  await page.goto('/atom-loss/#atom-loss-results');
  const figures = page.locator('.loss-result-figure');
  await expect(figures).toHaveCount(3);
  for (const figure of await figures.all()) {
    await figure.scrollIntoViewIfNeeded();
    await expect.poll(() => figure.locator('img').evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
    await expect(figure.locator('figcaption')).toContainText(/shots|Distance/);
    const href = await figure.locator('a').getAttribute('href');
    const response = await page.request.get(new URL(href, page.url()).href);
    expect(response.ok()).toBe(true);
    expect(await response.text()).toContain('<svg');
  }
  await page.locator('.loss-full-sweep summary').click();
  const full = page.locator('.loss-full-figure img');
  await full.scrollIntoViewIfNeeded();
  await expect.poll(() => full.evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
  await expect(page.locator('.loss-full-figure figcaption')).toContainText('0.000599');
  await page.locator('.loss-evidence-downloads summary').click();
  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('link', { name: 'Download result table (CSV)' }).click();
  expect((await downloadPromise).suggestedFilename()).toBe('summary.csv');
  const data = await (await page.request.get('/data/atom-loss/decoding.json')).json();
  expect(data).toHaveLength(15);
  expect(data.every(row => row.shots === 5000)).toBe(true);
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
});
