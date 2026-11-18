/**
 * Release readiness: verify identity, ref, qualification evidence, and produce
 * a release manifest for the exact qualified Flow tarball.
 *
 * No registry call, tag, or publish occurs here. The workflow runs this script
 * with `--require-matrix --require-releaseable`; local dry runs may use
 * `--allow-provisional` and omit the matrix requirement.
 */

import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { release as osRelease, version as osVersion } from "node:os";
import { resolve } from "node:path";

import {
  DEFAULT_DIST_TAG,
  REGISTRY,
  assertReleaseIdentity,
  expectedReleaseTag,
  parseReleaseTag,
  readManifest,
  tarballNames,
} from "./identity.mjs";
import {
  artifactsRoot,
  gitInfo,
  readJson,
  sha256File,
  targetRoot,
  writeJson,
} from "../qualification/lib/artifacts.mjs";
import { verifyReleaseTarball } from "./release-verify.mjs";
import { isMain, packageRoot, parseArgs, repoRoot } from "./util.mjs";

const releaseRoot = resolve(repoRoot, "target/typescript-release");
const evidenceRoot = resolve(targetRoot, "evidence");

function commandVersion(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : "unknown";
}

function git(args) {
  const result = spawnSync("git", args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
  return result.status === 0 ? result.stdout.trim() : undefined;
}

function assertReleaseRef({ tag, local }) {
  const head = git(["rev-parse", "HEAD"]);
  if (typeof head !== "string" || head.length === 0) {
    throw new Error("cannot resolve HEAD");
  }
  const tagCommit = git(["rev-parse", `refs/tags/${tag}`]);
  if (tagCommit === undefined) {
    if (!local) throw new Error(`release tag ${tag} does not exist`);
    process.stdout.write(`warning: tag ${tag} does not exist (local mode)\n`);
  } else if (tagCommit !== head) {
    throw new Error(
      `release tag ${tag} points to ${tagCommit}, not HEAD ${head}`,
    );
  }

  if (!local && process.env.GITHUB_ACTIONS === "true") {
    const expectedRef = `refs/tags/${tag}`;
    if (process.env.GITHUB_REF !== expectedRef) {
      throw new Error(
        `workflow ref ${String(process.env.GITHUB_REF)} is not ${expectedRef}`,
      );
    }
    if (process.env.GITHUB_SHA !== head) {
      throw new Error(
        `workflow sha ${String(process.env.GITHUB_SHA)} is not HEAD ${head}`,
      );
    }
  }

  if (!local) {
    const status = git(["status", "--porcelain"]);
    if (status !== undefined && status.length > 0) {
      throw new Error("release checkout is not clean");
    }
  }
  return head;
}

function packageSource(manifest) {
  const repository = manifest.repository ?? {};
  const url =
    typeof repository.url === "string"
      ? repository.url.replace(/\.git$/, "")
      : process.env.GITHUB_REPOSITORY !== undefined
        ? `https://github.com/${process.env.GITHUB_REPOSITORY}`
        : "unknown";
  return {
    repository: url,
    directory: repository.directory ?? "packages/cribra",
  };
}

async function collectEvidence() {
  let names;
  try {
    names = (await readdir(evidenceRoot)).filter(
      (name) => name.endsWith(".json") && name !== "summary.json",
    );
  } catch {
    return { files: [], records: [], matrix: undefined };
  }
  names.sort();
  const records = [];
  const matrix = {
    node: { flow: [], core: [] },
    typescript: [],
    browsers: [],
    bundlers: [],
    nativeSafari: "unverified",
  };

  for (const name of names) {
    const path = resolve(evidenceRoot, name);
    const record = await readJson(path);
    records.push({
      file: name,
      sha256: await sha256File(path),
      bytes: (await readFile(path)).length,
    });
    if (name.startsWith("node-")) {
      const major = String(record.nodeVersion ?? "")
        .replace(/^v/, "")
        .split(".")[0];
      if (major.length > 0 && Array.isArray(matrix.node[record.profile])) {
        matrix.node[record.profile].push(major);
      }
    } else if (name.startsWith("typescript-")) {
      matrix.typescript.push({
        file: name,
        compilers: Array.isArray(record.compilers)
          ? record.compilers.map((entry) => entry.compiler)
          : [],
      });
    } else if (name.startsWith("browser-")) {
      matrix.browsers.push({
        file: name,
        engine: record.engine,
        profile: record.profile,
        browserVersion: record.browserVersion,
      });
    } else if (name === "vite.json" || name === "webpack.json") {
      matrix.bundlers.push({
        file: name,
        bundler: record.bundler,
        bundlerVersion: record.bundlerVersion,
        engine: record.engine,
      });
    } else if (name === "safari-native.json") {
      matrix.nativeSafari = record.status ?? "unknown";
    }
  }
  for (const profile of ["flow", "core"]) {
    matrix.node[profile] = [...new Set(matrix.node[profile])].sort();
  }
  return { files: names, records, matrix };
}

function assertQualificationMatrix(matrix) {
  if (matrix === undefined) {
    throw new Error("no qualification evidence was collected");
  }
  for (const major of ["22", "24", "26"]) {
    for (const profile of ["flow", "core"]) {
      if (!matrix.node[profile].includes(major)) {
        throw new Error(
          `missing Node ${major} ${profile} consumer qualification evidence`,
        );
      }
    }
  }
  if (matrix.typescript.length === 0) {
    throw new Error("missing TypeScript consumer qualification evidence");
  }
  const compilerVersions = new Set(
    matrix.typescript.flatMap((entry) => entry.compilers),
  );
  for (const compiler of ["5.9.3", "7.0.2"]) {
    if (!compilerVersions.has(compiler)) {
      throw new Error(`missing TypeScript ${compiler} qualification evidence`);
    }
  }
  const browserKeys = new Set(
    matrix.browsers.map((entry) => `${entry.engine}-${entry.profile}`),
  );
  for (const key of [
    "chrome-flow",
    "chromium-flow",
    "firefox-flow",
    "webkit-flow",
    "chrome-core",
  ]) {
    if (!browserKeys.has(key)) {
      throw new Error(`missing browser qualification evidence: ${key}`);
    }
  }
  for (const bundler of ["vite", "webpack"]) {
    if (!matrix.bundlers.some((entry) => entry.bundler === bundler)) {
      throw new Error(`missing ${bundler} qualification evidence`);
    }
  }
}

async function main() {
  const flags = parseArgs(process.argv.slice(2));
  const local = flags.get("local") === true;
  const skipQualification = flags.get("skip-qualification") === true;
  const allowProvisional = flags.get("allow-provisional") === true;
  const requireMatrix = flags.get("require-matrix") === true;
  const requireReleaseable = flags.get("require-releaseable") === true;
  const outputRoot =
    typeof flags.get("output") === "string"
      ? resolve(repoRoot, flags.get("output"))
      : releaseRoot;

  const manifest = await readManifest();
  const tag =
    typeof flags.get("tag") === "string"
      ? flags.get("tag")
      : expectedReleaseTag(manifest.version);
  const tagVersion = parseReleaseTag(tag);
  if (tagVersion !== manifest.version) {
    throw new Error(
      `tag ${tag} does not match package.json version ${manifest.version}`,
    );
  }

  if (!allowProvisional) assertReleaseIdentity(manifest);
  const commit = assertReleaseRef({ tag, local });
  const gitState = gitInfo();
  const source = packageSource(manifest);
  const names = tarballNames(manifest);

  if (!skipQualification) {
    execFileSync("npm", ["run", "qualify:all"], {
      cwd: packageRoot,
      stdio: "inherit",
    });
  }

  const packEvidencePath = resolve(artifactsRoot, "evidence.json");
  let packEvidence;
  try {
    packEvidence = await readJson(packEvidencePath);
  } catch {
    throw new Error(
      `missing pack evidence at ${packEvidencePath}; run npm run qualify:pack`,
    );
  }
  if (packEvidence.package?.name !== manifest.name) {
    throw new Error(
      `pack evidence name ${packEvidence.package?.name} does not match ` +
        `${manifest.name}`,
    );
  }
  if (packEvidence.package?.version !== manifest.version) {
    throw new Error(
      `pack evidence version ${packEvidence.package?.version} does not match ` +
        `${manifest.version}`,
    );
  }
  if (packEvidence.repeat?.byteForByteTarballMatch !== true) {
    throw new Error(
      "repeat pack was not byte-for-byte reproducible; refusing to certify",
    );
  }
  if (
    Array.isArray(packEvidence.repeat?.fileDifferences) &&
    packEvidence.repeat.fileDifferences.length > 0
  ) {
    throw new Error(
      "repeat pack produced different file contents; refusing to certify",
    );
  }
  if (packEvidence.core?.manifest?.private !== true) {
    throw new Error("test-only Core tarball is not private");
  }
  if (packEvidence.flow?.wasm?.sha256 === packEvidence.core?.wasm?.sha256) {
    throw new Error("Flow and Core WASM digests are identical");
  }

  const flowTarball = resolve(artifactsRoot, "flow", names.flow);
  const releaseable = !allowProvisional;
  const evidence = await verifyReleaseTarball({
    tarball: flowTarball,
    manifest,
    release: releaseable,
  });
  if (evidence.tarball.sha256 !== packEvidence.flow.tarball.sha256) {
    throw new Error("qualified Flow tarball digest changed during verification");
  }
  if (evidence.wasm.sha256 !== packEvidence.flow.wasm.sha256) {
    throw new Error("qualified Flow WASM digest changed during verification");
  }

  const collected = await collectEvidence();
  if (requireMatrix) assertQualificationMatrix(collected.matrix);
  if (requireReleaseable && !releaseable) {
    throw new Error(
      "release readiness requires an approved, non-provisional package identity",
    );
  }

  const generatedAt = new Date().toISOString();
  const releaseManifest = {
    schemaVersion: 1,
    generatedAt,
    releaseable,
    package: {
      name: manifest.name,
      version: manifest.version,
      tag,
      distTag: manifest.publishConfig?.tag ?? DEFAULT_DIST_TAG,
      access: manifest.publishConfig?.access ?? "public",
      registry: manifest.publishConfig?.registry ?? REGISTRY,
    },
    source: {
      ...source,
      commit,
      ref: gitState.branch,
      tag,
      clean: gitState.status === "",
    },
    toolchain: {
      node: process.version,
      npm: commandVersion("npm", ["--version"]),
      rustc: packEvidence.toolchain?.rustc ?? "unknown",
      cargo: packEvidence.toolchain?.cargo ?? "unknown",
      wasmBindgen:
        packEvidence.toolchain?.wasmBindgen ??
        packEvidence.wasmBindgenVersion ??
        "unknown",
      typescript: packEvidence.toolchain?.typescript ?? "unknown",
      platform: process.platform,
      arch: process.arch,
      os: `${osVersion()} (kernel ${osRelease()})`,
    },
    adapter: {
      crate: "cribra-wasm",
      version: packEvidence.adapterVersion,
      wasmBindgenVersion: packEvidence.wasmBindgenVersion,
    },
    artifact: {
      filename: names.flow,
      path: `flow/${names.flow}`,
      sha256: evidence.tarball.sha256,
      size: evidence.tarball.size,
      wasmSha256: evidence.wasm.sha256,
      npmIntegrity: evidence.npmPack?.integrity,
      files: evidence.files,
    },
    qualification: {
      command: skipQualification
        ? "reused workflow_call qualification evidence"
        : "npm run qualify:all",
      evidenceRecords: collected.records,
      matrix: collected.matrix ?? { status: "not-collected" },
      repeat: packEvidence.repeat,
      packGeneratedAt: packEvidence.generatedAt,
    },
    core: {
      filename: names.core,
      path: `core/${names.core}`,
      sha256: packEvidence.core.tarball.sha256,
      size: packEvidence.core.tarball.size,
      wasmSha256: packEvidence.core.wasm.sha256,
      private: true,
      excludedFromRelease: true,
    },
    warnings: [
      "Native Safari remains unverified and is not advertised as qualified.",
      ...(releaseable
        ? []
        : ["Provisional development identity; this manifest is not publishable."]),
    ],
  };

  await rm(outputRoot, { recursive: true, force: true });
  await mkdir(resolve(outputRoot, "flow"), { recursive: true });
  await copyFile(flowTarball, resolve(outputRoot, "flow", names.flow));
  await writeJson(resolve(outputRoot, "release-manifest.json"), releaseManifest);
  await writeFile(
    resolve(outputRoot, "SHA256SUMS"),
    `${releaseManifest.artifact.sha256}  flow/${names.flow}\n`,
  );

  process.stdout.write(
    [
      `release readiness: ${manifest.name}@${manifest.version}`,
      `  tag=${tag} commit=${commit}`,
      `  releaseable=${releaseable}`,
      `  flow sha256=${releaseManifest.artifact.sha256}`,
      `  core sha256=${releaseManifest.core.sha256} (test-only, excluded)`,
      `  manifest=${resolve(outputRoot, "release-manifest.json")}`,
    ].join("\n") + "\n",
  );
}

if (isMain(import.meta.url)) {
  await main();
}
