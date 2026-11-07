/** Materializes the public-surface type test with the manifest package name. */

import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { PACKAGE_SPECIFIER_TOKEN, readManifest } from "./identity.mjs";
import { isMain, packageRoot } from "./util.mjs";

export async function generateTestTypes() {
  const manifest = await readManifest();
  const templatePath = resolve(
    packageRoot,
    "test/types/public-api.template.ts",
  );
  const outputPath = resolve(packageRoot, "test/types/public-api.ts");
  const template = await readFile(templatePath, "utf8");
  const output = template.replaceAll(PACKAGE_SPECIFIER_TOKEN, manifest.name);
  if (output.includes(PACKAGE_SPECIFIER_TOKEN)) {
    throw new Error("public-api template still contains the specifier token");
  }
  await writeFile(outputPath, output, "utf8");
  return manifest.name;
}

if (isMain(import.meta.url)) {
  const name = await generateTestTypes();
  process.stdout.write(`generated public type test for ${name}\n`);
}
