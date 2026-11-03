/** Builds the exact wasm-bindgen CLI-compatible adapter artifact. */

import { execFileSync } from "node:child_process";
import { access, mkdir, readFile, rm } from "node:fs/promises";
import { constants } from "node:fs";
import { resolve } from "node:path";

import { isMain, packageRoot, parseArgs, repoRoot } from "./util.mjs";

const PROFILES = new Set(["core", "flow"]);

async function lockedWasmBindgenVersion() {
  const lock = await readFile(resolve(repoRoot, "Cargo.lock"), "utf8");
  const match = lock.match(/name = "wasm-bindgen"\nversion = "([^"]+)"/);
  if (match === null) {
    throw new Error("Cargo.lock does not pin wasm-bindgen");
  }
  return match[1];
}

function installedWasmBindgenVersion() {
  let output;
  try {
    output = execFileSync("wasm-bindgen", ["--version"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch {
    throw new Error(
      "wasm-bindgen is not installed; install the CLI version pinned by Cargo.lock",
    );
  }
  const match = output.trim().match(/(\d+\.\d+\.\d+(?:[-+][^\s]+)?)$/);
  if (match === null) {
    throw new Error(`unexpected wasm-bindgen --version output: ${output.trim()}`);
  }
  return match[1];
}

async function requireFile(path) {
  await access(path, constants.R_OK);
}

export async function buildWasm({ profile = "flow", outDir }) {
  if (!PROFILES.has(profile)) {
    throw new Error(`unknown profile: ${profile}`);
  }
  if (outDir === undefined) {
    throw new Error("buildWasm requires an output directory");
  }

  const expected = await lockedWasmBindgenVersion();
  const installed = installedWasmBindgenVersion();
  if (installed !== expected) {
    throw new Error(
      `wasm-bindgen CLI ${installed} does not match Cargo.lock ${expected}`,
    );
  }

  const cargoArgs = [
    "build",
    "-p",
    "cribra-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "--release",
    "--locked",
  ];
  if (profile === "core") {
    cargoArgs.push("--no-default-features");
  }
  execFileSync("cargo", cargoArgs, { cwd: repoRoot, stdio: "inherit" });

  await rm(outDir, { recursive: true, force: true });
  await mkdir(outDir, { recursive: true });
  execFileSync(
    "wasm-bindgen",
    [
      "--target",
      "web",
      "--out-dir",
      outDir,
      "--out-name",
      "cribra",
      resolve(
        repoRoot,
        "target/wasm32-unknown-unknown/release/cribra_wasm.wasm",
      ),
    ],
    { cwd: repoRoot, stdio: "inherit" },
  );

  for (const file of [
    "cribra.js",
    "cribra.d.ts",
    "cribra_bg.wasm",
    "cribra_bg.wasm.d.ts",
  ]) {
    await requireFile(resolve(outDir, file));
  }
  process.stdout.write(`built ${profile} WASM bindings in ${outDir}\n`);
}

if (isMain(import.meta.url)) {
  const flags = parseArgs(process.argv.slice(2));
  const profile = flags.get("profile") ?? "flow";
  const out = flags.get("out");
  if (typeof out !== "string") {
    throw new Error("usage: build-wasm.mjs --profile <core|flow> --out <dir>");
  }
  await buildWasm({ profile, outDir: resolve(packageRoot, out) });
}
