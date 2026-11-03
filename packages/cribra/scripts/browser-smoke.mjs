/** Chrome/Chromium headless smoke test over a local Node HTTP server. */

import { spawn, spawnSync } from "node:child_process";
import { accessSync, constants } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, resolve, sep } from "node:path";

import { isMain, packageRoot, parseArgs } from "./util.mjs";

const MIME = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".wasm", "application/wasm"],
]);

function executable(candidate) {
  if (candidate.includes(sep) || candidate.includes("/")) {
    try {
      accessSync(candidate, constants.X_OK);
      return candidate;
    } catch {
      return undefined;
    }
  }
  const found = spawnSync("which", [candidate], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
  if (found.status === 0) {
    const value = found.stdout.trim();
    if (value.length > 0) return value;
  }
  return undefined;
}

function findChrome() {
  const candidates = [
    process.env.CHROME_BIN,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "google-chrome",
    "chromium",
    "chromium-browser",
  ].filter((value) => typeof value === "string" && value.length > 0);

  for (const candidate of candidates) {
    const resolved = executable(candidate);
    if (resolved !== undefined) return resolved;
  }
  throw new Error(
    "Chrome/Chromium not found; set CHROME_BIN or install Chrome/Chromium",
  );
}

function inside(base, path) {
  const prefix = base.endsWith(sep) ? base : base + sep;
  return path === base || path.startsWith(prefix);
}

function startServer(distDir) {
  const browserDir = resolve(packageRoot, "test/browser");
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://localhost");
      let pathname = decodeURIComponent(url.pathname);
      if (pathname === "/") pathname = "/test/browser/index.html";

      let base;
      let relative;
      if (pathname.startsWith("/dist/")) {
        base = distDir;
        relative = pathname.slice("/dist/".length);
      } else if (pathname.startsWith("/test/browser/")) {
        base = browserDir;
        relative = pathname.slice("/test/browser/".length);
      } else {
        response.writeHead(404).end("not found");
        return;
      }

      const file = resolve(base, relative);
      if (!inside(base, file)) {
        response.writeHead(403).end("forbidden");
        return;
      }

      const body = await readFile(file);
      response.writeHead(200, {
        "Content-Type": MIME.get(extname(file)) ?? "application/octet-stream",
        "Cache-Control": "no-store",
      });
      response.end(body);
    } catch {
      response.writeHead(404).end("not found");
    }
  });

  return new Promise((resolvePromise, rejectPromise) => {
    server.once("error", rejectPromise);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (address === null || typeof address === "string") {
        rejectPromise(new Error("HTTP server did not expose a TCP port"));
        return;
      }
      resolvePromise({
        url: `http://127.0.0.1:${address.port}`,
        close: () =>
          new Promise((closeResolve) => server.close(() => closeResolve())),
      });
    });
  });
}

function runChrome(chrome, args, timeoutMs) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(chrome, args, {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    let settled = false;

    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      child.kill("SIGKILL");
      rejectPromise(
        new Error(`Chrome did not finish within ${timeoutMs} ms`),
      );
    }, timeoutMs);

    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });

    child.once("error", (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      rejectPromise(new Error(`Chrome failed to start: ${error.message}`));
    });

    child.once("close", (code) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (code !== 0) {
        rejectPromise(
          new Error(
            `Chrome exited with code ${code}: ${stderr.trim().slice(0, 2000)}`,
          ),
        );
        return;
      }
      resolvePromise(stdout);
    });
  });
}

export async function runBrowserSmoke({ distDir, profile, version }) {
  const chrome = findChrome();
  const server = await startServer(distDir);
  const userDataDir = await mkdtemp(resolve(tmpdir(), "cribra-browser-"));
  try {
    const query = new URLSearchParams({ profile, version });
    const url = `${server.url}/?${query.toString()}`;
    const output = await runChrome(
      chrome,
      [
        "--headless=new",
        "--disable-gpu",
        "--no-sandbox",
        "--disable-dev-shm-usage",
        `--user-data-dir=${userDataDir}`,
        "--virtual-time-budget=20000",
        "--dump-dom",
        url,
      ],
      90_000,
    );
    const match = output.match(/<pre id="result">([^<]*)<\/pre>/);
    if (match === null) {
      throw new Error(
        `Chrome produced no result marker. Output: ${output.slice(0, 2000)}`,
      );
    }
    const value = match[1].trim();
    if (value !== "PASS") {
      throw new Error(`Browser smoke failed: ${value}`);
    }
    process.stdout.write(
      `browser smoke passed (${profile}) via ${chrome}\n`,
    );
  } finally {
    await server.close();
    await rm(userDataDir, { recursive: true, force: true });
  }
}

if (isMain(import.meta.url)) {
  const flags = parseArgs(process.argv.slice(2));
  const dist = flags.get("dist");
  const profile = flags.get("profile") ?? "flow";
  const version = flags.get("version");
  if (typeof dist !== "string" || typeof version !== "string") {
    throw new Error(
      "usage: browser-smoke.mjs --dist <dir> --profile <core|flow> --version <version>",
    );
  }
  await runBrowserSmoke({
    distDir: resolve(packageRoot, dist),
    profile,
    version,
  });
}
