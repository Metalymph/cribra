/**
 * Packaging-boundary regression tests.
 *
 * These run after both builds:
 * - `npm run build:qualification` produces the WASM host qualification `dist/`;
 * - `npm run build:content-qualification` produces the separate
 *   `.build/content-dist/` content package.
 *
 * The production manifest must remain content-free. The qualification manifest
 * must be localhost-only. The content script must be a classic IIFE with no
 * WASM, network, storage, remote code, or page-world injection.
 */

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const packageRoot = resolve(import.meta.dirname, "..");
const productionDist = resolve(packageRoot, "dist");
const contentDist = resolve(packageRoot, ".build/content-dist");

function requireBuild(path, label) {
  assert.equal(
    existsSync(path),
    true,
    `${label} is missing at ${path}; run the qualification builds first`,
  );
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function listFiles(root) {
  if (!existsSync(root)) return [];
  const files = [];
  for (const entry of await readdir(root, {
    recursive: true,
    withFileTypes: true,
  })) {
    if (entry.isFile()) files.push(join(entry.parentPath, entry.name));
  }
  return files;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

test("production manifest remains content-free", async () => {
  requireBuild(resolve(productionDist, "manifest.json"), "production manifest");
  const manifest = await readJson(resolve(productionDist, "manifest.json"));
  assert.equal(manifest.manifest_version, 3);
  assert.equal(manifest.background?.service_worker, "service-worker.js");
  assert.equal(manifest.background?.type, "module");
  assert.deepEqual(manifest.permissions ?? [], []);
  assert.deepEqual(manifest.host_permissions ?? [], []);
  assert.deepEqual(manifest.content_scripts ?? [], []);
  assert.deepEqual(manifest.web_accessible_resources ?? [], []);
  assert.equal(existsSync(resolve(productionDist, "content-script.js")), false);
  assert.equal(
    existsSync(resolve(productionDist, "manifest.qualification.json")),
    false,
  );
  assert.equal(
    existsSync(resolve(productionDist, "fixtures/fixture-support.js")),
    false,
  );
});

test("content qualification manifest is localhost-only and minimal", async () => {
  requireBuild(
    resolve(contentDist, "manifest.json"),
    "content qualification manifest",
  );
  const manifest = await readJson(resolve(contentDist, "manifest.json"));
  assert.equal(manifest.manifest_version, 3);
  assert.deepEqual(manifest.permissions ?? [], []);
  assert.deepEqual(manifest.host_permissions ?? [], ["http://127.0.0.1/*"]);
  assert.deepEqual(manifest.web_accessible_resources ?? [], []);

  assert.equal(Array.isArray(manifest.content_scripts), true);
  assert.equal(manifest.content_scripts.length, 1);
  const [contentScript] = manifest.content_scripts;
  assert.ok(contentScript !== undefined);
  assert.deepEqual(contentScript.matches ?? [], ["http://127.0.0.1/*"]);
  assert.deepEqual(contentScript.js ?? [], ["content-script.js"]);
  assert.equal(contentScript.run_at, "document_start");
  assert.equal(contentScript.all_frames, false);

  const forbidden = [
    "debugger",
    "webRequestBlocking",
    "scripting",
    "storage",
    "tabs",
    "nativeMessaging",
  ];
  for (const permission of manifest.permissions ?? []) {
    assert.equal(forbidden.includes(permission), false);
  }

  for (const file of [
    "content-script.js",
    "service-worker.js",
    "wasm/cribra_bg.wasm",
    "artifact.json",
  ]) {
    assert.equal(
      existsSync(resolve(contentDist, file)),
      true,
      `missing content qualification file: ${file}`,
    );
  }
  assert.equal(existsSync(resolve(contentDist, "qualification.html")), false);
  assert.equal(existsSync(resolve(contentDist, "qualification.js")), false);
  assert.equal(
    existsSync(resolve(contentDist, "fixtures/fixture-support.js")),
    false,
  );
});

test("content script is a source-free classic IIFE", async () => {
  const path = resolve(contentDist, "content-script.js");
  requireBuild(path, "content script");
  const source = await readFile(path, "utf8");

  assert.match(source, /(?:^|\n)\s*(?:"use strict";\s*)?\(\(\)\s*=>\s*\{/);
  assert.doesNotMatch(source, /^\s*import\s/m);
  assert.doesNotMatch(source, /^\s*export\s/m);
  assert.doesNotMatch(source, /\bimport\s*\(/);

  const forbidden = [
    /WebAssembly\b/,
    /wasm_bindgen/,
    /instantiateStreaming/,
    /compileStreaming/,
    /wasm\/cribra_bg\.wasm/,
    /innerHTML/,
    /outerHTML/,
    /insertAdjacentHTML/,
    /document\.write/,
    /\beval\s*\(/,
    /new\s+Function\s*\(/,
    /\bfetch\s*\(/,
    /XMLHttpRequest/,
    /WebSocket/,
    /sendBeacon/,
    /chrome\.storage/,
    /localStorage/,
    /sessionStorage/,
    /indexedDB/,
    /https?:\/\//,
    /\bdebugger\b/,
    /webRequestBlocking/,
    /chrome\.scripting/,
    /chrome\.tabs/,
    /console\./,
    /__CRIBRA_ARTIFACT/,
  ];
  for (const pattern of forbidden) {
    assert.doesNotMatch(source, pattern, `content script matched ${pattern}`);
  }

  // The qualification build deliberately includes the isolated-world hook.
  assert.match(source, /__cribraGuardQualification/);
});

test("the content hook is absent from the production package", async () => {
  const productionFiles = await listFiles(productionDist);
  assert.equal(
    productionFiles.some((file) => file.endsWith("content-script.js")),
    false,
  );
  for (const file of productionFiles) {
    const bytes = await readFile(file);
    assert.equal(
      bytes.includes(Buffer.from("__cribraGuardQualification", "utf8")),
      false,
      `production package contains the content hook marker: ${file}`,
    );
  }
});

test("content snapshot source limit matches the scanner protocol limit", async () => {
  const protocol = await readFile(
    resolve(packageRoot, "src/protocol.ts"),
    "utf8",
  );
  const snapshot = await readFile(
    resolve(packageRoot, "src/content/snapshot.ts"),
    "utf8",
  );
  const protocolMatch = /MAX_SOURCE_LENGTH\s*=\s*([\d_]+)/.exec(protocol);
  const snapshotMatch =
    /MAX_SNAPSHOT_SOURCE_LENGTH\s*=\s*([\d_]+)/.exec(snapshot);
  assert.ok(protocolMatch !== null);
  assert.ok(snapshotMatch !== null);
  assert.equal(
    snapshotMatch[1].replace(/_/g, ""),
    protocolMatch[1].replace(/_/g, ""),
  );
});

test("artifact reuse requires a matching lock entry", async () => {
  const glue = resolve(packageRoot, ".build/flow/cribra.js");
  requireBuild(glue, "Flow glue");
  const temporary = await mkdtemp(resolve(tmpdir(), "cribra-reuse-"));
  try {
    const artifactDir = resolve(temporary, "artifact");
    const outDir = resolve(temporary, "out");
    await mkdir(artifactDir, { recursive: true });
    await writeFile(
      resolve(artifactDir, "cribra_bg.wasm"),
      Buffer.from("not the locked artifact"),
    );
    await cp(glue, resolve(artifactDir, "cribra.js"));

    const result = spawnSync(
      process.execPath,
      [
        resolve(packageRoot, "scripts/build.mjs"),
        "--reuse-artifact",
        "--artifact-dir",
        artifactDir,
        "--out",
        outDir,
      ],
      { cwd: packageRoot, encoding: "utf8" },
    );

    assert.notEqual(result.status, 0);
    assert.match(
      `${result.stdout}\n${result.stderr}`,
      /artifact\.lock\.json mismatch|sha256/,
    );
    assert.equal(existsSync(outDir), false);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});

test("WASM artifacts and provenance remain unchanged", async () => {
  const lock = await readJson(resolve(packageRoot, "artifact.lock.json"));
  const key = `${process.platform}-${process.arch}`;
  const locked = lock.artifacts?.[key];
  assert.ok(locked !== undefined, `artifact.lock.json has no entry for ${key}`);

  const productionWasm = await readFile(
    resolve(productionDist, "wasm/cribra_bg.wasm"),
  );
  const contentWasm = await readFile(
    resolve(contentDist, "wasm/cribra_bg.wasm"),
  );
  assert.equal(productionWasm.byteLength, locked.productionWasm.bytes);
  assert.equal(sha256(productionWasm), locked.productionWasm.sha256);
  assert.equal(contentWasm.byteLength, productionWasm.byteLength);
  assert.equal(sha256(contentWasm), sha256(productionWasm));

  const productionArtifact = await readJson(
    resolve(productionDist, "artifact.json"),
  );
  const contentArtifact = await readJson(
    resolve(contentDist, "artifact.json"),
  );
  assert.equal(productionArtifact.artifact.sha256, locked.productionWasm.sha256);
  assert.equal(contentArtifact.artifact.sha256, locked.productionWasm.sha256);
  assert.equal(productionArtifact.extension.contentScript, undefined);
  assert.ok(contentArtifact.extension.contentScript !== undefined);
  assert.equal(contentArtifact.extension.contentScript.qualificationOnly, true);
  assert.equal(
    contentArtifact.extension.contentScript.sha256,
    sha256(await readFile(resolve(contentDist, "content-script.js"))),
  );
});
