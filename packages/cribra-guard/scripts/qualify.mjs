/**
 * Real Chromium MV3 qualification for the Cribra Guard Flow host.
 *
 * Every request goes through `chrome.runtime.sendMessage` from an extension
 * page. The service worker is inspected through CDP only for lifecycle,
 * heap-usage, and storage-state evidence; scanning never imports worker
 * internals.
 */

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { gzipSync } from "node:zlib";

import { chromium } from "playwright-core";

const packageRoot = resolve(import.meta.dirname, "..");
const distDir = resolve(packageRoot, "dist");
const releaseDistDir = resolve(packageRoot, ".build/release-dist");
const failureRoot = resolve(packageRoot, ".build/failure");
const reportRoot = resolve(packageRoot, ".build/qualification");
const buildScript = resolve(packageRoot, "scripts/build.mjs");
const headed = process.argv.includes("--headed");

const TOKEN = "ghp_AbCdEf0123456789_AbCdEf0123456789";
const UNICODE_PREFIX = "identité 😀 日本語 ";
const UNICODE_SUFFIX = " fin";
const UNICODE_SOURCE = `${UNICODE_PREFIX}${TOKEN}${UNICODE_SUFFIX}`;
const ASCII_SOURCE = `token=${TOKEN}`;
const REDACTED_UNICODE_SOURCE = `${UNICODE_PREFIX}[REDACTED]${UNICODE_SUFFIX}`;
const TEMPLATE_UNICODE_SOURCE = `${UNICODE_PREFIX}<CRIBRA:github.classic-pat:1>${UNICODE_SUFFIX}`;

const checks = [];
const failures = [];

function delay(milliseconds) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, milliseconds));
}

function check(name, condition, details) {
  const passed = Boolean(condition);
  checks.push({
    name,
    pass: passed,
    ...(details === undefined ? {} : { details }),
  });
  process.stdout.write(`${passed ? "PASS" : "FAIL"} ${name}\n`);
  if (!passed) failures.push(name);
  return passed;
}

function must(name, condition, details) {
  if (!check(name, condition, details)) {
    throw new Error(`qualification check failed: ${name}`);
  }
}

function summarize(values) {
  const sorted = [...values].sort((left, right) => left - right);
  if (sorted.length === 0) return null;
  const at = (fraction) =>
    sorted[Math.min(sorted.length - 1, Math.ceil(fraction * sorted.length) - 1)];
  return {
    count: sorted.length,
    min: Number(sorted[0].toFixed(3)),
    p50: Number(at(0.5).toFixed(3)),
    p95: Number(at(0.95).toFixed(3)),
    max: Number(sorted[sorted.length - 1].toFixed(3)),
    mean: Number(
      (sorted.reduce((total, value) => total + value, 0) / sorted.length).toFixed(
        3,
      ),
    ),
  };
}

function extensionIdFor(path) {
  return createHash("sha256")
    .update(path)
    .digest("hex")
    .slice(0, 32)
    .replace(/[0-9a-f]/g, (character) =>
      String.fromCharCode(97 + Number.parseInt(character, 16)),
    );
}

async function fileSha256(path) {
  return createHash("sha256")
    .update(await readFile(path))
    .digest("hex");
}

/**
 * Reads the initial linear-memory size declared by the WASM memory section.
 * This is static module metadata, not a runtime allocation measurement.
 */
function wasmMemoryInitialBytes(bytes) {
  let offset = 8; // magic + version
  const readUleb = () => {
    let value = 0;
    let shift = 0;
    for (;;) {
      const byte = bytes[offset];
      offset += 1;
      value += (byte & 0x7f) * 2 ** shift;
      if ((byte & 0x80) === 0) return value;
      shift += 7;
    }
  };
  while (offset < bytes.length) {
    const sectionId = bytes[offset];
    offset += 1;
    const sectionSize = readUleb();
    const sectionEnd = offset + sectionSize;
    if (sectionId === 5) {
      const count = readUleb();
      if (count !== 1) return null;
      const flags = readUleb();
      const minimum = readUleb();
      if ((flags & 0x01) !== 0) readUleb(); // declared maximum
      return minimum * 65_536;
    }
    offset = sectionEnd;
  }
  return null;
}

async function sumFiles(root) {
  let bytes = 0;
  for (const entry of await readdir(root, {
    recursive: true,
    withFileTypes: true,
  })) {
    if (!entry.isFile()) continue;
    const path = join(entry.parentPath, entry.name);
    bytes += (await stat(path)).size;
  }
  return bytes;
}

async function walkFiles(root, limit = 4000) {
  if (!existsSync(root)) return [];
  const files = [];
  for (const entry of await readdir(root, {
    recursive: true,
    withFileTypes: true,
  })) {
    if (!entry.isFile()) continue;
    files.push(join(entry.parentPath, entry.name));
    if (files.length >= limit) break;
  }
  return files;
}

async function findSentinel(root, sentinel) {
  const needle = Buffer.from(sentinel, "utf8");
  const matches = [];
  for (const file of await walkFiles(root)) {
    let contents;
    try {
      contents = await readFile(file);
    } catch {
      continue;
    }
    if (contents.includes(needle)) matches.push(file);
  }
  return matches;
}

async function openHarness(page, extensionId) {
  let lastError;
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      await page.goto(
        `chrome-extension://${extensionId}/qualification.html`,
        { waitUntil: "domcontentloaded", timeout: 5_000 },
      );
      lastError = undefined;
      break;
    } catch (error) {
      lastError = error;
      await delay(250);
    }
  }
  if (lastError !== undefined) {
    throw new Error(`extension page did not open: ${lastError.message}`);
  }
  await page.waitForFunction(
    () => typeof window.cribraQualification === "object",
    undefined,
    { timeout: 15_000 },
  );
}

