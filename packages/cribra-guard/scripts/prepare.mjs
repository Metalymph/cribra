/**
 * Prepares the generated TypeScript surface needed to type-check the wrapper
 * source without a prior `@metalymph/cribra` package build.
 *
 * `packages/cribra/src/wasm` and `src/generated/package-version.ts` are
 * generated and gitignored by the Cribra package. The extension build always
 * produces its own glue in `.build/flow`; this script only exposes the matching
 * declaration file to `tsc`.
 */

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { cp, mkdir } from "node:fs/promises";
import { resolve } from "node:path";

const packageRoot = resolve(import.meta.dirname, "..");
const repoRoot = resolve(packageRoot, "../..");
const cribraRoot = resolve(repoRoot, "packages/cribra");
const artifactDir = resolve(packageRoot, ".build/flow");

execFileSync(
  process.execPath,
  [resolve(cribraRoot, "scripts/generate-version.mjs")],
  { stdio: "inherit" },
);

const declarations = resolve(artifactDir, "cribra.d.ts");
if (!existsSync(declarations)) {
  throw new Error(
    "missing .build/flow/cribra.d.ts; run `npm run build` before `npm run check`",
  );
}

const target = resolve(cribraRoot, "src/wasm");
await mkdir(target, { recursive: true });
await cp(declarations, resolve(target, "cribra.d.ts"));
await cp(
  resolve(artifactDir, "cribra_bg.wasm.d.ts"),
  resolve(target, "cribra_bg.wasm.d.ts"),
);
process.stdout.write("prepared generated Cribra TypeScript declarations\n");
