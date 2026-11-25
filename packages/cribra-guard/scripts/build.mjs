/**
 * Builds the Flow WASM artifact from the repository source, verifies it against
 * `artifact.lock.json`, and emits the unpacked MV3 extension.
 *
 * This script deliberately never reads `packages/cribra/src/wasm` or
 * `packages/cribra/dist`; both are generated locations that may hold a
 * different build. The glue always comes from the artifact directory produced
 * in this same invocation.
 */

import { execFileSync } from "node:child_process";
import {
  access,
  cp,
  mkdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { constants, existsSync } from "node:fs";
import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import * as esbuild from "esbuild";

import {
  assertLock,
  assertUnverifiedOutput,
  mergeLock,
  platformKey,
  resolveLockMode,
} from "./artifact-lock.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const packageRoot = resolve(scriptDir, "..");
const repoRoot = resolve(packageRoot, "../..");
const outDir = resolve(packageRoot, "dist");
const rawDir = resolve(packageRoot, ".build/raw");
const defaultArtifactDir = resolve(packageRoot, ".build/flow");
const lockPath = resolve(packageRoot, "artifact.lock.json");

const FLOW_FEATURES = ["transform"];

function parseArgs(argv) {
  const parsed = new Map();
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) {
      throw new Error(`unexpected argument: ${argument}`);
    }
    const name = argument.slice(2);
    const next = argv[index + 1];
    if (next === undefined || next.startsWith("--")) {
      parsed.set(name, true);
    } else {
      parsed.set(name, next);
      index += 1;
    }
  }
  return parsed;
}

const flags = parseArgs(process.argv.slice(2));
const skipBuild = flags.get("skip-build") === true;
const writeLock = flags.get("write-lock") === true;
const allowUnverified = flags.get("allow-unverified") === true;
const withQualification = flags.get("with-qualification") === true;
const wasmOverride =
  typeof flags.get("wasm") === "string" ? resolve(flags.get("wasm")) : undefined;
const glueOverride =
  typeof flags.get("glue") === "string" ? resolve(flags.get("glue")) : undefined;
const artifactDir =
  typeof flags.get("artifact-dir") === "string"
    ? resolve(flags.get("artifact-dir"))
    : defaultArtifactDir;
const targetOutDir =
  typeof flags.get("out") === "string"
    ? resolve(flags.get("out"))
    : outDir;

function run(command, args, options = {}) {
  process.stdout.write(`$ ${command} ${args.join(" ")}\n`);
  const output = execFileSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
    ...options,
  });
  return typeof output === "string" ? output.trim() : "";
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

/**
 * Panic locations embed absolute registry, workspace, and rust-src paths.
 * Remapping them removes host-specific paths from the WASM. The lock still
 * pins one digest per host because rustc's crate disambiguators and symbol
 * ordering can differ across hosts.
 */
function reproducibleRustFlags() {
  const cargoHome = process.env.CARGO_HOME ?? resolve(homedir(), ".cargo");
  const sysroot = run("rustc", ["--print", "sysroot"]).trim();
  const commit = run("rustc", ["-vV"]).match(/^commit-hash:\s*(\S+)$/m)?.[1];
  const flags = [
    `--remap-path-prefix=${resolve(cargoHome, "registry")}=/cargo/registry`,
    `--remap-path-prefix=${repoRoot}=/cribra`,
  ];
  if (commit !== undefined) {
    flags.push(
      `--remap-path-prefix=${resolve(sysroot, "lib/rustlib/src/rust")}=/rustc/${commit}`,
    );
  }
  return flags.join(" ");
}

function rustFlags() {
  return [process.env.RUSTFLAGS, reproducibleRustFlags()]
    .filter((value) => value !== undefined && value.length > 0)
    .join(" ");
}

async function requireFile(path) {
  await access(path, constants.R_OK);
}

