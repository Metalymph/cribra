/** Removes generated build output only. */

import { rm } from "node:fs/promises";
import { resolve } from "node:path";

import { isMain, packageRoot } from "./util.mjs";

export async function cleanPackage() {
  for (const path of [
    "dist",
    "src/wasm",
    "src/generated/package-version.ts",
  ]) {
    await rm(resolve(packageRoot, path), { recursive: true, force: true });
  }
}

if (isMain(import.meta.url)) {
  await cleanPackage();
  process.stdout.write("cleaned package build output\n");
}
