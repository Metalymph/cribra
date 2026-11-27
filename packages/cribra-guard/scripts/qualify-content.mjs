/**
 * Real Chromium extension-context qualification for the content layer.
 *
 * Fixtures are served from a local HTTP server bound to 127.0.0.1. The
 * qualification-only manifest grants access only to that origin and registers
 * one content script at document_start. Interception evidence comes from
 * fixture-side counters, page submit handlers, and local endpoint receipts, not
 * merely from `event.defaultPrevented`.
 *
 * The isolated-world qualification hook is reached through CDP; the page
 * cannot see it. All hook results are source-free.
 */

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { readFile, readdir, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";

import { chromium } from "playwright-core";

const packageRoot = resolve(import.meta.dirname, "..");
const contentDistDir = resolve(packageRoot, ".build/content-dist");
const reportRoot = resolve(packageRoot, ".build/content-qualification");
const headed = process.argv.includes("--headed");

const SENTINEL = "CRIBRA_CONTENT_SENTINEL_7f3a9c";
const SENSITIVE_SENTINEL = "CRIBRA_SENSITIVE_SECRET_5d2b";
const TEXTAREA_SOURCE = `identité 😀 日本語\n${SENTINEL}`;
const INPUT_SOURCE = `input 😀 ${SENTINEL}`;
const CONTENTEDITABLE_SOURCE = `content 😀\n日本語 ${SENTINEL}`;
const SAFE_EDITOR_SOURCE = "safe editor source";
/**
 * One UTF-16 code unit beyond the scanner protocol's 1,000,000 limit. The
 * packaging test asserts that the content constant stays aligned with
 * src/protocol.ts.
 */
const OVERSIZED_SOURCE = "a".repeat(1_000_001);

const checks = [];
const failures = [];
const consoleSamples = [];

function delay(milliseconds) {
  return new Promise((resolvePromise) =>
    setTimeout(resolvePromise, milliseconds),
  );
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
    throw new Error(`content qualification check failed: ${name}`);
  }
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

function escapeHtml(value) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function escapeAttribute(value) {
  return escapeHtml(value).replace(/"/g, "&quot;");
}

function pageShell(scenario, formMarkup, config) {
  return `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>${scenario}</title></head>
  <body>
    <iframe name="sink" id="sink" hidden></iframe>
    ${formMarkup}
    <script>window.__fixtureConfig = ${JSON.stringify(config)};</script>
    <script src="/fixtures/fixture-support.js"></script>
  </body>
</html>`;
}

function scenarioPage(scenario) {
  const base = 'id="fixture-form" action="/submitted" method="get" target="sink"';
  const config = { formId: "fixture-form", earlierWindowHandler: false };
  const textarea = (value, required = false) =>
    `<textarea id="source" name="source" data-cribra-editor="source"${
      required ? " required" : ""
    }>${escapeHtml(value)}</textarea>`;

  switch (scenario) {
    case "native-textarea":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="native-form">
          ${textarea(TEXTAREA_SOURCE)}
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        config,
      );
    case "native-input":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="native-form">
          <input id="source" name="source" type="text" data-cribra-editor="source" value="${escapeAttribute(
            INPUT_SOURCE,
          )}" />
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        config,
      );
    case "sensitive":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="native-form">
          ${textarea(SAFE_EDITOR_SOURCE)}
          <input id="password" name="password" type="password" value="${escapeAttribute(
            SENSITIVE_SENTINEL,
          )}" />
          <input id="hidden" name="hidden" type="hidden" value="${escapeAttribute(
            SENSITIVE_SENTINEL,
          )}" />
          <input id="unrelated" name="unrelated" type="text" value="${escapeAttribute(
            SENSITIVE_SENTINEL,
          )}" />
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        config,
      );
    case "contenteditable":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="contenteditable">
          <div id="source" name="source" data-cribra-editor="source" contenteditable="true">${escapeHtml(
            CONTENTEDITABLE_SOURCE,
          )}</div>
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        config,
      );
    case "validation":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="native-form">
          ${textarea("", true)}
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        config,
      );
    case "multiple-buttons":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="native-form">
          ${textarea(TEXTAREA_SOURCE)}
          <button id="save" type="submit" name="action" value="save">Save</button>
          <button id="publish" type="submit" name="action" value="publish">Publish</button>
        </form>`,
        config,
      );
    case "unsupported":
      return pageShell(
        scenario,
        `<form ${base}>
          ${textarea(TEXTAREA_SOURCE)}
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        config,
      );
    case "rich-unsupported":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="native-form">
          <canvas id="source" data-cribra-editor="source" width="320" height="120"></canvas>
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        config,
      );
    case "oversized":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="native-form">
          ${textarea(OVERSIZED_SOURCE)}
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        config,
      );
    case "limitations":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="native-form">
          ${textarea(TEXTAREA_SOURCE)}
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        { ...config, earlierWindowHandler: true },
      );
    case "replacement":
      return pageShell(
        scenario,
        `<form ${base} data-cribra-adapter="native-form">
          ${textarea("replacement source")}
          <button id="submit" type="submit" name="action" value="send">Send</button>
        </form>`,
        config,
      );
    default:
      throw new Error(`unknown fixture scenario: ${scenario}`);
  }
}

async function startFixtureServer() {
  const state = { receipts: [], requests: [] };
  const supportPath = resolve(packageRoot, "scripts/fixtures/fixture-support.js");

  function readBody(request) {
    return new Promise((resolveBody, rejectBody) => {
      let body = "";
      request.on("data", (chunk) => {
        body += chunk;
      });
      request.on("end", () => resolveBody(body));
      request.on("error", rejectBody);
    });
  }

  function respond(response, status, contentType, body) {
    response.writeHead(status, {
      "content-type": contentType,
      "cache-control": "no-store",
    });
    response.end(body);
  }

  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      if (url.pathname === "/fixture") {
        const scenario = url.searchParams.get("scenario") ?? "native-textarea";
        respond(
          response,
          200,
          "text/html; charset=utf-8",
          scenarioPage(scenario),
        );
        return;
      }
      if (url.pathname === "/fixtures/fixture-support.js") {
        respond(
          response,
          200,
          "text/javascript; charset=utf-8",
          await readFile(supportPath, "utf8"),
        );
        return;
      }
      if (url.pathname === "/receipt" && request.method === "POST") {
        const body = await readBody(request);
        try {
          const parsed = JSON.parse(body);
          state.receipts.push({
            kind: typeof parsed.kind === "string" ? parsed.kind : "invalid",
            detail: parsed.detail ?? null,
            at: Date.now(),
          });
        } catch {
          state.receipts.push({ kind: "invalid-receipt", at: Date.now() });
        }
        respond(response, 204, "text/plain", "");
        return;
      }
      if (url.pathname === "/submitted") {
        state.requests.push({
          kind: "submitted",
          method: request.method ?? "GET",
          url: request.url ?? "/submitted",
          at: Date.now(),
        });
        respond(response, 204, "text/plain", "");
        return;
      }
      if (url.pathname === "/direct") {
        state.requests.push({ kind: "direct", at: Date.now() });
        respond(response, 204, "text/plain", "");
        return;
      }
      if (url.pathname === "/autosave") {
        state.requests.push({ kind: "autosave", at: Date.now() });
        respond(response, 204, "text/plain", "");
        return;
      }
      if (url.pathname === "/__state") {
        respond(
          response,
          200,
          "application/json",
          JSON.stringify(state),
        );
        return;
      }
      if (url.pathname === "/__reset") {
        state.receipts.length = 0;
        state.requests.length = 0;
        respond(response, 204, "text/plain", "");
        return;
      }
      respond(response, 404, "text/plain", "not found");
    } catch (error) {
      respond(
        response,
        500,
        "text/plain",
        error instanceof Error ? error.message : "server error",
      );
    }
  });

  await new Promise((resolveListen) =>
    server.listen(0, "127.0.0.1", resolveListen),
  );
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("fixture server did not bind to a TCP port");
  }
  const origin = `http://127.0.0.1:${address.port}`;

  function reset() {
    state.receipts.length = 0;
    state.requests.length = 0;
  }

  function count(kind) {
    return (
      state.receipts.filter((entry) => entry.kind === kind).length +
      state.requests.filter((entry) => entry.kind === kind).length
    );
  }

  function has(kind) {
    return count(kind) > 0;
  }

  async function close() {
    await new Promise((resolveClose) => server.close(resolveClose));
  }

  return {
    origin,
    state,
    reset,
    count,
    has,
    receipts: () => [...state.receipts],
    requests: () => [...state.requests],
    close,
  };
}

