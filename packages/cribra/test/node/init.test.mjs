import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  loadPackage,
  packageVersion,
  profile,
} from "../helpers.mjs";

function expectCribra(error, code, domain) {
  assert.equal(error.name, "CribraError");
  assert.equal(error.code, code);
  if (domain !== undefined) assert.equal(error.domain, domain);
  return true;
}

test("pre-init calls fail structurally without auto-initialization", async () => {
  const api = await loadPackage();
  assert.equal(api.isInitialized(), false);
  assert.throws(
    () => api.createScanner(),
    (error) => expectCribra(error, "not_initialized", "init"),
  );
  assert.throws(
    () => api.getCapabilities(),
    (error) => expectCribra(error, "not_initialized", "init"),
  );
  assert.throws(
    () => api.getVersion(),
    (error) => expectCribra(error, "not_initialized", "init"),
  );
  assert.equal(api.isInitialized(), false);
});

test("invalid init options do not poison retry", async () => {
  const api = await loadPackage();
  await assert.rejects(
    api.init({ wasm: 42 }),
    (error) => expectCribra(error, "invalid_argument", "init"),
  );
  assert.equal(api.isInitialized(), false);
});

test("failed instantiation is retryable and concurrent init shares work", async () => {
  const api = await loadPackage();
  await assert.rejects(
    api.init({ wasm: new Uint8Array([0, 1, 2, 3]) }),
    (error) => expectCribra(error, "wasm_instantiate_failed", "init"),
  );
  assert.equal(api.isInitialized(), false);

  const first = api.init();
  const second = api.init();
  const third = api.init();
  assert.equal(first, second, "concurrent calls share one promise");
  assert.equal(second, third, "concurrent calls share one promise");
  await Promise.all([first, second, third]);
  assert.equal(api.isInitialized(), true);

  await api.init();
  await Promise.all([api.init(), api.init()]);
  assert.equal(api.isInitialized(), true);

  const manifest = JSON.parse(
    await readFile(new URL("../../package.json", import.meta.url), "utf8"),
  );
  const capabilities = api.getCapabilities();
  assert.equal(Object.isFrozen(capabilities), true);
  assert.equal(capabilities.backend, "wasm");
  assert.equal(capabilities.profile, profile);
  assert.equal(capabilities.singleSourceScan, true);
  assert.equal(capabilities.batchScan, true);
  assert.equal(capabilities.streaming, true);
  assert.equal(capabilities.incremental, true);
  assert.equal(capabilities.ruleMetadata, true);
  assert.equal(capabilities.explanations, true);
  assert.equal(capabilities.parallel, false);
  assert.equal(capabilities.utf16OffsetConversion, true);
  const transform = profile === "flow";
  assert.equal(capabilities.redaction, transform);
  assert.equal(capabilities.templating, transform);
  assert.equal(capabilities.pseudonymization, transform);
  assert.equal(capabilities.synthesis, transform);
  assert.equal(capabilities.shareBundle, transform);
  assert.equal(api.getCapabilities(), capabilities, "frozen snapshot identity");

  const version = api.getVersion();
  assert.equal(Object.isFrozen(version), true);
  assert.equal(version.package, manifest.version);
  assert.equal(version.package, packageVersion);
  assert.match(version.wasmAdapter, /^\d+\.\d+\.\d+/);
  assert.equal("core" in version, false);

  const scanner = api.createScanner({ builtins: "none" });
  assert.equal(scanner.rulesCount, 0);
  scanner.dispose();
});
