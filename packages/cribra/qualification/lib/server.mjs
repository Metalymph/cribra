/** Deterministic static HTTP server used by installed-package browser tests. */

import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";

const MIME = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".wasm", "application/wasm"],
  [".svg", "image/svg+xml"],
  [".map", "application/json; charset=utf-8"],
]);

function inside(base, path) {
  const prefix = base.endsWith(sep) ? base : base + sep;
  return path === base || path.startsWith(prefix);
}

/**
 * Mounts a set of URL prefixes to filesystem directories.
 * Example: mounts = [{ prefix: "/dist/", directory: "/.../dist" }]
 */
export async function startServer({ mounts, rootRedirect }) {
  const normalized = mounts.map((mount) => ({
    prefix: mount.prefix.endsWith("/") ? mount.prefix : mount.prefix + "/",
    directory: resolve(mount.directory),
  }));

  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      let pathname = decodeURIComponent(url.pathname);

      if (pathname === "/favicon.ico") {
        response.writeHead(204).end();
        return;
      }

      if (
        rootRedirect !== undefined &&
        (pathname === "/" || pathname === "/index.html")
      ) {
        response.writeHead(302, { Location: rootRedirect }).end();
        return;
      }

      for (const mount of normalized) {
        if (!pathname.startsWith(mount.prefix)) continue;
        let relative = pathname.slice(mount.prefix.length);
        if (relative === "" || relative.endsWith("/")) {
          relative += "index.html";
        }
        const file = resolve(mount.directory, relative);
        if (!inside(mount.directory, file)) {
          response.writeHead(403).end("forbidden");
          return;
        }
        const body = await readFile(file);
        response.writeHead(200, {
          "Content-Type": MIME.get(extname(file)) ?? "application/octet-stream",
          "Cache-Control": "no-store",
          "Cross-Origin-Opener-Policy": "same-origin",
        });
        response.end(body);
        return;
      }

      response.writeHead(404).end("not found");
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