async function waitForWorker(context, extensionId, timeout = 15_000) {
  const expected = `chrome-extension://${extensionId}/service-worker.js`;
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const known = context
      .serviceWorkers()
      .find((worker) => worker.url() === expected);
    if (known !== undefined) return known;
    try {
      return await context.waitForEvent("serviceworker", {
        timeout: Math.min(500, deadline - Date.now()),
        predicate: (worker) => worker.url() === expected,
      });
    } catch {
      // Poll again until the deadline.
    }
  }
  return undefined;
}

async function sendMessage(page, message) {
  return page.evaluate(
    (value) => window.cribraQualification.send(value),
    message,
  );
}

async function sendWithRetry(page, message, attempts = 80) {
  let lastError;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      return await sendMessage(page, message);
    } catch (error) {
      lastError = error;
      if (
        !/Receiving end does not exist|Could not establish connection|message port closed/i.test(
          error.message,
        )
      ) {
        throw error;
      }
      await delay(250);
    }
  }
  throw lastError;
}

async function sendAndForget(page, message) {
  await page.evaluate(
    (value) => window.cribraQualification.sendAndForget(value),
    message,
  );
}

async function attachWorkerCdp(context, extensionId) {
  const browser = context.browser();
  if (browser === null) throw new Error("persistent context has no browser");
  const browserSession = await browser.newBrowserCDPSession();
  const targets = await browserSession.send("Target.getTargets");
  const target = targets.targetInfos.find(
    (candidate) =>
      candidate.type === "service_worker" &&
      candidate.url ===
        `chrome-extension://${extensionId}/service-worker.js`,
  );
  if (target === undefined) {
    await browserSession.detach().catch(() => {});
    throw new Error("service worker target is not available");
  }

  const { sessionId } = await browserSession.send("Target.attachToTarget", {
    targetId: target.targetId,
    flatten: false,
  });
  let nextId = 1;
  const pending = new Map();
  const events = [];

  browserSession.on(
    "Target.receivedMessageFromTarget",
    ({ sessionId: incoming, message }) => {
      if (incoming !== sessionId) return;
      let parsed;
      try {
        parsed = JSON.parse(message);
      } catch {
        return;
      }
      if (parsed.id !== undefined && pending.has(parsed.id)) {
        const entry = pending.get(parsed.id);
        pending.delete(parsed.id);
        clearTimeout(entry.timer);
        if (parsed.error !== undefined) {
          entry.reject(
            new Error(`${parsed.error.message} (${parsed.error.code})`),
          );
        } else {
          entry.resolve(parsed.result);
        }
        return;
      }
      events.push(parsed);
    },
  );

  function call(method, params = {}, timeout = 15_000) {
    const id = nextId;
    nextId += 1;
    return new Promise((resolvePromise, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`CDP timeout: ${method}`));
      }, timeout);
      pending.set(id, { resolve: resolvePromise, reject, timer });
      browserSession
        .send("Target.sendMessageToTarget", {
          sessionId,
          message: JSON.stringify({ id, method, params }),
        })
        .catch((error) => {
          pending.delete(id);
          clearTimeout(timer);
          reject(error);
        });
    });
  }

  async function detach() {
    try {
      await browserSession.send("Target.detachFromTarget", { sessionId });
    } catch {
      // The target may already be gone.
    }
    try {
      await browserSession.detach();
    } catch {
      // Already detached.
    }
  }

  return { call, events, detach };
}

async function evalInWorker(cdp, expression) {
  const response = await cdp.call("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails !== undefined) {
    throw new Error(
      response.exceptionDetails.exception?.description ??
        response.exceptionDetails.text,
    );
  }
  return response.result?.value;
}

async function heapUsage(cdp) {
  await cdp.call("HeapProfiler.enable");
  await cdp.call("HeapProfiler.collectGarbage");
  return cdp.call("Runtime.getHeapUsage");
}

async function launchExtension(extensionPath, label) {
  const profileDir = await mkdtemp(
    resolve(tmpdir(), `cribra-guard-${label}-`),
  );
  const launchStartedAt = performance.now();
  const context = await chromium.launchPersistentContext(profileDir, {
    headless: !headed,
    channel: "chromium",
    args: [
      `--disable-extensions-except=${extensionPath}`,
      `--load-extension=${extensionPath}`,
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-features=DisableLoadExtensionCommandLineSwitch",
    ],
  });

  const extensionId = extensionIdFor(extensionPath);
  const consoleMessages = [];
  const requestUrls = [];
  context.on("request", (request) => {
    requestUrls.push(request.url());
  });

  const page = await context.newPage();
  page.on("console", (message) => {
    if (message.type() === "error") consoleMessages.push(message.text());
  });
  page.on("pageerror", (error) => {
    consoleMessages.push(error.message);
  });
  await openHarness(page, extensionId);

  const firstResponse = await sendWithRetry(page, {
    type: "cribra.capabilities",
    requestId: "cold-capabilities",
  });
  const firstResponseAt = performance.now();
  const worker = await waitForWorker(context, extensionId);
  if (worker === undefined) {
    await context.close();
    await rm(profileDir, { recursive: true, force: true });
    throw new Error(
      "the MV3 service worker never started; extension context unavailable",
    );
  }

  const pageCdp = await context.newCDPSession(page);
  return {
    label,
    extensionPath,
    extensionId,
    profileDir,
    context,
    page,
    worker,
    firstResponse,
    coldStartMs: firstResponseAt - launchStartedAt,
    consoleMessages,
    requestUrls,
    pageCdp,
    lifecycle: await createLifecycle(pageCdp, extensionId),
    cdp: await attachWorkerCdp(context, extensionId),
  };
}

async function closeExtension(state) {
  try {
    await state.cdp?.detach();
  } catch {
    // Ignore teardown failures.
  }
  try {
    await state.pageCdp?.detach();
  } catch {
    // Ignore teardown failures.
  }
  try {
    await state.context.close();
  } catch {
    // Ignore teardown failures.
  }
  await rm(state.profileDir, { recursive: true, force: true }).catch(() => {});
}

