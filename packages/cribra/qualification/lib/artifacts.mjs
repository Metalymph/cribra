/** Packing, extraction, integrity, and evidence helpers for consumer qualification. */

import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cp, mkdir, mkdtemp, readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises";
import { dirname, extname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import {
  assertReleaseIdentity,
  publishConfig,
  readManifest,
  tarballNames,
} from "../../scripts/identity.mjs";

const libDir = dirname(fileURLToPath(import.meta.url));

export const qualificationRoot = resolve(libDir, "..");
export const packageRoot = resolve(qualificationRoot, "..");
export const repoRoot = resolve(packageRoot, "..", "..");
export const targetRoot = resolve(repoRoot, "target/typescript-qualification");
export const artifactsRoot = resolve(targetRoot, "artifacts");
export const consumersRoot = resolve(targetRoot, "consumers");

/** Manifest-derived identity plus the exact tarball paths used by consumers. */
export async function artifactPaths() {
  const manifest = await readManifest();
  const names = tarballNames(manifest);
  return {
    manifest,
    names,
    flow: resolve(artifactsRoot, "flow", names.flow),
    core: resolve(artifactsRoot, "core", names.core),
  };
}

export const REQUIRED_FILES = [
  "LICENSE",
  "README.md",
  "package.json",
  "dist/index.js",
  "dist/index.d.ts",
  "dist/index.browser.js",
  "dist/index.node.js",
  "dist/generated/package-version.js",
  "dist/wasm/cribra.js",
  "dist/wasm/cribra_bg.wasm",
  "dist/wasm/cribra_bg.wasm.d.ts",
  "dist/wasm/cribra.d.ts",
  "dist/wasm/package.json",
];

const FORBIDDEN_PREFIXES = [
  "src/",
  "test/",
  "scripts/",
  "qualification/",
  "node_modules/",
  "target/",
];

const TEXT_EXTENSIONS = new Set([
  ".js",
  ".mjs",
  ".cjs",
  ".ts",
  ".d.ts",
  ".json",
  ".md",
  ".html",
  ".css",
  ".txt",
]);

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

export async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

export async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

export function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export async function sha256File(path) {
  return sha256(await readFile(path));
}

export async function fileSize(path) {
  return (await stat(path)).size;
}

export function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 64 * 1024 * 1024,
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status}: ${result.stderr}`,
    );
  }
  return result.stdout;
}

export function runInherit(command, args, options = {}) {
  execFileSync(command, args, { stdio: "inherit", ...options });
}

export async function walkFiles(root, current = root, result = []) {
  const entries = await readdir(current, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const path = resolve(current, entry.name);
    if (entry.isDirectory()) {
      await walkFiles(root, path, result);
    } else if (entry.isFile()) {
      result.push(path.slice(root.length + 1));
    }
  }
  return result;
}

function isForbiddenPath(path) {
  if (path.startsWith("/") || path.split("/").includes("..")) return true;
  if (FORBIDDEN_PREFIXES.some((prefix) => path.startsWith(prefix))) return true;
  if (path.endsWith(".map") || path.endsWith(".tgz") || path.endsWith(".tsbuildinfo")) {
    return true;
  }
  return false;
}

function textFile(path) {
  return TEXT_EXTENSIONS.has(extname(path)) || path.endsWith(".d.ts");
}

export async function adapterVersion() {
  const manifest = await readFile(
    resolve(repoRoot, "crates/cribra-wasm/Cargo.toml"),
    "utf8",
  );
  const match = manifest.match(/^version = "([^"]+)"$/m);
  if (match === null) throw new Error("cribra-wasm Cargo.toml has no version");
  return match[1];
}

export async function lockedWasmBindgenVersion() {
  const lock = await readFile(resolve(repoRoot, "Cargo.lock"), "utf8");
  const match = lock.match(/name = "wasm-bindgen"\nversion = "([^"]+)"/);
  if (match === null) throw new Error("Cargo.lock does not pin wasm-bindgen");
  return match[1];
}

export function gitInfo() {
  const runGit = (args) => {
    const result = spawnSync("git", args, {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return result.status === 0 ? result.stdout.trim() : undefined;
  };
  return {
    branch: runGit(["branch", "--show-current"]),
    commit: runGit(["rev-parse", "HEAD"]),
    status: runGit(["status", "--short"]),
  };
}

export async function npmPack({ cwd, destination }) {
  await mkdir(destination, { recursive: true });
  const output = run(
    "npm",
    ["pack", "--ignore-scripts", "--json", "--pack-destination", destination],
    { cwd },
  );
  const parsed = JSON.parse(output);
  const metadata = parsed[0];
  return {
    metadata,
    tarball: resolve(destination, metadata.filename),
  };
}

export async function extractTarball(tarball, destination) {
  await mkdir(destination, { recursive: true });
  run("tar", ["-xzf", tarball, "-C", destination]);
  // npm tarballs contain a single top-level `package/` directory.
  return resolve(destination, "package");
}

async function verifyInstalledPackage(
  packageDir,
  tarball,
  { mode = "auto", expectedManifest } = {},
) {
  const expected = expectedManifest ?? (await readManifest());
  const manifest = await readJson(resolve(packageDir, "package.json"));
  const resolvedMode =
    mode === "auto"
      ? manifest.private === true
        ? "provisional"
        : "release"
      : mode;
  if (resolvedMode !== "provisional" && resolvedMode !== "release") {
    throw new Error(`unknown tarball verification mode: ${String(resolvedMode)}`);
  }
  if (manifest.type !== "module") throw new Error("installed package is not ESM");
  if (manifest.name !== expected.name) {
    throw new Error(
      `unexpected package name: ${manifest.name}; expected ${expected.name}`,
    );
  }
  if (manifest.version !== expected.version) {
    throw new Error(
      `unexpected package version: ${manifest.version}; expected ${expected.version}`,
    );
  }
  if (resolvedMode === "provisional") {
    if (manifest.private !== true) {
      throw new Error("installed package must stay private");
    }
  } else {
    assertReleaseIdentity(expected);
    if (manifest.private === true) {
      throw new Error("release package must not be private");
    }
    const config = publishConfig(manifest);
    if (config.access !== "public") {
      throw new Error("release package publishConfig.access must be public");
    }
    if (config.provenance !== true) {
      throw new Error("release package publishConfig.provenance must be true");
    }
  }
  if (manifest.engines?.node !== ">=22") {
    throw new Error(`unexpected node engine range: ${manifest.engines?.node}`);
  }
  for (const key of ["dependencies", "optionalDependencies", "peerDependencies"]) {
    const value = manifest[key];
    if (value !== undefined && Object.keys(value).length > 0) {
      throw new Error(`unexpected ${key}: ${Object.keys(value).join(", ")}`);
    }
  }
  if (manifest.bundleDependencies !== undefined) {
    throw new Error("unexpected bundleDependencies");
  }
  for (const script of LIFECYCLE_SCRIPTS) {
    if (manifest.scripts?.[script] !== undefined) {
      throw new Error(`unexpected lifecycle script: ${script}`);
    }
  }
  const expectedFiles = ["dist", "README.md", "LICENSE"];
  if (JSON.stringify(manifest.files) !== JSON.stringify(expectedFiles)) {
    throw new Error(`unexpected files list: ${JSON.stringify(manifest.files)}`);
  }

  const files = await walkFiles(packageDir);
  for (const file of files) {
    if (isForbiddenPath(file)) {
      throw new Error(`tarball contains forbidden path: ${file}`);
    }
  }
  for (const required of REQUIRED_FILES) {
    if (!files.includes(required)) {
      throw new Error(`tarball is missing ${required}`);
    }
  }

  const generatedVersion = await readFile(
    resolve(packageDir, "dist/generated/package-version.js"),
    "utf8",
  );
  if (!generatedVersion.includes(JSON.stringify(manifest.version))) {
    throw new Error(
      `installed generated version does not match ${manifest.version}`,
    );
  }

  const absoluteNeedles = [packageRoot, repoRoot, "/Users/", "/private/"];
  const records = [];
  for (const file of files) {
    const path = resolve(packageDir, file);
    const bytes = await readFile(path);
    if (textFile(path)) {
      const text = bytes.toString("utf8");
      for (const needle of absoluteNeedles) {
        if (text.includes(needle)) {
          throw new Error(`tarball text file contains absolute path ${needle}: ${file}`);
        }
      }
    } else if (file.endsWith(".wasm")) {
      // Compiled dependencies legitimately embed toolchain/crate panic
      // locations. Only repository-specific paths would indicate a packaging
      // leak.
      for (const needle of [packageRoot, repoRoot]) {
        if (bytes.includes(Buffer.from(needle))) {
          throw new Error(`tarball WASM contains repository path ${needle}`);
        }
      }
    }
    records.push({ path: file, size: bytes.length, sha256: sha256(bytes) });
  }

  const wasm = await readFile(resolve(packageDir, "dist/wasm/cribra_bg.wasm"));
  if (wasm.length === 0) throw new Error("packaged WASM is empty");
  if (
    wasm[0] !== 0x00 ||
    wasm[1] !== 0x61 ||
    wasm[2] !== 0x73 ||
    wasm[3] !== 0x6d
  ) {
    throw new Error("packaged WASM has no WebAssembly magic header");
  }

  const tarballSha256 = await sha256File(tarball);
  return {
    manifest: {
      name: manifest.name,
      version: manifest.version,
      private: manifest.private,
      type: manifest.type,
      engines: manifest.engines,
      exports: manifest.exports,
      files: manifest.files,
      publishConfig: manifest.publishConfig,
    },
    tarball: {
      path: tarball,
      size: await fileSize(tarball),
      sha256: tarballSha256,
    },
    wasm: {
      size: wasm.length,
      sha256: sha256(wasm),
    },
    files: records,
  };
}

export async function verifyTarball({
  tarball,
  extractRoot,
  mode = "auto",
  manifest,
}) {
  await mkdir(extractRoot, { recursive: true });
  const extraction = await mkdtemp(resolve(extractRoot, "verify-"));
  try {
    const packageDir = await extractTarball(tarball, extraction);
    return await verifyInstalledPackage(packageDir, tarball, {
      mode,
      expectedManifest: manifest,
    });
  } finally {
    await rm(extraction, { recursive: true, force: true });
  }
}

export async function packAndVerify({
  cwd,
  destination,
  extractRoot,
  renameTo,
  mode = "auto",
}) {
  const packed = await npmPack({ cwd, destination });
  let tarball = packed.tarball;
  if (renameTo !== undefined && renameTo !== tarball) {
    await rename(tarball, renameTo);
    tarball = renameTo;
  }
  const evidence = await verifyTarball({ tarball, extractRoot, mode });
  return {
    ...evidence,
    npmPack: {
      size: packed.metadata.size,
      unpackedSize: packed.metadata.unpackedSize,
      shasum: packed.metadata.shasum,
      integrity: packed.metadata.integrity,
      entryCount: packed.metadata.entryCount,
    },
  };
}

export async function copyPackageMetadata(
  source,
  destination,
  { forcePrivate = false } = {},
) {
  await mkdir(destination, { recursive: true });
  for (const name of ["package.json", "README.md", "LICENSE"]) {
    await cp(resolve(source, name), resolve(destination, name));
  }
  if (forcePrivate) {
    const manifestPath = resolve(destination, "package.json");
    const manifest = await readJson(manifestPath);
    manifest.private = true;
    await writeJson(manifestPath, manifest);
  }
}

export async function copyDirectory(source, destination) {
  await cp(source, destination, { recursive: true });
}

export function relativeToRepo(path) {
  return relative(repoRoot, path).split(sep).join("/");
}