async function launchContentExtension() {
  const profileDir = await mkdtemp(resolve(tmpdir(), "cribra-content-"));
  const context = await chromium.launchPersistentContext(profileDir, {
    headless: !headed,
    channel: "chromium",
    args: [
      `--disable-extensions-except=${contentDistDir}`,
      `--load-extension=${contentDistDir}`,
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-features=DisableLoadExtensionCommandLineSwitch",
    ],
  });
  const requestUrls = [];
  context.on("request", (request) => requestUrls.push(request.url()));
  return { context, profileDir, requestUrls };
}

async function createContentCdp(context, page) {
  const session = await context.newCDPSession(page);
  const contexts = new Map();
  const consoleMessages = [];
  const exceptions = [];
  let cachedContextId;

  session.on("Runtime.executionContextCreated", ({ context: created }) => {
    contexts.set(created.id, created);
  });
  session.on(
    "Runtime.executionContextDestroyed",
    ({ executionContextId }) => {
      contexts.delete(executionContextId);
      if (cachedContextId === executionContextId) cachedContextId = undefined;
    },
  );
  session.on("Runtime.executionContextsCleared", () => {
    contexts.clear();
    cachedContextId = undefined;
  });
  session.on("Runtime.consoleAPICalled", ({ args, type }) => {
    const text = args
      .map((argument) => argument.value ?? argument.description ?? argument.type)
      .join(" ");
    consoleMessages.push({ type, text });
  });
  session.on("Runtime.exceptionThrown", ({ exceptionDetails }) => {
    exceptions.push(
      exceptionDetails.exception?.description ?? exceptionDetails.text ?? "",
    );
  });

  await session.send("Runtime.enable");

  async function evaluate(expression, contextId) {
    const response = await session.send("Runtime.evaluate", {
      expression,
      contextId,
      awaitPromise: true,
      returnByValue: true,
    });
    if (response.exceptionDetails !== undefined) {
      throw new Error(
        response.exceptionDetails.exception?.description ??
          response.exceptionDetails.text ??
          "content evaluation failed",
      );
    }
    return response.result?.value;
  }

  async function findContext(timeout = 12_000) {
    if (cachedContextId !== undefined && contexts.has(cachedContextId)) {
      return cachedContextId;
    }
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      for (const candidate of contexts.values()) {
        try {
          const value = await evaluate(
            "typeof globalThis.__cribraGuardQualification",
            candidate.id,
          );
          if (value === "object") {
            cachedContextId = candidate.id;
            return candidate.id;
          }
        } catch {
          // The context may have been destroyed between enumeration and use.
        }
      }
      await delay(50);
    }
    throw new Error("content qualification hook was not found");
  }

  async function evalInContent(expression) {
    const contextId = await findContext();
    try {
      return await evaluate(expression, contextId);
    } catch {
      cachedContextId = undefined;
      const retryId = await findContext();
      return evaluate(expression, retryId);
    }
  }

  return {
    session,
    contexts,
    consoleMessages,
    exceptions,
    async waitForHook() {
      await findContext();
    },
    async eval(expression) {
      return evalInContent(expression);
    },
    async captures() {
      return evalInContent(
        "globalThis.__cribraGuardQualification.captures()",
      );
    },
    async counts() {
      return evalInContent("globalThis.__cribraGuardQualification.counts()");
    },
    async pending() {
      return evalInContent("globalThis.__cribraGuardQualification.pending()");
    },
    async installed() {
      return evalInContent(
        "globalThis.__cribraGuardQualification.installed()",
      );
    },
    async maxSnapshots() {
      return evalInContent(
        "globalThis.__cribraGuardQualification.maxSnapshots",
      );
    },
    async status(snapshotId) {
      return evalInContent(
        `globalThis.__cribraGuardQualification.status(${JSON.stringify(
          snapshotId,
        )})`,
      );
    },
    async metadata(snapshotId) {
      return evalInContent(
        `globalThis.__cribraGuardQualification.metadata(${JSON.stringify(
          snapshotId,
        )})`,
      );
    },
    async verify(snapshotId) {
      return evalInContent(
        `globalThis.__cribraGuardQualification.verify(${JSON.stringify(
          snapshotId,
        )})`,
      );
    },
    async sourceEquals(snapshotId, candidate) {
      return evalInContent(
        `globalThis.__cribraGuardQualification.sourceEquals(${JSON.stringify(
          snapshotId,
        )}, ${JSON.stringify(candidate)})`,
      );
    },
    async replace(snapshotId, text) {
      return evalInContent(
        `globalThis.__cribraGuardQualification.replace(${JSON.stringify(
          snapshotId,
        )}, ${JSON.stringify(text)})`,
      );
    },
    async release(snapshotId) {
      return evalInContent(
        `globalThis.__cribraGuardQualification.release(${JSON.stringify(
          snapshotId,
        )})`,
      );
    },
    async dispose() {
      return evalInContent("globalThis.__cribraGuardQualification.dispose()");
    },
    async detach() {
      await session.detach().catch(() => {});
    },
  };
}

