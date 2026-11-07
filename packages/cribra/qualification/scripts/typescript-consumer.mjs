/** Installs the tarball and type-checks real public API usage. */

import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

import {
  artifactPaths,
  artifactsRoot,
  qualificationRoot,
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
const artifacts = await artifactPaths();
const tarball = resolve(flagString(flags, "tarball", artifacts.flow));
const requestedCompiler = flags.get("compiler");
const compilers = [
  { version: "5.9.3", tsc: resolve(qualificationRoot, "node_modules/typescript-5/bin/tsc") },
  { version: "7.0.2", tsc: resolve(qualificationRoot, "node_modules/typescript/bin/tsc") },
].filter(
  (compiler) =>
    typeof requestedCompiler !== "string" ||
    compiler.version === requestedCompiler,
);
if (compilers.length === 0) {
  throw new Error(`unsupported TypeScript compiler: ${String(requestedCompiler)}`);
}

const evidence = await readJson(resolve(artifactsRoot, "evidence.json"));
const consumer = await createConsumer({ label: "typescript", tarball });

async function exists(path) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

function relativeSpecifiers(text) {
  const result = new Set();
  for (const match of text.matchAll(
    /(?:from\s+|import\s*\(\s*)["']([^"']+)["']/g,
  )) {
    result.add(match[1]);
  }
  return result;
}

async function closure(entry) {
  const seen = new Set();
  const queue = [entry];
  const files = [];
  while (queue.length > 0) {
    const file = queue.pop();
    if (seen.has(file)) continue;
    seen.add(file);
    const text = await readFile(file, "utf8");
    files.push({ file, text });
    for (const specifier of relativeSpecifiers(text)) {
      if (!specifier.startsWith(".")) continue;
      const base = resolve(dirname(file), specifier);
      const candidate = base.endsWith(".js")
        ? base.replace(/\.js$/, ".d.ts")
        : `${base}.d.ts`;
      if (await exists(candidate)) queue.push(candidate);
      else if (await exists(base)) queue.push(base);
      else throw new Error(`declaration import does not resolve: ${specifier} from ${file}`);
    }
  }
  return files;
}

async function checkDeclarations(packageDir) {
  const entry = resolve(packageDir, "dist/index.d.ts");
  const files = await closure(entry);
  const combined = files.map((file) => file.text).join("\n");
  for (const { file, text } of files) {
    if (text.includes("/wasm/")) {
      throw new Error(`public declaration references wasm internals: ${file}`);
    }
    if (text.includes("packages/cribra/src") || text.includes("/Users/")) {
      throw new Error(`public declaration contains a local path: ${file}`);
    }
  }
  for (const name of [
    "init",
    "createScanner",
    "Scanner",
    "ScanResult",
    "BatchResult",
    "StreamSession",
    "IncrementalSession",
    "TransformRequest",
    "TransformResult",
    "ShareBundle",
    "CribraError",
    "getCapabilities",
    "getVersion",
    "byteSpanToTextRange",
  ]) {
    if (!combined.includes(name)) {
      throw new Error(`public declaration graph is missing ${name}`);
    }
  }
  for (const forbidden of [
    "ScanEngine",
    "ShareBundleBuilder",
    "TransformError",
    "ShareBundleError",
  ]) {
    if (new RegExp(`\\b${forbidden}\\b`).test(combined)) {
      throw new Error(`public declaration graph exposes ${forbidden}`);
    }
  }
  return files.length;
}

try {
  if (process.env.CRIBRA_TYPESCRIPT_COMPILERS === undefined) {
    // The runner may be invoked directly without npm; the fixture still works.
  }
  const usagePath = resolve(consumer.directory, "usage.ts");
  await copyFixture(
    resolve(qualificationRoot, "apps/typescript/usage.ts"),
    usagePath,
  );
  await copyFixture(
    resolve(qualificationRoot, "apps/typescript/tsconfig.nodenext.json"),
    resolve(consumer.directory, "tsconfig.nodenext.json"),
  );
  await copyFixture(
    resolve(qualificationRoot, "apps/typescript/tsconfig.bundler.json"),
    resolve(consumer.directory, "tsconfig.bundler.json"),
  );

  const usage = await readFile(usagePath, "utf8");
  for (const forbidden of [
    "packages/cribra/src",
    "packages/cribra/dist",
    "../src/",
    "../dist/",
    "/dist/",
  ]) {
    if (usage.includes(forbidden)) {
      throw new Error(`usage fixture imports local path ${forbidden}`);
    }
  }
  if (!usage.includes(`from "${consumer.packageName}"`)) {
    throw new Error(
      `usage fixture does not import ${consumer.packageName} by name`,
    );
  }

  const declarationCount = await checkDeclarations(consumer.installed);
  const results = [];
  for (const compiler of compilers) {
    for (const mode of ["nodenext", "bundler"]) {
      const { stdout } = await runChild({
        args: [
          compiler.tsc,
          "-p",
          resolve(consumer.directory, `tsconfig.${mode}.json`),
        ],
        cwd: consumer.directory,
        label: `typescript ${compiler.version} ${mode}`,
        timeout: 120_000,
      });
      results.push({
        compiler: compiler.version,
        resolution: mode,
        passed: true,
        output: stdout.trim(),
      });
      process.stdout.write(
        `typescript ${compiler.version} ${mode}: passed\n`,
      );
    }
  }

  const report = {
    tarball,
    tarballSha256: evidence.flow.tarball.sha256,
    packageVersion: evidence.package.version,
    declarationCount,
    compilers: results,
  };
  await mkdir(resolve(targetRoot, "evidence"), { recursive: true });
  await writeJson(
    resolve(
      targetRoot,
      "evidence",
      `typescript-${String(requestedCompiler ?? "all").replace(/[^0-9.]/g, "") || "all"}.json`,
    ),
    report,
  );
  process.stdout.write(
    `typescript qualification passed: ${results.length} compiler/resolution combinations\n`,
  );
} finally {
  await removeConsumer(consumer.directory);
}
