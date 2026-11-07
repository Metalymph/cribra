/**
 * Single source of truth for the npm package identity and release conventions.
 *
 * The package name and version live in `package.json`. Qualification, packing,
 * and release scripts must derive every path and specifier from this module so
 * that renaming the package is a manifest change, not a repository-wide edit.
 */

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { packageRoot } from "./util.mjs";

/** The provisional development identity. Release mode rejects this name. */
export const PROVISIONAL_PACKAGE_NAME = "cribra-typescript";

/** Git tag namespace for the independently versioned TypeScript package. */
export const RELEASE_TAG_PREFIX = "typescript-v";

/** Registry and distribution defaults for the public Flow package. */
export const REGISTRY = "https://registry.npmjs.org/";
export const DEFAULT_DIST_TAG = "latest";

/** Placeholder rewritten in qualification fixtures from the manifest. */
export const PACKAGE_SPECIFIER_TOKEN = "__CRIBRA_PACKAGE_SPECIFIER__";

/** Custom manifest field describing the approved release identity. */
export const RELEASE_POLICY_FIELD = "cribraRelease";

const SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

export async function readManifest(
  path = resolve(packageRoot, "package.json"),
) {
  const manifest = JSON.parse(await readFile(path, "utf8"));
  if (typeof manifest.name !== "string" || manifest.name.length === 0) {
    throw new Error("package.json has no name");
  }
  if (typeof manifest.version !== "string" || manifest.version.length === 0) {
    throw new Error("package.json has no version");
  }
  return manifest;
}

export function assertSemver(version, label = "version") {
  if (typeof version !== "string" || !SEMVER.test(version)) {
    throw new Error(`${label} is not valid SemVer: ${String(version)}`);
  }
  return version;
}

/** `@scope/name` -> `scope-name`; `name` -> `name`. */
export function packageSlug(name) {
  const slug = name.replace(/^@/, "").replace(/\//g, "-");
  if (!/^[a-z0-9][a-z0-9._-]*$/.test(slug)) {
    throw new Error(`package name cannot be converted to a tarball slug: ${name}`);
  }
  return slug;
}

/** Filesystem path of an installed package below a consumer root. */
export function installedPackagePath(consumerDirectory, name) {
  return resolve(consumerDirectory, "node_modules", ...name.split("/"));
}

export function tarballNames(manifest) {
  const slug = packageSlug(manifest.name);
  return {
    slug,
    flow: `${slug}-${manifest.version}.tgz`,
    core: `${slug}-${manifest.version}-core.tgz`,
  };
}

export function expectedReleaseTag(version) {
  return `${RELEASE_TAG_PREFIX}${assertSemver(version)}`;
}

export function parseReleaseTag(tag) {
  if (typeof tag !== "string" || !tag.startsWith(RELEASE_TAG_PREFIX)) {
    throw new Error(
      `release tag must start with ${RELEASE_TAG_PREFIX}: ${String(tag)}`,
    );
  }
  return assertSemver(tag.slice(RELEASE_TAG_PREFIX.length), "release tag version");
}

export function releasePolicy(manifest) {
  const policy = manifest[RELEASE_POLICY_FIELD];
  return policy !== null && typeof policy === "object" ? policy : {};
}

export function isProvisional(manifest) {
  const policy = releasePolicy(manifest);
  return (
    manifest.name === PROVISIONAL_PACKAGE_NAME ||
    policy.state !== "approved"
  );
}

export function publishConfig(manifest) {
  const config =
    manifest.publishConfig !== null &&
    typeof manifest.publishConfig === "object"
      ? manifest.publishConfig
      : {};
  return {
    access: config.access,
    provenance: config.provenance,
    registry: config.registry,
    tag: config.tag,
  };
}

/**
 * Fail closed unless the manifest is an approved, publishable Flow package.
 *
 * `cribraRelease.state` must be `approved` and `cribraRelease.approvedName`
 * must equal `name`. The provisional manifest deliberately keeps state
 * `pending-ownership`, so staging cannot run before the rename is authorized.
 */
export function assertReleaseIdentity(manifest) {
  const policy = releasePolicy(manifest);
  if (policy.state !== "approved") {
    throw new Error(
      `release identity is not approved (${RELEASE_POLICY_FIELD}.state=` +
        `${String(policy.state ?? "missing")}); rename the package only after ` +
        "authenticated npm ownership is verified",
    );
  }
  if (manifest.name !== policy.approvedName) {
    throw new Error(
      `package name ${manifest.name} does not match the approved release name ` +
        `${String(policy.approvedName)}`,
    );
  }
  if (manifest.name === PROVISIONAL_PACKAGE_NAME) {
    throw new Error(
      `release identity is still the provisional name ${PROVISIONAL_PACKAGE_NAME}`,
    );
  }
  if (manifest.private === true) {
    throw new Error("release package must not be marked private");
  }
  if (policy.tagPrefix !== RELEASE_TAG_PREFIX) {
    throw new Error(
      `${RELEASE_POLICY_FIELD}.tagPrefix must be ${RELEASE_TAG_PREFIX}`,
    );
  }
  if (policy.distTag !== DEFAULT_DIST_TAG) {
    throw new Error(
      `${RELEASE_POLICY_FIELD}.distTag must be ${DEFAULT_DIST_TAG}`,
    );
  }
  if (policy.registry !== REGISTRY) {
    throw new Error(`${RELEASE_POLICY_FIELD}.registry must be ${REGISTRY}`);
  }
  assertSemver(manifest.version);
  if (manifest.type !== "module") {
    throw new Error("release package must be ESM-only");
  }
  if (manifest.engines?.node !== ">=22") {
    throw new Error(
      `release package must declare engines.node >=22, got ` +
        `${String(manifest.engines?.node)}`,
    );
  }

  const config = publishConfig(manifest);
  if (config.access !== "public") {
    throw new Error("publishConfig.access must be public");
  }
  if (config.provenance !== true) {
    throw new Error("publishConfig.provenance must be true");
  }
  if (config.registry !== REGISTRY) {
    throw new Error(`publishConfig.registry must be ${REGISTRY}`);
  }
  if (config.tag !== DEFAULT_DIST_TAG) {
    throw new Error(`publishConfig.tag must be ${DEFAULT_DIST_TAG}`);
  }

  return manifest;
}
