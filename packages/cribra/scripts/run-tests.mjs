/** Runs the Node test suite against one built dist directory. */

import { spawnSync } from "node:child_process";
import { readdir } from "node:fs/promises";
import { resolve } from "node:path";

import { isMain, packageRoot, parseArgs } from "./util.mjs";

export function runNodeTests({ distDir, profile = "flow" }) {
  const testDir = resolve(packageRoot, "test/node");
  const files = [];
  return readdir(testDir).then((names) => {
    for (const name of names.sort()) {
      if (name.endsWith(".test.mjs")) {
        files.push(resolve(testDir, name));
      }
    }
    if (files.length === 0) {
      throw new Error("no Node test files found");
    }
    const result = spawnSync(process.execPath, ["--test", ...files], {
      cwd: packageRoot,
      stdio: "inherit",
      env: {
        ...process.env,
        CRIBRA_PACKAGE_DIST: distDir,
        CRIBRA_PROFILE: profile,
      },
    });
    if (result.status !== 0) {
      throw new Error(
        `Node tests failed with exit code ${result.status ?? "unknown"}`,
      );
    }
  });
}

if (isMain(import.meta.url)) {
  const flags = parseArgs(process.argv.slice(2));
  const dist = flags.get("dist");
  const distDir =
    typeof dist === "string" ? resolve(packageRoot, dist) : resolve(packageRoot, "dist");
  await runNodeTests({ distDir });
}