async function openScenario(state, server, scenario) {
  const page = await state.context.newPage();
  const pageConsole = [];
  const pageErrors = [];
  page.on("console", (message) => {
    pageConsole.push(message.text());
    consoleSamples.push(message.text());
  });
  page.on("pageerror", (error) => {
    pageErrors.push(error.message);
    consoleSamples.push(error.message);
  });
  const cdp = await createContentCdp(state.context, page);
  await page.goto(`${server.origin}/fixture?scenario=${scenario}`, {
    waitUntil: "domcontentloaded",
  });
  await cdp.waitForHook();
  server.reset();
  await page.evaluate(() => window.__fixture.resetCounters());
  return {
    page,
    cdp,
    pageConsole,
    pageErrors,
    async close() {
      for (const entry of cdp.consoleMessages) consoleSamples.push(entry.text);
      for (const entry of cdp.exceptions) consoleSamples.push(entry);
      await cdp.detach();
      await page.close();
    },
  };
}

async function readFixture(page) {
  return page.evaluate(() => ({
    submitHandlerCalls: window.__fixture.submitHandlerCalls,
    invalidCalls: window.__fixture.invalidCalls,
    earlierWindowHandlerCalls: window.__fixture.earlierWindowHandlerCalls,
    autosaveCalls: window.__fixture.autosaveCalls,
    inputEvents: window.__fixture.inputEvents,
    changeEvents: window.__fixture.changeEvents,
    lastSubmitDefaultPrevented:
      window.__fixture.lastSubmitDefaultPrevented,
    formDataKeys: window.__fixture.formDataKeys
      ? window.__fixture.formDataKeys()
      : [],
  }));
}

async function waitForCapture(cdp, previousCount, timeout = 4_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const captures = await cdp.captures();
    if (captures.length > previousCount) {
      return {
        capture: captures[captures.length - 1],
        captures,
      };
    }
    await delay(25);
  }
  throw new Error("timed out waiting for a content capture");
}

async function waitForServer(server, predicate, timeout = 4_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate()) return true;
    await delay(50);
  }
  return false;
}

async function clickSubmit(page, selector = "#submit") {
  await page.click(selector, { noWaitAfter: true });
}

async function dispatchSubmit(page) {
  await page.evaluate(() => {
    const form = document.getElementById("fixture-form");
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
  });
}

async function captureAfter(page, cdp, action) {
  const before = (await cdp.captures()).length;
  await action();
  return waitForCapture(cdp, before);
}

async function qualifyNativeTextarea(state, server) {
  const scenario = await openScenario(state, server, "native-textarea");
  try {
    const { page, cdp } = scenario;
    must("native textarea: hook is installed", (await cdp.installed()) === true);
    const { capture, captures } = await captureAfter(page, cdp, () =>
      clickSubmit(page),
    );
    must("native textarea: click path captures", capture.kind === "captured");
    must(
      "native textarea: capture is blocked and cancelled",
      capture.blocked === true && capture.cancelled === true,
    );
    must(
      "native textarea: exact source equality",
      (await cdp.sourceEquals(capture.snapshotId, TEXTAREA_SOURCE)) === true,
    );
    const metadata = await cdp.metadata(capture.snapshotId);
    must(
      "native textarea: metadata is source-free",
      metadata !== undefined &&
        !JSON.stringify(metadata).includes(SENTINEL) &&
        !JSON.stringify(metadata).includes("element"),
    );
    check(
      "native textarea: capture result is source-free",
      !JSON.stringify(captures).includes(SENTINEL),
    );

    const fixtureAfterFirst = await readFixture(page);
    check(
      "native textarea: page submit handler did not run",
      fixtureAfterFirst.submitHandlerCalls === 0,
    );
    check(
      "native textarea: no fixture endpoint invocation",
      server.count("submitted") === 0,
    );

    const duplicate = await captureAfter(page, cdp, () => clickSubmit(page));
    must("native textarea: duplicate is reported", duplicate.capture.kind === "duplicate");
    must(
      "native textarea: duplicate reuses the pending snapshot",
      duplicate.capture.snapshotId === capture.snapshotId,
    );
    const pendingAfterDuplicate = await cdp.pending();
    check(
      "native textarea: duplicate creates no second snapshot",
      pendingAfterDuplicate.length === 1,
    );

    must(
      "native textarea: explicit release succeeds",
      (await cdp.release(capture.snapshotId)) === true,
    );
    const recaptured = await captureAfter(page, cdp, () => clickSubmit(page));
    must(
      "native textarea: recapture after release succeeds",
      recaptured.capture.kind === "captured",
    );
    check(
      "native textarea: recapture uses a new opaque id",
      recaptured.capture.snapshotId !== capture.snapshotId,
    );
    await cdp.release(recaptured.capture.snapshotId);

    const requestSubmit = await captureAfter(page, cdp, () =>
      page.evaluate(() => window.__fixture.requestSubmit()),
    );
    must(
      "native textarea: requestSubmit path captures",
      requestSubmit.capture.kind === "captured",
    );
    check(
      "native textarea: requestSubmit records a supported submitter kind",
      requestSubmit.capture.submitterKind === "implicit" ||
        requestSubmit.capture.submitterKind === "submit",
      { submitterKind: requestSubmit.capture.submitterKind },
    );
    await cdp.release(requestSubmit.capture.snapshotId);
    check(
      "native textarea: no endpoint invocation after all blocked paths",
      server.count("submitted") === 0,
    );
  } finally {
    await scenario.close();
  }
}

async function qualifyNativeInput(state, server) {
  const scenario = await openScenario(state, server, "native-input");
  try {
    const { page, cdp } = scenario;
    await page.focus("#source");
    const { capture } = await captureAfter(page, cdp, () =>
      page.keyboard.press("Enter"),
    );
    must("native input: keyboard submission captures", capture.kind === "captured");
    check(
      "native input: keyboard submission records a supported submitter kind",
      capture.submitterKind === "implicit" || capture.submitterKind === "submit",
      { submitterKind: capture.submitterKind },
    );
    must(
      "native input: keyboard submission remains blocked",
      capture.blocked === true && capture.cancelled === true,
    );
    must(
      "native input: exact source equality",
      (await cdp.sourceEquals(capture.snapshotId, INPUT_SOURCE)) === true,
    );
    const fixture = await readFixture(page);
    check(
      "native input: page handler did not run",
      fixture.submitHandlerCalls === 0,
    );
    check("native input: no endpoint invocation", server.count("submitted") === 0);
  } finally {
    await scenario.close();
  }
}

async function qualifySensitiveFields(state, server) {
  const scenario = await openScenario(state, server, "sensitive");
  try {
    const { page, cdp } = scenario;
    const { capture } = await captureAfter(page, cdp, () => clickSubmit(page));
    must("sensitive fields: capture succeeds", capture.kind === "captured");
    must(
      "sensitive fields: snapshot contains only the designated editor",
      (await cdp.sourceEquals(capture.snapshotId, SAFE_EDITOR_SOURCE)) === true,
    );
    const fixture = await readFixture(page);
    check(
      "sensitive fields: browser form serialization includes more than the editor",
      fixture.formDataKeys.includes("source") &&
        fixture.formDataKeys.includes("password") &&
        fixture.formDataKeys.includes("hidden") &&
        fixture.formDataKeys.includes("unrelated"),
      { formDataKeys: fixture.formDataKeys },
    );
    check(
      "sensitive fields: snapshot excludes password and hidden values",
      !(await cdp.sourceEquals(
        capture.snapshotId,
        `${SAFE_EDITOR_SOURCE}\n${SENSITIVE_SENTINEL}`,
      )),
    );
    check(
      "sensitive fields: capture results exclude sensitive values",
      !JSON.stringify(await cdp.captures()).includes(SENSITIVE_SENTINEL),
    );
    check(
      "sensitive fields: page handler did not run",
      fixture.submitHandlerCalls === 0,
    );
  } finally {
    await scenario.close();
  }
}

