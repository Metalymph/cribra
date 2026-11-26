import assert from "node:assert/strict";
import test from "node:test";

import {
  assertLock,
  assertUnverifiedOutput,
  mergeLock,
  platformKey,
  resolveLockMode,
} from "./artifact-lock.mjs";

const artifact = {
  wasmBindgenVersion: "0.2.126",
  wasmOptVersion: "133",
  productionWasm: { bytes: 10, sha256: "wasm-digest" },
};
const glue = { bytes: 5, sha256: "glue-digest" };
const raw = { bytes: 20, sha256: "raw-digest" };

function lockFor(key = "linux-x64", overrides = {}) {
  return {
    profile: "flow",
    features: ["transform"],
    wasmBindgenVersion: "0.2.126",
    wasmOptVersion: "133",
    artifacts: {
      [key]: {
        productionWasm: artifact.productionWasm,
        glue,
        rawWasm: raw,
        ...overrides,
      },
    },
  };
}

test("platformKey is platform-arch", () => {
  assert.equal(platformKey("linux", "x64"), "linux-x64");
});

test("a normal build verifies the lock", () => {
  assert.equal(resolveLockMode({}), "verify");
});

test("allow-unverified without an override still verifies", () => {
  assert.equal(resolveLockMode({ allowUnverified: true }), "verify");
});

test("write-lock is explicit", () => {
  assert.equal(resolveLockMode({ writeLock: true }), "write");
});

test("skip-build requires allow-unverified", () => {
  assert.throws(
    () => resolveLockMode({ skipBuild: true }),
    /--skip-build with --allow-unverified/,
  );
  assert.equal(
    resolveLockMode({ skipBuild: true, allowUnverified: true }),
    "unverified",
  );
});

test("artifact overrides require skip-build and allow-unverified", () => {
  assert.throws(
    () => resolveLockMode({ wasmOverride: true }),
    /--skip-build with --allow-unverified/,
  );
  assert.throws(
    () => resolveLockMode({ glueOverride: true, allowUnverified: true }),
    /--skip-build with --allow-unverified/,
  );
  assert.equal(
    resolveLockMode({
      skipBuild: true,
      wasmOverride: true,
      glueOverride: true,
      allowUnverified: true,
    }),
    "unverified",
  );
});

test("unverified builds cannot write the release dist", () => {
  assert.throws(
    () =>
      assertUnverifiedOutput({
        mode: "unverified",
        targetOutDir: "/repo/packages/cribra-guard/dist",
        releaseOutDir: "/repo/packages/cribra-guard/dist",
      }),
    /refusing to replace the release dist/,
  );
  assert.doesNotThrow(() =>
    assertUnverifiedOutput({
      mode: "unverified",
      targetOutDir: "/tmp/failure-dist",
      releaseOutDir: "/repo/packages/cribra-guard/dist",
    }),
  );
  assert.doesNotThrow(() =>
    assertUnverifiedOutput({
      mode: "verify",
      targetOutDir: "/repo/packages/cribra-guard/dist",
      releaseOutDir: "/repo/packages/cribra-guard/dist",
    }),
  );
});

test("missing lock is rejected", () => {
  assert.throws(
    () =>
      assertLock(undefined, {
        key: "linux-x64",
        artifact,
        glue,
        raw,
      }),
    /artifact\.lock\.json is missing/,
  );
});

test("unknown host is rejected", () => {
  assert.throws(
    () =>
      assertLock(lockFor("linux-x64"), {
        key: "darwin-arm64",
        artifact,
        glue,
        raw,
      }),
    /no entry for darwin-arm64/,
  );
});

test("mismatched production digest is rejected", () => {
  const lock = lockFor("linux-x64", {
    productionWasm: { bytes: 10, sha256: "other" },
  });
  assert.throws(
    () => assertLock(lock, { key: "linux-x64", artifact, glue, raw }),
    /production WASM sha256/,
  );
});

test("mismatched raw digest is rejected when pinned", () => {
  const lock = lockFor("linux-x64", {
    rawWasm: { bytes: 20, sha256: "other" },
  });
  assert.throws(
    () => assertLock(lock, { key: "linux-x64", artifact, glue, raw }),
    /raw WASM sha256/,
  );
});

test("a matching lock passes even without a raw entry", () => {
  const lock = lockFor("linux-x64");
  delete lock.artifacts["linux-x64"].rawWasm;
  assert.doesNotThrow(() =>
    assertLock(lock, { key: "linux-x64", artifact, glue, raw }),
  );
});

test("mergeLock adds the host and preserves the others", () => {
  const existing = lockFor("darwin-arm64");
  const merged = mergeLock({
    existing,
    key: "linux-x64",
    artifact,
    glue,
    raw,
    features: ["transform"],
  });
  assert.deepEqual(Object.keys(merged.artifacts), [
    "darwin-arm64",
    "linux-x64",
  ]);
  assert.equal(merged.artifacts["darwin-arm64"].productionWasm.sha256, "wasm-digest");
  assert.equal(merged.artifacts["linux-x64"].productionWasm.sha256, "wasm-digest");
});

test("mergeLock keeps an existing raw entry when raw is not measured", () => {
  const existing = lockFor("linux-x64");
  const merged = mergeLock({
    existing,
    key: "linux-x64",
    artifact,
    glue,
    raw: null,
    features: ["transform"],
  });
  assert.deepEqual(merged.artifacts["linux-x64"].rawWasm, raw);
});
