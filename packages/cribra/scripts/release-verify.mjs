/**
 * Strict verification of a release tarball and release manifest.
 *
 * This module never builds, packs, or publishes. It only proves that the bytes
 * handed to the staging job are the bytes that were qualified.
 */

import { readFile } from "node:fs/promises";
import { basename, resolve } from "node:path";

import {
  DEFAULT_DIST_TAG,
  REGISTRY,
  assertReleaseIdentity,
  expectedReleaseTag,
  publishConfig,
  readManifest,
  tarballNames,
} from "./identity.mjs";
import {
  sha256File,
  targetRoot,
  verifyTarball,
} from "../qualification/lib/artifacts.mjs";
import { isMain, parseArgs } from "./util.mjs";

const verificationRoot = resolve(targetRoot, "release-verification");

export async function verifyReleaseTarball({
  tarball,
  manifest,
  release = true,
}) {
  const expected = manifest ?? (await readManifest());
  const names = tarballNames(expected);
  const actualName = basename(tarball);
  if (actualName !== names.flow) {
    throw new Error(
      `release tarball is ${actualName}; expected ${names.flow}`,
    );
  }
  if (release) assertReleaseIdentity(expected);

  const evidence = await verifyTarball({
    tarball,
    extractRoot: verificationRoot,
    mode: release ? "release" : "provisional",
    manifest: expected,
  });

  if (evidence.manifest.name !== expected.name) {
    throw new Error(
      `tarball package name ${evidence.manifest.name} does not match ` +
        `${expected.name}`,
    );
  }
  if (evidence.manifest.version !== expected.version) {
    throw new Error(
      `tarball package version ${evidence.manifest.version} does not match ` +
        `${expected.version}`,
    );
  }
  if (release) {
    const config = publishConfig(evidence.manifest);
    if (config.access !== "public") {
      throw new Error("release tarball publishConfig.access must be public");
    }
    if (config.provenance !== true) {
      throw new Error("release tarball publishConfig.provenance must be true");
    }
    if (config.registry !== REGISTRY) {
      throw new Error(`release tarball publishConfig.registry must be ${REGISTRY}`);
    }
    if (config.tag !== DEFAULT_DIST_TAG) {
      throw new Error(
        `release tarball publishConfig.tag must be ${DEFAULT_DIST_TAG}`,
      );
    }
  }

  for (const file of evidence.files) {
    if (
      file.path.startsWith("core/") ||
      file.path.startsWith("test/") ||
      file.path.startsWith("tests/") ||
      file.path.includes("/core/") ||
      file.path.startsWith("dist/wasm/core")
    ) {
      throw new Error(`release tarball contains Core-only path: ${file.path}`);
    }
  }

  return evidence;
}

export async function verifyReleaseManifest({
  manifest,
  tarball,
  release = true,
}) {
  const local = await readManifest();
  if (manifest.schemaVersion !== 1) {
    throw new Error(`unsupported release manifest schema: ${manifest.schemaVersion}`);
  }
  if (release && manifest.releaseable !== true) {
    throw new Error("release manifest is not marked releaseable");
  }
  if (manifest.package?.name !== local.name) {
    throw new Error(
      `release manifest package ${manifest.package?.name} does not match ` +
        `${local.name}`,
    );
  }
  if (manifest.package?.version !== local.version) {
    throw new Error(
      `release manifest version ${manifest.package?.version} does not match ` +
        `${local.version}`,
    );
  }
  if (manifest.package?.tag !== expectedReleaseTag(local.version)) {
    throw new Error(
      `release manifest tag ${manifest.package?.tag} does not match ` +
        `${expectedReleaseTag(local.version)}`,
    );
  }
  if (release) assertReleaseIdentity(local);
  if (manifest.source?.commit === undefined) {
    throw new Error("release manifest has no source commit");
  }
  if (manifest.core?.private !== true) {
    throw new Error("release manifest must record a test-only private Core tarball");
  }
  if (manifest.core?.excludedFromRelease !== true) {
    throw new Error("release manifest must exclude the Core tarball from release");
  }
  if (manifest.artifact?.filename !== basename(tarball)) {
    throw new Error(
      `release manifest artifact ${manifest.artifact?.filename} does not match ` +
        `${basename(tarball)}`,
    );
  }
  if (manifest.artifact?.sha256 === undefined) {
    throw new Error("release manifest has no artifact sha256");
  }
  if (manifest.artifact?.wasmSha256 === manifest.core?.wasmSha256) {
    throw new Error(
      "Flow and Core WASM digests are identical; the Flow tarball may be Core",
    );
  }

  const evidence = await verifyReleaseTarball({
    tarball,
    manifest: local,
    release,
  });
  const actualSha256 = await sha256File(tarball);
  if (actualSha256 !== manifest.artifact.sha256) {
    throw new Error(
      `tarball sha256 ${actualSha256} does not match manifest ` +
        `${manifest.artifact.sha256}`,
    );
  }
  if (evidence.tarball.size !== manifest.artifact.size) {
    throw new Error(
      `tarball size ${evidence.tarball.size} does not match manifest ` +
        `${manifest.artifact.size}`,
    );
  }
  if (evidence.wasm.sha256 !== manifest.artifact.wasmSha256) {
    throw new Error(
      `tarball WASM sha256 ${evidence.wasm.sha256} does not match manifest ` +
        `${manifest.artifact.wasmSha256}`,
    );
  }
  if (manifest.artifact.files !== undefined) {
    const actual = JSON.stringify(evidence.files);
    const recorded = JSON.stringify(manifest.artifact.files);
    if (actual !== recorded) {
      throw new Error("release manifest file records do not match the tarball");
    }
  }
  return { manifest, evidence };
}

if (isMain(import.meta.url)) {
  const flags = parseArgs(process.argv.slice(2));
  const release = flags.get("provisional") !== true;
  const releaseRoot =
    typeof flags.get("release-root") === "string"
      ? resolve(flags.get("release-root"))
      : undefined;

  let tarball =
    typeof flags.get("tarball") === "string" ? flags.get("tarball") : undefined;
  let manifestPath =
    typeof flags.get("manifest") === "string"
      ? flags.get("manifest")
      : undefined;
  let manifest;

  if (releaseRoot !== undefined) {
    manifestPath = resolve(releaseRoot, "release-manifest.json");
    manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    if (typeof manifest.artifact?.path !== "string") {
      throw new Error("release manifest has no artifact path");
    }
    if (
      manifest.artifact.path.startsWith("/") ||
      manifest.artifact.path.split("/").includes("..")
    ) {
      throw new Error(
        `release manifest artifact path escapes the release root: ${manifest.artifact.path}`,
      );
    }
    tarball = resolve(releaseRoot, manifest.artifact.path);
  } else {
    if (tarball === undefined) {
      throw new Error("missing required --tarball or --release-root");
    }
    if (manifestPath !== undefined) {
      manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    }
  }

  if (manifest === undefined) {
    const evidence = await verifyReleaseTarball({
      tarball: resolve(tarball),
      release,
    });
    process.stdout.write(
      `tarball verified: ${evidence.manifest.name}@${evidence.manifest.version} ` +
        `sha256=${evidence.tarball.sha256}\n`,
    );
  } else {
    const result = await verifyReleaseManifest({
      manifest,
      tarball: resolve(tarball),
      release,
    });
    process.stdout.write(
      `release verified: ${result.manifest.package.name}@` +
        `${result.manifest.package.version} ` +
        `sha256=${result.manifest.artifact.sha256}\n`,
    );
  }
}
