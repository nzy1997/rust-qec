import { expect, test } from '@playwright/test';

test('a standalone development snapshot identifies itself on every page', async ({ page, request }) => {
  const response = await request.get('/versions.json');
  const catalog = await response.json();
  expect(catalog.current).toBe('master');
  expect(catalog.versions.map(version => version.id)).toEqual(['master']);
  await page.goto('/');
  await expect(page.locator('.version-strip')).toBeVisible();
  for (const path of ['/', '/get-started/', '/qp101/protocol/']) {
    await page.goto(path);
    await expect(page.locator('[data-version-label]')).toHaveText('RustQEC 0.3 · Development');
    await expect(page.getByLabel('Documentation version')).toBeHidden();
    await expect(page.locator('.version-strip')).not.toContainText('Stable');
  }
});

test('version identity remains available without a catalog', async ({ page }) => {
  await page.route('**/versions.json', route => route.abort());
  await page.goto('/docs/');
  await expect(page.locator('[data-version-label]')).toBeVisible();
  await expect(page.locator('[data-version-label]')).toHaveText('RustQEC 0.3 · Development');
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
  { name: 'missing page explains the version homepage fallback', start: '/qp101/protocol/#protocol-format', pages: { '': ['top'] }, target: '/versions/test/', fallback: 'QP101 format specification' },
  { name: 'missing anchor is discarded', start: '/qp101/protocol/#missing-heading', pages: { '': ['top'], 'qp101/protocol/': ['top'] }, target: '/versions/test/qp101/protocol/' },
]) {
  test(`switching versions: ${scenario.name}`, async ({ page }) => {
    const versions = [
      { id: 'master', label: 'Development · master', root: './', pages: { '': ['top'] } },
      { id: 'test', label: 'Test edition', root: 'versions/test/', pages: scenario.pages },
    ];
    await page.route('**/versions.json', route => route.fulfill({ json: {
      current: new URL(route.request().url()).pathname.includes('/versions/test/') ? 'test' : 'master',
      versions,
    } }));
    await page.route('**/versions/test/**', (route) => {
      if (route.request().resourceType() !== 'document') return route.fallback();
      return route.fulfill({ contentType: 'text/html', body: `<!doctype html><meta charset="utf-8"><body data-docs-version="test" data-root=".">
        <div class="version-strip"><div class="version-inner"><span data-version-label>Test edition</span><label for="docs-version" hidden>Documentation version</label><select id="docs-version" hidden></select></div></div>
        <h1>Test edition</h1><script src="/js/versions.js"></script></body>` });
    });
    await page.goto(scenario.start);
    await expect(page.getByLabel('Documentation version')).toBeVisible();
    await page.getByLabel('Documentation version').selectOption('test');
    await expect(page).toHaveURL(`http://127.0.0.1:8765${scenario.target}`);
    await expect(page.getByRole('heading', { name: 'Test edition' })).toBeVisible();
    if (scenario.fallback) {
      await expect(page.getByRole('status')).toContainText(`“${scenario.fallback}” is not available in Test edition`);
      await expect(page.getByLabel('Documentation version')).toBeVisible();
    }
  });
}
