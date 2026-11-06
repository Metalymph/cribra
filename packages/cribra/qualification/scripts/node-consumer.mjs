/** Installs the tarball into a clean consumer and runs the Node suite. */

import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import {
  artifactsRoot,
  packageRoot,
  readJson,
  targetRoot,
  writeJson,
} from "../lib/artifacts.mjs";
import {
  copyFixture,
  createConsumer,
  removeConsumer,
  runChild,
} from "../lib/consumer.mjs";
import { flagString, parseArgs } from "../lib/cli.mjs";

const flags = parseArgs(process.argv.slice(2));
const profile = flagString(flags, "profile", "flow");
const nodeBinary = flagString(flags, "node", process.execPath);
const defaultTarball =
  profile === "core"
    ? resolve(artifactsRoot, "core/cribra-typescript-0.0.0-dev-core.tgz")
    : resolve(artifactsRoot, "flow/cribra-typescript-0.0.0-dev.tgz");
const tarball = resolve(flagString(flags, "tarball", defaultTarball));
const evidencePath = resolve(artifactsRoot, "evidence.json");
const evidence = await readJson(evidencePath);
const expected = profile === "core" ? evidence.core : evidence.flow;
const nodeVersion = execFileSync(nodeBinary, ["--version"], {
  encoding: "utf8",
}).trim();

const suiteSource = resolve(packageRoot, "qualification/apps/node/suite.mjs");
const consumer = await createConsumer({
  label: `node-${profile}-${nodeVersion.replace(/[^0-9]/g, "")}`,
  tarball,
});

try {
  const installedManifest = JSON.parse(
    await readFile(resolve(consumer.installed, "package.json"), "utf8"),
  );
  if (installedManifest.engines?.node !== ">=22") {
    throw new Error(`installed package has unexpected engines: ${installedManifest.engines?.node}`);
  }
  if (installedManifest.dependencies !== undefined) {
    throw new Error("installed package has runtime dependencies");
  }
  await copyFixture(suiteSource, resolve(consumer.directory, "suite.mjs"));

  const assets = ["default", "bytes", "arraybuffer", "file-url", "path", "http-url"];
  const results = [];
  for (const asset of assets) {
    const args = [
      "suite.mjs",
      "--profile",
      profile,
      "--asset",
      asset,
      ...(asset === "default" ? [] : ["--quick"]),
    ];
    const { stdout } = await runChild({
      command: nodeBinary,
      args,
      cwd: consumer.directory,
      label: `node ${nodeVersion} ${profile} asset=${asset}`,
      timeout: 120_000,
      env: {
        CRIBRA_EXPECTED_PACKAGE: evidence.package.version,
        CRIBRA_EXPECTED_ADAPTER: evidence.adapterVersion,
      },
    });
    results.push({
      asset,
      passed: stdout.includes("all ") || stdout.includes("checks passed"),
      output: stdout.trim().split("\n").slice(-1)[0],
    });
    process.stdout.write(`node ${nodeVersion} ${profile} asset=${asset}: ${stdout.trim().split("\n").slice(-1)[0]}\n`);
  }

  const report = {
    profile,
    nodeVersion,
    tarball,
    tarballSha256: expected.tarball.sha256,
    wasmSha256: expected.wasm.sha256,
    packageVersion: evidence.package.version,
    adapterVersion: evidence.adapterVersion,
    assets: results,
  };
  const outputPath = resolve(
    targetRoot,
    "evidence",
    `node-${profile}-${nodeVersion.replace(/[^0-9]/g, "")}.json`,
  );
  await mkdir(resolve(targetRoot, "evidence"), { recursive: true });
  await writeJson(outputPath, report);
  process.stdout.write(`node qualification passed (${profile}, ${nodeVersion})\n`);
} finally {
  await removeConsumer(consumer.directory);
}
