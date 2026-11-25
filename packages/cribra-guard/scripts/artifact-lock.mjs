/**
 * Pure artifact-lock policy.
 *
 * The build script and its regression tests share these functions so the
 * release-path guarantees are exercised without invoking Cargo or Chromium.
 */

export function platformKey(platform = process.platform, arch = process.arch) {
  return `${platform}-${arch}`;
}

/**
 * Normal builds (`verify`) must have a matching lock entry. Lock creation
 * (`write`) is explicit. Test-only builds that skip or override the artifact
 * must opt in twice: `--skip-build` plus `--allow-unverified`.
 */
export function resolveLockMode({
  writeLock = false,
  allowUnverified = false,
  skipBuild = false,
  wasmOverride = false,
  glueOverride = false,
}) {
  if (writeLock) return "write";
  if (skipBuild || wasmOverride || glueOverride) {
    if (!skipBuild || !allowUnverified) {
      throw new Error(
        "test-only build overrides (--skip-build, --wasm, --glue) bypass " +
          "artifact.lock.json verification; use --skip-build with " +
          "--allow-unverified explicitly",
      );
    }
    return "unverified";
  }
  return "verify";
}

export function assertUnverifiedOutput({ mode, targetOutDir, releaseOutDir }) {
  if (mode === "unverified" && targetOutDir === releaseOutDir) {
    throw new Error(
      "unverified test builds must write to an explicit --out directory; " +
        "refusing to replace the release dist",
    );
  }
}

export function assertLock(lock, { key, artifact, glue, raw = null }) {
  if (lock === undefined || lock === null) {
    throw new Error(
      "artifact.lock.json is missing; a normal build requires it. " +
        "Pass --write-lock to create it.",
    );
  }

  const expected = lock.artifacts?.[key];
  if (expected === undefined) {
    throw new Error(
      `artifact.lock.json has no entry for ${key}. ` +
        "Run `node scripts/build.mjs --write-lock` on this platform to add one.",
    );
  }

  const mismatches = [];
  if (lock.profile !== "flow") mismatches.push("profile");
  if (lock.wasmBindgenVersion !== artifact.wasmBindgenVersion) {
    mismatches.push("wasm-bindgen version");
  }
  if (lock.wasmOptVersion !== artifact.wasmOptVersion) {
    mismatches.push("wasm-opt version");
  }
  for (const field of ["bytes", "sha256"]) {
    if (expected.productionWasm?.[field] !== artifact.productionWasm[field]) {
      mismatches.push(`production WASM ${field}`);
    }
    if (expected.glue?.[field] !== glue[field]) {
      mismatches.push(`glue ${field}`);
    }
  }
  if (raw !== null && expected.rawWasm !== undefined) {
    for (const field of ["bytes", "sha256"]) {
      if (expected.rawWasm[field] !== raw[field]) {
        mismatches.push(`raw WASM ${field}`);
      }
    }
  }
  if (mismatches.length > 0) {
    throw new Error(
      `artifact.lock.json mismatch for ${key}: ${mismatches.join(", ")}. ` +
        `Computed production WASM ${artifact.productionWasm.bytes} bytes ` +
        `sha256 ${artifact.productionWasm.sha256}; glue ${glue.bytes} bytes ` +
        `sha256 ${glue.sha256}. ` +
        "Rebuild intentionally and pass --write-lock to update provenance.",
    );
  }
}

/** Adds or replaces one host entry while preserving every other host. */
export function mergeLock({ existing, key, artifact, glue, raw, features }) {
  const previous = existing?.artifacts?.[key] ?? {};
  const artifacts = {
    ...(existing?.artifacts ?? {}),
    [key]: {
      productionWasm: artifact.productionWasm,
      glue,
      ...(raw === null
        ? previous.rawWasm === undefined
          ? {}
          : { rawWasm: previous.rawWasm }
        : { rawWasm: raw }),
    },
  };
  return {
    profile: "flow",
    features,
    wasmBindgenVersion: artifact.wasmBindgenVersion,
    wasmOptVersion: artifact.wasmOptVersion,
    artifacts: Object.fromEntries(
      Object.entries(artifacts).sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    ),
  };
}
