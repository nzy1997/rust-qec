import { test, expect } from '@playwright/test';

// The tutorial/concepts support labels must be rendered from the version-bound
// support matrix, not from hand-edited markup. The matrix is served verbatim at
// /support/envelope-support.json; fixtures below replace it to prove that
// missing, failed, or pre-release (v0.3.0) evidence never renders a Supported
// claim from the new release.

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
  return published;
}

const matchingBadge = page => page.locator('[data-decoder-badge="envelope-matching"]');
const mleBadge = page => page.locator('[data-decoder-badge="envelope-mle"]');

test('tutorial and concepts render the version-bound Supported label with evidence links', async ({ page, request }, testInfo) => {
  const published = matchingPublication(await servedMatrix(request));

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
});

test('missing support evidence keeps the Beta label', async ({ page }) => {
  await page.route(MATRIX_ROUTE, route => route.fulfill({ status: 404, body: 'not found' }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingBadge(page)).toHaveText('Envelope matching · Beta');
  await expect(matchingBadge(page).locator('a')).toHaveCount(0);
});

test('failed support evidence keeps the Beta label', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  // A publication claim without its evidence bundle must not upgrade the label.
  matrix.decoders['envelope-matching'].published_support.evidence_asset = '';
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  await expect(matchingBadge(page)).toHaveText('Envelope matching · Beta');
  await expect(matchingBadge(page).locator('a')).toHaveCount(0);
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
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  const published = matchingPublication(matrix);
  await expect(matchingBadge(page).locator('a')).toHaveText(`Envelope matching · Supported · ${published.release}`);
  const mle = mleBadge(page);
  await expect(mle).toHaveText('Envelope MLE · Beta');
  await expect(mle.locator('a')).toHaveCount(0);
  await expect(mle).not.toContainText('Supported');
});
