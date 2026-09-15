import { test, expect } from '@playwright/test';

test('atom-loss evidence exposes sampling costs, figures and downloadable measurements', async ({ page }) => {
  await page.goto('/atom-loss/#atom-loss-results');
  await expect(page).toHaveURL(/atom-loss-evidence\/#atom-loss-results$/);
  const figures = page.locator('.loss-result-figure');
  await expect(figures).toHaveCount(2);
  for (const figure of await figures.all()) {
    await figure.scrollIntoViewIfNeeded();
    await expect.poll(() => figure.locator('img').evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
    await expect(figure.locator('figcaption')).toContainText(/shots|Distance/);
    const href = await figure.locator('a').first().getAttribute('href');
    const response = await page.request.get(new URL(href, page.url()).href);
    expect(response.ok()).toBe(true);
    expect(await response.text()).toContain('<svg');
  }
  const caveat = page.locator('.loss-sampling-caveat');
  await expect(caveat).toContainText('Python reference is unoptimized; timing boundaries differ.');
  await expect(caveat).toContainText('Rust parsing is excluded');
  await expect(caveat).toContainText('does not measure native Stim performance');
  const table = page.locator('.loss-sampling-table');
  await table.scrollIntoViewIfNeeded();
  await expect(table).toBeVisible();
  await expect(table.locator('caption')).toContainText('256-shot batch, in milliseconds');
  const sampling = await (await page.request.get('/data/atom-loss/sampling.json')).json();
  await expect(table.locator('tbody tr')).toHaveCount(sampling.length);
  const rounded = value => String(Math.round(value * 100) / 100);
  for (const sample of sampling) {
    const row = table.locator('tbody tr').filter({ hasText: `d = ${sample.distance}` });
    for (const backend of ['rust', 'reference']) {
      const times = sample[backend].records.map(run => (run.sample_seconds + run.packing_seconds) * 1000).sort((a, b) => a - b);
      expect(times).toHaveLength(3);
      await expect(row.locator(`[data-backend="${backend}"]`)).toHaveText(`${rounded(times[1])}(${rounded(times[0])}–${rounded(times[2])})`);
    }
  }
  const accuracyTime = await (await page.request.get('/data/atom-loss/accuracy-time.svg')).text();
  expect(accuracyTime).toContain('RustQEC envelope matching (streaming)');
  expect(accuracyTime).toContain('RustQEC envelope matching (batch)');
  const timing = page.locator('.loss-timing-figure img');
  await timing.scrollIntoViewIfNeeded();
  await expect.poll(() => timing.evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
  await expect(page.locator('.loss-timing-figure figcaption')).toContainText('Native offline batch');
  const timingCsv = await (await page.request.get('/data/atom-loss/timing-sweep.csv')).text();
  expect(timingCsv.trim().split('\n')).toHaveLength(181);
  const seeds = await (await page.request.get('/data/atom-loss/accuracy-seeds.json')).json();
  expect(seeds.cases).toHaveLength(48);
  const seedFigure = page.locator('img[src*="accuracy-seeds.svg"]');
  await seedFigure.scrollIntoViewIfNeeded();
  await expect.poll(() => seedFigure.evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
  const samplingCheck = await (await page.request.get('/data/atom-loss/correctness.json')).json();
  expect(samplingCheck.analytic_noise_controls.distribution_probes.cases).toHaveLength(26);
  expect(samplingCheck.analytic_noise_controls.distribution_probes.channel_replacement_mutations.DEPOLARIZE2_ix_only.rejected).toBe(true);
  const full = page.locator('.loss-full-figure img');
  await full.scrollIntoViewIfNeeded();
  await expect.poll(() => full.evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
  await expect(page.locator('.loss-full-figure figcaption')).toContainText('0.000599');
  await expect(full).toHaveAttribute('src', '../data/atom-loss/logical-error-rate-full.svg');
  await expect(full).toBeVisible();
  const detail = page.locator('.loss-detail-figure img');
  await expect(detail).not.toBeVisible();
  await page.locator('.loss-detail-sweep summary').click();
  await expect(detail).toBeVisible();
  await expect.poll(() => detail.evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
  const stages = page.locator('.loss-stage-figure img');
  await stages.scrollIntoViewIfNeeded();
  await expect.poll(() => stages.evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
  await expect(page.locator('.loss-stage-figure figcaption')).toContainText('not establish a matching-kernel');
  const chain = await (await page.request.get('/data/atom-loss/chain-correctness.json')).json();
  expect(chain.status).toBe('PASS');
  for (const backend of ['pymatching-envelope', 'envelope-matching-offline']) {
    expect(chain.backends[backend].checked_rows).toBe(752);
    expect(chain.backends[backend].rejected_rows).toEqual([]);
    for (const mutation of ['empty_loss_mapping', 'relative_weights']) {
      expect(chain.graph_adapter_controls[mutation][backend].outcome).toBe('oracle_rejected');
      expect(chain.graph_adapter_controls[mutation][backend].rejected_rows.length).toBeGreaterThan(0);
    }
  }
  expect(chain.compiler_output_mutations_rejected).toEqual({ pauli_weight: true, loss_candidate: true });
  await page.locator('.loss-evidence-downloads summary').click();
  const sourceLink = page.getByRole('link', { name: 'Source and build manifest' });
  const sourceResponse = await page.request.get(new URL(await sourceLink.getAttribute('href'), page.url()).href);
  expect(sourceResponse.ok()).toBe(true);
  const source = await sourceResponse.json();
  const provenance = await (await page.request.get('/data/atom-loss/provenance-all.json')).json();
  expect(source.working_tree_dirty).toBe(false);
  expect(provenance.source_commit).toBe(source.source_commit);
  expect(source.inputs['rustqec-cli/src/decode.rs']).toBeDefined();
  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('link', { name: 'Download result table (CSV)' }).click();
  expect((await downloadPromise).suggestedFilename()).toBe('summary.csv');
  const archiveDownload = page.waitForEvent('download');
  await page.getByRole('link', { name: 'Download all 16 corpora and 198 prediction files (ZIP)' }).click();
  expect((await archiveDownload).suggestedFilename()).toBe('shot-data-v1.zip');
  const data = await (await page.request.get('/data/atom-loss/decoding.json')).json();
  expect(data).toHaveLength(15);
  expect(data.every(row => row.shots === 5000)).toBe(true);
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
});
