/**
 * Read-only native Safari probe. Never enables remote automation or changes
 * system security settings; reports `blocked` when the prerequisite is absent.
 */

import { spawn, spawnSync } from "node:child_process";
import { mkdir } from "node:fs/promises";
import { createServer } from "node:net";
import { resolve } from "node:path";

import {
  artifactPaths,
  artifactsRoot,
  packageRoot,
  readJson,
  targetRoot,
  writeJson,
} from "../lib/artifacts.mjs";
import { createConsumer, removeConsumer } from "../lib/consumer.mjs";
import { startServer } from "../lib/server.mjs";
import { flagString, parseArgs } from "../lib/cli.mjs";

const flags = parseArgs(process.argv.slice(2));
const profile = flagString(flags, "profile", "flow");
const artifacts = await artifactPaths();
const tarball = resolve(flagString(flags, "tarball", artifacts.flow));

function probeAutomation() {
  const driver = spawnSync("which", ["safaridriver"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
  if (driver.status !== 0) {
    return { enabled: false, reason: "safaridriver is not installed" };
  }
  const setting = spawnSync(
    "defaults",
    ["read", "com.apple.Safari", "AllowRemoteAutomation"],
    { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
  const value = setting.status === 0 ? setting.stdout.trim() : "";
  if (value !== "1") {
    return {
      enabled: false,
      reason:
        "Safari Develop > Allow Remote Automation is not enabled " +
        `(AllowRemoteAutomation=${value || "unset"}); not modifying system settings`,
      driver: driver.stdout.trim(),
    };
  }
  return { enabled: true, driver: driver.stdout.trim() };
}

function freePort() {
  return new Promise((resolvePromise, rejectPromise) => {
    const server = createServer();
    server.once("error", rejectPromise);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (address === null || typeof address === "string") {
        rejectPromise(new Error("could not allocate a local port"));
        return;
      }
      const { port } = address;
      server.close(() => resolvePromise(port));
    });
  });
}

async function waitForDriver(url, timeout = 15_000) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${url}/status`);
      if (response.ok) return;
      lastError = new Error(`safaridriver /status returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
  }
  throw lastError ?? new Error("safaridriver did not become ready");
}

async function webdriver(base, method, path, body) {
  const response = await fetch(`${base}${path}`, {
    method,
    headers: { "Content-Type": "application/json; charset=utf-8" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let parsed;
  try {
    parsed = text.length === 0 ? {} : JSON.parse(text);
  } catch {
    parsed = { value: text };
  }
  if (!response.ok) {
    throw new Error(
      `WebDriver ${method} ${path} failed (${response.status}): ${text}`,
    );
  }
  if (parsed.value?.error !== undefined) {
    throw new Error(
      `WebDriver ${method} ${path} error: ${parsed.value.error}: ${parsed.value.message}`,
    );
  }
  return parsed.value;
}

async function resultText(base, sessionId) {
  return webdriver(base, "POST", `/session/${sessionId}/execute/sync`, {
    script:
      "return document.getElementById('result')?.textContent ?? 'MISSING';",
    args: [],
  });
}

async function waitForResult(base, sessionId, timeout = 90_000) {
  const deadline = Date.now() + timeout;
  let value = "";
  while (Date.now() < deadline) {
    value = String(await resultText(base, sessionId));
    if (value !== "PENDING" && value !== "MISSING" && value !== "") return value;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 500));
  }
  throw new Error(`Safari did not finish within ${timeout} ms: ${value}`);
}

async function runCase({ name, base, sessionId, url }) {
  await webdriver(base, "POST", `/session/${sessionId}/url`, { url });
  const value = await waitForResult(base, sessionId);
  if (!value.startsWith("PASS")) {
    throw new Error(`${name} failed: ${value}`);
  }
  return { name, url, result: value };
}

async function main() {
  const evidence = await readJson(resolve(artifactsRoot, "evidence.json"));
  const expected = profile === "core" ? evidence.core : evidence.flow;
  const probe = probeAutomation();
  const report = {
    engine: "native-safari",
    label: "Native Safari (WebDriver)",
    profile,
    status: "blocked",
    probe,
    tarball,
    tarballSha256: expected.tarball.sha256,
    packageVersion: evidence.package.version,
    cases: [],
  };

  if (!probe.enabled) {
    report.reason = probe.reason;
    await mkdir(resolve(targetRoot, "evidence"), { recursive: true });
    await writeJson(resolve(targetRoot, "evidence", "safari-native.json"), report);
    process.stdout.write(`native Safari blocked: ${probe.reason}\n`);
    return;
  }

  const consumer = await createConsumer({
    label: `safari-${profile}`,
    tarball,
  });
  let server;
  let driver;
  try {
    server = await startServer({
      mounts: [
        { prefix: "/dist/", directory: resolve(consumer.installed, "dist") },
        {
          prefix: "/test/browser/",
          directory: resolve(packageRoot, "test/browser"),
        },
      ],
      rootRedirect: "/test/browser/index.html",
    });
    const port = await freePort();
    const base = `http://127.0.0.1:${port}`;
    driver = spawn(probe.driver, ["-p", String(port)], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let driverLog = "";
    driver.stdout.on("data", (chunk) => {
      driverLog += chunk;
    });
    driver.stderr.on("data", (chunk) => {
      driverLog += chunk;
    });
    try {
      await waitForDriver(base);
      const created = await webdriver(base, "POST", "/session", {
        capabilities: { alwaysMatch: { browserName: "safari" } },
      });
      const sessionId = created.sessionId ?? created.capabilities?.sessionId;
      if (typeof sessionId !== "string") {
        throw new Error("safaridriver returned no session id");
      }
      report.cases.push(
        await runCase({
          name: "default asset",
          base,
          sessionId,
          url: `${server.url}/test/browser/index.html?${new URLSearchParams({
            profile,
            version: evidence.package.version,
          }).toString()}`,
        }),
      );
      for (const mode of ["url", "bytes"]) {
        report.cases.push(
          await runCase({
            name: `explicit ${mode}`,
            base,
            sessionId,
            url: `${server.url}/test/browser/explicit.html?${new URLSearchParams(
              { profile, version: evidence.package.version, mode },
            ).toString()}`,
          }),
        );
      }
      const version = await webdriver(
        base,
        "GET",
        `/session/${sessionId}/se/browser/version`,
      ).catch(() => undefined);
      report.browserVersion = version ?? "unknown";
      report.status = "passed";
      await webdriver(base, "DELETE", `/session/${sessionId}`);
    } catch (error) {
      report.status = "blocked";
      report.reason = error instanceof Error ? error.message : String(error);
      report.driverLog = driverLog.trim().slice(0, 4000);
    }
  } finally {
    if (driver !== undefined) driver.kill("SIGTERM");
    if (server !== undefined) await server.close();
    await removeConsumer(consumer.directory);
  }

  await mkdir(resolve(targetRoot, "evidence"), { recursive: true });
  await writeJson(resolve(targetRoot, "evidence", "safari-native.json"), report);
  if (report.status === "passed") {
    process.stdout.write(
      `native Safari qualification passed (${report.cases.length} cases)\n`,
    );
  } else {
    process.stdout.write(`native Safari not qualified: ${report.reason}\n`);
  }
}

await main();
