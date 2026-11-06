/** Static, declaration, runtime-export, and package-artifact checks. */

import { execFileSync } from "node:child_process";
import { readFile, readdir, stat } from "node:fs/promises";
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
  "IncrementalError",
  "IncrementalErrorCode",
  "ProfileKind",
  "RemediationKind",
  "RuleMetadataView",
  "ScanBatchResult",
  "ScanEngine",
  "ScanEngineBuilder",
  "ScanSummaryView",
  "ScannerBuildError",
  "ScannerBuildErrorCode",
  "ShareBundleBuilder",
  "ShareBundleError",
  "ShareBundleErrorCode",
  "ShareManifestView",
  "StreamError",
  "StreamErrorCode",
  "TransformError",
  "TransformErrorCode",
];

const REQUIRED_SESSION_DECLARATIONS = [
  /\binterface\s+StreamOptions\b/,
  /\binterface\s+StreamLimits\b/,
  /\binterface\s+StreamSession\b/,
  /\binterface\s+IncrementalOptions\b/,
  /\binterface\s+IncrementalLimits\b/,
  /\binterface\s+IncrementalSession\b/,
  /\binterface\s+IncrementalSnapshot\b/,
  /\bpush\s*\(\s*chunk\s*:\s*Uint8Array\s*\)\s*:\s*void\b/,
  /\bfinish\s*\(\s*\)\s*:\s*ScanResult\b/,
  /\bapplyEdit\s*\(\s*expectedRevision\s*:\s*bigint\s*,\s*start\s*:\s*number\s*,\s*end\s*:\s*number\s*,\s*replacement\s*:\s*string\s*\)\s*:\s*bigint\b/,
  /\bsnapshot\s*\(\s*\)\s*:\s*IncrementalSnapshot\b/,
  /\bstream\s*\(\s*options\?\s*:\s*StreamOptions\s*\)\s*:\s*StreamSession\b/,
  /\bincremental\s*\(\s*source\s*:\s*string\s*,\s*options\?\s*:\s*IncrementalOptions\s*\)\s*:\s*IncrementalSession\b/,
  /\binterface\s+RedactRequest\b/,
  /\binterface\s+TemplateRequest\b/,
  /\binterface\s+PseudonymizeRequest\b/,
  /\binterface\s+SynthesizeRequest\b/,
  /\binterface\s+TransformResult\b/,
  /\btype\s+TransformRequest\s*=/,
  /\binterface\s+ShareBundleOptions\b/,
  /\btype\s+ShareMode\s*=/,
  /\btype\s+ShareModeKind\s*=/,
  /\binterface\s+ShareBundleEntry\b/,
  /\binterface\s+ShareManifest\b/,
  /\binterface\s+ShareBundle\b/,
  /\btransform\s*\(\s*source\s*:\s*string\s*,\s*request\s*:\s*TransformRequest\s*\)\s*:\s*TransformResult\b/,
  /\bcreateShareBundle\s*\(\s*scanner\s*:\s*Scanner\b/,
];

const EXPECTED_EXPORTS = [
  "CribraError",
  "byteSpanToTextRange",
  "createScanner",
  "createShareBundle",
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

async function assertNoSourceMaps(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      await assertNoSourceMaps(path);
    } else if (entry.name.endsWith(".map")) {
      throw new Error(`package artifact contains a source map: ${path}`);
    }
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
  let combined = "";
  for (const { file, text } of files) {
    combined += `\n${text}`;
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
  for (const pattern of REQUIRED_SESSION_DECLARATIONS) {
    if (!pattern.test(combined)) {
      throw new Error(
        `public declaration graph is missing session surface ${pattern}`,
      );
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

const LIFECYCLE_SCRIPTS = [
  "preinstall",
  "install",
  "postinstall",
  "prepare",
  "prepack",
  "postpack",
  "prepublish",
  "prepublishOnly",
];

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
    if (
      file.startsWith("src/") ||
      file.startsWith("test/") ||
      file.startsWith("scripts/") ||
      file.startsWith("qualification/") ||
      file.endsWith(".map") ||
      file.endsWith(".tgz")
    ) {
      throw new Error(`npm pack would include forbidden file ${file}`);
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
  if (manifest.engines?.node !== ">=22") {
    throw new Error(
      `package must declare engines.node >=22, got ${manifest.engines?.node}`,
    );
  }
  for (const key of [
    "dependencies",
    "optionalDependencies",
    "peerDependencies",
    "bundleDependencies",
  ]) {
    if (manifest[key] !== undefined) {
      throw new Error(`package must not declare ${key}`);
    }
  }
  for (const script of LIFECYCLE_SCRIPTS) {
    if (manifest.scripts?.[script] !== undefined) {
      throw new Error(`package must not define the ${script} lifecycle script`);
    }
  }
  const expectedFiles = ["dist", "README.md", "LICENSE"];
  if (JSON.stringify(manifest.files) !== JSON.stringify(expectedFiles)) {
    throw new Error(`unexpected files list: ${JSON.stringify(manifest.files)}`);
  }

  await assertNoSourceMaps(distDir);
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
