import { test, expect } from '@playwright/test';

// The tutorial/concepts/support claims start at Beta. The version-bound matrix
// names the intended release, while the GitHub release listing must also show
// the evidence bundle, sidecar, and last-uploaded verification marker. Fixtures
// below prove missing, incomplete, or pre-release evidence stays Beta.

const MATRIX_ROUTE = '**/support/envelope-support.json';

async function servedMatrix(request) {
  const response = await request.get('/support/envelope-support.json');
  expect(response.ok()).toBe(true);
  const matrix = await response.json();
  expect(matrix.schema_version).toBe('rustqec.envelope-support.v1');
  return matrix;
}

function decoderPublication(matrix, name) {
  const decoder = matrix.decoders[name];
  expect(decoder.current_maturity).toBe('supported');
  const published = decoder.published_support;
  expect(published).toBeTruthy();
  expect(published.maturity).toBe('supported');
  expect(published.release).toMatch(/^v\d+\.\d+\.\d+$/);
  expect(published.release_url).toContain(`/releases/tag/${published.release}`);
  expect(published.evidence_asset).toBe(`envelope-support-evidence-${published.release}.tar.gz`);
  expect(published.verification_asset).toBe(`envelope-support-verification-${published.release}.json`);
  expect(published.verification_url).toContain(`/releases/tags/${published.release}`);
  return published;
}

const matchingPublication = matrix => decoderPublication(matrix, 'envelope-matching');
const mlePublication = matrix => decoderPublication(matrix, 'envelope-mle');
function verifiedRelease(published, { includeMarker = true, supported = 'envelope-matching' } = {}) {
  const evidenceDigest = `sha256:${'b'.repeat(64)}`;
  const markerDigest = `sha256:${'c'.repeat(64)}`;
  const assets = [
    { name: published.evidence_asset, state: 'uploaded', digest: evidenceDigest, label: '' },
    { name: `${published.evidence_asset}.sha256`, state: 'uploaded', digest: `sha256:${'d'.repeat(64)}`, label: '' },
  ];
  if (includeMarker) {
    assets.push({
      name: published.verification_asset,
      state: 'uploaded',
      digest: markerDigest,
      label: `rustqec-envelope-v2;t=${published.release};s=${'a'.repeat(40)};e=${evidenceDigest.slice(7)};m=${markerDigest.slice(7)};d=${supported};ok=1`,
    });
  }
  return {
    tag_name: published.release,
    draft: false,
    prerelease: false,
    published_at: '2026-09-16T05:00:00Z',
    assets,
  };
}

test('verification metadata for both decoders fits the GitHub asset-label limit', async ({ request }) => {
  const published = mlePublication(await servedMatrix(request));
  const release = verifiedRelease(published, { supported: 'envelope-matching,envelope-mle' });
  const marker = release.assets.find(asset => asset.name === published.verification_asset);
  expect(marker.label.length).toBeLessThanOrEqual(255);
});

async function serveVerifiedPublication(page, published, supported = 'envelope-matching') {
  await page.route(published.verification_url, route => route.fulfill({
    json: verifiedRelease(published, { supported }),
  }));
}

const matchingCopy = page => page.locator('[data-decoder-support-copy="envelope-matching"]').first();
const mleCopy = page => page.locator('[data-decoder-support-copy="envelope-mle"]').first();

test('support remains version-bound in body text while hero badges stay removed', async ({ page, request }, testInfo) => {
  const matrix = await servedMatrix(request);
  const published = matchingPublication(matrix);
  const mlePublished = mlePublication(matrix);
  await serveVerifiedPublication(page, published, 'envelope-matching,envelope-mle');
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingCopy(page)).toHaveText(`Supported since ${published.release}`);
  await expect(mleCopy(page)).toHaveText(`Supported since ${mlePublished.release}`);
  await expect(page.locator('[data-evidence-link="envelope-matching"]')).toHaveAttribute('href', published.release_url);
  await expect(page.locator('[data-evidence-link="envelope-mle"]')).toHaveAttribute('href', mlePublished.release_url);
  await expect(page.locator('[data-decoder-badge]')).toHaveCount(0);
  await page.goto('/atom-loss/');
  await expect(matchingCopy(page)).toHaveText(`Supported since ${published.release}`);
  await expect(page.locator('[data-decoder-badge]')).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath('atom-loss-tutorial-support.png'), fullPage: true });
  await page.goto('/support/');
  for (const name of ['envelope-matching', 'envelope-mle']) {
    const copies = page.locator(`[data-decoder-support-copy="${name}"]`);
    await expect(copies).toHaveCount(2);
    await expect(copies.first()).toHaveText(`Supported since ${published.release}`);
    await expect(copies.last()).toHaveText(`Supported since ${published.release}`);
  }
});