async function qualifyContentEditable(state, server) {
  const scenario = await openScenario(state, server, "contenteditable");
  try {
    const { page, cdp } = scenario;
    const { capture } = await captureAfter(page, cdp, () => clickSubmit(page));
    must("contenteditable: capture succeeds", capture.kind === "captured");
    must(
      "contenteditable: exact textContent equality",
      (await cdp.sourceEquals(capture.snapshotId, CONTENTEDITABLE_SOURCE)) ===
        true,
    );
    check(
      "contenteditable: capture result is source-free",
      !JSON.stringify(await cdp.captures()).includes(SENTINEL),
    );

    const replacementText = "😀 surrogate pair\n日本語 line two";
    const replacement = await cdp.replace(capture.snapshotId, replacementText);
    must(
      "contenteditable: controlled replacement succeeds",
      replacement.status === "replaced" && replacement.readBackVerified === true,
    );
    must(
      "contenteditable: replacement reads back exactly in the page",
      (await page.evaluate(() => window.__fixture.editorValue())) ===
        replacementText,
    );
    must(
      "contenteditable: replacement invalidates the previous snapshot",
      (await cdp.status(capture.snapshotId)) === "invalidated",
    );
    const fixture = await readFixture(page);
    check(
      "contenteditable: input and change notifications were observed by the page",
      fixture.inputEvents > 0 && fixture.changeEvents > 0,
      { inputEvents: fixture.inputEvents, changeEvents: fixture.changeEvents },
    );
    check(
      "contenteditable: replacement does not submit",
      server.count("submitted") === 0 && fixture.submitHandlerCalls === 0,
    );

    const emptyCapture = await captureAfter(page, cdp, async () => {
      await page.evaluate(() => window.__fixture.setEditorValue("", false));
      await clickSubmit(page);
    });
    must("contenteditable: empty text captures", emptyCapture.capture.kind === "captured");
    must(
      "contenteditable: empty text equality",
      (await cdp.sourceEquals(emptyCapture.capture.snapshotId, "")) === true,
    );
    const emptyReplacement = await cdp.replace(
      emptyCapture.capture.snapshotId,
      "",
    );
    check(
      "contenteditable: empty replacement succeeds",
      emptyReplacement.status === "replaced",
    );
  } finally {
    await scenario.close();
  }
}

async function qualifyValidation(state, server) {
  const scenario = await openScenario(state, server, "validation");
  try {
    const { page, cdp } = scenario;
    await clickSubmit(page);
    await delay(150);
    const invalidFixture = await readFixture(page);
    check(
      "validation: invalid form does not reach the submit event",
      (await cdp.captures()).length === 0,
    );
    check(
      "validation: invalid event is observed by the fixture",
      invalidFixture.invalidCalls > 0,
      { invalidCalls: invalidFixture.invalidCalls },
    );
    check(
      "validation: invalid form does not invoke the endpoint",
      server.count("submitted") === 0,
    );

    await page.fill("#source", TEXTAREA_SOURCE);
    const { capture } = await captureAfter(page, cdp, () => clickSubmit(page));
    must("validation: valid form captures", capture.kind === "captured");
    check(
      "validation: valid form remains blocked",
      server.count("submitted") === 0,
    );
  } finally {
    await scenario.close();
  }
}

async function qualifyMultipleButtons(state, server) {
  const scenario = await openScenario(state, server, "multiple-buttons");
  try {
    const { page, cdp } = scenario;
    const first = await captureAfter(page, cdp, () => clickSubmit(page, "#save"));
    must("multiple buttons: first button captures", first.capture.kind === "captured");
    await cdp.release(first.capture.snapshotId);
    const second = await captureAfter(page, cdp, () =>
      clickSubmit(page, "#publish"),
    );
    must("multiple buttons: second button captures", second.capture.kind === "captured");
    check(
      "multiple buttons: submitter identity is preserved",
      first.capture.submitterToken !== second.capture.submitterToken &&
        first.capture.submitterToken !== null &&
        second.capture.submitterToken !== null,
    );
    check(
      "multiple buttons: both paths remain blocked",
      server.count("submitted") === 0,
    );
  } finally {
    await scenario.close();
  }
}

async function qualifyUnsupported(state, server) {
  const unsupported = await openScenario(state, server, "unsupported");
  try {
    const { page, cdp } = unsupported;
    await clickSubmit(page);
    await waitForServer(server, () => server.has("submitted"));
    const fixture = await readFixture(page);
    const unsupportedCaptures = await cdp.captures();
    check(
      "unsupported form: no capture is created",
      unsupportedCaptures.filter((entry) => entry.kind === "captured")
        .length === 0,
    );
    check(
      "unsupported form: unsupported path is diagnostic only",
      unsupportedCaptures.some(
        (entry) => entry.kind === "unsupported_ignored",
      ),
    );
    check(
      "unsupported form: page handler still runs",
      fixture.submitHandlerCalls > 0,
      { submitHandlerCalls: fixture.submitHandlerCalls },
    );
    check(
      "unsupported form: native endpoint is not claimed as protected",
      server.count("submitted") > 0,
    );
  } finally {
    await unsupported.close();
  }

  const rich = await openScenario(state, server, "rich-unsupported");
  try {
    const { page, cdp } = rich;
    await clickSubmit(page);
    await waitForServer(server, () => server.has("submitted"));
    const fixture = await readFixture(page);
    const richCaptures = await cdp.captures();
    check(
      "rich unsupported editor: no capture is created",
      richCaptures.filter((entry) => entry.kind === "captured").length === 0,
    );
    check(
      "rich unsupported editor: unsupported path is diagnostic only",
      richCaptures.some((entry) => entry.kind === "unsupported_ignored"),
    );
    check(
      "rich unsupported editor: page handler still runs",
      fixture.submitHandlerCalls > 0,
    );
    check(
      "rich unsupported editor: endpoint is not claimed as protected",
      server.count("submitted") > 0,
    );
  } finally {
    await rich.close();
  }
}