async function createLifecycle(pageCdp, extensionId) {
  const expected = `chrome-extension://${extensionId}/service-worker.js`;
  const versions = new Map();
  pageCdp.on(
    "ServiceWorker.workerVersionUpdated",
    ({ versions: update }) => {
      for (const version of update) {
        versions.set(version.versionId, version);
      }
    },
  );
  await pageCdp.send("ServiceWorker.enable");
  await delay(300);

  async function waitFor(predicate, timeout = 15_000) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      for (const version of versions.values()) {
        if (predicate(version)) return version;
      }
      await delay(100);
    }
    return undefined;
  }

  return {
    async stop() {
      const running = await waitFor(
        (version) =>
          version.scriptURL === expected &&
          version.runningStatus === "running",
      );
      if (running === undefined) {
        throw new Error("no running service worker version to stop");
      }
      const startedAt = performance.now();
      await pageCdp.send("ServiceWorker.stopWorker", {
        versionId: running.versionId,
      });
      const stopped = await waitFor(
        (version) =>
          version.versionId === running.versionId &&
          version.runningStatus === "stopped",
      );
      if (stopped === undefined) {
        throw new Error("service worker did not report a stopped state");
      }
      return performance.now() - startedAt;
    },
  };
}

async function stopServiceWorker(state) {
  return state.lifecycle.stop();
}

async function buildFailureVariant(name, wasmBytes) {
  const artifactDir = resolve(failureRoot, `${name}-artifact`);
  const outDir = resolve(failureRoot, `${name}-dist`);
  const wasmPath = resolve(failureRoot, `${name}.wasm`);
  await mkdir(failureRoot, { recursive: true });
  await writeFile(wasmPath, wasmBytes);
  execFileSync(
    process.execPath,
    [
      buildScript,
      "--skip-build",
      "--with-qualification",
      "--artifact-dir",
      artifactDir,
      "--wasm",
      wasmPath,
      "--glue",
      resolve(packageRoot, ".build/flow/cribra.js"),
      "--out",
      outDir,
      "--allow-unverified",
    ],
    { cwd: packageRoot, stdio: "inherit" },
  );
  return outDir;
}

async function qualifyFailureVariant(
  name,
  extensionPath,
  expectedCode,
  expectedMessage,
) {
  let state;
  try {
    state = await launchExtension(extensionPath, `failure-${name}`);
    const first = state.firstResponse;
    check(
      `${name}: initialization fails closed`,
      first.ok === false,
      { code: first.ok === false ? first.error.code : "ok" },
    );
    check(
      `${name}: expected structured code`,
      first.ok === false && first.error.code === expectedCode,
      { expectedCode, actual: first.ok === false ? first.error.code : "ok" },
    );
    if (first.ok === false) {
      check(
        `${name}: error uses a fixed source-free message`,
        first.error.message === expectedMessage &&
          !("stack" in first.error) &&
          !/RuntimeError|TypeError|ReferenceError|into the file|wasm_bindgen|at https?:|file:\/\/|WebAssembly\.(?:compile|instantiate)/i.test(
            first.error.message,
          ),
        { message: first.error.message, expectedMessage },
      );
      check(
        `${name}: error exposes only fixed fields`,
        Object.keys(first.error).every((key) =>
          ["code", "domain", "message", "ruleId"].includes(key),
        ),
        { keys: Object.keys(first.error) },
      );
    }
    const second = await sendWithRetry(state.page, {
      type: "cribra.capabilities",
      requestId: `${name}-second`,
    });
    check(
      `${name}: repeated request stays failed and responsive`,
      second.ok === false && second.error.code === expectedCode,
      { code: second.ok === false ? second.error.code : "ok" },
    );
    check(
      `${name}: worker console has no CSP or compile crash`,
      !state.consoleMessages.some((message) =>
        /Content Security Policy|Refused to compile|WebAssembly\.compile/i.test(
          message,
        ),
      ),
      { messages: state.consoleMessages },
    );
  } finally {
    if (state !== undefined) await closeExtension(state);
  }
}

