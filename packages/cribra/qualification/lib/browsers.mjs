/** Minimal Playwright launcher and result reader for qualification. */

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { chromium, firefox, webkit } from "playwright";

let firefoxHome;

function firefoxEnvironment() {
  if (process.platform !== "darwin") return undefined;
  // Playwright Firefox 157 cannot locate its profile under the TCC-protected
  // macOS 27 application-support directory. A private CFFIXED_USER_HOME avoids
  // that lookup without touching host settings. Firefox ACLs these directories,
  // so the fake home stays in the system temp directory rather than under the
  // qualification tree that `npm run qualify` removes.
  firefoxHome ??= mkdtempSync(join(tmpdir(), "cribra-firefox-home-"));
  return { ...process.env, CFFIXED_USER_HOME: firefoxHome };
}

const ENGINES = {
  chrome: {
    launch: () => chromium.launch({ channel: "chrome", headless: true }),
    label: "Google Chrome",
  },
  chromium: {
    launch: () => chromium.launch({ headless: true }),
    label: "Playwright Chromium",
  },
  firefox: {
    launch: () => firefox.launch({ headless: true, env: firefoxEnvironment() }),
    label: "Playwright Firefox",
  },
  webkit: {
    launch: () => webkit.launch({ headless: true }),
    label: "Playwright WebKit",
  },
};

export function browserLabel(engine) {
  return ENGINES[engine]?.label ?? engine;
}

export async function launchBrowser(engine) {
  const definition = ENGINES[engine];
  if (definition === undefined) {
    throw new Error(`unknown browser engine: ${engine}`);
  }
  const browser = await definition.launch();
  return {
    browser,
    engine,
    label: definition.label,
    version: browser.version(),
  };
}

export async function runPage(browser, url, { timeout = 90_000 } = {}) {
  const page = await browser.newPage();
  const consoleErrors = [];
  const pageErrors = [];
  const wasmResponses = [];
  const failedResponses = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("response", (response) => {
    if (response.status() >= 400) {
      failedResponses.push({ url: response.url(), status: response.status() });
    }
    if (response.url().includes(".wasm")) {
      wasmResponses.push({
        url: response.url(),
        status: response.status(),
        contentType: response.headers()["content-type"],
      });
    }
  });
  try {
    await page.goto(url, { waitUntil: "domcontentloaded", timeout });
    await page.waitForFunction(
      () => {
        const element = document.getElementById("result");
        return (
          element !== null &&
          element.textContent !== null &&
          element.textContent !== "PENDING"
        );
      },
      undefined,
      { timeout },
    );
    const result = await page.evaluate(
      () => document.getElementById("result")?.textContent ?? "",
    );
    return {
      result: result.trim(),
      consoleErrors,
      pageErrors,
      wasmResponses,
      failedResponses,
    };
  } finally {
    await page.close();
  }
}

export async function expectPass({
  browser,
  url,
  label,
  timeout = 90_000,
  allowConsoleErrors = false,
}) {
  const { result, consoleErrors, pageErrors, wasmResponses, failedResponses } =
    await runPage(browser, url, { timeout });
  if (pageErrors.length > 0) {
    throw new Error(`${label} page error: ${pageErrors.join(" | ")}`);
  }
  if (!allowConsoleErrors && consoleErrors.length > 0) {
    throw new Error(
      `${label} console error: ${consoleErrors.join(" | ")} ` +
        `(failed responses: ${JSON.stringify(failedResponses)})`,
    );
  }
  if (!result.startsWith("PASS")) {
    throw new Error(`${label} failed: ${result}`);
  }
  const successfulWasm = wasmResponses.filter(
    (response) =>
      response.status === 200 &&
      String(response.contentType ?? "").includes("application/wasm"),
  );
  if (successfulWasm.length === 0) {
    throw new Error(
      `${label} did not load a WASM asset with application/wasm: ${JSON.stringify(wasmResponses)}`,
    );
  }
  return { result, consoleErrors, pageErrors, wasmResponses, failedResponses };
}
