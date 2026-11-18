/**
 * Fail-closed gate that must pass immediately before `npm stage publish`.
 *
 * It refuses the provisional identity, an unapproved manifest, a stale
 * tarball, an unverified ref, a missing explicit confirmation, and a version
 * that already exists in the registry.
 */

import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import {
  assertReleaseIdentity,
  expectedReleaseTag,
  readManifest,
} from "./identity.mjs";
import { verifyReleaseManifest } from "./release-verify.mjs";
import { isMain, parseArgs } from "./util.mjs";

function assertNotPublished(name, version) {
  const result = spawnSync(
    "npm",
    ["view", `${name}@${version}`, "--json"],
    { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
  const output = `${result.stdout}\n${result.stderr}`;
  if (result.status === 0 && output.trim().length > 0) {
    throw new Error(
      `${name}@${version} already exists in the registry; refusing to stage`,
    );
  }
  if (!/E404|not found|404/i.test(output)) {
    throw new Error(
      `could not prove ${name}@${version} is unpublished: ${output.trim()}`,
    );
  }
}

export async function checkStageAuthorization({
  tag,
  manifestPath,
  tarball,
  confirmation,
  authorized,
}) {
  const local = await readManifest();
  assertReleaseIdentity(local);

  if (!authorized) {
    throw new Error("staging requires explicit --authorized");
  }
  if (process.env.CRIBRA_RELEASE_AUTHORIZED !== "1") {
    throw new Error(
      "staging requires CRIBRA_RELEASE_AUTHORIZED=1 from the protected environment",
    );
  }
  if (confirmation !== `stage ${tag}`) {
    throw new Error(
      `confirmation must be exactly "stage ${tag}" for this release`,
    );
  }
  if (tag !== expectedReleaseTag(local.version)) {
    throw new Error(
      `tag ${tag} does not match the package version ` +
        `${expectedReleaseTag(local.version)}`,
    );
  }

  const manifest = JSON.parse(await readFile(resolve(manifestPath), "utf8"));
  if (manifest.releaseable !== true) {
    throw new Error("release manifest is not releaseable");
  }
  if (manifest.source?.tag !== tag) {
    throw new Error(
      `release manifest tag ${String(manifest.source?.tag)} is not ${tag}`,
    );
  }

  if (process.env.GITHUB_ACTIONS === "true") {
    if (process.env.GITHUB_REF !== `refs/tags/${tag}`) {
      throw new Error(
        `workflow ref ${String(process.env.GITHUB_REF)} is not ` +
          `refs/tags/${tag}`,
      );
    }
    if (process.env.GITHUB_SHA !== manifest.source?.commit) {
      throw new Error(
        `workflow sha ${String(process.env.GITHUB_SHA)} does not match the ` +
          `qualified commit ${String(manifest.source?.commit)}`,
      );
    }
  }

  const verified = await verifyReleaseManifest({
    manifest,
    tarball,
    release: true,
  });
  assertNotPublished(local.name, local.version);
  return verified;
}

if (isMain(import.meta.url)) {
  const flags = parseArgs(process.argv.slice(2));
  const tag = flags.get("tag");
  const manifestPath = flags.get("manifest");
  const tarball = flags.get("tarball");
  const confirmation = flags.get("confirmation");
  if (typeof tag !== "string") throw new Error("missing required --tag");
  if (typeof manifestPath !== "string") {
    throw new Error("missing required --manifest");
  }
  if (typeof tarball !== "string") {
    throw new Error("missing required --tarball");
  }
  if (typeof confirmation !== "string") {
    throw new Error("missing required --confirmation");
  }

  const result = await checkStageAuthorization({
    tag,
    manifestPath,
    tarball,
    confirmation,
    authorized: flags.get("authorized") === true,
  });
  process.stdout.write(
    `staging authorized: ${result.manifest.package.name}@` +
      `${result.manifest.package.version} sha256=` +
      `${result.manifest.artifact.sha256}\n`,
  );
}
