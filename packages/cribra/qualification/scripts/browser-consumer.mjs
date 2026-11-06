/** Runs the installed browser entry in a real Playwright browser. */

import { mkdir, readFile } from "node:fs/promises";
import { release, version } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  artifactsRoot,
  packageRoot,
  readJson,
  targetRoot,
  writeJson,
} from "../lib/artifacts.mjs";
import {
  browserLabel,
  expectPass,
  launchBrowser,
} from "../lib/browsers.mjs";
import { createConsumer, removeConsumer } from "../lib/consumer.mjs";
import { startServer } from "../lib/server.mjs";
import { flagString, parseArgs } from "../lib/cli.mjs";

const qualificationRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "..",
);
const flags = parseArgs(process.argv.slice(2));
const profile = flagString(flags, "profile", "flow");
const engine = flagString(flags, "engine", "chrome");
const defaultTarball =
  profile === "core"
    ? resolve(artifactsRoot, "core/cribra-typescript-0.0.0-dev-core.tgz")
    : resolve(artifactsRoot, "flow/cribra-typescript-0.0.0-dev.tgz");
const tarball = resolve(flagString(flags, "tarball", defaultTarball));
const evidence = await readJson(resolve(artifactsRoot, "evidence.json"));
const expected = profile === "core" ? evidence.core : evidence.flow;
const browserDir = resolve(packageRoot, "test/browser");
const consumer = await createConsumer({
  label: `browser-${engine}-${profile}`,
  tarball,
});

let server;
let launched;
try {
  server = await startServer({
    mounts: [
      { prefix: "/dist/", directory: resolve(consumer.installed, "dist") },
      { prefix: "/test/browser/", directory: browserDir },
    ],
    rootRedirect: "/test/browser/index.html",
  });
  launched = await launchBrowser(engine);
  const query = new URLSearchParams({
    profile,
    version: evidence.package.version,
  });
  const mainUrl = `${server.url}/test/browser/index.html?${query.toString()}`;
  const main = await expectPass({
    browser: launched.browser,
    url: mainUrl,
    label: `${launched.label} ${profile} default asset`,
  });

  const explicit = [];
  for (const mode of ["url", "bytes"]) {
    const explicitQuery = new URLSearchParams({
      profile,
      version: evidence.package.version,
      mode,
    });
    const result = await expectPass({
      browser: launched.browser,
      url: `${server.url}/test/browser/explicit.html?${explicitQuery.toString()}`,
      label: `${launched.label} ${profile} explicit ${mode}`,
      timeout: 90_000,
    });
    explicit.push({ mode, wasmResponses: result.wasmResponses });
  }

  const report = {
    profile,
    engine,
    label: browserLabel(engine),
    browserVersion: launched.version,
    playwrightVersion: JSON.parse(
      await readFile(
        resolve(qualificationRoot, "node_modules/playwright/package.json"),
        "utf8",
      ),
    ).version,
    os: `${process.platform} ${version()} (kernel ${release()})`,
    servedFrom: "installed tarball node_modules/cribra-typescript/dist",
    tarball,
    tarballSha256: expected.tarball.sha256,
    packageVersion: evidence.package.version,
    adapterVersion: evidence.adapterVersion,
    main: { result: main.result, wasmResponses: main.wasmResponses },
    explicit,
  };
  await mkdir(resolve(targetRoot, "evidence"), { recursive: true });
  await writeJson(
    resolve(targetRoot, "evidence", `browser-${engine}-${profile}.json`),
    report,
  );
  process.stdout.write(
    `browser qualification passed (${launched.label} ${launched.version}, ${profile})\n`,
  );
} finally {
  if (launched !== undefined) await launched.browser.close();
  if (server !== undefined) await server.close();
  await removeConsumer(consumer.directory);
}
