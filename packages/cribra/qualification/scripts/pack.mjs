/** Builds Flow and test-only Core tarballs and records qualification evidence. */

import { mkdir, rm, writeFile } from "node:fs/promises";
import { release, version } from "node:os";
import { resolve } from "node:path";

import { buildPackage } from "../../scripts/build.mjs";
import { checkPackage } from "../../scripts/package-check.mjs";
import { tarballNames } from "../../scripts/identity.mjs";
import {
  adapterVersion,
  artifactsRoot,
  copyPackageMetadata,
  gitInfo,
  lockedWasmBindgenVersion,
  packAndVerify,
  packageRoot,
  readJson,
  run,
  targetRoot,
  writeJson,
} from "../lib/artifacts.mjs";
import { parseArgs } from "../lib/cli.mjs";

const flowArtifacts = resolve(artifactsRoot, "flow");
const coreArtifacts = resolve(artifactsRoot, "core");
const verificationRoot = resolve(targetRoot, "verification");

function toolVersion(command, args) {
  try {
    return run(command, args).trim();
  } catch {
    return "unknown";
  }
}

function compareFileRecords(left, right) {
  const differences = [];
  const leftMap = new Map(left.map((entry) => [entry.path, entry.sha256]));
  const rightMap = new Map(right.map((entry) => [entry.path, entry.sha256]));
  for (const path of new Set([...leftMap.keys(), ...rightMap.keys()])) {
    if (leftMap.get(path) !== rightMap.get(path)) {
      differences.push({
        path,
        first: leftMap.get(path) ?? null,
        second: rightMap.get(path) ?? null,
      });
    }
  }
  return differences;
}

async function packFlow({ destination, renameTo }) {
  const dist = await buildPackage({ profile: "flow" });
  const manifest = await checkPackage({ distDir: dist });
  const evidence = await packAndVerify({
    cwd: packageRoot,
    destination,
    extractRoot: verificationRoot,
    renameTo,
  });
  return { dist, manifest, evidence };
}

async function packCore(names) {
  const corePackage = resolve(coreArtifacts, "package");
  await mkdir(corePackage, { recursive: true });
  await copyPackageMetadata(packageRoot, corePackage, { forcePrivate: true });
  const dist = resolve(corePackage, "dist");
  await buildPackage({ profile: "core", distDir: dist });
  await checkPackage({
    distDir: dist,
    mode: "provisional",
    manifest: await readJson(resolve(corePackage, "package.json")),
  });
  const evidence = await packAndVerify({
    cwd: corePackage,
    destination: coreArtifacts,
    extractRoot: verificationRoot,
    renameTo: resolve(coreArtifacts, names.core),
    mode: "provisional",
  });
  return evidence;
}

async function main() {
  const flags = parseArgs(process.argv.slice(2));
  const repeat = flags.get("repeat") === true;

  await rm(artifactsRoot, { recursive: true, force: true });
  await rm(verificationRoot, { recursive: true, force: true });
  await mkdir(flowArtifacts, { recursive: true });
  await mkdir(coreArtifacts, { recursive: true });
  await mkdir(verificationRoot, { recursive: true });

  const manifest = await readJson(resolve(packageRoot, "package.json"));
  const names = tarballNames(manifest);

  const flow = await packFlow({
    destination: flowArtifacts,
    renameTo: resolve(flowArtifacts, names.flow),
  });

  let repeatEvidence;
  if (repeat) {
    const repeatDir = resolve(artifactsRoot, "flow-repeat");
    await mkdir(repeatDir, { recursive: true });
    const second = await packFlow({
      destination: repeatDir,
      renameTo: resolve(repeatDir, names.flow),
    });
    repeatEvidence = {
      firstTarballSha256: flow.evidence.tarball.sha256,
      secondTarballSha256: second.evidence.tarball.sha256,
      byteForByteTarballMatch:
        flow.evidence.tarball.sha256 === second.evidence.tarball.sha256,
      fileDifferences: compareFileRecords(
        flow.evidence.files,
        second.evidence.files,
      ),
    };
  }

  const core = await packCore(names);
  const git = gitInfo();
  const evidence = {
    generatedAt: new Date().toISOString(),
    package: {
      name: manifest.name,
      version: manifest.version,
      private: manifest.private,
      engines: manifest.engines,
    },
    adapterVersion: await adapterVersion(),
    wasmBindgenVersion: await lockedWasmBindgenVersion(),
    environment: {
      node: process.version,
      npm: process.env.npm_config_user_agent ?? "unknown",
      platform: process.platform,
      arch: process.arch,
      os: `${version()} (kernel ${release()})`,
    },
    toolchain: {
      rustc: toolVersion("rustc", ["--version"]),
      cargo: toolVersion("cargo", ["--version"]),
      wasmBindgen: toolVersion("wasm-bindgen", ["--version"]),
      typescript: toolVersion(process.execPath, [
        resolve(packageRoot, "node_modules/typescript/bin/tsc"),
        "--version",
      ]),
    },
    git,
    flow: flow.evidence,
    core: core,
    repeat: repeatEvidence,
  };
  await writeJson(resolve(artifactsRoot, "evidence.json"), evidence);
  await writeFile(
    resolve(artifactsRoot, "SHA256SUMS"),
    [
      `${flow.evidence.tarball.sha256}  flow/${names.flow}`,
      `${core.tarball.sha256}  core/${names.core}`,
      "",
    ].join("\n"),
    "utf8",
  );

  process.stdout.write(
    [
      `packed flow: ${flow.evidence.tarball.path}`,
      `  size=${flow.evidence.tarball.size} sha256=${flow.evidence.tarball.sha256}`,
      `packed core: ${core.tarball.path}`,
      `  size=${core.tarball.size} sha256=${core.tarball.sha256}`,
      repeatEvidence === undefined
        ? "repeat build: not requested"
        : `repeat build: tarballMatch=${repeatEvidence.byteForByteTarballMatch} fileDifferences=${repeatEvidence.fileDifferences.length}`,
      `evidence: ${resolve(artifactsRoot, "evidence.json")}`,
    ].join("\n") + "\n",
  );
}

await main();
