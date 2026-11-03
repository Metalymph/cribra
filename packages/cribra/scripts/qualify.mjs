/** Builds and qualifies Core and Flow packages end to end. */

import { execFileSync } from "node:child_process";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { buildPackage } from "./build.mjs";
import { runBrowserSmoke } from "./browser-smoke.mjs";
import { checkPackage } from "./package-check.mjs";
import { runNodeTests } from "./run-tests.mjs";
import { isMain, packageRoot } from "./util.mjs";

const qualificationRoot = resolve(packageRoot, "../../target/typescript-qualification");
const tsc = resolve(packageRoot, "node_modules/typescript/bin/tsc");

async function typeCheck() {
  execFileSync(process.execPath, [tsc, "-p", "tsconfig.test.json"], {
    cwd: packageRoot,
    stdio: "inherit",
  });
}

async function main() {
  const manifest = JSON.parse(
    await readFile(resolve(packageRoot, "package.json"), "utf8"),
  );
  const version = manifest.version;

  await rm(qualificationRoot, { recursive: true, force: true });
  await mkdir(qualificationRoot, { recursive: true });

  // Default dist is the Flow package artifact and the type-check target.
  const flowDefault = await buildPackage({ profile: "flow" });

  const flowDist = resolve(qualificationRoot, "flow/dist");
  await cp(flowDefault, flowDist, { recursive: true });

  const coreDist = resolve(qualificationRoot, "core/dist");
  await buildPackage({ profile: "core", distDir: coreDist });

  for (const profile of ["flow", "core"]) {
    await writeFile(
      resolve(qualificationRoot, profile, "package.json"),
      `${JSON.stringify({ type: "module" }, null, 2)}\n`,
      "utf8",
    );
  }

  await typeCheck();
  process.stdout.write("type check passed\n");

  await runNodeTests({ distDir: flowDist, profile: "flow" });
  process.stdout.write("Node tests passed (flow)\n");
  await runNodeTests({ distDir: coreDist, profile: "core" });
  process.stdout.write("Node tests passed (core)\n");

  await checkPackage({ distDir: flowDefault });
  await checkPackage({ distDir: flowDist });
  await checkPackage({ distDir: coreDist });

  await runBrowserSmoke({ distDir: flowDist, profile: "flow", version });
  await runBrowserSmoke({ distDir: coreDist, profile: "core", version });

  process.stdout.write("qualification passed: core and flow\n");
}

if (isMain(import.meta.url)) {
  await main();
}