test('missing support evidence keeps the Beta label', async ({ page }) => {
  await page.route(MATRIX_ROUTE, route => route.fulfill({ status: 404, body: 'not found' }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingCopy(page)).toHaveText('Beta unless publication verification succeeds');
  await expect(mleCopy(page)).toHaveText('Beta unless publication verification succeeds');
});

test('missing publication verification marker keeps badge and body at Beta', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  const published = matchingPublication(matrix);
  const mlePublished = mlePublication(matrix);
  await page.route(published.verification_url, route => route.fulfill({ status: 404, body: 'not found' }));
  await page.route(mlePublished.verification_url, route => route.fulfill({ status: 404, body: 'not found' }));
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingCopy(page)).toHaveText('Beta unless publication verification succeeds');
  await expect(page.locator('[data-decoder-support-copy="envelope-matching"]')).toContainText('Beta');
  await expect(page.locator('[data-evidence-link="envelope-matching"]')).toContainText('not published');
  await expect(page.locator('[data-evidence-link="envelope-matching"]')).not.toHaveAttribute('href');
  await page.goto('/support/');
  await expect(page.locator('[data-decoder-support-copy="envelope-matching"]').first()).toContainText('Beta');
  await expect(page.locator('[data-decoder-support-copy="envelope-mle"]').first()).toContainText('Beta');
});

test('marker metadata without Matching support keeps the Beta label', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  const published = matchingPublication(matrix);
  await page.route(published.verification_url, route => route.fulfill({
    json: verifiedRelease(published, { supported: 'envelope-mle' }),
  }));
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingCopy(page)).toHaveText('Beta unless publication verification succeeds');
  await expect(page.locator('[data-decoder-support-copy="envelope-matching"]')).toContainText('Beta');
});

test('marker metadata for a different evidence hash keeps the Beta label', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  const published = matchingPublication(matrix);
  const release = verifiedRelease(published);
  const marker = release.assets.find(asset => asset.name === published.verification_asset);
  marker.label = marker.label.replace(/;e=[0-9a-f]{64}/, `;e=${'e'.repeat(64)}`);
  await page.route(published.verification_url, route => route.fulfill({ json: release }));
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingCopy(page)).toHaveText('Beta unless publication verification succeeds');
  await expect(page.locator('[data-decoder-support-copy="envelope-matching"]')).toContainText('Beta');
});

test('a prerelease with a verification marker keeps both decoders at Beta', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  const published = matchingPublication(matrix);
  const release = verifiedRelease(published, { supported: 'envelope-matching,envelope-mle' });
  release.prerelease = true;
  await page.route(published.verification_url, route => route.fulfill({ json: release }));
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingCopy(page)).toHaveText('Beta unless publication verification succeeds');
  await expect(mleCopy(page)).toHaveText('Beta unless publication verification succeeds');
});

test('failed MLE evidence keeps only MLE at Beta', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  matrix.decoders['envelope-mle'].published_support.evidence_asset = '';
  const matching = matchingPublication(matrix);
  await serveVerifiedPublication(page, matching);
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingCopy(page)).toHaveText(`Supported since ${matching.release}`);
  await expect(mleCopy(page)).toHaveText('Beta unless publication verification succeeds');
});

test('a v0.3.0 edition cannot render the new Supported claim', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  // v0.3.0 predates every envelope evidence bundle: both decoders are Beta.
  for (const name of ['envelope-matching', 'envelope-mle']) {
    delete matrix.decoders[name].published_support;
    matrix.decoders[name].current_maturity = 'beta';
    matrix.decoders[name].proposed_release_maturity = 'beta';
  }
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingCopy(page)).toHaveText('Beta unless publication verification succeeds');
  await expect(matchingCopy(page)).not.toContainText('Supported');
  await expect(mleCopy(page)).toHaveText('Beta unless publication verification succeeds');
  await expect(mleCopy(page)).not.toContainText('Supported');
});

test('the previous Matching-only release does not promote MLE', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  // A Matching-only candidate must not promote MLE.
  delete matrix.decoders['envelope-mle'].published_support;
  matrix.decoders['envelope-mle'].current_maturity = 'beta';
  matrix.decoders['envelope-mle'].proposed_release_maturity = 'beta';
  const published = matchingPublication(matrix);
  await serveVerifiedPublication(page, published);
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingCopy(page)).toHaveText(`Supported since ${published.release}`);
  const mle = mleCopy(page);
  await expect(mle).toHaveText('Beta unless publication verification succeeds');
  await expect(mle).not.toContainText('Supported');
});
