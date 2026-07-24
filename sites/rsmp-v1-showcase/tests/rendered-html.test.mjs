import assert from "node:assert/strict";
import test from "node:test";

async function render() {
  const workerUrl = new URL("../dist/server/index.js", import.meta.url);
  workerUrl.searchParams.set("test", `${process.pid}-${Date.now()}`);
  const { default: worker } = await import(workerUrl.href);

  return worker.fetch(
    new Request("http://localhost/", {
      headers: { accept: "text/html" },
    }),
    {
      ASSETS: {
        fetch: async () => new Response("Not found", { status: 404 }),
      },
    },
    {
      waitUntil() {},
      passThroughOnException() {},
    },
  );
}

test("server-renders the RSMP v1 showcase contract", async () => {
  const response = await render();
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /^text\/html\b/i);

  const html = await response.text();
  assert.match(html, /RSMP v1/);
  assert.match(html, /11\.98%/);
  assert.match(html, /57\.14%/);
  assert.match(html, /1\.516 GB/);
  assert.match(html, /181\.668 MB/);
  assert.match(html, /pack_samples/);
  assert.match(html, /unpack_samples/);
  assert.match(html, /verify_only/);
  assert.match(html, /Projected/);
  assert.match(html, /requires the original circuit, not a DEM/i);
  assert.match(html, /Sweep-bit circuits are unsupported/i);
  assert.match(html, /sequential access/i);
  assert.match(html, /clean exported checkout/i);
  assert.match(html, /non-hermetic checker behavior/i);
  assert.doesNotMatch(html, /codex-preview|Building your site|react-loading-skeleton/);
});