async function qualifyOversized(state, server) {
  const scenario = await openScenario(state, server, "oversized");
  try {
    const { page, cdp } = scenario;
    await clickSubmit(page);
    await delay(250);
    const captures = await cdp.captures();
    check(
      "oversized source: no captured snapshot is created",
      captures.filter((entry) => entry.kind === "captured").length === 0,
    );
    const last = captures.at(-1);
    check(
      "oversized source: blocked path is reported as oversized",
      last?.kind === "invalidated" && last?.reason === "oversized_source",
    );
    check(
      "oversized source: no pending snapshot remains",
      (await cdp.pending()).length === 0,
    );
    check(
      "oversized source: retained source is not accumulated",
      (await cdp.counts()).retainedSourceChars === 0,
    );
    const fixture = await readFixture(page);
    check(
      "oversized source: page handler did not run",
      fixture.submitHandlerCalls === 0,
    );
    check(
      "oversized source: endpoint was not invoked",
      server.count("submitted") === 0,
    );
  } finally {
    await scenario.close();
  }
}

async function qualifyRestoration(state, server) {
  const scenario = await openScenario(state, server, "native-textarea");
  try {
    const { page, cdp } = scenario;
    const first = await captureAfter(page, cdp, () => clickSubmit(page));
    const before = await cdp.counts();

    await cdp.eval(`(() => {
      const event = new Event("pagehide");
      Object.defineProperty(event, "persisted", { value: true });
      window.dispatchEvent(event);
      return true;
    })()`);
    await delay(100);
    const suspended = await cdp.counts();
    check(
      "restoration: persisted pagehide keeps interception installed",
      (await cdp.installed()) === true,
    );
    check(
      "restoration: persisted pagehide invalidates the old snapshot",
      (await cdp.status(first.capture.snapshotId)) === "invalidated",
    );
    check(
      "restoration: persisted pagehide clears pending captures",
      (await cdp.pending()).length === 0,
    );
    check(
      "restoration: suspension is recorded",
      suspended.suspensions > before.suspensions,
    );
    check(
      "restoration: suspension clears retained source",
      suspended.retainedSourceChars === 0,
    );

    await cdp.eval(`(() => {
      const event = new Event("pageshow");
      Object.defineProperty(event, "persisted", { value: true });
      window.dispatchEvent(event);
      return true;
    })()`);
    await delay(100);
    const restored = await cdp.counts();
    check(
      "restoration: persisted pageshow keeps interception installed",
      (await cdp.installed()) === true,
    );
    check(
      "restoration: restoration is recorded",
      restored.restorations > before.restorations,
    );

    const second = await captureAfter(page, cdp, () => clickSubmit(page));
    check(
      "restoration: restored document captures a fresh snapshot",
      second.capture.kind === "captured" &&
        second.capture.snapshotId !== first.capture.snapshotId,
    );
    check(
      "restoration: restored blocked path does not invoke the endpoint",
      server.count("submitted") === 0,
    );
  } finally {
    await scenario.close();
  }

  const back = await openScenario(state, server, "native-textarea");
  try {
    const { page, cdp } = back;
    await captureAfter(page, cdp, () => clickSubmit(page));
    await page.goto(`${server.origin}/fixture?scenario=native-input`, {
      waitUntil: "domcontentloaded",
    });
    await cdp.waitForHook();
    await page.goBack({ waitUntil: "domcontentloaded" });
    await cdp.waitForHook();
    check(
      "restoration: back navigation leaves an active interceptor",
      (await cdp.installed()) === true,
    );
    const recaptured = await captureAfter(page, cdp, () => clickSubmit(page));
    check(
      "restoration: back navigation captures and blocks",
      recaptured.capture.kind === "captured",
    );
    check(
      "restoration: back navigation does not invoke the endpoint",
      server.count("submitted") === 0,
    );
  } finally {
    await back.close();
  }
}

async function qualifyLimitations(state, server) {
  const scenario = await openScenario(state, server, "limitations");
  try {
    const { page, cdp } = scenario;

    await clickSubmit(page);
    await waitForServer(server, () => server.has("earlier-window-handler"));
    const earlier = await readFixture(page);
    check(
      "limitations: earlier window capture handler runs before interception",
      earlier.earlierWindowHandlerCalls > 0,
    );
    check(
      "limitations: blocked path prevents the form bubble handler",
      earlier.submitHandlerCalls === 0,
    );
    check(
      "limitations: blocked path prevents the native endpoint",
      server.count("submitted") === 0,
    );
    check(
      "limitations: supported path is still captured",
      (await cdp.captures()).length === 1,
    );
    const pending = await cdp.pending();
    if (pending[0] !== undefined) await cdp.release(pending[0]);

    server.reset();
    await page.evaluate(() => window.__fixture.resetCounters());
    const beforeNonCancellable = (await cdp.captures()).length;
    await page.evaluate(() => window.__fixture.dispatchNonCancellableSubmit());
    await waitForServer(server, () => server.has("submit-handler"));
    const nonCancellable = await readFixture(page);
    const nonCancellableCapture = (await cdp.captures()).at(-1);
    check(
      "limitations: non-cancellable event is reported as not blocked",
      nonCancellableCapture?.kind === "not_cancellable" &&
        nonCancellableCapture.blocked === false &&
        nonCancellableCapture.cancelled === false,
    );
    check(
      "limitations: non-cancellable event adds exactly one diagnostic result",
      (await cdp.captures()).length === beforeNonCancellable + 1,
    );
    check(
      "limitations: non-cancellable event is not default-prevented",
      nonCancellable.lastSubmitDefaultPrevented === false,
    );
    check(
      "limitations: non-cancellable event reaches the fixture handler",
      nonCancellable.submitHandlerCalls > 0,
    );

    server.reset();
    await page.evaluate(() => window.__fixture.resetCounters());
    const beforeFormSubmit = (await cdp.captures()).length;
    await page.evaluate(() => window.__fixture.callFormSubmit());
    await waitForServer(server, () => server.has("submitted"));
    const formSubmit = await readFixture(page);
    check(
      "limitations: form.submit() bypasses the submit event",
      (await cdp.captures()).length === beforeFormSubmit,
    );
    check(
      "limitations: form.submit() reaches the endpoint",
      server.count("submitted") > 0,
    );
    check(
      "limitations: form.submit() does not run the page submit handler",
      formSubmit.submitHandlerCalls === 0,
    );

    server.reset();
    await page.evaluate(() => window.__fixture.resetCounters());
    const beforeDirect = (await cdp.captures()).length;
    await page.evaluate(() => window.__fixture.directFetch());
    await waitForServer(server, () => server.has("direct"));
    check(
      "limitations: direct fetch is outside interception",
      server.count("direct") > 0 &&
        (await cdp.captures()).length === beforeDirect,
    );

    server.reset();
    await page.evaluate(() => window.__fixture.resetCounters());
    const beforeAutosave = (await cdp.captures()).length;
    await page.evaluate(() => window.__fixture.autosave());
    await waitForServer(server, () => server.has("autosave"));
    const autosave = await readFixture(page);
    await clickSubmit(page);
    await delay(150);
    check(
      "limitations: autosave request occurs even when submission is blocked",
      server.count("autosave") > 0 && autosave.autosaveCalls > 0,
    );
    check(
      "limitations: blocked submission creates one capture after autosave",
      (await cdp.captures()).length === beforeAutosave + 1,
    );
    check(
      "limitations: blocked submission endpoint remains untouched after autosave",
      server.count("submitted") === 0,
    );
  } finally {
    await scenario.close();
  }
}

