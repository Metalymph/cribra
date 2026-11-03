/** Static, declaration, runtime-export, and package-artifact checks. */

import { execFileSync } from "node:child_process";
import { readFile, stat } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { isMain, packageRoot, parseArgs } from "./util.mjs";

const FORBIDDEN_DECLARATION_NAMES = [
  "BackendKind",
  "CandidateView",
  "CapabilityManifest",
  "DetectionModeKind",
  "ExplanationKind",
  "FindingConfidence",
  "FindingSeverity",
  "FindingView",
  "ProfileKind",
  "RemediationKind",
  "RuleMetadataView",
  "ScanBatchResult",
  "ScanEngine",
  "ScanEngineBuilder",
  "ScanSummaryView",
  "ScannerBuildError",
  "ScannerBuildErrorCode",
];

const EXPECTED_EXPORTS = [
  "CribraError",
  "byteSpanToTextRange",
  "createScanner",
  "getCapabilities",
  "getVersion",
  "init",
  "isInitialized",
];

async function exists(path) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

function relativeSpecifiers(text) {
  const specifiers = new Set();
  for (const match of text.matchAll(
    /(?:from\s+|import\s*\(\s*)["']([^"']+)["']/g,
  )) {
    specifiers.add(match[1]);
  }
  return specifiers;
}

async function walkDeclarations(entry) {
  const seen = new Set();
  const queue = [entry];
  const files = [];
  while (queue.length > 0) {
    const file = queue.pop();
    if (seen.has(file)) continue;
    seen.add(file);
    const text = await readFile(file, "utf8");
    files.push({ file, text });
    for (const specifier of relativeSpecifiers(text)) {
      if (!specifier.startsWith(".")) continue;
      const base = resolve(dirname(file), specifier);
      const candidate = base.endsWith(".js")
        ? base.replace(/\.js$/, ".d.ts")
        : `${base}.d.ts`;
      if (await exists(candidate)) queue.push(candidate);
    }
  }
  return files;
}

async function walkJavaScript(entry) {
  const seen = new Set();
  const queue = [entry];
  const files = [];
  while (queue.length > 0) {
    const file = queue.pop();
    if (seen.has(file)) continue;
    seen.add(file);
    const text = await readFile(file, "utf8");
    files.push({ file, text });
    for (const specifier of relativeSpecifiers(text)) {
      if (specifier.startsWith("node:")) {
        throw new Error(
          `browser graph contains a Node built-in import (${specifier}) in ${file}`,
        );
      }
      if (!specifier.startsWith(".")) continue;
      const resolved = resolve(dirname(file), specifier);
      if (await exists(resolved)) queue.push(resolved);
    }
  }
  return files;
}

async function checkDeclarations(distDir) {
  const files = await walkDeclarations(resolve(distDir, "index.d.ts"));
  for (const { file, text } of files) {
    for (const match of text.matchAll(/["']([^"']*\/wasm\/[^"']*)["']/g)) {
      throw new Error(
        `public declaration graph references generated WASM types: ${match[1]} (${file})`,
      );
    }
    for (const name of FORBIDDEN_DECLARATION_NAMES) {
      const pattern = new RegExp(`\\b${name}\\b`);
      if (pattern.test(text)) {
        throw new Error(
          `public declaration graph exposes generated name ${name} (${file})`,
        );
      }
    }
  }
  return files.length;
}

async function checkRuntime(distDir) {
  const module = await import(
    pathToFileURL(resolve(distDir, "index.node.js")).href
  );
  const actual = Object.keys(module).sort();
  if (JSON.stringify(actual) !== JSON.stringify(EXPECTED_EXPORTS)) {
    throw new Error(
      `unexpected runtime exports: ${actual.join(", ")}`,
    );
  }
  return actual.length;
}

async function checkAsset(distDir) {
  const wasmPath = resolve(distDir, "wasm/cribra_bg.wasm");
  const bytes = await readFile(wasmPath);
  if (bytes.length === 0) {
    throw new Error("packaged WASM asset is empty");
  }
  if (
    bytes[0] !== 0x00 ||
    bytes[1] !== 0x61 ||
    bytes[2] !== 0x73 ||
    bytes[3] !== 0x6d
  ) {
    throw new Error("packaged WASM asset has no WebAssembly magic header");
  }
  return bytes.length;
}

async function checkPack() {
  const output = execFileSync(
    "npm",
    ["pack", "--dry-run", "--json", "--ignore-scripts"],
    {
      cwd: packageRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      maxBuffer: 32 * 1024 * 1024,
    },
  );
  const parsed = JSON.parse(output);
  const files = new Set(parsed[0].files.map((entry) => entry.path));
  for (const required of [
    "dist/index.js",
    "dist/index.d.ts",
    "dist/index.browser.js",
    "dist/index.node.js",
    "dist/wasm/cribra.js",
    "dist/wasm/cribra_bg.wasm",
    "README.md",
    "LICENSE",
  ]) {
    if (!files.has(required)) {
      throw new Error(`npm pack would omit ${required}`);
    }
  }
  for (const file of files) {
    if (file.startsWith("src/") || file.startsWith("test/")) {
      throw new Error(`npm pack would include source/test file ${file}`);
    }
  }
  return parsed[0].files.length;
}

export async function checkPackage({ distDir }) {
  const manifest = JSON.parse(
    await readFile(resolve(packageRoot, "package.json"), "utf8"),
  );
  if (manifest.type !== "module") {
    throw new Error("package.json must be ESM-only");
  }
  if (manifest.private !== true) {
    throw new Error("provisional package must stay private");
  }
  if (manifest.dependencies !== undefined) {
    throw new Error("package must have no runtime dependencies");
  }

  await checkAsset(distDir);
  const declarationCount = await checkDeclarations(distDir);
  const exportCount = await checkRuntime(distDir);

  const browserFiles = await walkJavaScript(
    resolve(distDir, "index.browser.js"),
  );

  let packCount = 0;
  const defaultDist = resolve(packageRoot, "dist");
  if (resolve(distDir) === defaultDist) {
    packCount = await checkPack();
  }

  process.stdout.write(
    [
      `package check passed: ${declarationCount} declarations, ` +
        `${exportCount} runtime exports, ${browserFiles.length} browser modules` +
        (packCount === 0 ? "" : `, ${packCount} npm pack files`),
    ].join("") + "\n",
  );
  return manifest;
}

if (isMain(import.meta.url)) {
  const flags = parseArgs(process.argv.slice(2));
  const dist = flags.get("dist");
  const distDir =
    typeof dist === "string"
      ? resolve(packageRoot, dist)
      : resolve(packageRoot, "dist");
  await checkPackage({ distDir });
}
