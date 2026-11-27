/**
 * Runs the pure Guard unit tests with Node's built-in test runner.
 *
 * esbuild bundles the TypeScript test entry into `.build/guard-tests` so the
 * tests use the existing toolchain without adding a test framework or a
 * TypeScript loader dependency.
 */

import { spawnSync } from "node:child_process";
import { mkdir, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = resolve(packageRoot, ".build/guard-tests");
const outfile = resolve(outDir, "guard.test.mjs");

await rm(outDir, { recursive: true, force: true });
await mkdir(outDir, { recursive: true });

await build({
  entryPoints: [resolve(packageRoot, "src/guard/guard.test.ts")],
  outfile,
  bundle: true,
  platform: "node",
  format: "esm",
  target: ["node24"],
  sourcemap: false,
  legalComments: "none",
  logLevel: "warning",
});

const result = spawnSync(process.execPath, ["--test", outfile], {
  stdio: "inherit",
});
process.exit(result.status ?? 1);