async function qualifyReplacement(state, server) {
  const scenario = await openScenario(state, server, "replacement");
  try {
    const { page, cdp } = scenario;
    const first = await captureAfter(page, cdp, () => clickSubmit(page));
    must("replacement: initial capture succeeds", first.capture.kind === "captured");

    await page.evaluate(() => window.__fixture.replaceEditor());
    await page.evaluate(() =>
      window.__fixture.setEditorValue("replacement source", false),
    );
    const detachedReplacement = await cdp.replace(
      first.capture.snapshotId,
      "new text",
    );
    check(
      "replacement: a detached original editor is never reported as success",
      detachedReplacement.status === "failed" &&
        ["snapshot_unavailable", "editor_detached", "form_detached"].includes(
          detachedReplacement.reason,
        ),
      { reason: detachedReplacement.reason },
    );

    const before = (await cdp.captures()).length;
    await clickSubmit(page);
    await delay(200);
    const after = await cdp.captures();
    check(
      "replacement: editor replacement does not silently recapture",
      after.length === before + 1 &&
        after.at(-1)?.kind === "invalidated",
    );
    check(
      "replacement: editor replacement is reported as invalidated",
      after.at(-1)?.kind === "invalidated" &&
        after.at(-1)?.reason === "editor_replaced",
    );
    check(
      "replacement: editor replacement remains blocked",
      server.count("submitted") === 0,
    );

    const formScenario = await openScenario(state, server, "replacement");
    try {
      const formPage = formScenario.page;
      const formCdp = formScenario.cdp;
      const original = await captureAfter(formPage, formCdp, () =>
        clickSubmit(formPage),
      );
      must("form replacement: initial capture succeeds", original.capture.kind === "captured");
      await formPage.evaluate(() => window.__fixture.replaceForm(true));
      await formPage.evaluate(() =>
        window.__fixture.setEditorValue("new form source", false),
      );
      const recaptured = await captureAfter(formPage, formCdp, () =>
        clickSubmit(formPage),
      );
      must(
        "form replacement: a new recognized form is a new protected path",
        recaptured.capture.kind === "captured" &&
          recaptured.capture.snapshotId !== original.capture.snapshotId,
      );
      check(
        "form replacement: the old snapshot is invalidated",
        (await formCdp.status(original.capture.snapshotId)) === "invalidated",
      );
      check(
        "form replacement: coverage loss is reported",
        (await formCdp.eval("globalThis.__cribraGuardQualification.losses()"))
          .some((entry) => entry.reason === "form_replaced"),
      );
      check(
        "form replacement: the new protected path is blocked",
        server.count("submitted") === 0,
      );
    } finally {
      await formScenario.close();
    }

    const unmarked = await openScenario(state, server, "replacement");
    try {
      const unmarkedPage = unmarked.page;
      const unmarkedCdp = unmarked.cdp;
      const original = await captureAfter(unmarkedPage, unmarkedCdp, () =>
        clickSubmit(unmarkedPage),
      );
      must("unmarked form replacement: initial capture succeeds", original.capture.kind === "captured");
      await unmarkedPage.evaluate(() => window.__fixture.replaceForm(false));
      await unmarkedPage.evaluate(() =>
        window.__fixture.setEditorValue("unmarked source", false),
      );
      await clickSubmit(unmarkedPage);
      await waitForServer(server, () => server.has("submitted"));
      const fixture = await readFixture(unmarkedPage);
      const captures = await unmarkedCdp.captures();
      check(
        "unmarked form replacement: no protection is claimed for the new form",
        captures.at(-1)?.kind === "unsupported_ignored",
      );
      check(
        "unmarked form replacement: the new form reaches the endpoint",
        server.count("submitted") > 0,
      );
      check(
        "unmarked form replacement: coverage loss is reported",
        (await unmarkedCdp.eval(
          "globalThis.__cribraGuardQualification.losses()",
        )).some((entry) => entry.reason === "form_replaced"),
      );
      check(
        "unmarked form replacement: page handler runs for the new form",
        fixture.submitHandlerCalls > 0,
      );
    } finally {
      await unmarked.close();
    }
  } finally {
    await scenario.close();
  }
}

async function qualifySnapshotLifecycle(state, server) {
  const scenario = await openScenario(state, server, "native-textarea");
  try {
    const { page, cdp } = scenario;
    const first = await captureAfter(page, cdp, () => clickSubmit(page));
    must("snapshot lifecycle: initial capture succeeds", first.capture.kind === "captured");
    await page.evaluate(() =>
      window.__fixture.setEditorValue("B value", true),
    );
    must(
      "snapshot lifecycle: observable A-to-B change invalidates",
      (await cdp.verify(first.capture.snapshotId)) === false,
    );
    must(
      "snapshot lifecycle: invalidated status is sticky",
      (await cdp.status(first.capture.snapshotId)) === "invalidated",
    );
    await page.evaluate(
      (value) => window.__fixture.setEditorValue(value, true),
      TEXTAREA_SOURCE,
    );
    must(
      "snapshot lifecycle: returning to A does not revive the snapshot",
      (await cdp.verify(first.capture.snapshotId)) === false,
    );

    const second = await captureAfter(page, cdp, () => clickSubmit(page));
    must("snapshot lifecycle: a fresh capture is required", second.capture.kind === "captured");
    await page.evaluate(() =>
      window.__fixture.setEditorValue("silent B", false),
    );
    must(
      "snapshot lifecycle: silent programmatic change is detected by read-back",
      (await cdp.verify(second.capture.snapshotId)) === false,
    );

    await page.evaluate(
      (value) => window.__fixture.setEditorValue(value, false),
      TEXTAREA_SOURCE,
    );
    const third = await captureAfter(page, cdp, () => clickSubmit(page));
    must("snapshot lifecycle: third capture succeeds", third.capture.kind === "captured");
    await page.evaluate(() =>
      window.__fixture.setEditorValue("transient B", false),
    );
    await page.evaluate(
      (value) => window.__fixture.setEditorValue(value, false),
      TEXTAREA_SOURCE,
    );
    check(
      "snapshot lifecycle: silent transient A-to-B-to-A is not observable",
      (await cdp.verify(third.capture.snapshotId)) === true,
    );
  } finally {
    await scenario.close();
  }

  const contenteditable = await openScenario(state, server, "contenteditable");
  try {
    const { page, cdp } = contenteditable;
    const first = await captureAfter(page, cdp, () => clickSubmit(page));
    must("mutation flush: contenteditable capture succeeds", first.capture.kind === "captured");
    await page.evaluate(() =>
      window.__fixture.setEditorValue("mutation B", false),
    );
    must(
      "mutation flush: pending MutationObserver records are processed before verification",
      (await cdp.verify(first.capture.snapshotId)) === false,
    );
  } finally {
    await contenteditable.close();
  }
}

