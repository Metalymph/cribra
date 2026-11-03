import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import test from "node:test";

import { distDir, loadPackage } from "../helpers.mjs";

test("init accepts explicit Uint8Array bytes", async () => {
  const api = await loadPackage();
  const bytes = await readFile(resolve(distDir, "wasm/cribra_bg.wasm"));
  assert.equal(bytes instanceof Uint8Array, true);
  await api.init({ wasm: bytes });
  assert.equal(api.isInitialized(), true);
});