async function main() {
  await mkdir(reportRoot, { recursive: true });

  const artifact = JSON.parse(
    await readFile(resolve(distDir, "artifact.json"), "utf8"),
  );
  const lock = JSON.parse(
    await readFile(resolve(packageRoot, "artifact.lock.json"), "utf8"),
  );
  const lockedArtifact =
    lock.artifacts?.[`${process.platform}-${process.arch}`];
  if (lockedArtifact === undefined) {
    throw new Error(
      `artifact.lock.json has no entry for ${process.platform}-${process.arch}`,
    );
  }
  const manifest = JSON.parse(
    await readFile(resolve(distDir, "manifest.json"), "utf8"),
  );
  if (
    !existsSync(resolve(distDir, "qualification.html")) ||
    !existsSync(resolve(distDir, "qualification.js"))
  ) {
    throw new Error(
      "qualification harness is missing; run `npm run build:qualification` before `npm run qualify`",
    );
  }
  const serviceWorkerSource = await readFile(
    resolve(distDir, "service-worker.js"),
    "utf8",
  );
  const packagedWasm = await readFile(
    resolve(distDir, "wasm/cribra_bg.wasm"),
  );
  const wasmSha256 = createHash("sha256").update(packagedWasm).digest("hex");

  must(
    "artifact lock matches packaged Flow WASM",
    lockedArtifact.productionWasm.sha256 === wasmSha256 &&
      lockedArtifact.productionWasm.bytes === packagedWasm.byteLength &&
      artifact.artifact.sha256 === wasmSha256 &&
      artifact.artifact.bytes === packagedWasm.byteLength,
    {
      lock: lockedArtifact.productionWasm,
      artifact: artifact.artifact,
      measured: { bytes: packagedWasm.byteLength, sha256: wasmSha256 },
    },
  );
  must(
    "artifact lock pins the expected toolchain",
    lock.wasmBindgenVersion === artifact.toolchain.wasmBindgen &&
      lock.wasmOptVersion === artifact.toolchain.wasmOpt,
    {
      bindgen: artifact.toolchain.wasmBindgen,
      wasmOpt: artifact.toolchain.wasmOpt,
    },
  );

  const report = {
    startedAt: new Date().toISOString(),
    environment: {
      platform: process.platform,
      arch: process.arch,
      node: process.version,
      playwright: "1.64.1",
      headed,
    },
    artifact,
    manifest,
    checks,
    measurements: {},
    failureInjection: {},
    knownLimitations: [],
  };

  const state = await launchExtension(distDir, "main");
  let mainCdp = state.cdp;
  try {
    report.environment.browser = state.context.browser()?.version();
    report.environment.chromeExecutable = chromium.executablePath();
    report.environment.extensionId = state.extensionId;
    report.environment.coldStartToFirstResponseMs = Number(
      state.coldStartMs.toFixed(3),
    );

    const first = state.firstResponse;
    must("extension returns a capabilities response", first.ok === true, first);
    must(
      "Flow WASM initializes in the MV3 worker",
      first.capabilities.backend === "wasm" &&
        first.capabilities.profile === "flow" &&
        first.init.artifactVerified === true &&
        first.init.cold === true,
      {
        backend: first.capabilities.backend,
        profile: first.capabilities.profile,
        verified: first.init.artifactVerified,
        cold: first.init.cold,
      },
    );
    must(
      "runtime loaded the pinned production artifact",
      first.init.wasmSha256 === lockedArtifact.productionWasm.sha256 &&
        first.init.wasmBytes === lockedArtifact.productionWasm.bytes,
      {
        sha256: first.init.wasmSha256,
        bytes: first.init.wasmBytes,
      },
    );
    must(
      "capabilities identify the Flow feature set",
      first.capabilities.singleSourceScan === true &&
        first.capabilities.batchScan === true &&
        first.capabilities.streaming === true &&
        first.capabilities.incremental === true &&
        first.capabilities.redaction === true &&
        first.capabilities.templating === true &&
        first.capabilities.pseudonymization === true &&
        first.capabilities.synthesis === true &&
        first.capabilities.shareBundle === true &&
        first.capabilities.ruleMetadata === true &&
        first.capabilities.explanations === true &&
        first.capabilities.parallel === false &&
        first.capabilities.utf16OffsetConversion === true,
      first.capabilities,
    );
    must(
      "scanner constructed with the built-in catalog",
      first.scanner.rulesCount >= 90 && first.scanner.disposed === false,
      first.scanner,
    );

    const runtimeInfo = await evalInWorker(
      mainCdp,
      `(() => {
        const manifest = chrome.runtime.getManifest();
        return {
          manifestVersion: manifest.manifest_version,
          background: manifest.background,
          permissions: manifest.permissions ?? [],
          hostPermissions: manifest.host_permissions ?? [],
          contentScripts: manifest.content_scripts ?? [],
          webAccessibleResources: manifest.web_accessible_resources ?? [],
          extensionPagesCsp:
            manifest.content_security_policy?.extension_pages ?? null,
          sandbox: manifest.content_security_policy?.sandbox ?? null,
          workerOrigin: self.location.origin,
          workerHref: self.location.href,
          hasChromeStorage: typeof chrome.storage !== "undefined",
          hasIndexedDb: typeof indexedDB !== "undefined",
          hasCaches: typeof caches !== "undefined",
          crossOriginIsolated: self.crossOriginIsolated,
        };
      })()`,
    );
    must(
      "manifest is MV3 with a module service worker",
      runtimeInfo.manifestVersion === 3 &&
        runtimeInfo.background?.service_worker === "service-worker.js" &&
        runtimeInfo.background?.type === "module",
      runtimeInfo.background,
    );
    must(
      "permissions are empty",
      runtimeInfo.permissions.length === 0 &&
        runtimeInfo.hostPermissions.length === 0,
      {
        permissions: runtimeInfo.permissions,
        hostPermissions: runtimeInfo.hostPermissions,
      },
    );
    must(
      "no content scripts or web-accessible resources",
      runtimeInfo.contentScripts.length === 0 &&
        runtimeInfo.webAccessibleResources.length === 0,
    );
    must(
      "extension CSP permits local WASM compilation only",
      typeof runtimeInfo.extensionPagesCsp === "string" &&
        runtimeInfo.extensionPagesCsp.includes("'wasm-unsafe-eval'") &&
        runtimeInfo.extensionPagesCsp.includes("script-src 'self'") &&
        runtimeInfo.extensionPagesCsp.includes("object-src 'self'"),
      { csp: runtimeInfo.extensionPagesCsp },
    );
    check(
      "worker runs under the extension origin",
      runtimeInfo.workerOrigin === `chrome-extension://${state.extensionId}`,
      { origin: runtimeInfo.workerOrigin },
    );
    check(
      "extension console has no CSP or compile refusal",
      !state.consoleMessages.some((message) =>
        /Content Security Policy|Refused to compile|WebAssembly\.compile/i.test(
          message,
        ),
      ),
      { messages: state.consoleMessages },
    );

    report.measurements.cold = {
      launchToFirstResponseMs: report.environment.coldStartToFirstResponseMs,
      workerInitMs: first.init.durationMs,
      workerStartedAt: first.init.workerStartedAt,
      initializedAt: first.init.initializedAt,
      scannerInitMs: first.scanner.initMs,
      firstScanMs: null,
      firstTransformMs: null,
    };
    report.measurements.csp = {
      extensionPagesCsp: runtimeInfo.extensionPagesCsp,
      workerOrigin: runtimeInfo.workerOrigin,
      workerHref: runtimeInfo.workerHref,
      artifactUrl: `chrome-extension://${state.extensionId}/wasm/cribra_bg.wasm`,
      offlineColdInitVerified: null,
    };

    // ---- Scan, redaction, template, rescan -------------------------------
    const scan = await sendWithRetry(state.page, {
      type: "cribra.scan",
      requestId: "unicode-scan",
      source: UNICODE_SOURCE,
      includeExplanations: true,
    });
    must("synthetic scan succeeds", scan.ok === true, scan);
    report.measurements.cold.firstScanMs = scan.timing.durationMs;
    must(
      "synthetic scan reports the GitHub classic PAT rule",
      scan.scan.findingCount === 1 &&
        scan.scan.findings[0].ruleId === "github.classic-pat" &&
        scan.scan.findings[0].severity === "critical" &&
        scan.scan.findings[0].confidence === "high" &&
        scan.scan.hasCritical === true &&
        scan.scan.needsReview === true,
      scan.scan.findings[0],
    );
    must(
      "scan metadata uses UTF-8 byte length",
      scan.scan.sourceBytes === Buffer.byteLength(UNICODE_SOURCE, "utf8"),
      {
        sourceBytes: scan.scan.sourceBytes,
        expected: Buffer.byteLength(UNICODE_SOURCE, "utf8"),
      },
    );
    const finding = scan.scan.findings[0];
    must(
      "UTF-8 spans differ from UTF-16 indices",
      finding.span.start !== finding.textRange.start &&
        UNICODE_SOURCE.slice(
          finding.textRange.start,
          finding.textRange.end,
        ) === TOKEN,
      {
        span: finding.span,
        textRange: finding.textRange,
        sliceMatchesToken:
          UNICODE_SOURCE.slice(
            finding.textRange.start,
            finding.textRange.end,
          ) === TOKEN,
      },
    );
    check(
      "scan response never echoes the matched value",
      !JSON.stringify(scan).includes(TOKEN),
    );
    check(
      "finding carries no matched-value field",
      !("value" in finding) && !("match" in finding) && !("source" in scan),
    );

    const asciiScan = await sendWithRetry(state.page, {
      type: "cribra.scan",
      requestId: "ascii-scan",
      source: ASCII_SOURCE,
    });
    must("ASCII scan succeeds", asciiScan.ok === true, asciiScan);
    check(
      "ASCII spans and UTF-16 indices coincide",
      asciiScan.scan.findings[0].span.start ===
        asciiScan.scan.findings[0].textRange.start &&
        ASCII_SOURCE.slice(
          asciiScan.scan.findings[0].textRange.start,
          asciiScan.scan.findings[0].textRange.end,
        ) === TOKEN,
      asciiScan.scan.findings[0],
    );

    const redact = await sendWithRetry(state.page, {
      type: "cribra.transform",
      requestId: "unicode-redact",
      source: UNICODE_SOURCE,
      transform: { kind: "redact" },
    });
    must("redaction succeeds", redact.ok === true, redact);
    report.measurements.cold.firstTransformMs = redact.timing.durationMs;
    must(
      "redaction rewrites only the finding and preserves Unicode",
      redact.transform.output === REDACTED_UNICODE_SOURCE &&
        redact.transform.findingCount === 1 &&
        !redact.transform.output.includes(TOKEN),
      { output: redact.transform.output },
    );
    check(
      "redaction response never echoes the matched value",
      !JSON.stringify(redact).includes(TOKEN),
    );

    const redactedRescan = await sendWithRetry(state.page, {
      type: "cribra.scan",
      requestId: "redacted-rescan",
      source: redact.transform.output,
    });
    must(
      "redacted result rescans clean",
      redactedRescan.ok === true &&
        redactedRescan.scan.findingCount === 0 &&
        redactedRescan.scan.needsReview === false,
      redactedRescan.ok ? redactedRescan.scan : redactedRescan,
    );

    const template = await sendWithRetry(state.page, {
      type: "cribra.transform",
      requestId: "unicode-template",
      source: UNICODE_SOURCE,
      transform: { kind: "template", numbered: true },
    });
    must("template transform succeeds", template.ok === true, template);
    must(
      "template transform emits a stable placeholder",
      template.transform.output === TEMPLATE_UNICODE_SOURCE &&
        template.transform.findingCount === 1,
      { output: template.transform.output },
    );
    const templateRescan = await sendWithRetry(state.page, {
      type: "cribra.scan",
      requestId: "template-rescan",
      source: template.transform.output,
    });
    must(
      "templated result rescans clean",
      templateRescan.ok === true && templateRescan.scan.findingCount === 0,
      templateRescan.ok ? templateRescan.scan : templateRescan,
    );

    // ---- Offsets and sanitized failures ---------------------------------
    const surrogateSource = `\ud800 ${TOKEN}`;
    const surrogate = await sendWithRetry(state.page, {
      type: "cribra.scan",
      requestId: "surrogate",
      source: surrogateSource,
    });
    must(
      "unpaired surrogate fails safely during offset projection",
      surrogate.ok === false &&
        surrogate.error.code === "invalid_unicode_string" &&
        surrogate.error.domain === "offset",
      surrogate.ok === false ? surrogate.error : "ok",
    );
    check(
      "offset failure does not expose source or matched value",
      !JSON.stringify(surrogate).includes(TOKEN) &&
        !JSON.stringify(surrogate).includes("\\ud800"),
    );

    const malformed = await sendWithRetry(state.page, {
      type: "cribra.capabilities",
      requestId: "x".repeat(65),
    }).catch((error) => ({ transportError: error.message }));
    check(
      "oversized request id is rejected",
      malformed.ok === false && malformed.error.code === "invalid_message",
      malformed,
    );

    const oversized = await sendWithRetry(state.page, {
      type: "cribra.scan",
      requestId: "oversized",
      source: "a".repeat(1_000_001),
    });
    check(
      "oversized source is rejected before scanning",
      oversized.ok === false && oversized.error.code === "message_too_large",
      oversized.ok === false ? oversized.error : "ok",
    );

    const unsupportedTransform = await sendWithRetry(state.page, {
      type: "cribra.transform",
      requestId: "unsupported-transform",
      source: TOKEN,
      transform: { kind: "pseudonymize", key: [1, 2, 3] },
    });
    check(
      "unsupported transform is rejected",
      unsupportedTransform.ok === false &&
        unsupportedTransform.error.code === "unsupported_transform",
      unsupportedTransform.ok === false
        ? unsupportedTransform.error
        : "ok",
    );

    const malformedScan = await sendWithRetry(state.page, {
      type: "cribra.scan",
      requestId: "malformed-scan",
      source: 42,
    });
    check(
      "non-string source is rejected",
      malformedScan.ok === false &&
        malformedScan.error.code === "invalid_message",
      malformedScan.ok === false ? malformedScan.error : "ok",
    );

    check(
      "error responses omit stack traces",
      [malformed, oversized, unsupportedTransform, malformedScan].every(
        (response) =>
          response.ok === false &&
          !("stack" in response.error) &&
          Object.keys(response.error).every((key) =>
            ["code", "domain", "message", "ruleId"].includes(key),
          ),
      ),
    );

    // ---- Concurrency -----------------------------------------------------
    const concurrentMessages = Array.from({ length: 16 }, (_, index) => {
      const source = `case-${index} ${UNICODE_PREFIX}${TOKEN}${UNICODE_SUFFIX}`;
      return index % 2 === 0
        ? {
            type: "cribra.scan",
            requestId: `concurrent-scan-${index}`,
            source,
            includeExplanations: true,
          }
        : {
            type: "cribra.transform",
            requestId: `concurrent-transform-${index}`,
            source,
            transform: { kind: "redact" },
          };
    });
    const concurrent = await state.page.evaluate(
      (messages) =>
        Promise.all(
          messages.map((message) =>
            window.cribraQualification.send(message),
          ),
        ),
      concurrentMessages,
    );
    check(
      "concurrent requests all succeed with correlated ids",
      concurrent.every(
        (response, index) =>
          response.ok === true &&
          response.requestId === concurrentMessages[index].requestId,
      ),
      {
        count: concurrent.length,
        failures: concurrent
          .map((response, index) =>
            response.ok ? null : { index, code: response.error.code },
          )
          .filter(Boolean),
      },
    );
    check(
      "concurrent results do not cross-contaminate",
      concurrent.every((response, index) => {
        if (!response.ok) return false;
        const result = response.scan ?? response.transform;
        const expectedPrefix = `case-${index} ${UNICODE_PREFIX}`;
        if (index % 2 === 0) {
          const finding = result.findings[0];
          return (
            result.findingCount === 1 &&
            result.sourceBytes ===
              Buffer.byteLength(concurrentMessages[index].source, "utf8") &&
            finding.span.start === Buffer.byteLength(expectedPrefix, "utf8") &&
            finding.textRange.start === expectedPrefix.length
          );
        }
        return (
          result.output ===
          `${expectedPrefix}[REDACTED]${UNICODE_SUFFIX}`
        );
      }),
    );

    // ---- Warm latency and memory ----------------------------------------
    const warmScans = await state.page.evaluate(async (source) => {
      const durations = [];
      const startedAt = performance.now();
      for (let index = 0; index < 200; index += 1) {
        const response = await window.cribraQualification.send({
          type: "cribra.scan",
          requestId: `warm-scan-${index}`,
          source,
          includeExplanations: true,
        });
        if (!response.ok) throw new Error(response.error.code);
        durations.push(response.timing.durationMs);
      }
      return {
        durations,
        wallMs: performance.now() - startedAt,
      };
    }, UNICODE_SOURCE);
    const warmTransforms = await state.page.evaluate(async (source) => {
      const durations = [];
      const startedAt = performance.now();
      for (let index = 0; index < 50; index += 1) {
        const response = await window.cribraQualification.send({
          type: "cribra.transform",
          requestId: `warm-transform-${index}`,
          source,
          transform: { kind: "redact" },
        });
        if (!response.ok) throw new Error(response.error.code);
        durations.push(response.timing.durationMs);
      }
      return {
        durations,
        wallMs: performance.now() - startedAt,
      };
    }, UNICODE_SOURCE);

    const memoryBefore = await heapUsage(mainCdp);
    await state.page.evaluate(async (source) => {
      for (let index = 0; index < 300; index += 1) {
        const response = await window.cribraQualification.send({
          type: "cribra.scan",
          requestId: `memory-a-${index}`,
          source,
          includeExplanations: true,
        });
        if (!response.ok) throw new Error(response.error.code);
      }
    }, UNICODE_SOURCE);
    const memoryAfter300 = await heapUsage(mainCdp);
    await state.page.evaluate(async (source) => {
      for (let index = 0; index < 1_200; index += 1) {
        const response = await window.cribraQualification.send({
          type: "cribra.scan",
          requestId: `memory-b-${index}`,
          source,
          includeExplanations: true,
        });
        if (!response.ok) throw new Error(response.error.code);
      }
    }, UNICODE_SOURCE);
    const memoryAfter1500 = await heapUsage(mainCdp);
    const retentionDelta =
      memoryAfter1500.usedSize - memoryAfter300.usedSize;

    report.measurements.warm = {
      scan: summarize(warmScans.durations),
      scanWallMs: Number(warmScans.wallMs.toFixed(3)),
      scanMeanRoundTripMs: Number(
        (warmScans.wallMs / warmScans.durations.length).toFixed(3),
      ),
      transform: summarize(warmTransforms.durations),
      transformWallMs: Number(warmTransforms.wallMs.toFixed(3)),
      transformMeanRoundTripMs: Number(
        (warmTransforms.wallMs / warmTransforms.durations.length).toFixed(3),
      ),
    };
    report.measurements.memory = {
      before: memoryBefore,
      after300: memoryAfter300,
      after1500: memoryAfter1500,
      retentionDeltaBytes: retentionDelta,
    };
    check(
      "repeated scans do not retain sources",
      retentionDelta < 8 * 1024 * 1024,
      { retentionDeltaBytes: retentionDelta },
    );

    // ---- Storage and network evidence -----------------------------------
    const storageState = await evalInWorker(
      mainCdp,
      `(async () => ({
        chromeStorage: typeof chrome.storage,
        localStorage: typeof localStorage,
        sessionStorage: typeof sessionStorage,
        indexedDbDatabases:
          typeof indexedDB === "object" ? await indexedDB.databases() : null,
        cacheKeys: typeof caches === "object" ? await caches.keys() : null,
      }))()`,
    );
    check(
      "no extension storage API is available",
      storageState.chromeStorage === "undefined" &&
        storageState.localStorage === "undefined" &&
        storageState.sessionStorage === "undefined",
      storageState,
    );
    check(
      "worker databases and caches stay empty",
      Array.isArray(storageState.indexedDbDatabases) &&
        storageState.indexedDbDatabases.length === 0 &&
        Array.isArray(storageState.cacheKeys) &&
        storageState.cacheKeys.length === 0,
      storageState,
    );

    const profileCandidates = [
      resolve(
        state.profileDir,
        "Default/Local Extension Settings",
        state.extensionId,
      ),
      resolve(state.profileDir, "Default/IndexedDB"),
      resolve(state.profileDir, "Default/Service Worker/CacheStorage"),
      resolve(state.profileDir, "Default/Session Storage"),
    ];
    const persisted = [];
    for (const root of profileCandidates) {
      persisted.push(...(await findSentinel(root, TOKEN)));
    }
    check(
      "no scanned value appears in extension profile storage",
      persisted.length === 0,
      { filesScanned: profileCandidates, matches: persisted },
    );

    const remoteRequests = state.requestUrls.filter((url) =>
      /^https?:/i.test(url),
    );
    check(
      "no remote HTTP(S) request was observed",
      remoteRequests.length === 0,
      { remoteRequests, requestCount: state.requestUrls.length },
    );
    check(
      "worker bundle contains no remote code or dynamic evaluation",
      !/https?:\/\//i.test(serviceWorkerSource) &&
        !/\beval\s*\(/.test(serviceWorkerSource) &&
        !/new\s+Function\s*\(/.test(serviceWorkerSource) &&
        !/importScripts\s*\(/.test(serviceWorkerSource) &&
        !/chrome\.offscreen/.test(serviceWorkerSource),
    );
    check(
      "manifest carries no remote-code permissions",
      !manifest.permissions?.some((permission) =>
        ["debugger", "webRequestBlocking", "nativeMessaging"].includes(
          permission,
        ),
      ) &&
        !(manifest.host_permissions ?? []).some(
          (permission) =>
            permission === "<all_urls>" ||
            permission.includes("*://*/*") ||
            permission.includes("http://") ||
            permission.includes("https://"),
        ),
    );

    // ---- Worker restart, offline init, and in-flight loss ---------------
    // Playwright's offline emulation must be applied while the worker target
    // still exists; toggling it after ServiceWorker.stopWorker hangs Chrome.
    await state.context.setOffline(true);
    const stopDuration = await stopServiceWorker(state);
    await state.cdp.detach().catch(() => {});
    const offlineRestart = await sendWithRetry(state.page, {
      type: "cribra.capabilities",
      requestId: "offline-restart",
    });
    await state.context.setOffline(false);
    must(
      "worker restarts and reinitializes Flow while offline",
      offlineRestart.ok === true &&
        offlineRestart.init.cold === true &&
        offlineRestart.init.artifactVerified === true &&
        offlineRestart.init.wasmSha256 === lockedArtifact.productionWasm.sha256,
      offlineRestart.ok
        ? {
            cold: offlineRestart.init.cold,
            verified: offlineRestart.init.artifactVerified,
            sha256: offlineRestart.init.wasmSha256,
          }
        : offlineRestart,
    );

    const restartedWorker = await waitForWorker(state.context, state.extensionId);
    must("restarted worker is visible to Playwright", restartedWorker !== undefined);
    const restartedCdp = await attachWorkerCdp(
      state.context,
      state.extensionId,
    );
    const restartedInfo = await evalInWorker(
      restartedCdp,
      `({ startedAt: performance.timeOrigin, initialized: typeof chrome.runtime === "object" })`,
    );
    await restartedCdp.detach();
    report.measurements.restart = {
      stopDurationMs: Number(stopDuration.toFixed(3)),
      workerInitMs: offlineRestart.ok ? offlineRestart.init.durationMs : null,
      scannerInitMs: offlineRestart.ok ? offlineRestart.scanner.initMs : null,
      restartedAt: restartedInfo.startedAt,
      offlineColdInit: offlineRestart.ok,
    };
    report.measurements.csp.offlineColdInitVerified =
      offlineRestart.ok === true;
    mainCdp = await attachWorkerCdp(state.context, state.extensionId);

    const inFlightSource = `in-flight ${"a".repeat(400_000)} ${TOKEN}`;
    await sendAndForget(state.page, {
      type: "cribra.scan",
      requestId: "in-flight-stop",
      source: inFlightSource,
    });
    await stopServiceWorker(state).catch(() => {});
    const afterInFlightStop = await sendWithRetry(state.page, {
      type: "cribra.capabilities",
      requestId: "after-in-flight-stop",
    });
    check(
      "worker survives in-flight termination and answers again",
      afterInFlightStop.ok === true && afterInFlightStop.init.cold === true,
      afterInFlightStop.ok
        ? { cold: afterInFlightStop.init.cold }
        : afterInFlightStop,
    );
    await mainCdp.detach().catch(() => {});
    mainCdp = await attachWorkerCdp(state.context, state.extensionId);

    const closingPage = await state.context.newPage();
    await openHarness(closingPage, state.extensionId);
    await sendAndForget(closingPage, {
      type: "cribra.scan",
      requestId: "disconnect",
      source: `disconnect ${"b".repeat(200_000)} ${TOKEN}`,
    });
    await closingPage.close();
    await delay(500);
    const recoveredPage = await state.context.newPage();
    await openHarness(recoveredPage, state.extensionId);
    const afterDisconnect = await sendWithRetry(recoveredPage, {
      type: "cribra.capabilities",
      requestId: "after-disconnect",
    });
    check(
      "page disconnect does not corrupt worker state",
      afterDisconnect.ok === true && afterDisconnect.scanner.rulesCount >= 90,
      afterDisconnect.ok
        ? { rulesCount: afterDisconnect.scanner.rulesCount }
        : afterDisconnect,
    );
    await recoveredPage.close();

    // ---- Failure injection ----------------------------------------------
    const corrupt = await buildFailureVariant(
      "corrupt",
      Uint8Array.from({ length: 64 }, (_, index) => (index * 37 + 11) & 0xff),
    );
    const unsupported = await buildFailureVariant(
      "unsupported",
      new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]),
    );

    const tamperedDir = resolve(failureRoot, "tampered-dist");
    await rm(tamperedDir, { recursive: true, force: true });
    await cp(distDir, tamperedDir, { recursive: true });
    const tamperedWasmPath = resolve(tamperedDir, "wasm/cribra_bg.wasm");
    const tamperedBytes = Buffer.from(await readFile(tamperedWasmPath));
    tamperedBytes[tamperedBytes.length - 1] ^= 0xff;
    await writeFile(tamperedWasmPath, tamperedBytes);

    const missingDir = resolve(failureRoot, "missing-dist");
    await rm(missingDir, { recursive: true, force: true });
    await cp(distDir, missingDir, { recursive: true });
    await rm(resolve(missingDir, "wasm/cribra_bg.wasm"));

    await qualifyFailureVariant(
      "corrupt",
      corrupt,
      "wasm_instantiate_failed",
      "The Cribra WebAssembly module could not be initialized.",
    );
    await qualifyFailureVariant(
      "unsupported",
      unsupported,
      "wasm_instantiate_failed",
      "The Cribra WebAssembly module could not be initialized.",
    );
    await qualifyFailureVariant(
      "tampered",
      tamperedDir,
      "wasm_load_failed",
      "The Cribra WebAssembly asset could not be loaded.",
    );
    await qualifyFailureVariant(
      "missing",
      missingDir,
      "wasm_load_failed",
      "The Cribra WebAssembly asset could not be loaded.",
    );

    report.failureInjection = {
      corrupt: "wasm_instantiate_failed",
      unsupported: "wasm_instantiate_failed",
      tampered: "wasm_load_failed",
      missing: "wasm_load_failed",
    };

    // ---- Packaging -------------------------------------------------------
    execFileSync(
      process.execPath,
      [buildScript, "--out", releaseDistDir],
      { cwd: packageRoot, stdio: "inherit" },
    );
    const packageBytes = await sumFiles(distDir);
    const releasePackageBytes = await sumFiles(releaseDistDir);
    const harnessBytes =
      (await stat(resolve(distDir, "qualification.html"))).size +
      (await stat(resolve(distDir, "qualification.js"))).size;
    report.measurements.package = {
      releaseUnpackedBytes: releasePackageBytes,
      qualificationUnpackedBytes: packageBytes,
      harnessBytes,
      wasmBytes: packagedWasm.byteLength,
      wasmGzipBytes: gzipSync(packagedWasm, { level: 9 }).byteLength,
      serviceWorkerBytes: Buffer.byteLength(serviceWorkerSource, "utf8"),
      wasmLinearMemoryInitialBytes: wasmMemoryInitialBytes(packagedWasm),
    };
  } finally {
    await mainCdp.detach().catch(() => {});
    await closeExtension(state);
  }

  report.finishedAt = new Date().toISOString();
  report.checks = checks;
  report.failures = failures;
  report.knownLimitations = [
    "Every detected finding or candidate still requires user review; the host does not claim universal pre-send interception.",
    "The protocol returns UTF-8 byte spans plus UTF-16 text ranges; consumers must never use byte spans as JavaScript indices.",
    "A source containing an unpaired UTF-16 surrogate and a finding fails with invalid_unicode_string during offset projection.",
    "Playwright attaches to the MV3 worker, so natural idle suspension is not observed; restart is exercised with ServiceWorker.stopWorker and an offline cold reinitialization.",
    "No content script, page adapter, approval UI, offscreen document, telemetry, or persistence is implemented in this milestone.",
    "The qualification page and message bridge are emitted only by `npm run build:qualification`; the ordinary release build omits them.",
    "CDP Runtime.getHeapUsage reports the worker's V8 heap only; it does not include WASM linear memory, browser engine overhead, or total process memory.",
    "Runtime network capture covers page requests; the offline cold-restart test and static bundle audit provide the remote-code/network evidence for the worker.",
  ];

  const serialized = JSON.stringify(report);
  if (serialized.includes(TOKEN)) {
    throw new Error("qualification report leaked a matched value");
  }
  await writeFile(
    resolve(reportRoot, "report.json"),
    `${JSON.stringify(report, null, 2)}\n`,
    "utf8",
  );
  process.stdout.write(
    `\nqualification report: ${resolve(reportRoot, "report.json")}\n`,
  );
  process.stdout.write(
    `checks: ${checks.filter((entry) => entry.pass).length}/${checks.length} passed\n`,
  );

  if (failures.length > 0) {
    process.stdout.write(`failed checks: ${failures.join(", ")}\n`);
    process.exitCode = 1;
  }
}

await main();
