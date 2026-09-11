import { expect, test } from '@playwright/test';

test('only master is advertised, including nested pages', async ({ page, request }) => {
  const response = await request.get('/versions.json');
  const catalog = await response.json();
  expect(catalog.current).toBe('master');
  expect(catalog.versions.map(version => version.id)).toEqual(['master']);
  for (const path of ['/', '/get-started/', '/qp101/protocol/']) {
    await page.goto(path);
    await expect(page.locator('[data-version-label]')).toHaveText('Development · master');
    await expect(page.getByLabel('Documentation version')).toBeHidden();
    await expect(page.locator('.version-strip')).not.toContainText('Stable');
  }
});

test('version identity remains available without a catalog', async ({ page }) => {
  await page.route('**/versions.json', route => route.abort());
  await page.goto('/docs/');
  await expect(page.locator('[data-version-label]')).toBeVisible();
  await expect(page.locator('[data-version-label]')).toHaveText('Development · master');
  await expect(page.getByLabel('Documentation version')).toBeHidden();
});

test('switching back from a nested edition respects the GitHub Pages prefix', async ({ page }) => {
  await page.route('**/rust-qec/**', async route => {
    const url = new URL(route.request().url());
    const nested = url.pathname.startsWith('/rust-qec/versions/test/');
    url.pathname = url.pathname.replace(/^\/rust-qec\/(versions\/test\/)?/, '/');
    const response = await route.fetch({ url: url.href });
    if (nested && route.request().resourceType() === 'document') {
      const body = (await response.text()).replace('data-docs-version="master"', 'data-docs-version="test"');
      await route.fulfill({ response, body });
    } else {
      await route.fulfill({ response });
    }
  });
  await page.route('**/versions.json', route => route.fulfill({ json: {
    current: 'test', versions: [
      { id: 'master', label: 'Development · master', root: '../../', pages: { '': ['top'], 'qp101/protocol/': ['top'] } },
      { id: 'test', label: 'Test edition', root: './', pages: { '': ['top'] } },
    ],
  } }));
  await page.goto('/rust-qec/versions/test/qp101/protocol/#top');
  await expect(page.getByLabel('Documentation version')).toBeVisible();
  await page.getByLabel('Documentation version').selectOption('master');
  await expect(page).toHaveURL('http://127.0.0.1:8765/rust-qec/qp101/protocol/#top');
});

// A second edition is a test fixture only; no release documentation is published.
for (const scenario of [
  { name: 'equivalent page and anchor', start: '/qp101/protocol/?q=noise#top', pages: { '': ['top'], 'qp101/protocol/': ['top'] }, target: '/versions/test/qp101/protocol/?q=noise#top' },
  { name: 'missing page falls back to the version homepage', start: '/qp101/protocol/#protocol-format', pages: { '': ['top'] }, target: '/versions/test/' },
  { name: 'missing anchor is discarded', start: '/qp101/protocol/#missing-heading', pages: { '': ['top'], 'qp101/protocol/': ['top'] }, target: '/versions/test/qp101/protocol/' },
]) {
  test(`switching versions: ${scenario.name}`, async ({ page }) => {
    await page.route('**/versions.json', route => route.fulfill({ json: {
      current: 'master', versions: [
        { id: 'master', label: 'Development · master', root: './', pages: { '': ['top'] } },
        { id: 'test', label: 'Test edition', root: 'versions/test/', pages: scenario.pages },
      ],
    } }));
    await page.route('**/versions/test/**', route => route.fulfill({ contentType: 'text/html', body: '<h1>Test edition</h1>' }));
    await page.goto(scenario.start);
    await expect(page.getByLabel('Documentation version')).toBeVisible();
    await page.getByLabel('Documentation version').selectOption('test');
    await expect(page).toHaveURL(`http://127.0.0.1:8765${scenario.target}`);
    await expect(page.getByRole('heading', { name: 'Test edition' })).toBeVisible();
  });
}
