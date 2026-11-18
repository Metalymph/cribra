import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { resolve } from "node:path";
import test from "node:test";

import { distDir, loadPackage } from "../helpers.mjs";

test("init accepts an HTTP URL through the Node loader", async () => {
  const api = await loadPackage();
  const bytes = await readFile(resolve(distDir, "wasm/cribra_bg.wasm"));
  const server = createServer((request, response) => {
    response.writeHead(200, {
      "Content-Type": "application/wasm",
      "Content-Length": String(bytes.length),
    });
    response.end(bytes);
  });

  await new Promise((resolvePromise, rejectPromise) => {
    server.once("error", rejectPromise);
    server.listen(0, "127.0.0.1", resolvePromise);
  });

  try {
    const address = server.address();
    assert.notEqual(address, null);
    assert.equal(typeof address, "object");
    const url = `http://127.0.0.1:${address.port}/cribra_bg.wasm`;
    await api.init({ wasm: url });
    assert.equal(api.isInitialized(), true);
  } finally {
    await new Promise((resolvePromise) => server.close(resolvePromise));
  }
});
