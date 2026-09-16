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

function decoderPublication(matrix, name) {
  const decoder = matrix.decoders[name];
  expect(decoder.current_maturity).toBe('supported');
  const published = decoder.published_support;
  expect(published).toBeTruthy();
  expect(published.maturity).toBe('supported');
  expect(published.release).toMatch(/^v\d+\.\d+\.\d+$/);
  expect(published.release_url).toContain(`/releases/tag/${published.release}`);
  expect(published.evidence_asset).toBe(`envelope-support-evidence-${published.release}.tar.gz`);
  return published;
}

const matchingPublication = matrix => decoderPublication(matrix, 'envelope-matching');
const mlePublication = matrix => decoderPublication(matrix, 'envelope-mle');

const matchingBadge = page => page.locator('[data-decoder-badge="envelope-matching"]');
const mleBadge = page => page.locator('[data-decoder-badge="envelope-mle"]');

test('tutorial and concepts render the version-bound Supported label with evidence links', async ({ page, request }, testInfo) => {
  const matrix = await servedMatrix(request);
  const published = matchingPublication(matrix);
  const mlePublished = mlePublication(matrix);

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
  await expect(conceptMle.locator('a')).toHaveText(`Envelope MLE · Supported · ${mlePublished.release}`);
  await expect(conceptMle.locator('a')).toHaveAttribute('href', mlePublished.release_url);
  await expect(conceptMle).toHaveAttribute('data-release', mlePublished.release);
  await expect(conceptMle).toHaveAttribute('data-evidence-asset', mlePublished.evidence_asset);
  await expect(page.locator('[data-evidence-link="envelope-mle"]')).toHaveAttribute('href', mlePublished.release_url);
  await expect(boundary).toContainText('four exact measured points');
  await expect(boundary).toContainText('requires an ILP-capable build');
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
  await expect(mleBadge(page)).toHaveText('Envelope MLE · Beta');
  await expect(mleBadge(page).locator('a')).toHaveCount(0);
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

test('failed MLE evidence keeps only MLE at Beta', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  matrix.decoders['envelope-mle'].published_support.evidence_asset = '';
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  const matching = matchingPublication(matrix);
  await expect(matchingBadge(page).locator('a')).toHaveText(`Envelope matching · Supported · ${matching.release}`);
  await expect(mleBadge(page)).toHaveText('Envelope MLE · Beta');
  await expect(mleBadge(page).locator('a')).toHaveCount(0);
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
  await expect(matchingBadge(page)).toHaveText('Envelope matching · Beta');
  await expect(matchingBadge(page).locator('a')).toHaveCount(0);
  await expect(matchingBadge(page)).not.toContainText('Supported');
  await expect(mleBadge(page)).toHaveText('Envelope MLE · Beta');
  await expect(mleBadge(page).locator('a')).toHaveCount(0);
  await expect(mleBadge(page)).not.toContainText('Supported');
});

test('the previous Matching-only release does not promote MLE', async ({ page, request }) => {
  const matrix = await servedMatrix(request);
  // v0.3.1 published Matching only; MLE had no publication record.
  delete matrix.decoders['envelope-mle'].published_support;
  matrix.decoders['envelope-mle'].current_maturity = 'beta';
  matrix.decoders['envelope-mle'].proposed_release_maturity = 'beta';
  await page.route(MATRIX_ROUTE, route => route.fulfill({ json: matrix }));
  await page.goto('/atom-loss-concepts/');
  const published = matchingPublication(matrix);
  await expect(matchingBadge(page).locator('a')).toHaveText(`Envelope matching · Supported · ${published.release}`);
  const mle = mleBadge(page);
  await expect(mle).toHaveText('Envelope MLE · Beta');
  await expect(mle.locator('a')).toHaveCount(0);
  await expect(mle).not.toContainText('Supported');
});
