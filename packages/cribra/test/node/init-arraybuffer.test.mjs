import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import test from "node:test";

import { distDir, loadPackage } from "../helpers.mjs";

test("init accepts an explicit ArrayBuffer", async () => {
  const api = await loadPackage();
  const bytes = await readFile(resolve(distDir, "wasm/cribra_bg.wasm"));
  const buffer = bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  );
  assert.equal(buffer instanceof ArrayBuffer, true);
  await api.init({ wasm: buffer });
  assert.equal(api.isInitialized(), true);
});