async function qualifyLifecycle(state, server) {
  const scenario = await openScenario(state, server, "native-textarea");
  try {
    const { page, cdp } = scenario;
    const capture = await captureAfter(page, cdp, () => clickSubmit(page));
    must("lifecycle: capture before navigation succeeds", capture.capture.kind === "captured");
    await page.goto(`${server.origin}/fixture?scenario=native-input`, {
      waitUntil: "domcontentloaded",
    });
    await cdp.waitForHook();
    must(
      "lifecycle: navigation installs a fresh hook",
      (await cdp.installed()) === true,
    );
    check(
      "lifecycle: the old document snapshot is unknown in the new document",
      (await cdp.status(capture.capture.snapshotId)) === "unknown",
    );
    const fresh = await captureAfter(page, cdp, () => {
      return page.evaluate(() => {
        const form = document.getElementById("fixture-form");
        form.dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        );
      });
    });
    check(
      "lifecycle: the new document creates an independent snapshot",
      fresh.capture.kind === "captured" &&
        fresh.capture.snapshotId !== capture.capture.snapshotId,
    );
  } finally {
    await scenario.close();
  }

  const cleanup = await openScenario(state, server, "native-textarea");
  try {
    const { page, cdp } = cleanup;
    await captureAfter(page, cdp, () => clickSubmit(page));
    await cdp.dispose();
    check(
      "cleanup: disposal removes the installed listener",
      (await cdp.installed()) === false,
    );
    check(
      "cleanup: disposal clears pending snapshots",
      (await cdp.pending()).length === 0,
    );
    await clickSubmit(page);
    await waitForServer(server, () => server.has("submitted"));
    const fixture = await readFixture(page);
    check(
      "cleanup: disposed interceptor does not capture",
      (await cdp.captures()).length === 0,
    );
    check(
      "cleanup: disposed interceptor leaves the page path untouched",
      fixture.submitHandlerCalls > 0 && server.count("submitted") > 0,
    );
  } finally {
    await cleanup.close();
  }
}

async function qualifyMultipleTabs(state, server) {
  const left = await openScenario(state, server, "native-textarea");
  const right = await openScenario(state, server, "native-textarea");
  try {
    const leftCapture = await captureAfter(left.page, left.cdp, () =>
      clickSubmit(left.page),
    );
    const rightCapture = await captureAfter(right.page, right.cdp, () =>
      clickSubmit(right.page),
    );
    must("multiple tabs: left tab captures", leftCapture.capture.kind === "captured");
    must("multiple tabs: right tab captures", rightCapture.capture.kind === "captured");
    check(
      "multiple tabs: snapshot identifiers are independent",
      leftCapture.capture.snapshotId !== rightCapture.capture.snapshotId,
    );
    check(
      "multiple tabs: each hook sees only its own capture",
      (await left.cdp.captures()).length === 1 &&
        (await right.cdp.captures()).length === 1,
    );
    await left.cdp.dispose();
    check(
      "multiple tabs: disposing one tab does not affect the other",
      (await left.cdp.installed()) === false &&
        (await right.cdp.installed()) === true &&
        (await right.cdp.pending()).length === 1,
    );
    check(
      "multiple tabs: no endpoint invocation on either qualified path",
      server.count("submitted") === 0,
    );
  } finally {
    await left.close();
    await right.close();
  }
}

async function qualifyBoundedStore(state, server) {
  const scenario = await openScenario(state, server, "native-textarea");
  try {
    const { page, cdp } = scenario;
    const maxSnapshots = await cdp.maxSnapshots();
    must("bounded store: a maximum is exposed", typeof maxSnapshots === "number" && maxSnapshots > 0);
    await captureAfter(page, cdp, () => clickSubmit(page));
    for (let index = 0; index < maxSnapshots + 16; index += 1) {
      await page.evaluate((value) => {
        window.__fixture.setEditorValue(value, true);
        const form = document.getElementById("fixture-form");
        form.dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        );
      }, `bounded-${index}`);
      await delay(5);
    }
    const counts = await cdp.counts();
    check(
      "bounded store: snapshot storage does not grow without bound",
      counts.snapshots <= maxSnapshots,
      { snapshots: counts.snapshots, maxSnapshots },
    );
    check(
      "bounded store: repeated captures do not create duplicate pending state",
      counts.pending <= 1,
      { pending: counts.pending },
    );
  } finally {
    await scenario.close();
  }
}

async function qualifyPrivacy(state, server) {
  const scenario = await openScenario(state, server, "native-textarea");
  try {
    const { page, cdp } = scenario;
    const { capture, captures } = await captureAfter(page, cdp, () =>
      clickSubmit(page),
    );
    must("privacy: capture succeeds", capture.kind === "captured");
    const editorValue = await page.evaluate(() => window.__fixture.editorValue());
    check(
      "privacy: exact editor equality uses the editor source only",
      (await cdp.sourceEquals(capture.snapshotId, editorValue)) === true,
    );
    check(
      "privacy: capture results contain no source text",
      !JSON.stringify(captures).includes(SENTINEL),
    );
    check(
      "privacy: metadata contains no source text",
      !JSON.stringify(await cdp.metadata(capture.snapshotId)).includes(
        SENTINEL,
      ),
    );
    check(
      "privacy: counts contain no source text",
      !JSON.stringify(await cdp.counts()).includes(SENTINEL),
    );
  } finally {
    await scenario.close();
  }
}

async function findSentinel(root, sentinel) {
  if (!existsSync(root)) return [];
  const needle = Buffer.from(sentinel, "utf8");
  const matches = [];
  for (const entry of await readdir(root, {
    recursive: true,
    withFileTypes: true,
  })) {
    if (!entry.isFile()) continue;
    const path = join(entry.parentPath, entry.name);
    try {
      const contents = await readFile(path);
      if (contents.includes(needle)) matches.push(path);
    } catch {
      // Ignore unreadable profile files.
    }
  }
  return matches;
}