function crateVersion(manifest) {
  const section = manifest.match(/\[package\][\s\S]*?(?=\n\[|$)/);
  const version = section?.[0].match(/^version\s*=\s*"([^"]+)"/m);
  if (version?.[1] === undefined) {
    throw new Error("cribra-wasm Cargo.toml has no package version");
  }
  return version[1];
}

async function wasmBindgenVersion() {
  const local = resolve(packageRoot, ".tools/bin/wasm-bindgen");
  const binary = existsSync(local) ? local : "wasm-bindgen";
  const output = run(binary, ["--version"]).trim();
  const version = output.match(/(\d+\.\d+\.\d+(?:[-+][^\s]+)?)$/)?.[1];
  if (version === undefined) {
    throw new Error(`unexpected wasm-bindgen version output: ${output}`);
  }
  return { binary, version };
}

function wasmOptVersion() {
  const output = run("wasm-opt", ["--version"]).trim();
  const version = output.match(/wasm-opt version (\S+)/)?.[1];
  if (version === undefined) {
    throw new Error(`unexpected wasm-opt version output: ${output}`);
  }
  return version;
}

async function lockedVersion() {
  const lock = await readFile(resolve(repoRoot, "Cargo.lock"), "utf8");
  const match = lock.match(/name = "wasm-bindgen"\nversion = "([^"]+)"/);
  if (match?.[1] === undefined) {
    throw new Error("Cargo.lock does not pin wasm-bindgen");
  }
  return match[1];
}

async function buildArtifact(bindgen) {
  const expectedBindgen = await lockedVersion();
  if (bindgen.version !== expectedBindgen) {
    throw new Error(
      `wasm-bindgen ${bindgen.version} does not match Cargo.lock ${expectedBindgen}`,
    );
  }

  run("cargo", [
    "build",
    "-p",
    "cribra-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "--release",
    "--locked",
  ], {
    env: { ...process.env, CARGO_INCREMENTAL: "0", RUSTFLAGS: rustFlags() },
    stdio: ["ignore", "inherit", "inherit"],
  });

  await rm(rawDir, { recursive: true, force: true });
  await mkdir(rawDir, { recursive: true });
  run(bindgen.binary, [
    "--target",
    "web",
    "--out-dir",
    rawDir,
    "--out-name",
    "cribra",
    resolve(
      repoRoot,
      "target/wasm32-unknown-unknown/release/cribra_wasm.wasm",
    ),
  ], { stdio: ["ignore", "inherit", "inherit"] });

  await rm(artifactDir, { recursive: true, force: true });
  await mkdir(artifactDir, { recursive: true });
  await cp(resolve(rawDir, "cribra.js"), resolve(artifactDir, "cribra.js"));
  await cp(
    resolve(rawDir, "cribra.d.ts"),
    resolve(artifactDir, "cribra.d.ts"),
  );
  await cp(
    resolve(rawDir, "cribra_bg.wasm.d.ts"),
    resolve(artifactDir, "cribra_bg.wasm.d.ts"),
  );
  run("wasm-opt", [
    "-Oz",
    "--strip-producers",
    resolve(rawDir, "cribra_bg.wasm"),
    "-o",
    resolve(artifactDir, "cribra_bg.wasm"),
  ], { stdio: ["ignore", "inherit", "inherit"] });
}

async function copyOverrideWasm() {
  if (wasmOverride === undefined) return;
  const bytes = await readFile(wasmOverride);
  await mkdir(artifactDir, { recursive: true });
  await writeFile(resolve(artifactDir, "cribra_bg.wasm"), bytes);
}

async function prepareTypeScriptSurface() {
  const target = resolve(repoRoot, "packages/cribra/src/wasm");
  await mkdir(target, { recursive: true });
  await cp(
    resolve(artifactDir, "cribra.d.ts"),
    resolve(target, "cribra.d.ts"),
  );
  await cp(
    resolve(artifactDir, "cribra_bg.wasm.d.ts"),
    resolve(target, "cribra_bg.wasm.d.ts"),
  );
}

