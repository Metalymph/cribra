/** Builds/serves the installed package with Vite and runs it in a real browser. */

import { mkdir, readFile } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { build as viteBuild, createServer } from "vite";

import {
  artifactPaths,
  artifactsRoot,
  readJson,
  targetRoot,
  walkFiles,
  writeJson,
} from "../lib/artifacts.mjs";
import {
  copyFixture,
  createConsumer,
  removeConsumer,
} from "../lib/consumer.mjs";
import { startServer } from "../lib/server.mjs";
import { expectPass, launchBrowser } from "../lib/browsers.mjs";
import { flagString, parseArgs } from "../lib/cli.mjs";

const flags = parseArgs(process.argv.slice(2));
const engine = flagString(flags, "engine", "chrome");
const artifacts = await artifactPaths();
const tarball = resolve(flagString(flags, "tarball", artifacts.flow));
const evidence = await readJson(resolve(artifactsRoot, "evidence.json"));
const qualificationRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "..",
);
const configFile = resolve(qualificationRoot, "configs/vite.config.mjs");
const appSource = resolve(qualificationRoot, "apps/vite");
const sharedSuite = resolve(qualificationRoot, "apps/shared/app-suite.mjs");

const consumer = await createConsumer({ label: `vite-${engine}`, tarball });
process.env.CRIBRA_PACKAGE_NAME = consumer.packageName;
let launched;
let server;

function assertBundled(buildDir) {
  return walkFiles(buildDir).then((files) => {
    const wasm = files.filter((file) => file.endsWith(".wasm"));
    if (wasm.length === 0) throw new Error("Vite emitted no WASM asset");
    for (const file of wasm) {
      if (file.includes("node_modules")) {
        throw new Error(`WASM asset leaked node_modules path: ${file}`);
      }
      if (!/[.-][A-Za-z0-9_-]{8,}\.wasm$/.test(basename(file))) {
        throw new Error(`WASM asset is not content-hashed: ${file}`);
      }
    }
    const javascript = files.filter((file) => file.endsWith(".js"));
    if (javascript.length === 0) throw new Error("Vite emitted no JavaScript");
    for (const file of javascript) {
      if (!/[.-][A-Za-z0-9_-]{8,}\.js$/.test(basename(file))) {
        throw new Error(`JavaScript asset is not content-hashed: ${file}`);
      }
    }
    return { wasm, javascript };
  });
}

async function readJavaScript(buildDir, files) {
  const texts = [];
  for (const file of files.filter((entry) => entry.endsWith(".js"))) {
    texts.push(await readFile(resolve(buildDir, file), "utf8"));
  }
  return texts.join("\n");
}

