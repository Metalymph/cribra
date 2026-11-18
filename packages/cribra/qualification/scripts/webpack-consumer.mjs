/** Builds the installed package with webpack and runs the production bundle. */

import { mkdir, readFile } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import webpack from "webpack";

import {
  artifactPaths,
  artifactsRoot,
  readJson,
  targetRoot,
  walkFiles,
  writeJson,
} from "../lib/artifacts.mjs";
import { expectPass, launchBrowser } from "../lib/browsers.mjs";
import {
  copyFixture,
  createConsumer,
  removeConsumer,
} from "../lib/consumer.mjs";
import { startServer } from "../lib/server.mjs";
import { flagString, parseArgs } from "../lib/cli.mjs";

const flags = parseArgs(process.argv.slice(2));
const engine = flagString(flags, "engine", "chrome");
const qualificationRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const artifacts = await artifactPaths();
const tarball = resolve(flagString(flags, "tarball", artifacts.flow));
const evidence = await readJson(resolve(artifactsRoot, "evidence.json"));
const consumer = await createConsumer({ label: `webpack-${engine}`, tarball });
let launched;
let server;

function hashed(file, extension) {
  return new RegExp(`[.-][A-Za-z0-9_-]{8,}\\${extension}$`).test(basename(file));
}

try {
  const appDir = resolve(consumer.directory, "app");
  const outDir = resolve(consumer.directory, "dist");
  await mkdir(appDir, { recursive: true });
  await copyFixture(
    resolve(qualificationRoot, "apps/webpack/index.html"),
    resolve(appDir, "index.html"),
  );
  await copyFixture(
    resolve(qualificationRoot, "apps/webpack/main.mjs"),
    resolve(appDir, "main.mjs"),
  );
  await copyFixture(
    resolve(qualificationRoot, "apps/shared/app-suite.mjs"),
    resolve(appDir, "app-suite.mjs"),
  );

  process.env.CRIBRA_APP_DIR = appDir;
  process.env.CRIBRA_OUT_DIR = outDir;
  const configModule = await import(
    `${pathToFileURL(resolve(qualificationRoot, "configs/webpack.config.mjs")).href}?v=${Date.now()}`
  );
  const stats = await new Promise((resolvePromise, rejectPromise) => {
    webpack(configModule.default, (error, result) => {
      if (error !== null && error !== undefined) {
        rejectPromise(error);
        return;
      }
      if (result === undefined) {
        rejectPromise(new Error("webpack returned no stats"));
        return;
      }
      if (result.hasErrors()) {
        rejectPromise(
          new Error(
            `webpack reported errors: ${result
              .toJson({ all: false, errors: true })
              .errors.map((entry) => entry.message)
              .join(" | ")}`,
          ),
        );
        return;
      }
      resolvePromise(result);
    });
  });
  const files = await walkFiles(outDir);
  const wasm = files.filter((file) => file.endsWith(".wasm"));
  if (wasm.length === 0) throw new Error("webpack emitted no WASM asset");
  if (!hashed(wasm[0], ".wasm")) {
    throw new Error(`webpack WASM asset is not content-hashed: ${wasm[0]}`);
  }
  const javascript = files.filter(
    (file) => file.endsWith(".js") && !file.endsWith(".LICENSE.txt"),
  );
  if (javascript.length === 0) throw new Error("webpack emitted no JavaScript");
  for (const file of javascript) {
    if (!hashed(file, ".js")) {
      throw new Error(`webpack JavaScript is not content-hashed: ${file}`);
    }
  }
  const bundleText = (
    await Promise.all(
      javascript.map((file) => readFile(resolve(outDir, file), "utf8")),
    )
  ).join("\n");
  for (const forbidden of ["node:fs", "node:path", "node:url"]) {
    if (bundleText.includes(forbidden)) {
      throw new Error(`webpack production bundle contains ${forbidden}`);
    }
  }
  if (!bundleText.includes("cribra_bg")) {
    throw new Error("webpack production bundle does not reference the WASM asset");
  }
  const indexHtml = await readFile(resolve(outDir, "index.html"), "utf8");
  if (!indexHtml.includes("/app/")) {
    throw new Error("webpack production index does not use the configured base");
  }

  server = await startServer({
    mounts: [{ prefix: "/app/", directory: outDir }],
  });
  launched = await launchBrowser(engine);
  const production = await expectPass({
    browser: launched.browser,
    url: `${server.url}/app/index.html`,
    label: `webpack production ${launched.label}`,
  });
  const loadedWasm = production.wasmResponses.find(
    (response) => response.status === 200,
  );
  if (loadedWasm === undefined) throw new Error("webpack loaded no WASM asset");
  if (!loadedWasm.url.includes("/app/assets/")) {
    throw new Error(`webpack WASM URL is outside hashed assets: ${loadedWasm.url}`);
  }
  if (!String(loadedWasm.contentType ?? "").includes("application/wasm")) {
    throw new Error(
      `webpack WASM response has wrong MIME: ${loadedWasm.contentType}`,
    );
  }
  if (!loadedWasm.url.endsWith(wasm[0])) {
    throw new Error(`webpack loaded ${loadedWasm.url} but emitted ${wasm[0]}`);
  }

  const report = {
    bundler: "webpack",
    bundlerVersion: JSON.parse(
      await readFile(
        resolve(qualificationRoot, "node_modules/webpack/package.json"),
        "utf8",
      ),
    ).version,
    mode: "production",
    engine,
    browser: launched.label,
    browserVersion: launched.version,
    tarball,
    tarballSha256: evidence.flow.tarball.sha256,
    packageVersion: evidence.package.version,
    conditionalExport: "browser",
    assets: { wasm, javascript },
    wasmUrl: loadedWasm.url,
    wasmResponses: production.wasmResponses,
    emittedBundles: stats
      .toJson({ all: false, assets: true })
      .assets.map((asset) => asset.name),
  };
  await mkdir(resolve(targetRoot, "evidence"), { recursive: true });
  await writeJson(resolve(targetRoot, "evidence", "webpack.json"), report);
  process.stdout.write(
    `webpack qualification passed (${launched.label} ${launched.version})\n`,
  );
} finally {
  if (launched !== undefined) await launched.browser.close();
  if (server !== undefined) await server.close();
  await removeConsumer(consumer.directory);
}
