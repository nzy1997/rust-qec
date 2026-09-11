import { expect, test } from '@playwright/test';

test('favicon formats load from home and nested documentation paths', async ({ page, request }) => {
  for (const path of ['/', '/docs/', '/qp101/protocol/']) {
    await page.goto(path);
    const icons = await page.locator('head link[rel="icon"]').evaluateAll(links =>
      links.map(link => ({ type: link.type, href: link.href, sizes: link.sizes.value })),
    );
    expect(icons.map(icon => icon.type)).toEqual(['image/x-icon', 'image/png', 'image/svg+xml']);
    for (const icon of icons) {
      expect(new URL(icon.href).searchParams.get('v')).toBeTruthy();
      const response = await request.get(icon.href);
      expect(response.ok(), icon.href).toBe(true);
      const bytes = await response.body();
      if (icon.type === 'image/png') {
        expect(response.headers()['content-type']).toContain('image/png');
        expect(bytes.subarray(0, 8)).toEqual(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
        expect([bytes.readUInt32BE(16), bytes.readUInt32BE(20)]).toEqual([32, 32]);
        expect(icon.sizes).toBe('32x32');
      } else if (icon.type === 'image/x-icon') {
        expect(bytes.readUInt16LE(0)).toBe(0);
        expect(bytes.readUInt16LE(2)).toBe(1);
        expect(bytes.readUInt16LE(4)).toBe(3);
        const sizes = [];
        for (let index = 0; index < 3; index++) {
          const entry = 6 + 16 * index;
          sizes.push([bytes[entry], bytes[entry + 1]]);
          const length = bytes.readUInt32LE(entry + 8);
          const offset = bytes.readUInt32LE(entry + 12);
          expect(length).toBeGreaterThan(0);
          expect(offset + length).toBeLessThanOrEqual(bytes.length);
        }
        expect(sizes).toEqual([[16, 16], [32, 32], [48, 48]]);
      } else {
        expect(response.headers()['content-type']).toContain('image/svg+xml');
        expect(bytes.toString()).toContain('<title id="title">RustQEC</title>');
        expect(icon.sizes).toBe('any');
      }
    }
  }
});
