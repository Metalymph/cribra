import assert from "node:assert/strict";
import { resolve } from "node:path";
import test from "node:test";

import { distDir, loadPackage } from "../helpers.mjs";

test("filesystem path load failure is retryable, then succeeds", async () => {
  const api = await loadPackage();
  await assert.rejects(
    api.init({ wasm: "/definitely/missing/cribra_bg.wasm" }),
    (error) => error.code === "wasm_load_failed" && error.domain === "init",
  );
  assert.equal(api.isInitialized(), false);

  await api.init({ wasm: resolve(distDir, "wasm/cribra_bg.wasm") });
  assert.equal(api.isInitialized(), true);
});
