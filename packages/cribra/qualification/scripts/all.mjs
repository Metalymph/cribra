/** Umbrella runner for the isolated npm-tarball consumer qualification. */

import { spawnSync } from "node:child_process";
import { mkdir, readdir, readFile } from "node:fs/promises";
import { release, version } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  artifactsRoot,
  readJson,
  targetRoot,
  writeJson,
} from "../lib/artifacts.mjs";
import { flagString, parseArgs } from "../lib/cli.mjs";

const qualificationRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const scripts = resolve(qualificationRoot, "scripts");
const evidenceDir = resolve(targetRoot, "evidence");
const flags = parseArgs(process.argv.slice(2));

const flowTarball = resolve(artifactsRoot, "flow/cribra-typescript-0.0.0-dev.tgz");
const coreTarball = resolve(artifactsRoot, "core/cribra-typescript-0.0.0-dev-core.tgz");
const browsers = flagString(flags, "browsers", "chrome,chromium,firefox,webkit")
  .split(",")
  .map((entry) => entry.trim())
  .filter((entry) => entry.length > 0);
const nodeBinaries = flagString(flags, "nodes", process.execPath)
  .split(",")
  .map((entry) => entry.trim())
  .filter((entry) => entry.length > 0);

function step(label, script, args = []) {
  process.stdout.write(`\n=== ${label} ===\n`);
  const result = spawnSync(process.execPath, [resolve(scripts, script), ...args], {
    cwd: qualificationRoot,
    stdio: "inherit",
    env: process.env,
  });
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${label} failed with exit code ${result.status}`);
  }
}

async function readToolVersions() {
  const manifest = await readJson(resolve(qualificationRoot, "package.json"));
  const tool = async (name) =>
    JSON.parse(
      await readFile(
        resolve(qualificationRoot, "node_modules", name, "package.json"),
        "utf8",
      ),
    ).version;
  return {
    playwright: await tool("playwright"),
    typescript: await tool("typescript"),
    "typescript-5": await tool("typescript-5"),
    vite: await tool("vite"),
    webpack: await tool("webpack"),
    "webpack-cli": await tool("webpack-cli"),
    "html-webpack-plugin": await tool("html-webpack-plugin"),
    declared: manifest.devDependencies,
    node: process.version,
    npm: spawnSync("npm", ["--version"], { encoding: "utf8" }).stdout.trim(),
    platform: process.platform,
    arch: process.arch,
    os: `${version()} (kernel ${release()})`,
  };
}

async function writeSummary() {
  const entries = {};
  for (const name of await readdir(evidenceDir)) {
    if (!name.endsWith(".json") || name === "summary.json") continue;
    entries[name.replace(/\.json$/, "")] = await readJson(
      resolve(evidenceDir, name),
    );
  }
  const pack = await readJson(resolve(artifactsRoot, "evidence.json"));
  const summary = {
    generatedAt: new Date().toISOString(),
    tools: await readToolVersions(),
    package: pack.package,
    tarballs: {
      flow: {
        path: pack.flow.tarball.path,
        size: pack.flow.tarball.size,
        sha256: pack.flow.tarball.sha256,
        wasmSha256: pack.flow.wasm.sha256,
        fileCount: pack.flow.files.length,
      },
      core: {
        path: pack.core.tarball.path,
        size: pack.core.tarball.size,
        sha256: pack.core.tarball.sha256,
        wasmSha256: pack.core.wasm.sha256,
        fileCount: pack.core.files.length,
      },
    },
    adapterVersion: pack.adapterVersion,
    wasmBindgenVersion: pack.wasmBindgenVersion,
    repeat: pack.repeat,
    evidence: entries,
  };
  await writeJson(resolve(evidenceDir, "summary.json"), summary);
  process.stdout.write(`\nqualification evidence: ${evidenceDir}\n`);
  process.stdout.write(
    `flow sha256: ${pack.flow.tarball.sha256}\ncore sha256: ${pack.core.tarball.sha256}\n`,
  );
}

async function main() {
  await mkdir(evidenceDir, { recursive: true });

  if (flags.get("skip-pack") !== true) {
    step("pack", "pack.mjs", flags.get("repeat") === true ? ["--repeat"] : []);
  }

  for (const nodeBinary of nodeBinaries) {
    step("node flow", "node-consumer.mjs", [
      "--profile",
      "flow",
      "--node",
      nodeBinary,
      "--tarball",
      flowTarball,
    ]);
    step("node core", "node-consumer.mjs", [
      "--profile",
      "core",
      "--node",
      nodeBinary,
      "--tarball",
      coreTarball,
    ]);
  }

  step("typescript", "typescript-consumer.mjs", ["--tarball", flowTarball]);

  for (const engine of browsers) {
    step(`browser flow ${engine}`, "browser-consumer.mjs", [
      "--profile",
      "flow",
      "--engine",
      engine,
      "--tarball",
      flowTarball,
    ]);
  }
  if (browsers.includes("chrome")) {
    step("browser core chrome", "browser-consumer.mjs", [
      "--profile",
      "core",
      "--engine",
      "chrome",
      "--tarball",
      coreTarball,
    ]);
  }

  step("vite", "vite-consumer.mjs", ["--engine", "chrome", "--tarball", flowTarball]);
  step("webpack", "webpack-consumer.mjs", [
    "--engine",
    "chrome",
    "--tarball",
    flowTarball,
  ]);

  step("native safari", "safari-native.mjs", [
    "--profile",
    "flow",
    "--tarball",
    flowTarball,
  ]);

  await writeSummary();
  process.stdout.write("\nall consumer qualification steps passed\n");
}

await main();
