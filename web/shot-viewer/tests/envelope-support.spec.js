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

function matchingPublication(matrix) {
  const decoder = matrix.decoders['envelope-matching'];
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
      label: `rustqec-envelope-verification-v1;tag=${published.release};source=${'a'.repeat(40)};evidence=${evidenceDigest};marker=${markerDigest};supported=${supported};status=pass`,
    });
  }
  return {
    tag_name: published.release,
    draft: false,
    published_at: '2026-09-16T05:00:00Z',
    assets,
  };
}

async function serveVerifiedPublication(page, published) {
  await page.route(published.verification_url, route => route.fulfill({
    json: verifiedRelease(published),
  }));
}

const matchingBadge = page => page.locator('[data-decoder-badge="envelope-matching"]');
const mleBadge = page => page.locator('[data-decoder-badge="envelope-mle"]');

test('tutorial and concepts render the version-bound Supported label with evidence links', async ({ page, request }, testInfo) => {
  const published = matchingPublication(await servedMatrix(request));
  await serveVerifiedPublication(page, published);

  await page.goto('/atom-loss-concepts/');
  const conceptBadge = matchingBadge(page);
  await expect(conceptBadge.locator('a')).toHaveText(`Envelope matching · Supported · ${published.release}`);
  await expect(conceptBadge.locator('a')).toHaveAttribute('href', published.release_url);
  await expect(conceptBadge).toHaveAttribute('data-release', published.release);
  await expect(conceptBadge).toHaveAttribute('data-evidence-asset', published.evidence_asset);
  const evidenceLink = page.locator('[data-evidence-link="envelope-matching"]');
  await expect(evidenceLink).toHaveAttribute('href', published.release_url);
  const boundary = page.locator('.task-section').filter({ has: page.locator('#supported-circuits') });
  await expect(boundary).toContainText('Supported since');
  await expect(boundary).toContainText(published.release);
  const conceptMle = mleBadge(page);
  await expect(conceptMle).toHaveText('Envelope MLE · Beta');
  await expect(conceptMle.locator('a')).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath('atom-loss-concepts-support.png'), fullPage: true });

  await page.goto('/atom-loss/');
  const tutorialBadge = matchingBadge(page);
  await expect(tutorialBadge.locator('a')).toHaveText(`Envelope matching · Supported · ${published.release}`);
  await expect(tutorialBadge.locator('a')).toHaveAttribute('href', published.release_url);
  await expect(page.locator('.loss-hero-note')).toContainText(`Supported since ${published.release}`);
  await page.screenshot({ path: testInfo.outputPath('atom-loss-tutorial-support.png'), fullPage: true });

  await page.goto('/support/');
  const supportCopies = page.locator('[data-decoder-support-copy="envelope-matching"]');
  await expect(supportCopies).toHaveCount(2);
  await expect(supportCopies.first()).toHaveText(`Supported since ${published.release}`);
  await expect(supportCopies.last()).toHaveText(`Supported since ${published.release}`);
});

test('missing support evidence keeps the Beta label', async ({ page }) => {
  await page.route(MATRIX_ROUTE, route => route.fulfill({ status: 404, body: 'not found' }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingBadge(page)).toHaveText('Envelope matching · Beta');
  await expect(matchingBadge(page).locator('a')).toHaveCount(0);
});

test('missing publication verification marker keeps badge and body at Beta', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  const published = matchingPublication(matrix);
  await page.route(published.verification_url, route => route.fulfill({ status: 404, body: 'not found' }));
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingBadge(page)).toHaveText('Envelope matching · Beta');
  await expect(matchingBadge(page).locator('a')).toHaveCount(0);
  await expect(page.locator('[data-decoder-support-copy="envelope-matching"]')).toContainText('Beta');
  await expect(page.locator('[data-evidence-link="envelope-matching"]')).toContainText('not published');
  await expect(page.locator('[data-evidence-link="envelope-matching"]')).not.toHaveAttribute('href');
  await page.goto('/support/');
  await expect(page.locator('[data-decoder-support-copy="envelope-matching"]').first()).toContainText('Beta');
});

test('marker metadata without Matching support keeps the Beta label', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  const published = matchingPublication(matrix);
  await page.route(published.verification_url, route => route.fulfill({
    json: verifiedRelease(published, { supported: 'envelope-mle' }),
  }));
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingBadge(page)).toHaveText('Envelope matching · Beta');
  await expect(page.locator('[data-decoder-support-copy="envelope-matching"]')).toContainText('Beta');
});

test('marker metadata for a different evidence hash keeps the Beta label', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  const published = matchingPublication(matrix);
  const release = verifiedRelease(published);
  const marker = release.assets.find(asset => asset.name === published.verification_asset);
  marker.label = marker.label.replace(/evidence=sha256:[0-9a-f]{64}/, `evidence=sha256:${'e'.repeat(64)}`);
  await page.route(published.verification_url, route => route.fulfill({ json: release }));
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingBadge(page)).toHaveText('Envelope matching · Beta');
  await expect(page.locator('[data-decoder-support-copy="envelope-matching"]')).toContainText('Beta');
});

test('a v0.3.0 edition cannot render the new Supported claim', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  // v0.3.0 predates the evidence bundle: no published_support, maturity Beta.
  delete matrix.decoders['envelope-matching'].published_support;
  matrix.decoders['envelope-matching'].current_maturity = 'beta';
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingBadge(page)).toHaveText('Envelope matching · Beta');
  await expect(matchingBadge(page).locator('a')).toHaveCount(0);
  await expect(matchingBadge(page)).not.toContainText('Supported');
});

test('MLE does not inherit the matching Supported badge', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  // Hostile fixture: MLE claims supported maturity without any published evidence.
  matrix.decoders['envelope-mle'].current_maturity = 'supported';
  const published = matchingPublication(matrix);
  await serveVerifiedPublication(page, published);
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingBadge(page).locator('a')).toHaveText(`Envelope matching · Supported · ${published.release}`);
  const mle = mleBadge(page);
  await expect(mle).toHaveText('Envelope MLE · Beta');
  await expect(mle.locator('a')).toHaveCount(0);
  await expect(mle).not.toContainText('Supported');
});