try {
  await mkdir(resolve(consumer.directory, "app"), { recursive: true });
  const appDir = resolve(consumer.directory, "app");
  const outDir = resolve(consumer.directory, "dist");
  await copyFixture(resolve(appSource, "index.html"), resolve(appDir, "index.html"));
  await copyFixture(resolve(appSource, "main.mjs"), resolve(appDir, "main.mjs"));
  await copyFixture(sharedSuite, resolve(appDir, "app-suite.mjs"));
  process.env.CRIBRA_APP_DIR = appDir;
  process.env.CRIBRA_OUT_DIR = outDir;

  const query = new URLSearchParams({
    profile: "flow",
    version: evidence.package.version,
  });
  const devAttempts = [];
  let devFallback = false;
  for (const exclude of [false, true]) {
    if (exclude) devFallback = true;
    process.env.CRIBRA_VITE_EXCLUDE = exclude ? "1" : "0";
    let devServer;
    try {
      devServer = await createServer({
        configFile,
        logLevel: "error",
        server: { host: "127.0.0.1", port: 0 },
      });
      await devServer.listen();
      const addresses = devServer.resolvedUrls?.local ?? [];
      const base = addresses.find((url) => url.startsWith("http://127.0.0.1"));
      if (base === undefined) throw new Error("Vite did not expose a local URL");
      launched = await launchBrowser(engine);
      const devResult = await expectPass({
        browser: launched.browser,
        url: `${base}index.html?${query.toString()}`,
        label: `Vite dev ${launched.label}`,
      });
      devAttempts.push({
        exclude,
        passed: true,
        wasmResponses: devResult.wasmResponses,
      });
      break;
    } catch (error) {
      devAttempts.push({
        exclude,
        passed: false,
        error: error instanceof Error ? error.message : String(error),
      });
      if (exclude) throw error;
    } finally {
      if (launched !== undefined) {
        await launched.browser.close();
        launched = undefined;
      }
      if (devServer !== undefined) await devServer.close();
    }
  }
  if (!devAttempts.some((attempt) => attempt.passed)) {
    throw new Error(`Vite dev qualification failed: ${JSON.stringify(devAttempts)}`);
  }
  process.env.CRIBRA_VITE_EXCLUDE = "0";

  await viteBuild({ configFile, logLevel: "error" });
  const emitted = await assertBundled(outDir);
  const bundleText = await readJavaScript(outDir, emitted.javascript);
  for (const forbidden of ["node:fs", "node:path", "node:url"]) {
    if (bundleText.includes(forbidden)) {
      throw new Error(`Vite production bundle contains ${forbidden}`);
    }
  }
  if (!bundleText.includes("cribra_bg")) {
    throw new Error("Vite production bundle does not reference the WASM asset");
  }
  const indexHtml = await readFile(resolve(outDir, "index.html"), "utf8");
  if (!indexHtml.includes("/app/")) {
    throw new Error("Vite production index does not use the configured base");
  }

  server = await startServer({
    mounts: [{ prefix: "/app/", directory: outDir }],
  });
  launched = await launchBrowser(engine);
  const production = await expectPass({
    browser: launched.browser,
    url: `${server.url}/app/index.html?${query.toString()}`,
    label: `Vite production ${launched.label}`,
  });
  for (const response of production.wasmResponses) {
    if (response.status === 200) {
      if (!response.url.includes("/app/assets/")) {
        throw new Error(`Vite WASM URL is outside hashed assets: ${response.url}`);
      }
      if (!String(response.contentType ?? "").includes("application/wasm")) {
        throw new Error(
          `Vite WASM response has wrong MIME: ${response.contentType}`,
        );
      }
    }
  }
  const wasmUrl = production.wasmResponses.find(
    (response) => response.status === 200,
  )?.url;
  if (wasmUrl === undefined) throw new Error("Vite loaded no WASM asset");
  const emittedWasm = emitted.wasm[0];
  if (!wasmUrl.endsWith(emittedWasm)) {
    throw new Error(
      `Vite loaded ${wasmUrl} but emitted ${emittedWasm}`,
    );
  }

  const report = {
    bundler: "vite",
    bundlerVersion: JSON.parse(
      await readFile(
        resolve(qualificationRoot, "node_modules/vite/package.json"),
        "utf8",
      ),
    ).version,
    engine,
    browser: launched.label,
    browserVersion: launched.version,
    tarball,
    tarballSha256: evidence.flow.tarball.sha256,
    packageVersion: evidence.package.version,
    dev: devAttempts,
    devUsedExcludeFallback: devFallback,
    production: {
      base: "/app/",
      wasmAsset: emittedWasm,
      wasmUrl,
      wasmResponses: production.wasmResponses,
      assets: emitted,
    },
  };
  await mkdir(resolve(targetRoot, "evidence"), { recursive: true });
  await writeJson(resolve(targetRoot, "evidence", "vite.json"), report);
  process.stdout.write(
    `vite qualification passed (${launched.label} ${launched.version})` +
      (devFallback ? " with documented optimizeDeps.exclude" : "") +
      "\n",
  );
} finally {
  if (launched !== undefined) await launched.browser.close();
  if (server !== undefined) await server.close();
  await removeConsumer(consumer.directory);
}