function generatePackageVersion() {
  run(
    process.execPath,
    [resolve(repoRoot, "packages/cribra/scripts/generate-version.mjs")],
    { stdio: ["ignore", "inherit", "inherit"] },
  );
}

async function readLock() {
  try {
    return JSON.parse(await readFile(lockPath, "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT") return undefined;
    throw error;
  }
}

async function main() {
  const bindgen = await wasmBindgenVersion();
  const optVersion = wasmOptVersion();
  generatePackageVersion();

  if (!skipBuild) {
    await buildArtifact(bindgen);
    await prepareTypeScriptSurface();
  }
  await copyOverrideWasm();

  const artifactPath = resolve(artifactDir, "cribra_bg.wasm");
  const gluePath = glueOverride ?? resolve(artifactDir, "cribra.js");
  await requireFile(artifactPath);
  await requireFile(gluePath);

  const productionBytes = await readFile(artifactPath);
  const glueBytes = await readFile(gluePath);
  const productionWasm = {
    bytes: productionBytes.byteLength,
    sha256: sha256(productionBytes),
  };
  const glue = {
    bytes: glueBytes.byteLength,
    sha256: sha256(glueBytes),
  };

  let raw = null;
  if (!skipBuild && wasmOverride === undefined) {
    const rawBytes = await readFile(resolve(rawDir, "cribra_bg.wasm"));
    raw = {
      bytes: rawBytes.byteLength,
      sha256: sha256(rawBytes),
    };
  }

  const artifact = {
    profile: "flow",
    features: FLOW_FEATURES,
    wasmBindgenVersion: bindgen.version,
    wasmOptVersion: optVersion,
    productionWasm,
    glue,
  };

  const lock = await readLock();
  const lockMode = resolveLockMode({
    writeLock,
    allowUnverified,
    skipBuild,
    wasmOverride: wasmOverride !== undefined,
    glueOverride: glueOverride !== undefined,
  });
  assertUnverifiedOutput({
    mode: lockMode,
    targetOutDir,
    releaseOutDir: outDir,
  });
  if (lockMode === "verify") {
    assertLock(lock, {
      key: platformKey(),
      artifact,
      glue,
      raw,
    });
  } else if (lockMode === "write") {
    const key = platformKey();
    const next = mergeLock({
      existing: lock,
      key,
      artifact,
      glue,
      raw,
      features: FLOW_FEATURES,
    });
    await writeFile(lockPath, `${JSON.stringify(next, null, 2)}\n`, "utf8");
    process.stdout.write(`wrote ${lockPath} (${key})\n`);
  }

  const wasmManifest = await readFile(
    resolve(repoRoot, "crates/cribra-wasm/Cargo.toml"),
    "utf8",
  );
  const coreManifest = await readFile(resolve(repoRoot, "Cargo.toml"), "utf8");
  const gitCommit = run("git", ["rev-parse", "HEAD"]);
  const gitStatus = run("git", ["status", "--porcelain"]);
  const rustcVersion = run("rustc", ["--version"]);

  await rm(targetOutDir, { recursive: true, force: true });
  await mkdir(targetOutDir, { recursive: true });

  await esbuild.build({
    entryPoints: [resolve(packageRoot, "src/service-worker.ts")],
    outfile: resolve(targetOutDir, "service-worker.js"),
    bundle: true,
    format: "esm",
    platform: "browser",
    target: ["chrome120"],
    sourcemap: false,
    minify: false,
    legalComments: "none",
    logLevel: "warning",
    define: {
      __CRIBRA_ARTIFACT_BYTES__: String(productionWasm.bytes),
      __CRIBRA_ARTIFACT_SHA256__: JSON.stringify(productionWasm.sha256),
    },
    assetNames: "wasm/[name]",
    plugins: [
      {
        name: "cribra-production-glue",
        setup(build) {
          build.onResolve(
            { filter: /^\.\/wasm\/cribra\.js$/ },
            (args) => {
              const expectedResolveDir = resolve(repoRoot, "packages/cribra/src");
              return args.resolveDir === expectedResolveDir
                ? { path: gluePath }
                : null;
            },
          );
        },
      },
    ],
  });

  if (withQualification) {
    await esbuild.build({
      entryPoints: [resolve(packageRoot, "src/qualification.ts")],
      outfile: resolve(targetOutDir, "qualification.js"),
      bundle: true,
      format: "esm",
      platform: "browser",
      target: ["chrome120"],
      sourcemap: false,
      minify: false,
      legalComments: "none",
      logLevel: "warning",
    });
  }

  await mkdir(resolve(targetOutDir, "wasm"), { recursive: true });
  // esbuild may copy the pre-WASM-opt asset because the generated glue keeps a
  // default `new URL(...)`. The runtime always receives explicit bytes, so the
  // only asset that ships is the verified production artifact.
  await writeFile(resolve(targetOutDir, "wasm/cribra_bg.wasm"), productionBytes);

  await cp(
    resolve(packageRoot, "manifest.json"),
    resolve(targetOutDir, "manifest.json"),
  );
  if (withQualification) {
    await cp(
      resolve(packageRoot, "qualification.html"),
      resolve(targetOutDir, "qualification.html"),
    );
  }

  const serviceWorkerBytes = await readFile(
    resolve(targetOutDir, "service-worker.js"),
  );
  const qualificationBytes = withQualification
    ? await readFile(resolve(targetOutDir, "qualification.js"))
    : undefined;
  const manifest = JSON.parse(
    await readFile(resolve(packageRoot, "manifest.json"), "utf8"),
  );

  const provenance = {
    schemaVersion: 1,
    builtAt: new Date().toISOString(),
    platform: platformKey(),
    git: { commit: gitCommit, dirty: gitStatus.length > 0 },
    toolchain: {
      rustc: rustcVersion,
      wasmBindgen: bindgen.version,
      wasmOpt: optVersion,
      esbuild: "0.28.2",
      node: process.version,
    },
    source: {
      crate: "cribra-wasm",
      adapterVersion: crateVersion(wasmManifest),
      coreVersion: crateVersion(coreManifest),
      features: FLOW_FEATURES,
      commit: gitCommit,
      dirty: gitStatus.length > 0,
    },
    artifact: {
      path: "wasm/cribra_bg.wasm",
      profile: "flow",
      bytes: productionWasm.bytes,
      sha256: productionWasm.sha256,
      raw: raw,
      note:
        "wasm-opt -Oz --strip-producers applied to the wasm-bindgen output of the transform-enabled cribra-wasm crate; registry and toolchain paths are remapped for reproducibility",
    },
    glue: {
      path: "wasm/cribra.js",
      bytes: glue.bytes,
      sha256: glue.sha256,
    },
    extension: {
      manifest,
      serviceWorker: {
        bytes: serviceWorkerBytes.byteLength,
        sha256: sha256(serviceWorkerBytes),
      },
      ...(qualificationBytes === undefined
        ? {}
        : {
            qualificationHarness: {
              bytes: qualificationBytes.byteLength,
              sha256: sha256(qualificationBytes),
            },
          }),
    },
  };
  await writeFile(
    resolve(targetOutDir, "artifact.json"),
    `${JSON.stringify(provenance, null, 2)}\n`,
    "utf8",
  );

  const requiredFiles = [
    "manifest.json",
    "service-worker.js",
    "wasm/cribra_bg.wasm",
    "artifact.json",
  ];
  if (withQualification) {
    requiredFiles.push("qualification.html", "qualification.js");
  }
  for (const file of requiredFiles) {
    await requireFile(resolve(targetOutDir, file));
  }

  process.stdout.write(
    `built Flow extension in ${targetOutDir}\n` +
      `  wasm ${productionWasm.bytes} bytes sha256 ${productionWasm.sha256}\n` +
      `  service worker ${serviceWorkerBytes.byteLength} bytes\n`,
  );
}

await main();
