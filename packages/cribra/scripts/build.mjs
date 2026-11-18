/** Builds declarations/ESM plus the packaged WASM asset. */

import { execFileSync } from "node:child_process";
import { access, cp, mkdir, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { generateVersion } from "./generate-version.mjs";
import { buildWasm } from "./build-wasm.mjs";
import { isMain, packageRoot, parseArgs } from "./util.mjs";

const tsc = resolve(packageRoot, "node_modules/typescript/bin/tsc");

export async function buildPackage({
  profile = "flow",
  distDir = resolve(packageRoot, "dist"),
} = {}) {
  await generateVersion();

  const sourceWasm = resolve(packageRoot, "src/wasm");
  await buildWasm({ profile, outDir: sourceWasm });

  await rm(distDir, { recursive: true, force: true });
  await mkdir(distDir, { recursive: true });
  execFileSync(
    process.execPath,
    [tsc, "-p", "tsconfig.build.json", "--outDir", distDir],
    { cwd: packageRoot, stdio: "inherit" },
  );

  const targetWasm = resolve(distDir, "wasm");
  await cp(sourceWasm, targetWasm, { recursive: true });
  await writeFile(
    resolve(targetWasm, "package.json"),
    `${JSON.stringify({ type: "module" }, null, 2)}\n`,
    "utf8",
  );

  for (const file of [
    "index.js",
    "index.d.ts",
    "index.browser.js",
    "index.node.js",
    "wasm/cribra.js",
    "wasm/cribra_bg.wasm",
  ]) {
    await access(resolve(distDir, file));
  }

  process.stdout.write(`built ${profile} package in ${distDir}\n`);
  return distDir;
}

if (isMain(import.meta.url)) {
  const flags = parseArgs(process.argv.slice(2));
  const profile = flags.get("profile") ?? "flow";
  const dist = flags.get("dist");
  await buildPackage({
    profile,
    ...(typeof dist === "string"
      ? { distDir: resolve(packageRoot, dist) }
      : {}),
  });
}
