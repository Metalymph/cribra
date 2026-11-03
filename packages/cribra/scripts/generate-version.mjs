/** Generates src/generated/package-version.ts from package.json. */

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { isMain, packageRoot } from "./util.mjs";

export async function generateVersion() {
  const manifestPath = resolve(packageRoot, "package.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  if (typeof manifest.version !== "string" || manifest.version.length === 0) {
    throw new Error("package.json has no version");
  }

  const outputPath = resolve(packageRoot, "src/generated/package-version.ts");
  await mkdir(resolve(packageRoot, "src/generated"), { recursive: true });
  const source = [
    "/** Generated from package.json by scripts/generate-version.mjs. */",
    "",
    `export const PACKAGE_VERSION = ${JSON.stringify(manifest.version)};`,
    "",
  ].join("\n");
  await writeFile(outputPath, source, "utf8");
  return manifest.version;
}

if (isMain(import.meta.url)) {
  const version = await generateVersion();
  process.stdout.write(`generated package version ${version}\n`);
}
