import { expect, test } from '@playwright/test';

const pages = [
  'get-started', 'css-codes', 'atom-loss', 'decoding', 'sampling-data', 'rust-api',
  'reference', 'qec-code-cli', 'rsinter-cli', 'circuit-format', 'data-formats',
  'qp101/protocol', 'rsmp-v1-showcase', 'rust-api-reference',
  'validation', 'benchmarks/simulation', 'benchmarks/decoders', 'atom-loss-evidence',
  'support', 'versions',
];
for (const width of [390, 1280]) {
  test(`twenty canonical pages have usable navigation and contained layouts at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    for (const path of pages) {
      const response = await page.goto(`/${path}/`);
      expect(response.status(), path).toBe(200);
      await expect(page.locator('h1')).toHaveCount(1);
      const nav = page.locator('.docs-sidebar nav');
      expect(await nav.locator('p').allTextContents()).toEqual([
        'Tutorials', 'CLI & formats', 'Benchmarks', 'Support & versions',
      ]);
      await expect(nav.locator('a')).toHaveCount(20);
      await expect(nav.locator('a[aria-current="page"]')).toHaveCount(1);
      for (const stack of await page.locator('.evidence-stack[data-evidence-items]').all()) {
        await expect(stack.locator('.result-card').first()).toBeAttached();
      }
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth), { message: path }).toBe(width);
      expect(await page.locator('main').innerText()).not.toContain('{{ load_data');
    }
  });
}
for (const [old, target, heading] of [
  ['simulator/#simulation-results-title', 'benchmarks/simulation/#simulation-results-title', '#simulation-results-title'],
  ['detector-models/#dem-extraction', 'get-started/#detector-output', '#detector-output'],
  ['atom-loss-concepts/#choose-decoder', 'atom-loss/#choose-decoder', '#choose-decoder'],
  ['atom-loss-concepts/#supported-circuits', 'support/#atom-loss-support-boundary', '#atom-loss-support-boundary'],
]) {
  test(`old ${old} links reach their canonical explanation`, async ({ page }) => {
    await page.goto('/' + old);
    await expect(page).toHaveURL(new RegExp('/' + target.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '$'));
    await expect(page.locator(heading)).toBeInViewport();
  });
}
test('all CLI families expose generated help and deep links expand rstim details', async ({ page }) => {
  await page.goto('/reference/#help-rstim-render_svg');
  await expect(page.locator('#help-rstim-render_svg').locator('xpath=..')).toHaveAttribute('open', '');
  await expect(page.locator('#help-rstim-render_svg').locator('xpath=..')).toContainText('--sample_shot');
  await page.goto('/qec-code-cli/#help-qec-code-code-css-distance-exact');
  await expect(page.locator('main')).toContainText('--backend');
  await page.goto('/rsinter-cli/#help-rsinter-replay');
  await expect(page.locator('main')).toContainText('--predictions-out');
});
test('loss model is inline and decoder methods cite the pinned original paper', async ({ page }) => {
  await page.goto('/atom-loss/#loss-record');
  await expect(page.locator('pre[data-loss-model-example]')).toContainText('LOSS(1)');
  await expect(page.locator('main')).toContainText('1100');
  await expect(page.locator('main a[href="https://arxiv.org/abs/2603.04156v2"]')).toHaveCount(1);
  await expect(page.locator('.docs-sidebar a[href*="atom-loss-concepts"]')).toHaveCount(0);
});