async function verifyPackagedBoundary(extensionId) {
  const contentManifest = JSON.parse(
    await readFile(resolve(contentDistDir, "manifest.json"), "utf8"),
  );
  must(
    "packaging: qualification manifest is MV3",
    contentManifest.manifest_version === 3,
  );
  const contentScriptEntry = contentManifest.content_scripts?.[0];
  must(
    "packaging: content script is document_start and main-frame only",
    contentManifest.content_scripts?.length === 1 &&
      contentScriptEntry?.run_at === "document_start" &&
      contentScriptEntry?.all_frames === false,
  );
  must(
    "packaging: content script matches only 127.0.0.1",
    JSON.stringify(contentScriptEntry?.matches) ===
      JSON.stringify(["http://127.0.0.1/*"]),
  );
  must(
    "packaging: host permission is only 127.0.0.1",
    JSON.stringify(contentManifest.host_permissions) ===
      JSON.stringify(["http://127.0.0.1/*"]),
  );
  must(
    "packaging: no extra permissions",
    JSON.stringify(contentManifest.permissions ?? []) === "[]",
  );
  must(
    "packaging: no WASM qualification page in the content package",
    !existsSync(resolve(contentDistDir, "qualification.html")) &&
      !existsSync(resolve(contentDistDir, "qualification.js")),
  );
  const contentScript = await readFile(
    resolve(contentDistDir, "content-script.js"),
    "utf8",
  );
  must(
    "packaging: content script is a classic IIFE",
    /(?:^|\n)\s*(?:"use strict";\s*)?\(\(\)\s*=>\s*\{/.test(contentScript) &&
      !/\bimport\s*\(/.test(contentScript),
  );
  must(
    "packaging: content script contains no WASM or remote code",
    !/WebAssembly\b/.test(contentScript) &&
      !/wasm_bindgen/.test(contentScript) &&
      !/https?:\/\//.test(contentScript),
  );
  must(
    "packaging: content script contains no network or storage APIs",
    !/\bfetch\s*\(/.test(contentScript) &&
      !/XMLHttpRequest/.test(contentScript) &&
      !/WebSocket/.test(contentScript) &&
      !/chrome\.storage/.test(contentScript) &&
      !/localStorage/.test(contentScript) &&
      !/indexedDB/.test(contentScript),
  );
  must(
    "packaging: content script never uses innerHTML or dynamic evaluation",
    !/innerHTML/.test(contentScript) &&
      !/\beval\s*\(/.test(contentScript) &&
      !/new\s+Function\s*\(/.test(contentScript),
  );
  must(
    "packaging: content script contains no debugger or remote-logging calls",
    !/\bdebugger\b/.test(contentScript) &&
      !/console\./.test(contentScript),
  );
  must(
    "packaging: production extension id used for storage scan is stable",
    typeof extensionId === "string" && extensionId.length === 32,
  );
}

async function main() {
  if (!existsSync(contentDistDir)) {
    throw new Error(
      "content qualification package is missing; run `npm run build:content-qualification` first",
    );
  }
  if (
    !existsSync(resolve(contentDistDir, "content-script.js")) ||
    !existsSync(resolve(contentDistDir, "manifest.json"))
  ) {
    throw new Error(
      "content qualification package is incomplete; rebuild it before qualifying",
    );
  }

  const extensionId = extensionIdFor(contentDistDir);
  const server = await startFixtureServer();
  const state = await launchContentExtension();
  let report;

  try {
    await verifyPackagedBoundary(extensionId);

    const scenarios = [
      qualifyNativeTextarea,
      qualifyNativeInput,
      qualifySensitiveFields,
      qualifyContentEditable,
      qualifyValidation,
      qualifyMultipleButtons,
      qualifyUnsupported,
      qualifyOversized,
      qualifyLimitations,
      qualifyReplacement,
      qualifySnapshotLifecycle,
      qualifyLifecycle,
      qualifyRestoration,
      qualifyMultipleTabs,
      qualifyBoundedStore,
      qualifyPrivacy,
    ];

    for (const scenario of scenarios) {
      await scenario(state, server);
    }

    const remoteRequests = state.requestUrls.filter(
      (url) => /^https?:/i.test(url) && !url.startsWith(server.origin),
    );
    check(
      "privacy: fixture server never receives source text",
      !JSON.stringify(server.state).includes(SENTINEL) &&
        !JSON.stringify(server.state).includes(SENSITIVE_SENTINEL),
    );
    check(
      "privacy: no source text in page or isolated-world console output",
      !consoleSamples.some(
        (entry) =>
          entry.includes(SENTINEL) || entry.includes(SENSITIVE_SENTINEL),
      ),
      { samples: consoleSamples.length },
    );
    check(
      "privacy: no remote HTTP(S) request is required by the content script",
      remoteRequests.length === 0,
      { remoteRequests },
    );

    report = {
      startedAt: new Date().toISOString(),
      environment: {
        platform: process.platform,
        arch: process.arch,
        node: process.version,
        playwright: "1.64.1",
        chrome: state.context.browser()?.version() ?? null,
        headed,
        extensionId,
      },
      checks,
      failures,
      knownLimitations: [
        "preventDefault does not stop direct fetch/XHR, form.submit(), non-cancellable events, earlier window capture handlers, autosave, or unsupported editors.",
        "DOM observation cannot detect every transient or silent mutation. A silent textarea change that returns to its original value before read-back is not observable.",
        "The fixture markers (data-cribra-adapter and data-cribra-editor) are qualification contracts, not production trust.",
        "Capture results are not consent, browser identity, or authorization. A future controller must authenticate senders and bind authorization to exact snapshots.",
        "The snapshot covers only the designated editor text, not the entire browser form serialization, attachments, hidden fields, password fields, or page-controlled network channels.",
        "The qualification-only manifest grants access only to http://127.0.0.1/* and is never part of the production package.",
        "Source is limited to 1,000,000 UTF-16 code units per snapshot and 4,000,000 aggregate retained code units; oversized intercepted content stays blocked with no snapshot.",
        "Persisted pagehide keeps interception installed and invalidates all snapshots; a non-persisted pagehide disposes and a new document receives a fresh content script. Real Back/Forward Cache eligibility is browser/profile-controlled, so the qualification exercises both synthetic pagehide/pageshow and real back navigation but does not force a bfcache restore.",
      ],
    };
  } finally {
    await state.context.close().catch(() => {});
    const profileCandidates = [
      resolve(
        state.profileDir,
        "Default/Local Extension Settings",
        extensionId,
      ),
      resolve(state.profileDir, "Default/IndexedDB"),
      resolve(state.profileDir, "Default/Service Worker/CacheStorage"),
      resolve(state.profileDir, "Default/Session Storage"),
      resolve(state.profileDir, "Default/Local Storage"),
    ];
    const persisted = [];
    for (const root of profileCandidates) {
      persisted.push(...(await findSentinel(root, SENTINEL)));
      persisted.push(...(await findSentinel(root, SENSITIVE_SENTINEL)));
    }
    check(
      "privacy: no source sentinel appears in extension profile storage",
      persisted.length === 0,
      {
        filesScanned: profileCandidates,
        matches: persisted.map((path) => relative(state.profileDir, path)),
      },
    );
    await server.close();
    await rm(state.profileDir, { recursive: true, force: true }).catch(
      () => {},
    );
  }

  if (report === undefined) {
    report = {
      startedAt: new Date().toISOString(),
      checks,
      failures,
    };
  }
  report.finishedAt = new Date().toISOString();
  report.checks = checks;
  report.failures = failures;

  const serialized = JSON.stringify(report);
  if (serialized.includes(SENTINEL) || serialized.includes(SENSITIVE_SENTINEL)) {
    throw new Error("content qualification report leaked a source sentinel");
  }

  await import("node:fs/promises").then(({ writeFile, mkdir }) =>
    mkdir(reportRoot, { recursive: true }).then(() =>
      writeFile(
        resolve(reportRoot, "report.json"),
        `${JSON.stringify(report, null, 2)}\n`,
        "utf8",
      ),
    ),
  );
  process.stdout.write(
    `\ncontent qualification report: ${resolve(reportRoot, "report.json")}\n`,
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
